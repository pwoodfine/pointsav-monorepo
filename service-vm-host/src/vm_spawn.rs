use std::{fs, path::PathBuf, process::Command};
use system_vm_fleet_types::{HostPortMapping, VmRecord, VmState};

/// Root directory for VM disk images, seed ISOs, pid files, and monitor sockets.
/// Override with VM_DISK_DIR env var (default: /var/lib/vm-fleet).
pub fn disk_dir() -> PathBuf {
    std::env::var("VM_DISK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/vm-fleet"))
}

pub fn disk_path_for(vm_id: &str) -> PathBuf {
    disk_dir().join(format!("{vm_id}.qcow2"))
}

pub fn monitor_sock_for(vm_id: &str) -> PathBuf {
    disk_dir().join(format!("{vm_id}.monitor.sock"))
}

fn pid_path_for(vm_id: &str) -> PathBuf {
    disk_dir().join(format!("{vm_id}.pid"))
}

pub fn meta_path_for(vm_id: &str) -> PathBuf {
    disk_dir().join(format!("{vm_id}.meta.json"))
}

fn seed_iso_path_for(vm_id: &str) -> PathBuf {
    disk_dir().join(format!("{vm_id}-seed.iso"))
}

/// Create a qcow2 disk backed by the Ubuntu base image (copy-on-write — nearly instant).
///
/// Base image path: VM_BASE_IMAGE env var, falling back to the monorepo virt/work location.
/// If the base image is absent, returns an error — callers must ensure it exists.
pub fn create_boot_disk(vm_id: &str, size_gb: u32) -> Result<PathBuf, String> {
    let disk = disk_path_for(vm_id);
    if disk.exists() {
        return Ok(disk);
    }

    let base_image = std::env::var("VM_BASE_IMAGE").unwrap_or_else(|_| {
        "/srv/foundry/clones/project-infrastructure/infrastructure/virt/work/\
         ubuntu-24.04-server-cloudimg-amd64.img"
            .to_string()
    });

    if !std::path::Path::new(&base_image).exists() {
        return Err(format!(
            "base image not found at {base_image} — \
             download it with: curl -fL https://cloud-images.ubuntu.com/releases/noble/release/\
             ubuntu-24.04-server-cloudimg-amd64.img -o {base_image}"
        ));
    }

    if let Some(dir) = disk.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create VM_DISK_DIR {}: {e}", dir.display()))?;
    }

    // -b backing file: qcow2 writes go to disk, reads fall through to base image.
    // This is instantaneous regardless of base image size.
    let status = Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            "-F",
            "qcow2",
            "-b",
            &base_image,
            disk.to_str().unwrap(),
            &format!("{size_gb}G"),
        ])
        .status()
        .map_err(|e| format!("qemu-img not found: {e}"))?;

    if status.success() {
        Ok(disk)
    } else {
        Err(format!(
            "qemu-img create failed for {vm_id} (exit {:?})",
            status.code()
        ))
    }
}

