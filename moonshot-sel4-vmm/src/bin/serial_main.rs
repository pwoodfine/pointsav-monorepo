// Phase H2 — Serial PD + Console PD via seL4 IPC.
//
// Demonstrates the serial PD architectural pattern:
//   console_pd formats output and sends it as byte chunks via seL4 Endpoint.
//   serial_pd receives chunks and writes each byte to the debug serial port.
//
// Protocol: MR[0] = chunk byte count (0 = end-of-stream sentinel).
//           MR[1], MR[2], MR[3] = packed bytes, little-endian, up to 24 bytes.
//
// Priorities: serial_pd=200 > console_pd=100 > rootserver=0.
// After each Send/Recv rendezvous, serial_pd (higher priority) preempts
// console_pd immediately, processes the chunk, and returns to Recv — so
// serial_pd is always waiting before console_pd attempts the next Send.
//
// Gate: "Serial PD gate: PASSED" in QEMU serial output.
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h2-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-serial.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G \
//     -nographic -kernel build/system-image.bin

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, msginfo_new, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};
use moonshot_sel4_vmm::syscall;

#[repr(C, align(16))]
struct Stack([u8; 4096]);

static mut ROOTSERVER_STACK: Stack = Stack([0; 4096]);
static mut SERIAL_PD_STACK:  Stack = Stack([0; 4096]);
static mut CONSOLE_PD_STACK: Stack = Stack([0; 4096]);

static mut EP_SLOT: u64 = 0;

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".global _start",
    "_start:",
    "adrp x10, {rs_stack}",
    "add  x10, x10, :lo12:{rs_stack}",
    "add  x10, x10, #4096",
    "mov  sp,  x10",
    "mov  x29, xzr",
    "b    {main_fn}",
    rs_stack = sym ROOTSERVER_STACK,
    main_fn  = sym _main,
);

#[cfg(not(target_arch = "aarch64"))]
#[no_mangle]
pub unsafe extern "C" fn _start(_bi: *const BootInfo) -> ! { loop {} }

// Pack up to 24 bytes into three u64 words, little-endian.
unsafe fn pack_bytes(bytes: &[u8]) -> (u64, u64, u64) {
    let mut words = [0u64; 3];
    for (i, &b) in bytes.iter().enumerate().take(24) {
        words[i / 8] |= (b as u64) << ((i % 8) * 8);
    }
    (words[0], words[1], words[2])
}

// Send a byte slice to the serial PD in ≤24-byte chunks.
// MR[0] = chunk byte count; MR[1..3] = packed bytes.
unsafe fn send_str(ep: u64, s: &[u8]) {
    let info4 = msginfo_new(0, 0, 4);
    let mut offset = 0;
    while offset < s.len() {
        let end = s.len().min(offset + 24);
        let chunk = &s[offset..end];
        let (mr1, mr2, mr3) = pack_bytes(chunk);
        syscall::send_mrs4(ep, info4, chunk.len() as u64, mr1, mr2, mr3);
        offset = end;
    }
}

// Signal end-of-stream to the serial PD (MR[0] = 0).
unsafe fn send_end(ep: u64) {
    syscall::send_mrs4(ep, msginfo_new(0, 0, 1), 0, 0, 0, 0);
}

// Serial PD: receive chunks and write each byte to debug serial.
// On end-of-stream (len=0): print gate message and spin.
unsafe fn serial_pd_fn() -> ! {
    let ep = EP_SLOT;
    loop {
        let (_, _, len, mr1, mr2, mr3) = syscall::recv_mrs4(ep);
        let n = len as usize;
        if n == 0 {
            write_bytes(b"Serial PD gate: PASSED\r\n");
            spin()
        }
        let words = [mr1, mr2, mr3];
        for i in 0..n.min(24) {
            let byte = ((words[i / 8] >> ((i % 8) * 8)) & 0xff) as u8;
            putchar(byte);
        }
    }
}

// Console PD: format a bordered ASCII box and send it to the serial PD.
unsafe fn console_pd_fn() -> ! {
    let ep = EP_SLOT;
    write_bytes(b"[console] Phase H2 start\r\n");
    send_str(ep, b"+-------------------+\r\n");
    send_str(ep, b"| seL4  Phase  H2   |\r\n");
    send_str(ep, b"| Serial PD pattern |\r\n");
    send_str(ep, b"+-------------------+\r\n");
    send_end(ep);
    spin()
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    write_bytes(b"[serial] Phase H2 start\r\n");

    let ipc = (*bi).ipc_buffer;
    let ipc_buf_addr = ipc as u64;

    // One EP (2^4=16B) + two TCBs (2^11=2KiB each): 4 KiB total; use 2^13=8KiB UT.
    let ut = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => {
            write_bytes(b"[serial] no untyped >= 8 KiB\r\n");
            spin()
        }
    };

    let ep_slot      = (*bi).empty.start;
    let serial_slot  = (*bi).empty.start + 1;
    let console_slot = (*bi).empty.start + 2;

    bootstrap::untyped_retype(ipc, ut, obj_type::ENDPOINT, bootstrap::EP_SIZE_BITS, ep_slot);
    bootstrap::untyped_retype(ipc, ut, obj_type::TCB, bootstrap::TCB_SIZE_BITS, serial_slot);
    bootstrap::untyped_retype(ipc, ut, obj_type::TCB, bootstrap::TCB_SIZE_BITS, console_slot);

    EP_SLOT = ep_slot;

    bootstrap::tcb_configure(ipc, serial_slot, ipc_buf_addr);
    bootstrap::tcb_configure(ipc, console_slot, ipc_buf_addr);

    // serial_pd (200) > console_pd (100) > rootserver (lowered to 0).
    bootstrap::tcb_set_priority(ipc, serial_slot,  cap::INIT_TCB, 200);
    bootstrap::tcb_set_priority(ipc, console_slot, cap::INIT_TCB, 100);

    let serial_pc: u64 = { let f: unsafe fn() -> ! = serial_pd_fn;  f as usize as u64 };
    let serial_sp = core::ptr::addr_of!(SERIAL_PD_STACK)  as u64 + 4096;
    bootstrap::tcb_write_regs(ipc, serial_slot, serial_pc, serial_sp);

    let console_pc: u64 = { let f: unsafe fn() -> ! = console_pd_fn; f as usize as u64 };
    let console_sp = core::ptr::addr_of!(CONSOLE_PD_STACK) as u64 + 4096;
    bootstrap::tcb_write_regs(ipc, console_slot, console_pc, console_sp);

    // Lower rootserver to 0 so children preempt immediately on resume.
    bootstrap::tcb_set_priority(ipc, cap::INIT_TCB, cap::INIT_TCB, 0);

    // Resume serial_pd first: it calls seL4_Recv immediately → blocks.
    // Resume console_pd: it sends chunks; serial_pd wakes on each one.
    bootstrap::tcb_resume(serial_slot);
    bootstrap::tcb_resume(console_slot);

    spin()
}
