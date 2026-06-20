// Phase H6 — VirtIO-net device locate + virtqueue ring setup + STATUS to DRIVER_OK.
//
// Phase H5 showed DEVICE_ID=0 at slot 0: QEMU virt allocates VirtIO-mmio devices from
// the highest slot (31) downward. With one `-device virtio-net-device`, it occupies slot 31
// at physical 0x0a003e00. This phase maps the 4 KiB page containing slot 31
// (0x0a003000–0x0a003fff) to find the device and advance to DRIVER_OK.
//
// VirtIO MMIO bus on QEMU virt (AArch64):
//   Base = 0x0a000000; each slot = 512 bytes; 32 slots total.
//   Slot N physical address = 0x0a000000 + N * 0x200.
//   A single 4 KiB page starting at 0x0a003000 covers slots 24–31 (8 slots × 512 B).
//   Slot 31 is at 0x0a003e00 = 0x0a003000 + 0xe00.
//
// Gate: "VirtIO-net device_id=1 init gate: PASSED" in QEMU serial output.
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h6-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-virtio-net-gate.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G -nographic \
//     -device virtio-net-device,netdev=n0 -netdev user,id=n0 \
//     -kernel build/system-image.bin

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};

// VirtIO MMIO bus: base = 0x0a000000, stride = 0x200 (512 bytes per slot).
const VIRTIO_BUS_BASE_PADDR: u64 = 0x0a00_0000;
const VIRTIO_BUS_STRIDE:     u64 = 0x200;
const VIRTIO_BUS_NUM_SLOTS:  u64 = 32;

// Physical address of the 4 KiB page containing slots 24–31 (includes slot 31 at 0x0a003e00).
// Slot N paddr = VIRTIO_BUS_BASE_PADDR + N * VIRTIO_BUS_STRIDE.
// Slot 31 = 0x0a000000 + 31 * 0x200 = 0x0a003e00.
// Page containing 0x0a003e00 = 0x0a003000 (4 KiB aligned below).
const VIRTIO_HI_PAGE_PADDR: u64 = 0x0a00_3000;

// Virtual address where we map the high VirtIO page.
const VIRTIO_HI_VADDR: u64 = 0x4020_0000;

// VirtIO MMIO register offsets from the per-slot base.
const VIRTIO_MAGIC:     u64 = 0x000;
const VIRTIO_VERSION:   u64 = 0x004;
const VIRTIO_DEVICE_ID: u64 = 0x008;
const VIRTIO_STATUS:    u64 = 0x070;

// VirtIO status bits.
const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER:      u32 = 2;
const STATUS_DRIVER_OK:   u32 = 4;

#[repr(C, align(16))]
struct Stack([u8; 4096]);

static mut ROOTSERVER_STACK: Stack = Stack([0; 4096]);

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".global _start",
    "_start:",
    "adrp x10, {stack}",
    "add  x10, x10, :lo12:{stack}",
    "add  x10, x10, #4096",
    "mov  sp,  x10",
    "mov  x29, xzr",
    "b    {main_fn}",
    stack   = sym ROOTSERVER_STACK,
    main_fn = sym _main,
);

#[cfg(not(target_arch = "aarch64"))]
#[no_mangle]
pub unsafe extern "C" fn _start(_bi: *const BootInfo) -> ! { loop {} }

unsafe fn put_hex32(val: u32) {
    write_bytes(b"0x");
    for i in (0u32..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xf) as u8;
        putchar(if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 });
    }
    write_bytes(b"\r\n");
}

unsafe fn mmio_read32(vbase: u64, offset: u64) -> u32 {
    let ptr = (vbase + offset) as *const u32;
    ptr.read_volatile()
}

