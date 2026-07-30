// seL4 AArch64 system call ABI.
//
// Convention: x7 = syscall number (signed, negative),
//             x0–x6 = arguments, svc #0.
//
// AArch64 register asm is only available when compiling for AArch64.
// For host-target cargo check, stub implementations are provided.

pub mod num {
    pub const SYS_CALL: i64 = -31;
    pub const SYS_REPLY_RECV: i64 = -33;
    pub const SYS_REPLY: i64 = -34;
    pub const SYS_SEND: i64 = -35;
    pub const SYS_RECV: i64 = -40;
    pub const SYS_YIELD: i64 = -7;
    pub const SYS_DEBUG_PUT_CHAR: i64 = -9;
    pub const SYS_DEBUG_HALT: i64 = -8;
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

/// seL4 Call: send a message to `cap_slot` and block for reply.
///
/// Returns (reply_msginfo_raw, badge).
///
/// # Safety
/// `cap_slot` must be a valid CNode index in the current PD's CSpace.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn call(cap_slot: u64, msginfo: u64) -> (u64, u64) {
    let ret_info: u64;
    let ret_badge: u64;
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_CALL,
        in("x0") cap_slot,
        in("x1") msginfo,
        lateout("x0") ret_info,
        lateout("x1") ret_badge,
        lateout("x2") _, lateout("x3") _,
        lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack, preserves_flags)
    );
    (ret_info, ret_badge)
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn call(_cap_slot: u64, _msginfo: u64) -> (u64, u64) {
    (0, 0)
}

/// seL4 Recv: block on an endpoint or notification.
///
/// Returns (msginfo_raw, badge_or_sender).
///
/// # Safety
/// `cap_slot` must be valid.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn recv(cap_slot: u64) -> (u64, u64) {
    let ret_info: u64;
    let ret_badge: u64;
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_RECV,
        in("x0") cap_slot,
        lateout("x0") ret_info,
        lateout("x1") ret_badge,
        lateout("x2") _, lateout("x3") _,
        lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack, preserves_flags)
    );
    (ret_info, ret_badge)
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn recv(_cap_slot: u64) -> (u64, u64) {
    (0, 0)
}

/// seL4 Send: fire-and-forget message on an endpoint.
///
/// # Safety
/// Invokes an seL4 kernel syscall.
#[cfg(target_arch = "aarch64")]
#[inline]
pub unsafe fn send(cap_slot: u64, msginfo: u64) {
    core::arch::asm!(
        "svc #0",
        in("x7") num::SYS_SEND,
        in("x0") cap_slot,
        in("x1") msginfo,
        lateout("x0") _, lateout("x1") _, lateout("x2") _,
        lateout("x3") _, lateout("x4") _, lateout("x5") _, lateout("x6") _,
        options(nostack, preserves_flags)
    );
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
pub unsafe fn send(_cap_slot: u64, _msginfo: u64) {}
