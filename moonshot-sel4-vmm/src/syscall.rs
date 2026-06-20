// seL4 AArch64 system call ABI.
//
// Convention: x7 = syscall number (signed, negative),
//             x0–x6 = arguments, svc #0.
//
// AArch64 register asm is only available when compiling for AArch64.
// For host-target cargo check, stub implementations are provided.

pub mod num {
    pub const SYS_CALL: i64 = -1;
    pub const SYS_REPLY_RECV: i64 = -2;
    pub const SYS_SEND: i64 = -3;
    pub const SYS_NB_SEND: i64 = -4;
    pub const SYS_RECV: i64 = -5;
    pub const SYS_REPLY: i64 = -6;
    pub const SYS_YIELD: i64 = -7;
    pub const SYS_NB_RECV: i64 = -8;
    pub const SYS_DEBUG_PUT_CHAR: i64 = -9;     // requires KernelPrinting=ON
    pub const SYS_DEBUG_HALT: i64 = -11;         // requires CONFIG_DEBUG_BUILD
}

/// Write a single byte to the seL4 kernel's debug serial channel.
///
/// Requires `KernelPrinting=ON` in the kernel cmake configuration.
///
/// # Safety
/// Invokes an seL4 kernel syscall. Must only be called in a seL4 PD context.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn debug_put_char(c: u8) {
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_DEBUG_PUT_CHAR,
        in("x0") c as u64,
        lateout("x0") _,
        lateout("x1") _, lateout("x2") _, lateout("x3") _,
        lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack, preserves_flags)
    );
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn debug_put_char(_c: u8) {
    // Host-target stub — not callable from seL4 PD context.
}

/// Yield the current PD's timeslice to the seL4 scheduler.
///
/// # Safety
/// Invokes an seL4 kernel syscall.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn yield_cpu() {
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_YIELD,
        lateout("x0") _, lateout("x1") _, lateout("x2") _,
        lateout("x3") _, lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack, preserves_flags)
    );
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn yield_cpu() {}

/// seL4 Call: send message to `cap` and block for reply.
/// Passes MR0-MR3 in x2-x5; returns result msgInfo from x1.
/// Used for kernel object invocations (Untyped_Retype, TCB ops, etc.).
///
/// # Safety
/// `cap` must be a valid CNode slot in the current PD's CSpace.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn call_mrs(cap: u64, msginfo: u64, mr0: u64, mr1: u64, mr2: u64, mr3: u64) -> u64 {
    let ret_info: u64;
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_CALL,
        inlateout("x0") cap => _,
        inlateout("x1") msginfo => ret_info,
        inlateout("x2") mr0 => _,
        inlateout("x3") mr1 => _,
        inlateout("x4") mr2 => _,
        inlateout("x5") mr3 => _,
        lateout("x6") _,
        options(nostack)
    );
    ret_info
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn call_mrs(_c: u64, _i: u64, _m0: u64, _m1: u64, _m2: u64, _m3: u64) -> u64 { 0 }

/// seL4 Send: blocking rendezvous send with MR0 in x2.
/// Blocks until a receiver is ready (synchronous endpoint send).
///
/// # Safety
/// `cap` must be a valid endpoint cap slot.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn send_mr0(cap: u64, msginfo: u64, mr0: u64) {
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_SEND,
        inlateout("x0") cap => _,
        in("x1") msginfo,
        inlateout("x2") mr0 => _,
        lateout("x3") _, lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack)
    );
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn send_mr0(_c: u64, _i: u64, _mr0: u64) {}

/// seL4 Recv: block on an endpoint; returns (msginfo, badge, MR0).
///
/// # Safety
/// `cap` must be a valid endpoint cap slot.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn recv_mr0(cap: u64) -> (u64, u64, u64) {
    let ret_info: u64;
    let ret_badge: u64;
    let ret_mr0: u64;
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_RECV,
        inlateout("x0") cap => ret_badge,
        lateout("x1") ret_info,
        lateout("x2") ret_mr0,
        lateout("x3") _, lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack)
    );
    (ret_info, ret_badge, ret_mr0)
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn recv_mr0(_cap: u64) -> (u64, u64, u64) { (0, 0, 0) }
