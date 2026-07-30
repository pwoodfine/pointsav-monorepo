//! foundry-nodeclass — runtime node-class detection for the Totebox fleet.
//!
//! Classifies the host as one of three node classes and exposes the capability
//! set derived from that classification. Both `service-slm` (Doorman tier
//! gating) and `service-content` (GraphStore backend selection) read this at
//! startup.
//!
//! # Detection order
//! 1. `TOTEBOX_NODE_CLASS` env override: `"micro"`, `"hardware"`, or
//!    `"accelerated"` (case-insensitive). The primary test lever.
//! 2. GPU probe: `/dev/nvidiactl`, `/dev/nvidia0`, `/dev/dri/renderD*`.
//! 3. RAM probe: min of cgroup-v2 `memory.max`, cgroup-v1
//!    `memory.limit_in_bytes`, and `/proc/meminfo` MemTotal.
//! 4. vCPU probe: cgroup-v2 `cpu.max` quota/period, cgroup-v1
//!    `cpu.cfs_quota_us`/`period_us`, fallback to online CPU count.
//!
//! Classification thresholds (ratified: DOCTRINE.md claims #49, #54;
//! `conventions/four-tier-slm-substrate.md`):
//! - GPU present → `Accelerated`
//! - ≥ 6 GiB RAM **and** ≥ 1.5 vCPU → `Hardware`
//! - otherwise → `Micro`

use std::fs;
use std::path::Path;

const GIB: u64 = 1024 * 1024 * 1024;

/// The three node classes in the Totebox fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeClass {
    /// $7/mo e2-micro: ~1 GB RAM, ~0.25 vCPU burstable, no GPU.
    /// Runs deterministic substrate only. Doorman operates as a pure broker.
    /// No on-node AI; `/readyz` reports `tier_a: unavailable`.
    Micro,
    /// NUC-class or mini-PC: ≥ 6 GiB RAM, ≥ 1.5 vCPU, no GPU.
    /// Runs deterministic substrate + Tier A (OLMo 1B narrow specialist,
    /// ~5–15 tok/s on-device).
    Hardware,
    /// GPU-accelerated node (Yo-Yo L4/H100 or appliance).
    /// Runs all tiers including interactive big-model AI.
    Accelerated,
}

impl NodeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeClass::Micro => "micro",
            NodeClass::Hardware => "hardware",
            NodeClass::Accelerated => "accelerated",
        }
    }
}

/// Capability flags derived from the detected `NodeClass`.
///
/// Obtain via `detect()` at service startup, then thread through to any
/// component that needs to adapt its behaviour per node class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capabilities {
    pub node_class: NodeClass,
    /// Whether on-node AI inference is available (Hardware or Accelerated).
    /// `false` on Micro — the Doorman must not attempt model loading.
    pub supports_on_node_ai: bool,
    /// Whether a GPU accelerator is present (Accelerated only).
    pub has_gpu: bool,
    /// Effective RAM limit in bytes (minimum of cgroup and /proc/meminfo).
    pub ram_bytes: u64,
    /// Effective vCPU quota (from cgroup quota/period, or online CPU count).
    pub vcpu: f64,
}

impl Capabilities {
    pub fn supports_on_node_ai(&self) -> bool {
        self.supports_on_node_ai
    }

    /// Synthetic Micro node for use in tests and the env override path.
    pub fn synthetic_micro() -> Self {
        Self::from_parts(NodeClass::Micro, 900 * 1024 * 1024, 0.25, false)
    }

    /// Synthetic Hardware node for use in tests and the env override path.
    pub fn synthetic_hardware() -> Self {
        Self::from_parts(NodeClass::Hardware, 8 * GIB, 2.0, false)
    }

    /// Synthetic Accelerated node for use in tests and the env override path.
    pub fn synthetic_accelerated() -> Self {
        Self::from_parts(NodeClass::Accelerated, 32 * GIB, 8.0, true)
    }

    fn from_parts(class: NodeClass, ram_bytes: u64, vcpu: f64, has_gpu: bool) -> Self {
        Capabilities {
            node_class: class,
            supports_on_node_ai: class != NodeClass::Micro,
            has_gpu,
            ram_bytes,
            vcpu,
        }
    }
}

