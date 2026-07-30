#![no_std]
#![no_main]

use core::arch::global_asm;
use core::panic::PanicInfo;
use system_substrate_broadcom::probe_nic;
use system_network_interface::{scan_for_peers, send_genesis_handshake, DiscoveryResult};

// Multiboot2 header + 16 KiB stack. The bootloader loads us at 1 MiB; we
// set up RSP and call rust_main. On return (never), halt.
global_asm!(
    ".section .multiboot",
    ".align 8",
    "header_start:",
    ".long 0xe85250d6",           // Multiboot2 magic
    ".long 0",                    // architecture: i386/x86
    ".long header_end - header_start",
    ".long 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start))",
    ".short 0", ".short 0", ".long 8",
    "header_end:",
    ".section .text",
    ".global _start",
    "_start:",
    "lea rsp, [rip + stack_top]",
    "call rust_main",
    "halt_loop:",
    "hlt",
    "jmp halt_loop",
    ".section .bss",
    ".align 16",
    "stack_bottom:",
    ".skip 16384",
    "stack_top:"
);

// 8×8 bitmap font for hex digits 0–F. Used to render status codes on the
// framebuffer — the only I/O available before any OS personality is loaded.
const FONT: [[u8; 8]; 16] = [
    [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C], // 0
    [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C], // 1
    [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E], // 2
    [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x06, 0x66, 0x3C], // 3
    [0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C, 0x0C, 0x0C], // 4
    [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x06, 0x66, 0x3C], // 5
    [0x3C, 0x66, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x3C], // 6
    [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x30], // 7
    [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x66, 0x3C], // 8
    [0x3C, 0x66, 0x66, 0x66, 0x3E, 0x06, 0x66, 0x3C], // 9
    [0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x66], // A
    [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x66, 0x7C], // B
    [0x3C, 0x66, 0x60, 0x60, 0x60, 0x60, 0x66, 0x3C], // C
    [0x78, 0x6C, 0x66, 0x66, 0x66, 0x66, 0x6C, 0x78], // D
    [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x7E], // E
    [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x60], // F
];

// Draw one hex byte (two glyphs) at pixel (x, y) into the framebuffer.
// fb: linear framebuffer base address; stride assumed 1280×4 bytes/row.
unsafe fn draw_byte(fb: u64, x: u32, y: u32, val: u8) {
    let nibbles = [val >> 4, val & 0x0F];
    for (i, &n) in nibbles.iter().enumerate() {
        let glyph = FONT[n as usize];
        for row in 0..8u32 {
            for col in 0..8u32 {
                if (glyph[row as usize] & (1 << (7 - col))) != 0 {
                    let px = (fb
                        + ((y + row) as u64 * 1280 * 4)
                        + ((x + i as u32 * 9 + col) as u64 * 4))
                        as *mut u32;
                    px.write_volatile(0x00FF00); // PPN green
                }
            }
        }
    }
}

// Render a one-byte status code on `row` (0-indexed) across all known framebuffers.
// Each Genesis Protocol phase writes a distinct code so the operator can read the
// boot state from any attached display without a serial port.
//
// Phase codes:
//   0x00  BOOT       — rust_main entered; stack live
//   0x01  NIC        — NIC probe complete (probe_nic called)
//   0x10  SCAN       — mDNS scan in progress
//   0x20  SEED       — genesis-seed path (no peers found; this is node 0)
//   0x21  JOIN       — join path (peer discovered; initiating handshake)
//   0x30  SHAKE      — genesis handshake frame sent
//   0x40  HOLD       — waiting for operator claim via pairing ceremony
unsafe fn show(fbs: &[u64; 3], row: u32, code: u8) {
    for &fb in fbs {
        draw_byte(fb, 40, 40 + row * 16, code);
    }
}

// Genesis Protocol boot sequence.
//
// Phase 0: BOOT     — rust_main entered.
// Phase 1: NIC      — probe for Broadcom NIC (Step 2 will implement real PCI scan).
// Phase 2: SCAN     — scan for existing PPN peers via mDNS (Step 3 will implement).
// Phase 3: FORK     — genesis-seed (0x20) if no peers; join (0x21) if found.
// Phase 4: SHAKE    — send genesis handshake to pairing server (join path only).
// Phase 5: HOLD     — spin, waiting for operator to claim the node.
//
// With stub dependencies all paths resolve to: BOOT → NIC → SCAN → SEED → HOLD.
// When Step 3 lands and a live peer is reachable, the path becomes: → JOIN → SHAKE → HOLD.
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // Try the three common Apple framebuffer base addresses. Real deployments
    // pass the address via Multiboot2 tags; that parsing is a future step.
    let fbs: [u64; 3] = [0x80000000, 0x90000000, 0xC0000000];

    unsafe {
        // Phase 0: BOOT
        show(&fbs, 0, 0x00);

        // Phase 1: NIC probe
        let _nic_mmio = probe_nic(); // None until Step 2
        show(&fbs, 1, 0x01);

        // Phase 2: peer scan
        show(&fbs, 2, 0x10);
        let discovery = scan_for_peers(); // NotFound until Step 3

        // Phase 3: genesis fork
        let (phase_code, peer_addr): (u8, Option<[u8; 4]>) = match discovery {
            DiscoveryResult::NotFound => (0x20, None),
            DiscoveryResult::MdnsFound { addr } => (0x21, Some(addr)),
            DiscoveryResult::OperatorSupplied { addr } => (0x21, Some(addr)),
        };
        show(&fbs, 3, phase_code);

        // Phase 4: handshake (join path only)
        if let Some(addr) = peer_addr {
            // 8-char Crockford base32 SAS code — placeholder until Step 4.
            let short_code = b"AAAABBBB";
            let _accepted = send_genesis_handshake(addr, short_code);
            show(&fbs, 4, 0x30);
        }

        // Phase 5: HOLD — operator must claim the node via pairing ceremony.
        show(&fbs, 5, 0x40);
        loop {}
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
