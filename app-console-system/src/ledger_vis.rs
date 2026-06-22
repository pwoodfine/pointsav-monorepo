use std::time::{SystemTime, UNIX_EPOCH};

use system_core::{Capability, CapabilityType, LedgerAnchor, Right};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapVerdict {
    Allow,
    Revoked,
    Expired,
}

pub struct CapEntry {
    pub label: &'static str,
    pub cap: Capability,
}

pub struct LedgerLogEntry {
    pub height: u64,
    pub cap_label: String,
    pub action: &'static str,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn build_demo_caps() -> Vec<CapEntry> {
    let now = now_secs();
    let anchor = |origin: &'static str| LedgerAnchor {
        origin: origin.to_string(),
        tree_size: 1,
        root_hash: [0xAA; 32],
    };

    vec![
        CapEntry {
            label: "endpoint-a",
            cap: Capability {
                cap_type: CapabilityType::Endpoint,
                rights: vec![Right::Invoke, Right::Grant],
                expiry_t: None,
                witness_pubkey: None,
                ledger_anchor: anchor("demo.endpoint-a"),
            },
        },
        CapEntry {
            label: "memory-b",
            cap: Capability {
                cap_type: CapabilityType::Memory,
                rights: vec![Right::Read, Right::Write],
                expiry_t: None,
                witness_pubkey: None,
                ledger_anchor: anchor("demo.memory-b"),
            },
        },
        CapEntry {
            label: "irq-c",
            cap: Capability {
                cap_type: CapabilityType::Irq,
                rights: vec![Right::Invoke],
                expiry_t: Some(now.saturating_sub(3600)), // expired 1h ago
                witness_pubkey: None,
                ledger_anchor: anchor("demo.irq-c"),
            },
        },
        CapEntry {
            label: "notify-d",
            cap: Capability {
                cap_type: CapabilityType::Notification,
                rights: vec![Right::Invoke, Right::Read],
                expiry_t: None,
                witness_pubkey: None,
                ledger_anchor: anchor("demo.notify-d"),
            },
        },
        CapEntry {
            label: "cnode-e",
            cap: Capability {
                cap_type: CapabilityType::CNode,
                rights: vec![Right::Grant, Right::Revoke],
                expiry_t: Some(now + 3600 * 24 * 30), // 30 days
                witness_pubkey: None,
                ledger_anchor: anchor("demo.cnode-e"),
            },
        },
    ]
}

/// Compute a verdict by querying the live ledger's revocation set + expiry check.
/// This uses the real `RevocationSet::contains()` call — no mock.
pub fn compute_verdict(cap: &Capability, ledger: &system_ledger::InMemoryLedger) -> CapVerdict {
    if ledger.revocations.contains(&cap.hash()) {
        return CapVerdict::Revoked;
    }
    if let Some(expiry) = cap.expiry_t {
        if now_secs() >= expiry {
            return CapVerdict::Expired;
        }
    }
    CapVerdict::Allow
}

pub fn cap_type_label(ct: &CapabilityType) -> &'static str {
    match ct {
        CapabilityType::Endpoint     => "Endpoint",
        CapabilityType::Memory       => "Memory  ",
        CapabilityType::Irq          => "IRQ     ",
        CapabilityType::Notification => "Notify  ",
        CapabilityType::CNode        => "CNode   ",
    }
}

pub fn rights_label(rights: &[Right]) -> String {
    rights
        .iter()
        .map(|r| match r {
            Right::Read   => "read",
            Right::Write  => "write",
            Right::Invoke => "invoke",
            Right::Grant  => "grant",
            Right::Revoke => "revoke",
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn expiry_label(expiry_t: Option<u64>) -> String {
    match expiry_t {
        None    => "none     ".to_string(),
        Some(t) => {
            let now = now_secs();
            if now >= t {
                "EXPIRED  ".to_string()
            } else {
                let secs = t - now;
                let days = secs / 86400;
                if days > 0 {
                    format!("in {days:2}d    ")
                } else {
                    format!("in {:2}h    ", secs / 3600)
                }
            }
        }
    }
}
