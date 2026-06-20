//! moonshot-sel4-vmm — seL4 Microkit Protection Domain runtime.
//!
//! Provides:
//! - `syscall`: raw seL4 AArch64 ABI wrappers (unsafe, inline asm)
//! - `types`:   MsgInfo and ChannelId IPC types
//! - `debug`:   DebugPutChar serial output for PD development
//!
//! Phase H1 target: rootserver ELF that prints via DebugPutChar.
//! Phase H2 target: full Microkit PD event loop (notified / protected callbacks).
//!
//! Build target: aarch64-unknown-none (bare metal, no OS).
//! No heap allocator required for phase H1.

#![no_std]

pub mod debug;
pub mod pd;
pub mod syscall;
pub mod types;

pub use debug::{putchar, puts, puts_line, spin, write_bytes};
pub use pd::{channels, notify, PdEntry};
pub use types::{ChannelId, MsgInfo};

// Bare-metal panic handler.
// AArch64 (seL4 PD target): yield-loop so the hardware watchdog can expire.
// Other targets (host cargo check / test): tight loop; satisfies the handler
// contract so `cargo check` works on x86_64 without the AArch64 toolchain.
#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { loop { syscall::yield_cpu(); } }
}

#[cfg(not(target_arch = "aarch64"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
