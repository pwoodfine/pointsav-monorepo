use chrono::Utc;
use std::collections::HashMap;
use system_vm_fleet_types::{FleetStatus, NodeHeartbeat, NodeId, NodeRecord, VmId, VmRecord};

const STALE_THRESHOLD_SECS: i64 = 30;

pub struct NodeRegistry {
    nodes: HashMap<NodeId, NodeEntry>,
}

struct NodeEntry {
    record: NodeRecord,
    /// VMs known to be on this node (from heartbeats or create requests)
    vms: HashMap<VmId, VmRecord>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        NodeRegistry {
            nodes: HashMap::new(),
        }
    }

    pub fn update_node(&mut self, hb: &NodeHeartbeat) {
        let ram_available = hb.ram_total_mb.saturating_sub(hb.ram_used_mb);
        let vm_count = hb.vms.len() as u32;

        let entry = self
            .nodes
            .entry(hb.node_id.clone())
            .or_insert_with(|| NodeEntry {
                record: NodeRecord {
                    node_id: hb.node_id.clone(),
                    hostname: hb.hostname.clone(),
                    wg_ip: hb.wg_ip.clone(),
                    ram_available_mb: ram_available,
                    vm_count,
                    kvm_available: hb.kvm_available,
                    reserved: hb.reserved,
                    last_heartbeat: hb.timestamp_utc,
                },
                vms: HashMap::new(),
            });

        entry.record.hostname = hb.hostname.clone();
        entry.record.wg_ip = hb.wg_ip.clone();
        entry.record.ram_available_mb = ram_available;
        entry.record.vm_count = vm_count;
        entry.record.kvm_available = hb.kvm_available;
        entry.record.reserved = hb.reserved;
        entry.record.last_heartbeat = hb.timestamp_utc;

        // Sync VM records from heartbeat
        entry.vms.clear();
        for vm in &hb.vms {
            entry.vms.insert(vm.vm_id.clone(), vm.clone());
        }
    }

    /// Remove nodes that have not sent a heartbeat within STALE_THRESHOLD_SECS.
    pub fn evict_stale(&mut self) {
        let now = Utc::now();
        self.nodes.retain(|_, e| {
            let age = now
                .signed_duration_since(e.record.last_heartbeat)
                .num_seconds();
            if age > STALE_THRESHOLD_SECS {
                tracing::warn!(
                    node_id = %e.record.node_id,
                    age_secs = age,
                    "evicting stale node"
                );
                false
            } else {
                true
            }
        });
    }

    pub fn fleet_status(&self) -> FleetStatus {
        FleetStatus {
            nodes: self.nodes.values().map(|e| e.record.clone()).collect(),
            last_updated: Utc::now(),
        }
    }

    pub fn get_node(&self, node_id: &str) -> Option<NodeRecord> {
        self.nodes.get(node_id).map(|e| e.record.clone())
    }

    pub fn nodes_iter(&self) -> impl Iterator<Item = &NodeRecord> {
        self.nodes.values().map(|e| &e.record)
    }

    pub fn all_nodes(&self) -> Vec<NodeRecord> {
        self.nodes.values().map(|e| e.record.clone()).collect()
    }

    pub fn register_vm(&mut self, node_id: &str, vm: VmRecord) {
        if let Some(entry) = self.nodes.get_mut(node_id) {
            entry.vms.insert(vm.vm_id.clone(), vm);
            entry.record.vm_count = entry.vms.len() as u32;
        }
    }

    /// Remove a VM from whatever node owns it. Returns true if found.
    pub fn remove_vm(&mut self, vm_id: &str) -> bool {
        for entry in self.nodes.values_mut() {
            if entry.vms.remove(vm_id).is_some() {
                entry.record.vm_count = entry.vms.len() as u32;
                return true;
            }
        }
        false
    }

    /// Return the wg_ip of the node that owns the given VM, if any.
    pub fn find_vm_node_wg_ip(&self, vm_id: &str) -> Option<String> {
        self.nodes
            .values()
            .find(|e| e.vms.contains_key(vm_id))
            .map(|e| e.record.wg_ip.clone())
    }

    /// All VMs across all nodes, optionally filtered by tenant_id.
    pub fn all_vms(&self, tenant_filter: Option<&str>) -> Vec<VmRecord> {
        self.nodes
            .values()
            .flat_map(|e| e.vms.values().cloned())
            .filter(|vm| match tenant_filter {
                Some(t) => vm.tenant_id.as_deref() == Some(t),
                None => true,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use system_vm_fleet_types::NodeHeartbeat;

    fn make_heartbeat(node_id: &str, ram_total: u64, ram_used: u64) -> NodeHeartbeat {
        NodeHeartbeat {
            node_id: node_id.to_string(),
            wg_ip: "10.8.0.9".to_string(),
            hostname: node_id.to_string(),
            ram_total_mb: ram_total,
            ram_used_mb: ram_used,
            cpu_cores: 4,
            cpu_load_pct: 5.0,
            kvm_available: true,
            reserved: false,
            vms: vec![],
            boot_id: "boot-1".to_string(),
            timestamp_utc: Utc::now(),
        }
    }

    #[test]
    fn node_registers_on_heartbeat() {
        let mut reg = NodeRegistry::new();
        reg.update_node(&make_heartbeat("node-a", 8192, 2048));
        assert!(reg.get_node("node-a").is_some());
        assert_eq!(reg.get_node("node-a").unwrap().ram_available_mb, 6144);
    }

    #[test]
    fn stale_node_evicted() {
        use chrono::Duration;
        let mut reg = NodeRegistry::new();
        let mut hb = make_heartbeat("stale-node", 8192, 1024);
        // Backdate the heartbeat by 60 seconds to trigger eviction
        hb.timestamp_utc = Utc::now() - Duration::seconds(60);
        reg.update_node(&hb);
        reg.evict_stale();
        assert!(reg.get_node("stale-node").is_none());
    }

    #[test]
    fn fresh_node_survives_eviction() {
        let mut reg = NodeRegistry::new();
        reg.update_node(&make_heartbeat("fresh-node", 8192, 1024));
        reg.evict_stale();
        assert!(reg.get_node("fresh-node").is_some());
    }

    #[test]
    fn all_nodes_returns_registered_nodes() {
        let mut reg = NodeRegistry::new();
        assert!(reg.all_nodes().is_empty());
        reg.update_node(&make_heartbeat("node-a", 8192, 2048));
        reg.update_node(&make_heartbeat("node-b", 16384, 4096));
        assert_eq!(reg.all_nodes().len(), 2);
    }

    #[test]
    fn remove_vm_decrements_count() {
        let mut reg = NodeRegistry::new();
        reg.update_node(&make_heartbeat("node-a", 8192, 2048));
        use system_vm_fleet_types::{VmRecord, VmState};
        let vm = VmRecord {
            vm_id: "vm-1".to_string(),
            vm_type: "VmMediaKit".to_string(),
            state: VmState::Running,
            ram_alloc_mb: 2048,
            vcpu_count: 2,
            started_at: None,
            tenant_id: None,
            host_ports: vec![],
        };
        reg.register_vm("node-a", vm);
        assert_eq!(reg.get_node("node-a").unwrap().vm_count, 1);
        assert!(reg.remove_vm("vm-1"));
        assert_eq!(reg.get_node("node-a").unwrap().vm_count, 0);
    }
}