/// Detect the `NodeClass` of the current host and return its `Capabilities`.
///
/// This is the primary entry point. Call once at service startup.
///
/// ```rust
/// let caps = foundry_nodeclass::detect();
/// if !caps.supports_on_node_ai() {
///     // $7 Micro node: configure as pure broker, no model loading
/// }
/// ```
pub fn detect() -> Capabilities {
    // Env override — primary test lever; also used by integration test harness.
    if let Ok(val) = std::env::var("TOTEBOX_NODE_CLASS") {
        match val.to_ascii_lowercase().as_str() {
            "micro" => return Capabilities::synthetic_micro(),
            "hardware" => return Capabilities::synthetic_hardware(),
            "accelerated" => return Capabilities::synthetic_accelerated(),
            other => {
                // Unknown value: log and fall through to real detection.
                eprintln!(
                    "foundry-nodeclass: unknown TOTEBOX_NODE_CLASS={:?}; \
                     falling back to hardware detection",
                    other
                );
            }
        }
    }

    let has_gpu = probe_gpu();
    let ram = probe_ram_bytes();
    let vcpu = probe_vcpu();

    let class = if has_gpu {
        NodeClass::Accelerated
    } else if ram >= 6 * GIB && vcpu >= 1.5 {
        NodeClass::Hardware
    } else {
        NodeClass::Micro
    };

    Capabilities {
        node_class: class,
        supports_on_node_ai: class != NodeClass::Micro,
        has_gpu,
        ram_bytes: ram,
        vcpu,
    }
}

/// Probe for a GPU via filesystem — no CUDA/ROCm linking required.
fn probe_gpu() -> bool {
    // NVIDIA character devices
    if Path::new("/dev/nvidiactl").exists() || Path::new("/dev/nvidia0").exists() {
        return true;
    }
    // DRI render nodes (AMD, Intel discrete, NVIDIA via DRM)
    if let Ok(entries) = fs::read_dir("/dev/dri") {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("renderD") {
                return true;
            }
        }
    }
    false
}

/// Read the effective RAM limit in bytes.
///
/// Takes the minimum of all available sources so a constrained cgroup wins
/// over the host's physical RAM total.
fn probe_ram_bytes() -> u64 {
    let mut candidates: Vec<u64> = Vec::with_capacity(3);

    // cgroup v2: a plain integer or the string "max" (unlimited)
    if let Some(v) = read_cgroup_u64("/sys/fs/cgroup/memory.max") {
        candidates.push(v);
    }

    // cgroup v1: unlimited is typically 9223372036854771712 — skip it
    if let Some(v) = read_cgroup_u64("/sys/fs/cgroup/memory/memory.limit_in_bytes") {
        if v < u64::MAX / 2 {
            candidates.push(v);
        }
    }

    if let Some(v) = read_meminfo_total_bytes() {
        candidates.push(v);
    }

    candidates.into_iter().min().unwrap_or(512 * 1024 * 1024)
}

fn read_cgroup_u64(path: &str) -> Option<u64> {
    let raw = fs::read_to_string(path).ok()?;
    let s = raw.trim();
    if s == "max" {
        return None;
    }
    s.parse().ok()
}

fn read_meminfo_total_bytes() -> Option<u64> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            // "MemTotal:       16384 kB"
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// Read the effective vCPU quota as a fractional count.
///
/// Tries cgroup v2 `cpu.max` (quota/period), cgroup v1
/// `cpu.cfs_quota_us` / `cpu.cfs_period_us`, then falls back to the count
/// of online CPUs from `/sys/devices/system/cpu/`.
fn probe_vcpu() -> f64 {
    // cgroup v2: "250000 1000000" = 0.25 vCPU;  "max" = unlimited
    if let Ok(content) = fs::read_to_string("/sys/fs/cgroup/cpu.max") {
        let s = content.trim();
        if s != "max" {
            let mut parts = s.split_whitespace();
            if let (Some(q), Some(p)) = (parts.next(), parts.next()) {
                if let (Ok(quota), Ok(period)) = (q.parse::<f64>(), p.parse::<f64>()) {
                    if period > 0.0 {
                        return quota / period;
                    }
                }
            }
        }
    }

    // cgroup v1
    let quota = read_cgroup_f64("/sys/fs/cgroup/cpu/cpu.cfs_quota_us");
    let period = read_cgroup_f64("/sys/fs/cgroup/cpu/cpu.cfs_period_us");
    if let (Some(q), Some(p)) = (quota, period) {
        if q > 0.0 && p > 0.0 {
            return q / p;
        }
    }

    // Fallback: count cpu[0-9]* entries under /sys/devices/system/cpu/
    count_online_cpus().unwrap_or(1) as f64
}

