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
pub mod syscall;
pub mod types;

pub use debug::{putchar, puts, puts_line, spin, write_bytes};
pub use types::{ChannelId, MsgInfo};

// Bare-metal panic handler for AArch64 builds.
// On release builds panic = "abort" so this is unreachable in practice;
// it satisfies the linker for debug builds.
#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { loop { syscall::yield_cpu(); } }
}
