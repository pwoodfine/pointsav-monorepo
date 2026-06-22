use std::{path::PathBuf, process::Command};
use system_vm_fleet_types::VmRecord;

/// Directory where VM disk images and runtime files land.
/// Override with `VM_DISK_DIR` environment variable.
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

fn pid_file_for(vm_id: &str) -> PathBuf {
    disk_dir().join(format!("{vm_id}.pid"))
}

/// Create a blank qcow2 disk image. Idempotent — skips if image already exists.
pub fn create_blank_disk(vm_id: &str, size_gb: u32) -> Result<PathBuf, String> {
    let path = disk_path_for(vm_id);
    if path.exists() {
        return Ok(path);
    }
    // Ensure parent directory exists.
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create VM_DISK_DIR {}: {e}", dir.display()))?;
    }
    let status = Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            path.to_str().unwrap(),
            &format!("{size_gb}G"),
        ])
        .status()
        .map_err(|e| format!("qemu-img not found: {e}"))?;
    if status.success() {
        Ok(path)
    } else {
        Err(format!(
            "qemu-img create failed for {vm_id} (exit {:?})",
            status.code()
        ))
    }
}

/// Spawn a QEMU process for the VM in daemonized mode.
///
/// Uses user-mode networking (no tap/bridge required) — works on GCP without
/// elevated net privileges. KVM flag conditional on `kvm` param.
/// Monitor socket lands at `{disk_dir}/{vm_id}.monitor.sock` so that
/// `service-vm-host` can discover it via `/var/lib/vm-fleet/*.monitor.sock`.
pub fn spawn_qemu(record: &VmRecord, kvm: bool) -> Result<(), String> {
    let disk = disk_path_for(&record.vm_id);
    let monitor = monitor_sock_for(&record.vm_id);
    let pidfile = pid_file_for(&record.vm_id);

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args([
        "-m",
        &record.ram_alloc_mb.to_string(),
        "-smp",
        &record.vcpu_count.to_string(),
        "-drive",
        &format!("file={},format=qcow2,if=virtio", disk.display()),
        "-monitor",
        &format!("unix:{},server,nowait", monitor.display()),
        "-pidfile",
        pidfile.to_str().unwrap(),
        "-net",
        "nic,model=virtio",
        "-net",
        "user",
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
            kvm,
            "QEMU process spawned"
        );
        Ok(())
    } else {
        Err(format!(
            "qemu-system-x86_64 exited {:?} for {}",
            status.code(),
            record.vm_id
        ))
    }
}

/// Terminate a running QEMU process and clean up its runtime files (socket, pidfile).
/// The disk image is intentionally preserved — it contains tenant data.
pub fn kill_qemu(vm_id: &str) {
    let pidfile = pid_file_for(vm_id);
    if let Ok(contents) = std::fs::read_to_string(&pidfile) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .ok();
            tracing::info!(vm_id, pid, "sent SIGTERM to QEMU process");
        }
    }
    let _ = std::fs::remove_file(monitor_sock_for(vm_id));
    let _ = std::fs::remove_file(pidfile);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that mutate VM_DISK_DIR to prevent env-var races
    // between parallel test threads.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn disk_path_has_vm_id_and_extension() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("VM_DISK_DIR", "/tmp/test-vm-fleet");
        let p = disk_path_for("vm-totebox-123");
        assert!(p.to_str().unwrap().contains("vm-totebox-123"));
        assert!(p.to_str().unwrap().ends_with(".qcow2"));
    }

    #[test]
    fn monitor_sock_and_disk_share_parent_dir() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("VM_DISK_DIR", "/tmp/test-vm-fleet");
        let disk = disk_path_for("vm-x");
        let sock = monitor_sock_for("vm-x");
        assert_eq!(disk.parent(), sock.parent());
    }

    #[test]
    fn create_blank_disk_is_idempotent_if_file_exists() {
        let _g = ENV_LOCK.lock().unwrap();
        use std::io::Write;
        std::env::set_var("VM_DISK_DIR", "/tmp/test-vm-fleet-idem");
        std::fs::create_dir_all("/tmp/test-vm-fleet-idem").unwrap();
        let path = disk_path_for("vm-idempotent");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"x")
            .unwrap();
        // Should return Ok without calling qemu-img.
        let result = create_blank_disk("vm-idempotent", 8);
        assert!(result.is_ok());
        std::fs::remove_file(path).unwrap();
    }
}
