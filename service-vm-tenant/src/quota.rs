use system_vm_fleet_types::VmRecord;

use crate::tenant::TenantConfig;

pub struct QuotaStatus {
    pub vms_running: u32,
    pub ram_used_mb: u64,
}

impl QuotaStatus {
    pub fn from_vms(vms: &[VmRecord]) -> Self {
        QuotaStatus {
            vms_running: vms.len() as u32,
            ram_used_mb: vms.iter().map(|v| v.ram_alloc_mb).sum(),
        }
    }
}

pub enum QuotaCheck {
    Ok,
    VmLimitExceeded {
        current: u32,
        max: u32,
    },
    RamLimitExceeded {
        current_mb: u64,
        requested_mb: u64,
        max_mb: u64,
    },
}

pub fn check(config: &TenantConfig, vms: &[VmRecord], requested_ram_mb: u64) -> QuotaCheck {
    let status = QuotaStatus::from_vms(vms);

    if status.vms_running >= config.max_vms {
        return QuotaCheck::VmLimitExceeded {
            current: status.vms_running,
            max: config.max_vms,
        };
    }

    let ram_after = status.ram_used_mb + requested_ram_mb;
    if ram_after > config.max_ram_mb {
        return QuotaCheck::RamLimitExceeded {
            current_mb: status.ram_used_mb,
            requested_mb: requested_ram_mb,
            max_mb: config.max_ram_mb,
        };
    }

    QuotaCheck::Ok
}
