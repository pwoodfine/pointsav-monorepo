#![no_std]

use core::arch::asm;

// PCI TYPE 1 config mechanism (x86 port I/O).
// CONFIG_ADDRESS = 0xCF8: enable bit[31]=1, bus[23:16], dev[15:11], fn[10:8], reg[7:2].
// CONFIG_DATA = 0xCFC: read/write 32-bit config register.
unsafe fn pci_config_read32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset & 0xFC) as u32);
    asm!(
        "out dx, eax",
        in("dx") 0xCF8_u16,
        in("eax") addr,
        options(nomem, nostack, preserves_flags)
    );
    let data: u32;
    asm!(
        "in eax, dx",
        in("dx") 0xCFC_u16,
        lateout("eax") data,
        options(nomem, nostack, preserves_flags)
    );
    data
}

/// Probe for the Broadcom BCM57765 PCI NIC (vendor 0x14E4, device 0x16B4).
/// Enumerates the full PCI bus tree via TYPE 1 config-space access.
/// Returns the 64-bit MMIO base address from BAR0/BAR1 if the controller is found.
pub fn probe_nic() -> Option<u64> {
    // Register 0x00 layout: device_id[31:16] | vendor_id[15:0]
    const TARGET: u32 = (0x16B4_u32 << 16) | 0x14E4;

    for bus in 0_u8..=255 {
        for dev in 0_u8..32 {
            for func in 0_u8..8 {
                let id = unsafe { pci_config_read32(bus, dev, func, 0x00) };
                // 0xFFFF_FFFF = no device; 0x0000_0000 = not implemented
                if id == 0xFFFF_FFFF || id == 0x0000_0000 {
                    continue;
                }
                if id != TARGET {
                    continue;
                }
                // BAR0 at offset 0x10; BAR1 at 0x14 (upper 32 bits for 64-bit BARs).
                let bar0 = unsafe { pci_config_read32(bus, dev, func, 0x10) };
                if bar0 & 0x1 != 0 {
                    continue; // I/O space BAR — BCM57765 BAR0 is always memory; skip anomaly
                }
                let base: u64 = if bar0 & 0x6 == 0x4 {
                    // 64-bit prefetchable memory BAR
                    let bar1 = unsafe { pci_config_read32(bus, dev, func, 0x14) };
                    ((bar1 as u64) << 32) | ((bar0 & 0xFFFF_FFF0) as u64)
                } else {
                    // 32-bit memory BAR
                    (bar0 & 0xFFFF_FFF0) as u64
                };
                return Some(base);
            }
        }
    }
    None
}

pub fn system_status() -> &'static str {
    "system-substrate-broadcom: PCI probe active (BCM57765)"
}