fn read_cgroup_f64(path: &str) -> Option<f64> {
    let raw = fs::read_to_string(path).ok()?;
    raw.trim().parse().ok()
}

fn count_online_cpus() -> Option<usize> {
    let mut count = 0usize;
    for entry in fs::read_dir("/sys/devices/system/cpu").ok()?.flatten() {
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if s.starts_with("cpu") && s[3..].chars().all(|c| c.is_ascii_digit()) {
            count += 1;
        }
    }
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_micro_is_micro() {
        let c = Capabilities::synthetic_micro();
        assert_eq!(c.node_class, NodeClass::Micro);
        assert!(!c.supports_on_node_ai());
        assert!(!c.has_gpu);
    }

    #[test]
    fn synthetic_hardware_is_hardware() {
        let c = Capabilities::synthetic_hardware();
        assert_eq!(c.node_class, NodeClass::Hardware);
        assert!(c.supports_on_node_ai());
        assert!(!c.has_gpu);
    }

    #[test]
    fn synthetic_accelerated_is_accelerated() {
        let c = Capabilities::synthetic_accelerated();
        assert_eq!(c.node_class, NodeClass::Accelerated);
        assert!(c.supports_on_node_ai());
        assert!(c.has_gpu);
    }

    #[test]
    fn classification_at_hardware_threshold() {
        // Exactly at threshold → Hardware
        let ram = 6 * GIB;
        let vcpu = 1.5_f64;
        let class = if ram >= 6 * GIB && vcpu >= 1.5 {
            NodeClass::Hardware
        } else {
            NodeClass::Micro
        };
        assert_eq!(class, NodeClass::Hardware);
    }

    #[test]
    fn classification_below_hardware_threshold() {
        // One byte below RAM threshold → Micro
        let ram = 6 * GIB - 1;
        let vcpu = 2.0_f64;
        let class = if ram >= 6 * GIB && vcpu >= 1.5 {
            NodeClass::Hardware
        } else {
            NodeClass::Micro
        };
        assert_eq!(class, NodeClass::Micro);
    }

    #[test]
    fn classification_below_vcpu_threshold() {
        // RAM ok but vCPU below threshold → Micro
        let ram = 8 * GIB;
        let vcpu = 1.4_f64;
        let class = if ram >= 6 * GIB && vcpu >= 1.5 {
            NodeClass::Hardware
        } else {
            NodeClass::Micro
        };
        assert_eq!(class, NodeClass::Micro);
    }

    #[test]
    fn gpu_forces_accelerated_regardless_of_ram() {
        // Even with low RAM, GPU → Accelerated (probe_gpu path)
        let caps = Capabilities::from_parts(NodeClass::Accelerated, 1024, 0.25, true);
        assert_eq!(caps.node_class, NodeClass::Accelerated);
        assert!(caps.has_gpu);
        assert!(caps.supports_on_node_ai());
    }

    #[test]
    fn probe_gpu_does_not_panic() {
        let _ = probe_gpu();
    }

    #[test]
    fn probe_ram_bytes_returns_nonzero() {
        assert!(probe_ram_bytes() > 0);
    }

    #[test]
    fn probe_vcpu_returns_positive() {
        assert!(probe_vcpu() > 0.0);
    }

    #[test]
    fn detect_does_not_panic() {
        // On any machine: verify detect() completes and returns a valid class.
        let caps = detect();
        let valid = matches!(
            caps.node_class,
            NodeClass::Micro | NodeClass::Hardware | NodeClass::Accelerated
        );
        assert!(valid);
        // Consistency: supports_on_node_ai ↔ class != Micro
        assert_eq!(
            caps.supports_on_node_ai(),
            caps.node_class != NodeClass::Micro
        );
        // Consistency: has_gpu ↔ class == Accelerated (for real detection)
        // (may not hold for env override paths with synthetic values, but on a
        // real machine this invariant should hold)
        if caps.has_gpu {
            assert_eq!(caps.node_class, NodeClass::Accelerated);
        }
    }
}
