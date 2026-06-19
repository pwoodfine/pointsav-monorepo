//! os-console seL4 rootserver — Phase H2a milestone.
//!
//! Pure Rust PD binary. Entry point is `_start`; no C in the PD.
//! Prints a banner via SysDebugPutChar then spins via SysYield.
//!
//! Build: `cargo build --target aarch64-unknown-none --release --bin console_main`
//! Gate: "Hello from moonshot-sel4-vmm (Rust)" appears on QEMU serial.

#![no_std]
#![no_main]

use moonshot_sel4_vmm as vmm;

static BANNER: &[u8] = b"\r\n=== moonshot-sel4-vmm (Rust) ===\r\n\
Hello from moonshot-sel4-vmm (Rust)\r\n\
Phase H2a: Rust ABI wrappers confirmed end-to-end\r\n\
Geometric Protection: this PD is isolated by seL4 capability tokens\r\n\
===================================\r\n\r\n";

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    vmm::write_bytes(BANNER);
    vmm::spin()
}
