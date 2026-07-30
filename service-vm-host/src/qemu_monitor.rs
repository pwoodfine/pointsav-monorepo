use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};
use system_vm_fleet_types::{VmRecord, VmState};

/// Poll all running QEMU processes via their UNIX monitor sockets.
///
/// Scans `VM_DISK_DIR` (default: `/var/lib/vm-fleet`) for `*.monitor.sock` files.
/// For each socket: connects, exchanges QMP handshake, queries running state.
/// Sockets that don't respond within 500 ms are skipped silently.
pub fn poll_running_vms() -> Vec<VmRecord> {
    let disk_dir = std::env::var("VM_DISK_DIR").unwrap_or_else(|_| "/var/lib/vm-fleet".to_string());

    let read_dir = match std::fs::read_dir(&disk_dir) {
        Ok(rd) => rd,
        Err(_) => return vec![],
    };

    read_dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_owned();
            let vm_id = name.strip_suffix(".monitor.sock")?.to_owned();
            query_qmp_socket(&path, &vm_id)
        })
        .collect()
}

fn query_qmp_socket(path: &std::path::Path, vm_id: &str) -> Option<VmRecord> {
    let timeout = Duration::from_millis(500);
    let stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;

    let mut writer = stream.try_clone().ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // Read QMP greeting banner.
    reader.read_line(&mut line).ok()?;
    line.clear();

    // Negotiate capabilities.
    writer
        .write_all(b"{\"execute\":\"qmp_capabilities\"}\n")
        .ok()?;
    reader.read_line(&mut line).ok()?;
    line.clear();

    // Query running state.
    writer.write_all(b"{\"execute\":\"query-status\"}\n").ok()?;
    reader.read_line(&mut line).ok()?;

    // QEMU reports {"return":{"running":true,...}} when the VM is running.
    if !line.contains("\"running\":true") {
        return None;
    }

    // Restore full VmRecord from the metadata sidecar written at spawn time.
    // Falls back to a minimal record for VMs not launched by this service-vm-host.
    let mut record = read_vm_meta(vm_id).unwrap_or_else(|| VmRecord {
        vm_id: vm_id.to_string(),
        vm_type: "unknown".to_string(),
        state: VmState::Running,
        ram_alloc_mb: 0,
        vcpu_count: 0,
        started_at: None,
        tenant_id: None,
        host_ports: vec![],
    });
    // vm_id and state come from the live process, not the sidecar.
    record.vm_id = vm_id.to_string();
    record.state = VmState::Running;
    Some(record)
}

fn read_vm_meta(vm_id: &str) -> Option<VmRecord> {
    let path = crate::vm_spawn::meta_path_for(vm_id);
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // VM_DISK_DIR is process-wide; serialise tests that mutate it to prevent races.
    static VM_DISK_DIR_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn empty_or_absent_disk_dir_returns_empty() {
        let _lock = VM_DISK_DIR_LOCK.lock().unwrap();
        std::env::set_var("VM_DISK_DIR", "/tmp/nonexistent-vm-monitor-test-dir");
        let vms = poll_running_vms();
        assert!(vms.is_empty());
    }

    #[test]
    fn dir_with_no_sockets_returns_empty() {
        let _lock = VM_DISK_DIR_LOCK.lock().unwrap();
        let dir = "/tmp/vm-monitor-test-no-socks";
        std::fs::create_dir_all(dir).unwrap();
        std::env::set_var("VM_DISK_DIR", dir);
        let vms = poll_running_vms();
        assert!(vms.is_empty());
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn read_vm_meta_returns_none_for_absent_file() {
        let _lock = VM_DISK_DIR_LOCK.lock().unwrap();
        std::env::set_var("VM_DISK_DIR", "/tmp/nonexistent-vm-meta-test-dir");
        assert!(read_vm_meta("no-such-vm").is_none());
    }

    #[test]
    fn read_vm_meta_round_trips_vm_record() {
        let _lock = VM_DISK_DIR_LOCK.lock().unwrap();
        use system_vm_fleet_types::VmState;
        let dir = "/tmp/vm-meta-round-trip-test";
        std::fs::create_dir_all(dir).unwrap();
        std::env::set_var("VM_DISK_DIR", dir);

        let vm_id = "test-vm-99";
        let record = VmRecord {
            vm_id: vm_id.to_string(),
            vm_type: "VmTest".to_string(),
            state: VmState::Running,
            ram_alloc_mb: 512,
            vcpu_count: 2,
            started_at: None,
            tenant_id: Some("tenant-abc".to_string()),
            host_ports: vec![],
        };
        let json = serde_json::to_string(&record).unwrap();
        std::fs::write(crate::vm_spawn::meta_path_for(vm_id), &json).unwrap();

        let got = read_vm_meta(vm_id).expect("meta file should be readable");
        assert_eq!(got.vm_type, "VmTest");
        assert_eq!(got.ram_alloc_mb, 512);
        assert_eq!(got.vcpu_count, 2);
        assert_eq!(got.tenant_id.as_deref(), Some("tenant-abc"));

        let _ = std::fs::remove_file(crate::vm_spawn::meta_path_for(vm_id));
        let _ = std::fs::remove_dir(dir);
    }
}
