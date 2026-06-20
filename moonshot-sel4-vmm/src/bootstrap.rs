// seL4 kernel object invocation helpers for the rootserver bootstrap path.
//
// All functions issue seL4_Call against kernel-object caps (TCB, Untyped, etc.)
// using the IPC buffer for MRs beyond x5 and for extra-cap slots.
// Error handling: spin() on non-zero return label — visible as QEMU hang.

use crate::bootinfo::{cap, label, msginfo_new, IpcBuffer};
use crate::syscall;

unsafe fn check_ok(ret_info: u64) {
    if (ret_info >> 12) & 0x000f_ffff_ffff_ffff != 0 {
        crate::spin();
    }
}

/// seL4_Untyped_Retype: carve one object of (obj_type, size_bits) from `ut_cap`,
/// placing the new cap at `dest_slot` in the initial CNode.
pub unsafe fn untyped_retype(
    ipc: *mut IpcBuffer,
    ut_cap: u64,
    obj_type_val: u64,
    size_bits: u64,
    dest_slot: u64,
) {
    (*ipc).caps_or_badges[0] = cap::INIT_CNODE; // root CNode for placement
    (*ipc).msg[4] = dest_slot;                  // MR[4] = node_offset
    (*ipc).msg[5] = 1;                          // MR[5] = num_objects
    let r = syscall::call_mrs(
        ut_cap,
        msginfo_new(label::UNTYPED_RETYPE, 1, 6),
        obj_type_val, // MR[0] = type
        size_bits,    // MR[1] = size_bits
        0,            // MR[2] = node_index = 0 (use root directly)
        0,            // MR[3] = node_depth = 0 (use root directly)
    );
    check_ok(r);
}

/// seL4_TCB_Configure: attach CSpace, VSpace, and IPC buffer to a TCB.
/// Uses the rootserver's own CSpace, VSpace, and IPC buffer frame for all child threads
/// (same-VSpace same-CSpace threading; shared binary image).
pub unsafe fn tcb_configure(ipc: *mut IpcBuffer, tcb_cap: u64, ipc_buf_addr: u64) {
    (*ipc).caps_or_badges[0] = cap::INIT_CNODE;       // cspace_root
    (*ipc).caps_or_badges[1] = cap::INIT_VSPACE;      // vspace_root
    (*ipc).caps_or_badges[2] = cap::INIT_IPC_BUFFER;  // ipc_buffer_frame
    let r = syscall::call_mrs(
        tcb_cap,
        msginfo_new(label::TCB_CONFIGURE, 3, 4),
        0,           // MR[0] = faultEP = seL4_CapNull (no fault handler)
        0,           // MR[1] = cspace_root_data = 0 (no guard)
        0,           // MR[2] = vspace_root_data = 0
        ipc_buf_addr,// MR[3] = IPC buffer virtual address
    );
    check_ok(r);
}

/// seL4_TCB_SetPriority: set scheduling priority via `authority` TCB cap.
pub unsafe fn tcb_set_priority(
    ipc: *mut IpcBuffer,
    tcb_cap: u64,
    authority: u64,
    priority: u64,
) {
    (*ipc).caps_or_badges[0] = authority;
    let r = syscall::call_mrs(
        tcb_cap,
        msginfo_new(label::TCB_SET_PRIORITY, 1, 1),
        priority, // MR[0] = new priority
        0, 0, 0,
    );
    check_ok(r);
}

/// seL4_TCB_WriteRegisters: write pc and sp (count=2) without immediate resume.
///
/// Kernel ABI (decodeWriteRegisters in tcb.c):
///   MR[0] = flags = (resume_target & 1) | (arch_flags << 8)  — both packed in one word
///   MR[1] = count
///   MR[2] = regs[0] = pc
///   MR[3] = regs[1] = sp
/// Length = 4 (all four words fit in x2-x5; no IPC buffer needed for this case).
pub unsafe fn tcb_write_regs(_ipc: *mut IpcBuffer, tcb_cap: u64, pc: u64, sp: u64) {
    let r = syscall::call_mrs(
        tcb_cap,
        msginfo_new(label::TCB_WRITE_REGISTERS, 0, 4),
        0,  // MR[0] = flags: resume=0, archFlags=0 (packed: 0 | 0<<8 = 0)
        2,  // MR[1] = count = 2 (write regs[0]=pc and regs[1]=sp)
        pc, // MR[2] = regs[0] = pc   (in x4)
        sp, // MR[3] = regs[1] = sp   (in x5)
    );
    check_ok(r);
}

/// seL4_TCB_Resume: make a configured TCB runnable.
pub unsafe fn tcb_resume(tcb_cap: u64) {
    let r = syscall::call_mrs(
        tcb_cap,
        msginfo_new(label::TCB_RESUME, 0, 0),
        0, 0, 0, 0,
    );
    check_ok(r);
}

/// Convenience: shorthand for the two TCB size constants.
pub const EP_SIZE_BITS: u64 = 4;   // seL4_EndpointBits
pub const TCB_SIZE_BITS: u64 = 11; // seL4_TCBBits (non-MCS, non-debug)
