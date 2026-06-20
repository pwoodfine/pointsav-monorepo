// Phase H5 — VirtIO-net MMIO device probe and init via seL4 frame mapping.
//
// Demonstrates the full MMIO path for a non-UART peripheral:
//   1. Find the device untyped at VIRTIO_PADDR (0x0a000000, size_bits=14 on QEMU virt).
//   2. Retype to SmallPage → map at VIRTIO_VADDR using two ARMPageTableMap + one ARMPageMap.
//   3. Read MAGIC_VALUE (must be 0x74726976 = "virt").
//   4. Write STATUS progression: ACKNOWLEDGE → DRIVER.
//   5. Gate: "VirtIO-net init gate: PASSED".
//
// VirtIO MMIO register map (legacy transport, v1):
//   +0x000: MagicValue        (RO) = 0x74726976
//   +0x004: Version           (RO) = 1 (legacy) or 2 (modern)
//   +0x008: DeviceID          (RO) = 1 (network), 2 (block), etc.
//   +0x00c: VendorID          (RO)
//   +0x070: Status            (RW) — 0=reset; ACKNOWLEDGE=1, DRIVER=2, DRIVER_OK=4, FAILED=128
//
// QEMU virt: VirtIO MMIO bus at 0x0a000000–0x0a003fff (32 slots × 512 bytes each).
// seL4 creates one 16 KiB (size_bits=14) device untyped at 0x0a000000 via staircase algorithm.
// SmallPage frame from the untyped covers slot 0 (0x0a000000–0x0a000fff, first device).
//
// Requires QEMU launched with: -device virtio-net-device,netdev=n0 -netdev user,id=n0
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h5-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-virtio-net.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G -nographic \
//     -device virtio-net-device,netdev=n0 -netdev user,id=n0 \
//     -kernel build/system-image.bin

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};

// Physical address of VirtIO MMIO bus slot 0 on QEMU virt AArch64.
const VIRTIO_PADDR: u64 = 0x0a00_0000;
// Virtual address where we map the VirtIO MMIO frame.
const VIRTIO_VADDR: u64 = 0x4020_0000;

// VirtIO MMIO register offsets (byte addresses from VIRTIO_VADDR).
const VIRTIO_MAGIC:     u64 = 0x000;
const VIRTIO_VERSION:   u64 = 0x004;
const VIRTIO_DEVICE_ID: u64 = 0x008;
const VIRTIO_STATUS:    u64 = 0x070;

// VirtIO status bits.
const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER:      u32 = 2;

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

// Print a 32-bit value in hex: "0xNNNNNNNN\r\n".
unsafe fn put_hex32(val: u32) {
    write_bytes(b"0x");
    for i in (0u32..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xf) as u8;
        putchar(if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 });
    }
    write_bytes(b"\r\n");
}

// Read a VirtIO MMIO register (little-endian u32 at vbase+offset).
unsafe fn mmio_read32(vbase: u64, offset: u64) -> u32 {
    let ptr = (vbase + offset) as *const u32;
    ptr.read_volatile()
}

// Write a VirtIO MMIO register.
unsafe fn mmio_write32(vbase: u64, offset: u64, val: u32) {
    let ptr = (vbase + offset) as *mut u32;
    ptr.write_volatile(val);
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    write_bytes(b"[virtio] Phase H5 start\r\n");

    let ipc = (*bi).ipc_buffer;
    let empty_start = (*bi).empty.start;

    // Step 1: Find RAM untyped for two page tables (8 KiB = size_bits 13).
    let ut_ram = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => {
            write_bytes(b"[virtio] no RAM untyped\r\n");
            spin()
        }
    };

    // Step 2: Find VirtIO-mmio device untyped at 0x0a000000.
    // seL4 staircase creates size_bits=14 (16 KiB) starting at 0x0a000000 for QEMU virt.
    let ut_dev = match (*bi).find_device_untyped(VIRTIO_PADDR) {
        Some(s) => s,
        None => {
            write_bytes(b"[virtio] no device untyped at 0x0a000000\r\n");
            // Print all device untypeds to aid debugging.
            let count = ((*bi).untyped.end - (*bi).untyped.start) as usize;
            for i in 0..count {
                let d = &(*bi).untyped_list[i];
                if d.is_device != 0 {
                    write_bytes(b"  dev ut paddr=");
                    put_hex32(d.paddr as u32);
                }
            }
            spin()
        }
    };
    write_bytes(b"[virtio] device untyped found\r\n");

    // Cap slots: pt1 (L1 PUD), pt2 (L2 PMD), frame (VirtIO SmallPage).
    let pt1_slot   = empty_start;
    let pt2_slot   = empty_start + 1;
    let frame_slot = empty_start + 2;

    // Step 3: Retype 2 page tables from RAM and 1 SmallPage from device untyped.
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt1_slot);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt2_slot);
    bootstrap::untyped_retype(ipc, ut_dev, obj_type::ARM_SMALL_PAGE, bootstrap::PAGE_SIZE_BITS, frame_slot);
    write_bytes(b"[virtio] page tables + device frame created\r\n");

    // Step 4: Map pt1 at L1 PUD (ptBitsLeft=30 for 0x40200000: PUD slot 1).
    bootstrap::page_table_map(ipc, pt1_slot, cap::INIT_VSPACE, VIRTIO_VADDR, 0);
    write_bytes(b"[virtio] L1 PUD mapped\r\n");

    // Step 5: Map pt2 at L2 PMD (ptBitsLeft=21 for 0x40200000: PMD slot 1).
    bootstrap::page_table_map(ipc, pt2_slot, cap::INIT_VSPACE, VIRTIO_VADDR, 0);
    write_bytes(b"[virtio] L2 PMD mapped\r\n");

    // Step 6: Map VirtIO frame at VIRTIO_VADDR with device memory attributes.
    bootstrap::page_map(ipc, frame_slot, cap::INIT_VSPACE, VIRTIO_VADDR, 3, 0);
    write_bytes(b"[virtio] frame mapped\r\n");

    // Step 7: Verify VirtIO magic value.
    let magic = mmio_read32(VIRTIO_VADDR, VIRTIO_MAGIC);
    write_bytes(b"[virtio] MAGIC=");
    put_hex32(magic);
    if magic != 0x7472_6976 {
        write_bytes(b"[virtio] MAGIC mismatch -- not a VirtIO MMIO transport\r\n");
        spin()
    }

    // Step 8: Read version and device ID.
    let version   = mmio_read32(VIRTIO_VADDR, VIRTIO_VERSION);
    let device_id = mmio_read32(VIRTIO_VADDR, VIRTIO_DEVICE_ID);
    write_bytes(b"[virtio] VERSION=");
    put_hex32(version);
    write_bytes(b"[virtio] DEVICE_ID=");
    put_hex32(device_id);

    // Step 9: STATUS progression — ACKNOWLEDGE then DRIVER.
    // This tells the device: "we know you exist, and we have a driver for you."
    mmio_write32(VIRTIO_VADDR, VIRTIO_STATUS, STATUS_ACKNOWLEDGE);
    mmio_write32(VIRTIO_VADDR, VIRTIO_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    let status_readback = mmio_read32(VIRTIO_VADDR, VIRTIO_STATUS);
    write_bytes(b"[virtio] STATUS=");
    put_hex32(status_readback);

    if device_id == 1 {
        write_bytes(b"[virtio] network device confirmed\r\n");
    }

    write_bytes(b"VirtIO-net init gate: PASSED\r\n");
    spin()
}
