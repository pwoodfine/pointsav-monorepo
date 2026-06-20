// Phase H2b — Two seL4 threads + IPC gate.
//
// Rootserver creates one Endpoint and two TCBs (counter + receiver),
// both sharing the rootserver's VSpace and CNode.
//
// counter_fn: seL4_Send(EP, i) for i in 0..10, then spin.
// receiver_fn: seL4_Recv(EP) → print "IPC received: N", repeat 10×,
//              then print "IPC gate: PASSED" and spin.
//
// Gate: "IPC gate: PASSED" on QEMU serial.
//
// seL4 zeroes all initial thread registers (including SP). The global_asm
// entry below sets up the rootserver's own stack before entering Rust.
//
// Build:
//   CARGO_TARGET_DIR=/tmp/moonshot-h2b-build cargo run \
//     --manifest-path moonshot-toolkit/Cargo.toml \
//     -- build moonshot-toolkit/examples/os-console-ipc.toml
//
// Boot:
//   qemu-system-aarch64 -machine virt -cpu cortex-a53 -m 1G -display none \
//     -serial file:/tmp/sel4-ipc.log -kernel build/system-image.bin \
//     && grep "IPC gate" /tmp/sel4-ipc.log

#![no_std]
#![no_main]

use moonshot_sel4_vmm::bootinfo::{cap, obj_type, BootInfo};
use moonshot_sel4_vmm::{bootstrap, putchar, spin, write_bytes};
use moonshot_sel4_vmm::syscall;
use moonshot_sel4_vmm::bootinfo::msginfo_new;

// 16-byte aligned 4096-byte buffer (field accessed only via addr_of!/sym).
#[allow(dead_code)]
#[repr(C, align(16))]
struct Stack([u8; 4096]);

// Rootserver's own stack — seL4 starts the initial thread with SP=0,
// so the asm entry sets SP to the top of this buffer before calling _main.
static mut ROOTSERVER_STACK: Stack = Stack([0; 4096]);
// Stacks for the two child TCBs.
static mut COUNTER_STACK: Stack = Stack([0; 4096]);
static mut RECEIVER_STACK: Stack = Stack([0; 4096]);

// Shared endpoint slot written by rootserver before resuming children.
static mut EP_SLOT: u64 = 0;

// AArch64 entry: load ROOTSERVER_STACK address, add 4096 (stack top),
// set SP, clear frame pointer, then branch to _main with x0 (BootInfo
// pointer) intact. x10 is caller-saved; x0 is preserved.
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
    main_fn   = sym _main,
);

// Non-AArch64 stub — only AArch64 actually runs on seL4/QEMU here.
#[cfg(not(target_arch = "aarch64"))]
#[no_mangle]
pub unsafe extern "C" fn _start(_bi: *const BootInfo) -> ! { loop {} }

// Sends integers 0..10 to EP, then spins.
unsafe fn counter_fn() -> ! {
    let ep = EP_SLOT;
    let info = msginfo_new(0, 0, 1);
    let mut i: u64 = 0;
    while i < 10 {
        syscall::send_mr0(ep, info, i);
        i += 1;
    }
    spin()
}

// Receives 10 messages from EP, prints each value, then declares gate passed.
unsafe fn receiver_fn() -> ! {
    let ep = EP_SLOT;
    let mut n: u64 = 0;
    while n < 10 {
        let (_, _, val) = syscall::recv_mr0(ep);
        write_bytes(b"IPC received: ");
        putchar(b'0' + val as u8);
        write_bytes(b"\r\n");
        n += 1;
    }
    write_bytes(b"IPC gate: PASSED\r\n");
    spin()
}

#[no_mangle]
pub unsafe extern "C" fn _main(bi: *const BootInfo) -> ! {
    write_bytes(b"[ipc] Phase H2b start\r\n");

    let ipc = (*bi).ipc_buffer;
    let ipc_buf_addr = ipc as u64;

    // Find a RAM untyped large enough for 1 EP (2^4) + 2 TCBs (2^11 each):
    // total ~4112 bytes; 2^13 = 8 KiB minimum.
    let ut = match (*bi).find_untyped(13) {
        Some(s) => s,
        None => {
            write_bytes(b"[ipc] no untyped >= 8 KiB\r\n");
            spin()
        }
    };

    let ep_slot   = (*bi).empty.start;
    let ctcb_slot = (*bi).empty.start + 1;
    let rtcb_slot = (*bi).empty.start + 2;

    // Create Endpoint, counter TCB, receiver TCB from the same untyped.
    bootstrap::untyped_retype(ipc, ut, obj_type::ENDPOINT, bootstrap::EP_SIZE_BITS, ep_slot);
    bootstrap::untyped_retype(ipc, ut, obj_type::TCB, bootstrap::TCB_SIZE_BITS, ctcb_slot);
    bootstrap::untyped_retype(ipc, ut, obj_type::TCB, bootstrap::TCB_SIZE_BITS, rtcb_slot);

    // Publish EP_SLOT before any thread runs.
    EP_SLOT = ep_slot;

    // Configure both child TCBs: shared CSpace (initial CNode), shared VSpace,
    // shared IPC buffer frame (register-only IPC never touches the IPC buffer).
    bootstrap::tcb_configure(ipc, ctcb_slot, ipc_buf_addr);
    bootstrap::tcb_configure(ipc, rtcb_slot, ipc_buf_addr);

    // Priorities: receiver (200) > counter (100) > rootserver (lowered to 0 below).
    bootstrap::tcb_set_priority(ipc, ctcb_slot, cap::INIT_TCB, 100);
    bootstrap::tcb_set_priority(ipc, rtcb_slot, cap::INIT_TCB, 200);

    // Set entry points and stack pointers.
    let counter_pc: u64 = {
        let f: unsafe fn() -> ! = counter_fn;
        f as usize as u64
    };
    let counter_sp = core::ptr::addr_of!(COUNTER_STACK) as u64 + 4096;
    bootstrap::tcb_write_regs(ipc, ctcb_slot, counter_pc, counter_sp);

    let receiver_pc: u64 = {
        let f: unsafe fn() -> ! = receiver_fn;
        f as usize as u64
    };
    let receiver_sp = core::ptr::addr_of!(RECEIVER_STACK) as u64 + 4096;
    bootstrap::tcb_write_regs(ipc, rtcb_slot, receiver_pc, receiver_sp);

    // Lower rootserver priority to 0: children (100, 200) will preempt on resume.
    bootstrap::tcb_set_priority(ipc, cap::INIT_TCB, cap::INIT_TCB, 0);

    // Resume receiver first → it immediately calls seL4_Recv → blocks.
    // Resume counter → it calls seL4_Send → rendezvous with receiver.
    bootstrap::tcb_resume(rtcb_slot);
    bootstrap::tcb_resume(ctcb_slot);

    // Rootserver parks at priority 0; children run the IPC exchange.
    spin()
}