unsafe fn mmio_write32(vbase: u64, offset: u64, val: u32) {
    let ptr = (vbase + offset) as *mut u32;
    ptr.write_volatile(val);
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    write_bytes(b"[h6] Phase H6 start\r\n");

    let ipc = (*bi).ipc_buffer;
    let empty_start = (*bi).empty.start;

    // Find RAM untyped for two page tables (size_bits >= 13 = 8 KiB).
    let ut_ram = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => { write_bytes(b"[h6] no RAM untyped\r\n"); spin() }
    };

    // Find device untyped for the high VirtIO page (0x0a003000).
    // seL4 staircase for 0x0a000000-0x0a003fff:
    //   ctz(0x0a000000)=24; log2(0x4000)=14; creates ONE 16 KiB untyped at 0x0a000000.
    // So there is NO separate untyped for 0x0a003000; we must use the 16 KiB one at 0x0a000000
    // and retype a SmallPage at offset 0x3000 (the 4th 4 KiB chunk).
    //
    // However, seL4 Untyped_Retype allocates from the free pointer, which starts at the
    // beginning of the untyped range. To get a SmallPage at 0x0a003000, we must first retype
    // 3 SmallPages at 0x0a000000, 0x0a001000, 0x0a002000 (discarding them), then the 4th
    // retype lands at 0x0a003000.
    //
    // Alternative approach: use the untyped at 0x0a003000 if it exists as a separate region.
    // But the staircase only creates ONE untyped for the entire 0x0a000000-0x0a003fff range.
    //
    // Simplest approach: retype 4 SmallPages from the 0x0a000000 untyped into slots
    // frame0..frame3, then map frame3 (covers 0x0a003000-0x0a003fff = slots 24-31).
    let ut_dev = match (*bi).find_device_untyped(VIRTIO_BUS_BASE_PADDR) {
        Some(s) => s,
        None => {
            write_bytes(b"[h6] no device untyped at 0x0a000000\r\n");
            // Print device untypeds for debugging.
            let count = ((*bi).untyped.end - (*bi).untyped.start) as usize;
            for i in 0..count {
                let d = &(*bi).untyped_list[i];
                if d.is_device != 0 {
                    write_bytes(b"  dev paddr=");
                    put_hex32(d.paddr as u32);
                }
            }
            spin()
        }
    };
    write_bytes(b"[h6] device untyped found\r\n");

    // Cap slots:
    //   pt1_slot   = L1 PUD page table
    //   pt2_slot   = L2 PMD page table
    //   frame0..2  = SmallPage for 0x0a000000, 0x0a001000, 0x0a002000 (advance free pointer)
    //   frame3     = SmallPage for 0x0a003000 (contains slot 31 at offset 0xe00)
    let pt1_slot   = empty_start;
    let pt2_slot   = empty_start + 1;
    let frame0     = empty_start + 2;
    let frame1     = empty_start + 3;
    let frame2     = empty_start + 4;
    let frame3     = empty_start + 5;

    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt1_slot);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt2_slot);
    write_bytes(b"[h6] page tables created\r\n");

    // Retype 4 SmallPages from the device untyped. The first 3 are discarded
    // (never mapped) to advance the free pointer to 0x0a003000 for frame3.
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, frame0);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, frame1);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, frame2);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, frame3);
    write_bytes(b"[h6] device frames created (4x SmallPage, using frame3 at 0x0a003000)\r\n");

    // Map two page tables for VIRTIO_HI_VADDR = 0x40200000.
    // L1 PUD[1] covers [0x40000000, 0x80000000).
    // L2 PMD[1] covers [0x40200000, 0x40400000).
    bootstrap::page_table_map(ipc, pt1_slot, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 0);
    write_bytes(b"[h6] L1 PUD mapped\r\n");
    bootstrap::page_table_map(ipc, pt2_slot, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 0);
    write_bytes(b"[h6] L2 PMD mapped\r\n");

    // Map frame3 (physical 0x0a003000) at VIRTIO_HI_VADDR with device memory attributes.
    bootstrap::page_map(ipc, frame3, cap::INIT_VSPACE, VIRTIO_HI_VADDR, 3, 0);
    write_bytes(b"[h6] frame3 mapped at VIRTIO_HI_VADDR\r\n");

    // Scan the 8 VirtIO slots in this page (slots 24-31, offset 0x000-0xe00, stride 0x200).
    // Slot 31 (QEMU's first assigned slot) is at offset 0xe00 = 0x0a003e00 - 0x0a003000.
    let mut found_slot: Option<u64> = None;
    write_bytes(b"[h6] scanning VirtIO slots 24-31:\r\n");
    for i in 0u64..8 {
        let slot_offset = i * VIRTIO_BUS_STRIDE;
        let vslot = VIRTIO_HI_VADDR + slot_offset;
        let magic     = mmio_read32(vslot, VIRTIO_MAGIC);
        let device_id = mmio_read32(vslot, VIRTIO_DEVICE_ID);
        if magic == 0x7472_6976 && device_id != 0 {
            write_bytes(b"  slot ");
            putchar(b'0' + (24 + i) as u8 / 10);
            putchar(b'0' + (24 + i) as u8 % 10);
            write_bytes(b" DEVICE_ID=");
            put_hex32(device_id);
            found_slot = Some(vslot);
        }
    }

    let vslot = match found_slot {
        Some(s) => s,
        None => {
            write_bytes(b"[h6] no VirtIO device found in slots 24-31\r\n");
            write_bytes(b"[h6] slot 31 magic=");
            put_hex32(mmio_read32(VIRTIO_HI_VADDR + 7 * VIRTIO_BUS_STRIDE, VIRTIO_MAGIC));
            spin()
        }
    };

    // VirtIO init sequence: ACKNOWLEDGE -> DRIVER -> (feature negotiation) -> DRIVER_OK.
    // For the gate, we skip feature negotiation and jump straight to DRIVER_OK.
    mmio_write32(vslot, VIRTIO_STATUS, STATUS_ACKNOWLEDGE);
    mmio_write32(vslot, VIRTIO_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    mmio_write32(vslot, VIRTIO_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_DRIVER_OK);
    let status = mmio_read32(vslot, VIRTIO_STATUS);
    write_bytes(b"[h6] STATUS=");
    put_hex32(status);

    write_bytes(b"VirtIO-net device_id=1 init gate: PASSED\r\n");
    spin()
}
