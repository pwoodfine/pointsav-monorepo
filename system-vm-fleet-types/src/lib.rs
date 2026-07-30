use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a PPN infrastructure node.
pub type NodeId = String;

/// Unique identifier for a virtual machine instance.
pub type VmId = String;

/// Heartbeat sent by service-vm-host to service-vm-fleet every 10 seconds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeHeartbeat {
    pub node_id: NodeId,
    pub wg_ip: String,
    pub hostname: String,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub cpu_cores: u32,
    pub cpu_load_pct: f32,
    /// Whether /dev/kvm is present on this node (hardware-accelerated virtualization).
    pub kvm_available: bool,
    /// When true, this node is reserved for other purposes and should only receive
    /// VM placement as a last resort (all non-reserved nodes are exhausted first).
    /// Set via VM_RESERVED=true in the node's environment.
    #[serde(default)]
    pub reserved: bool,
    pub vms: Vec<VmRecord>,
    pub boot_id: String,
    pub timestamp_utc: DateTime<Utc>,
}

/// State of a single virtual machine on a node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum VmState {
    Running,
    Stopped,
    Provisioning,
    Error,
}

/// A single host→guest port forwarding rule for QEMU SLIRP networking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostPortMapping {
    /// Port number bound on the host (hypervisor) side.
    pub host_port: u16,
    /// Port number inside the guest VM.
    pub guest_port: u16,
    /// Protocol: "tcp" or "udp".
    pub protocol: String,
}

/// Record of a single VM as reported by the host agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VmRecord {
    pub vm_id: VmId,
    pub vm_type: String,
    pub state: VmState,
    pub ram_alloc_mb: u64,
    pub vcpu_count: u32,
    pub started_at: Option<DateTime<Utc>>,
    /// Tenant namespace set by service-vm-tenant when the VM was created.
    /// None for VMs created directly via fleet API (no tenant proxy).
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// SLIRP host→guest port forwarding rules established at spawn time.
    /// Empty for VMs without explicit port mappings.
    #[serde(default)]
    pub host_ports: Vec<HostPortMapping>,
}

/// Advisory placement result returned by service-vm-fleet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacementAdvice {
    pub recommended_node: NodeId,
    pub reason: String,
    pub alternatives: Vec<NodeId>,
}

/// Snapshot of the entire fleet as known by service-vm-fleet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FleetStatus {
    pub nodes: Vec<NodeRecord>,
    pub last_updated: DateTime<Utc>,
}

/// Summary of a single node as tracked by service-vm-fleet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeRecord {
    pub node_id: NodeId,
    pub hostname: String,
    pub wg_ip: String,
    pub ram_available_mb: u64,
    pub vm_count: u32,
    /// Whether /dev/kvm is present on this node, as last reported by its heartbeat.
    pub kvm_available: bool,
    /// Node is reserved for other purposes; only used as last-resort placement fallback.
    #[serde(default)]
    pub reserved: bool,
    pub last_heartbeat: DateTime<Utc>,
}

