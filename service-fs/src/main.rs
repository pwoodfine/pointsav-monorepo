#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
use core::panic::PanicInfo;

/// Entry point for the seL4 Unikernel.
/// The `system-security` Capability-Based Manager (CBM) routes execution here
/// after provisioning the isolated memory space.
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // 1. Initialize bare-metal state
    // 2. Map capability pointers passed by the CBM (Root-Task)

    // 3. The Sovereign Polling Loop (Gatekeeper State Machine)
    loop {
        // Block and wait for Inter-Process Communication (IPC) from system-network-interface
        // Handle read/write logic strictly within provisioned WORM boundaries
        core::hint::spin_loop();
    }
}

/// Bare-metal panic handler.
/// In a production environment, this triggers a fault IPC to the system-security Watchdog.
#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
