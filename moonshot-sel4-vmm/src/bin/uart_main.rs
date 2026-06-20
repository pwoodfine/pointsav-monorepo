// Phase H3 — PL011 UART MMIO direct write.
//
// Demonstrates seL4 device-untyped → frame mapping:
//   1. Find the device untyped covering the PL011 UART at 0x09000000.
//   2. Retype it to a SmallPage (seL4_ARM_SmallPageObject = 7).
//   3. Retype an ordinary untyped to a PageTable (seL4_ARM_PageTableObject = 9).
//   4. Map the page table into the init VSpace at virtual address 0x40000000.
//   5. Map the frame at 0x40000000 with device-memory attributes (attr=0).
//   6. Write bytes directly to the PL011 UARTDR register (offset 0x000).
//
// PL011 UART on QEMU virt machine: physical 0x09000000.
// PL011 UARTDR (data register): offset 0x000 from UART base.
// Device memory: uncached, non-buffered (VMSAv8-64 attribute index = normal non-cacheable,
//   but seL4 ARM_VMAttributes = 0 for device-memory/non-cacheable on virt machine).
//
// Gate: "UART gate: PASSED\r\n" appears in QEMU serial output via direct MMIO write,
//   NOT via seL4_DebugPutChar.
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h3-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-uart.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G \
//     -nographic -kernel build/system-image.bin \
//     -serial file:/tmp/sel4-h3-out.txt

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};

// Physical address of QEMU virt PL011 UART.
const UART_PADDR: u64 = 0x0900_0000;
// Virtual address where we will map the UART frame.
const UART_VADDR: u64 = 0x4000_0000;
// PL011 UARTDR offset (data register — write a byte here to transmit).
const UARTDR_OFFSET: u64 = 0x000;

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

/// Write a byte to the PL011 UART data register via mapped virtual address.
///
/// The PL011 UARTDR is a 32-bit register; writing the low byte transmits a character.
/// Polling for TXFF (bit 5 of UARTFR at offset 0x018) is not needed here because
/// QEMU's PL011 model has a deep FIFO and accepts characters without backpressure.
unsafe fn uart_putchar(vbase: u64, byte: u8) {
    let dr = (vbase + UARTDR_OFFSET) as *mut u32;
    dr.write_volatile(byte as u32);
}

/// Write a byte slice to the mapped UART.
unsafe fn uart_write(vbase: u64, s: &[u8]) {
    for &b in s {
        uart_putchar(vbase, b);
    }
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    // Phase H3 start marker via kernel debug channel (confirmed working in H1-H2).
    write_bytes(b"[uart] Phase H3 start\r\n");

    let ipc = (*bi).ipc_buffer;
    let empty_start = (*bi).empty.start;

    // Step 1: Find a non-device untyped large enough for one page table (4 KiB).
    // We need: 1 PageTable (4 KiB, size_bits=12) + 1 slot for the frame cap.
    // Use a 2^13 = 8 KiB untyped to get both from one retype pool.
    let ut_ram = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => {
            write_bytes(b"[uart] no RAM untyped >= 8 KiB\r\n");
            spin()
        }
    };

    // Step 2: Find the device untyped that covers the PL011 at 0x09000000.
    let ut_dev = match (*bi).find_device_untyped(UART_PADDR) {
        Some(s) => s,
        None => {
            write_bytes(b"[uart] no device untyped for UART paddr\r\n");
            spin()
        }
    };
    write_bytes(b"[uart] device untyped found\r\n");

    // Allocate cap slots:
    //   empty_start + 0 = page table cap (L1 PUD entry — covers 1 GiB at UART_VADDR)
    //   empty_start + 1 = page table cap (L2 PMD entry — covers 2 MiB at UART_VADDR)
    //   empty_start + 2 = frame cap (UART 4 KiB SmallPage)
    //
    // AArch64 4-level walk: PGD (VSpace) → PUD (L1) → PMD (L2) → PTE (L3/SmallPage).
    // lookupPTSlot walks until it finds an empty slot; ARMPageTableMap installs there.
    // First  call installs at L1 (ptBitsLeft=30, covers [0x40000000, 0x80000000)).
    // Second call installs at L2 (ptBitsLeft=21, covers [0x40000000, 0x40200000)).
    // Then   ARMPageMap succeeds (ptBitsLeft=12 == pageBitsForSize(SmallPage)).
    let pt1_slot   = empty_start;
    let pt2_slot   = empty_start + 1;
    let frame_slot = empty_start + 2;

    // Step 3: Retype RAM untyped → 2 PageTables (L1 PUD + L2 PMD, 4 KiB each).
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt1_slot);
    bootstrap::untyped_retype(ipc, ut_ram, obj_type::ARM_PAGE_TABLE, bootstrap::PT_SIZE_BITS, pt2_slot);
    write_bytes(b"[uart] page tables created\r\n");

    // Step 4: Retype device untyped → 1 SmallPage (the UART 4 KiB frame).
    bootstrap::untyped_retype(
        ipc,
        ut_dev,
        obj_type::ARM_SMALL_PAGE,
        bootstrap::PAGE_SIZE_BITS,
        frame_slot,
    );
    write_bytes(b"[uart] device frame created\r\n");

    // Step 5a: Map pt1 → installs as L1 PUD entry at ptBitsLeft=30.
    bootstrap::page_table_map(ipc, pt1_slot, cap::INIT_VSPACE, UART_VADDR, 0);
    write_bytes(b"[uart] L1 PUD mapped\r\n");

    // Step 5b: Map pt2 → installs as L2 PMD entry at ptBitsLeft=21.
    bootstrap::page_table_map(ipc, pt2_slot, cap::INIT_VSPACE, UART_VADDR, 0);
    write_bytes(b"[uart] L2 PMD mapped\r\n");

    // Step 6: Map the UART frame at UART_VADDR (now ptBitsLeft=12 == SmallPage).
    // rights = 3 (Read | Write); attr = 0 (device memory / non-cacheable).
    bootstrap::page_map(ipc, frame_slot, cap::INIT_VSPACE, UART_VADDR, 3, 0);
    write_bytes(b"[uart] frame mapped\r\n");

    // Step 7: Write to the UART via direct MMIO.
    // From here on we use uart_write instead of write_bytes (which uses the kernel debug channel).
    let msg = b"UART gate: PASSED\r\n";
    uart_write(UART_VADDR, msg);

    // Also echo via debug channel so both paths are visible.
    putchar(b'\n');
    write_bytes(b"[uart] Phase H3 complete\r\n");

    spin()
}