/// Build a minimal cloud-init seed ISO for the given VM.
///
/// The foundry user is created with the fleet automation SSH pubkey from
/// VM_SSH_PUBKEY env var so the fleet controller can SSH into spawned VMs.
pub fn build_seed_iso(vm_id: &str, vm_type: &str) -> Result<PathBuf, String> {
    let work_dir = disk_dir();
    fs::create_dir_all(&work_dir).map_err(|e| format!("cannot create disk dir: {e}"))?;

    let meta_path = work_dir.join(format!("{vm_id}-meta-data"));
    let user_path = work_dir.join(format!("{vm_id}-user-data"));
    let seed_path = seed_iso_path_for(vm_id);

    if seed_path.exists() {
        return Ok(seed_path);
    }

    let ssh_pubkey = std::env::var("VM_SSH_PUBKEY").unwrap_or_default();

    let meta = format!("instance-id: {vm_id}\nlocal-hostname: {vm_type}\n");
    let user = if ssh_pubkey.is_empty() {
        format!(
            "#cloud-config\nhostname: {vm_type}\nusers:\n  - name: foundry\n    groups: [sudo]\n    \
             shell: /bin/bash\n    sudo: \"ALL=(ALL) NOPASSWD:ALL\"\nssh_pwauth: false\n"
        )
    } else {
        format!(
            "#cloud-config\nhostname: {vm_type}\nusers:\n  - name: foundry\n    groups: [sudo]\n    \
             shell: /bin/bash\n    sudo: \"ALL=(ALL) NOPASSWD:ALL\"\n    ssh_authorized_keys:\n      \
             - {ssh_pubkey}\nssh_pwauth: false\n"
        )
    };

    fs::write(&meta_path, meta.as_bytes()).map_err(|e| format!("cannot write meta-data: {e}"))?;
    fs::write(&user_path, user.as_bytes()).map_err(|e| format!("cannot write user-data: {e}"))?;

    let status = Command::new("genisoimage")
        .args([
            "-output",
            seed_path.to_str().unwrap(),
            "-volid",
            "cidata",
            "-joliet",
            "-rock",
            meta_path.to_str().unwrap(),
            user_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| {
            format!("genisoimage not found (install: sudo apt install genisoimage): {e}")
        })?;

    if status.success() {
        // Temp files no longer needed once ISO is built
        let _ = fs::remove_file(&meta_path);
        let _ = fs::remove_file(&user_path);
        Ok(seed_path)
    } else {
        Err(format!(
            "genisoimage failed for {vm_id} (exit {:?})",
            status.code()
        ))
    }
}

/// Derive an ephemeral SSH host port for a VM.
///
/// Uses the last 4 decimal digits of a simple FNV-1a hash of the vm_id,
/// clamped to the range 10000–19999.  No collision detection — acceptable for MVP.
fn ssh_host_port_for(vm_id: &str) -> u16 {
    // FNV-1a 32-bit, no external dependencies.
    let mut hash: u32 = 2_166_136_261;
    for byte in vm_id.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    let offset = hash % 10_000;
    10_000 + offset as u16
}

/// Spawn a QEMU process for the VM using user-mode networking.
///
/// Requires: disk image at disk_path_for(vm_id), seed ISO at seed_iso_path_for(vm_id).
/// Boots Ubuntu 24.04 cloud image with cloud-init from the seed ISO.
/// Monitor socket at {disk_dir}/{vm_id}.monitor.sock — discovered by service-vm-host's
/// qemu_monitor module on subsequent heartbeats.
///
/// Returns the updated VmRecord with host_ports populated.
pub fn spawn_qemu(record: &VmRecord, kvm: bool) -> Result<VmRecord, String> {
    let disk = disk_path_for(&record.vm_id);
    let seed = seed_iso_path_for(&record.vm_id);
    let monitor = monitor_sock_for(&record.vm_id);
    let pidfile = pid_path_for(&record.vm_id);

    if !disk.exists() {
        return Err(format!("disk not found: {}", disk.display()));
    }
    if !seed.exists() {
        return Err(format!("seed ISO not found: {}", seed.display()));
    }

    let host_ssh_port = ssh_host_port_for(&record.vm_id);
    let hostfwd_arg = format!("user,hostfwd=tcp::{host_ssh_port}-:22");

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-m",
        &record.ram_alloc_mb.to_string(),
        "-smp",
        &record.vcpu_count.to_string(),
        "-drive",
        &format!("file={},format=qcow2,if=virtio", disk.display()),
        "-drive",
        &format!(
            "file={},format=raw,if=virtio,media=cdrom,readonly=on",
            seed.display()
        ),
        "-monitor",
        &format!("unix:{},server,nowait", monitor.display()),
        "-pidfile",
        pidfile.to_str().unwrap(),
        "-net",
        "nic,model=virtio",
        "-net",
        &hostfwd_arg,
        "-display",
        "none",
        "-daemonize",
    ]);

    if kvm {
        cmd.arg("-enable-kvm");
    }

    let status = cmd
        .status()
        .map_err(|e| format!("qemu-system-x86_64 not found: {e}"))?;

    if status.success() {
        tracing::info!(
            vm_id = %record.vm_id,
            ram_mb = record.ram_alloc_mb,
            host_ssh_port,
            kvm,
            "QEMU process spawned"
        );

        let mut updated = record.clone();
        updated.host_ports = vec![HostPortMapping {
            host_port: host_ssh_port,
            guest_port: 22,
            protocol: "tcp".to_string(),
        }];

        // Persist metadata sidecar so qemu_monitor can reconstruct VmRecord fields from
        // process state on subsequent heartbeats (vm_type, ram, vcpu, tenant_id would
        // otherwise be lost once the spawn handler returns).
        if let Ok(json) = serde_json::to_string(&updated) {
            let _ = fs::write(meta_path_for(&record.vm_id), json);
        }
        Ok(updated)
    } else {
        Err(format!(
            "qemu-system-x86_64 exited {:?} for {}",
            status.code(),
            record.vm_id
        ))
    }
}

/// Terminate a running QEMU process by pidfile, then clean up runtime files.
/// The disk image is intentionally preserved.
pub fn kill_qemu(vm_id: &str) {
    let pidfile = pid_path_for(vm_id);
    if let Ok(contents) = fs::read_to_string(&pidfile) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .ok();
            tracing::info!(vm_id, pid, "sent SIGTERM to QEMU process");
        }
    }
    let _ = fs::remove_file(monitor_sock_for(vm_id));
    let _ = fs::remove_file(pidfile);
    let _ = fs::remove_file(meta_path_for(vm_id));
}

/// Synthesise a VmRecord representing a VM that has just been accepted for provisioning.
pub fn provisioning_record(vm_id: &str, vm_type: &str, ram_mb: u64, vcpu_count: u32) -> VmRecord {
    VmRecord {
        vm_id: vm_id.to_string(),
        vm_type: vm_type.to_string(),
        state: VmState::Provisioning,
        ram_alloc_mb: ram_mb,
        vcpu_count,
        started_at: None,
        tenant_id: None,
        host_ports: vec![],
    }
}