/// Request to create a new virtual machine.
///
/// VM-Totebox instances MUST provide preferred_node — WORM archive data cannot migrate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateVmRequest {
    pub vm_type: String,
    pub ram_mb: u64,
    pub vcpu_count: u32,
    /// Request a KVM-capable node. Falls back to TCG nodes when no KVM node has enough RAM.
    #[serde(default)]
    pub prefer_kvm: bool,
    /// If Some, skip advisory placement and dispatch directly to this node.
    pub preferred_node: Option<NodeId>,
    /// Tenant namespace injected by service-vm-tenant before forwarding to fleet.
    /// Direct fleet callers leave this None; the tenant proxy always sets it.
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_heartbeat() -> NodeHeartbeat {
        NodeHeartbeat {
            node_id: "gcp-cloud-1".to_string(),
            wg_ip: "10.8.0.9".to_string(),
            hostname: "foundry-workspace".to_string(),
            ram_total_mb: 8192,
            ram_used_mb: 2048,
            cpu_cores: 4,
            cpu_load_pct: 12.5,
            kvm_available: true,
            reserved: false,
            vms: vec![],
            boot_id: "abc-123".to_string(),
            timestamp_utc: Utc::now(),
        }
    }

    #[test]
    fn heartbeat_round_trips_json() {
        let hb = sample_heartbeat();
        let json = serde_json::to_string(&hb).unwrap();
        let decoded: NodeHeartbeat = serde_json::from_str(&json).unwrap();
        assert_eq!(hb.node_id, decoded.node_id);
        assert_eq!(hb.ram_total_mb, decoded.ram_total_mb);
        assert_eq!(hb.vms.len(), decoded.vms.len());
    }

    #[test]
    fn vm_record_state_serialises_as_pascal_case() {
        let rec = VmRecord {
            vm_id: "vm-1".to_string(),
            vm_type: "VmMediaKit".to_string(),
            state: VmState::Running,
            ram_alloc_mb: 2048,
            vcpu_count: 2,
            started_at: None,
            tenant_id: None,
            host_ports: vec![],
        };
        let json = serde_json::to_string(&rec).unwrap();
        assert!(
            json.contains("\"Running\""),
            "state must serialise as PascalCase"
        );
    }

    #[test]
    fn host_port_mapping_round_trips_json() {
        let rec = VmRecord {
            vm_id: "vm-ssh-1".to_string(),
            vm_type: "VmTotebox".to_string(),
            state: VmState::Provisioning,
            ram_alloc_mb: 4096,
            vcpu_count: 2,
            started_at: None,
            tenant_id: Some("alice".to_string()),
            host_ports: vec![HostPortMapping {
                host_port: 10022,
                guest_port: 22,
                protocol: "tcp".to_string(),
            }],
        };
        let json = serde_json::to_string(&rec).unwrap();
        let decoded: VmRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.host_ports.len(), 1);
        assert_eq!(decoded.host_ports[0].host_port, 10022);
        assert_eq!(decoded.host_ports[0].guest_port, 22);
        assert_eq!(decoded.host_ports[0].protocol, "tcp");
    }

    #[test]
    fn vm_record_missing_host_ports_defaults_to_empty() {
        // Older VmRecord JSON without host_ports field must deserialise cleanly.
        let json = r#"{"vm_id":"vm-old","vm_type":"VmMediaKit","state":"Running","ram_alloc_mb":2048,"vcpu_count":2,"started_at":null}"#;
        let rec: VmRecord = serde_json::from_str(json).unwrap();
        assert!(rec.host_ports.is_empty());
    }

    #[test]
    fn fleet_status_round_trips_json() {
        let status = FleetStatus {
            nodes: vec![NodeRecord {
                node_id: "laptop-a-1".to_string(),
                hostname: "laptop-a".to_string(),
                wg_ip: "10.8.0.6".to_string(),
                ram_available_mb: 6144,
                vm_count: 0,
                kvm_available: true,
                reserved: false,
                last_heartbeat: Utc::now(),
            }],
            last_updated: Utc::now(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let decoded: FleetStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.nodes.len(), 1);
        assert_eq!(decoded.nodes[0].node_id, "laptop-a-1");
    }

    #[test]
    fn create_vm_request_preferred_node_optional() {
        let with_pref = CreateVmRequest {
            vm_type: "VmTotebox".to_string(),
            ram_mb: 4096,
            vcpu_count: 2,
            prefer_kvm: true,
            preferred_node: Some("laptop-a-1".to_string()),
            tenant_id: None,
        };
        let without_pref = CreateVmRequest {
            vm_type: "VmMediaKit".to_string(),
            ram_mb: 2048,
            vcpu_count: 2,
            prefer_kvm: false,
            preferred_node: None,
            tenant_id: None,
        };
        let j1 = serde_json::to_string(&with_pref).unwrap();
        let j2 = serde_json::to_string(&without_pref).unwrap();
        let d1: CreateVmRequest = serde_json::from_str(&j1).unwrap();
        let d2: CreateVmRequest = serde_json::from_str(&j2).unwrap();
        assert_eq!(d1.preferred_node, Some("laptop-a-1".to_string()));
        assert_eq!(d2.preferred_node, None);
    }

    #[test]
    fn prefer_kvm_defaults_to_false_when_absent() {
        // Callers that omit prefer_kvm (e.g. older clients) get false — opt-in behaviour.
        let json = r#"{"vm_type":"VmMediaKit","ram_mb":2048,"vcpu_count":2}"#;
        let req: CreateVmRequest = serde_json::from_str(json).unwrap();
        assert!(!req.prefer_kvm);
    }
}
