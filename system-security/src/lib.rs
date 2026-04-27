// Bare-metal staticlib: the seL4 unikernel build needs no_std + no_main
// + a custom panic_handler. Under `cargo test`, however, the test harness
// pulls in std (which already provides `panic_impl`) and requires a main
// entry — so we gate the bare-metal attributes behind `cfg(not(test))`.
// This lets `cargo test --workspace` link cleanly without giving up the
// bare-metal build shape.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

extern "C" {
    fn microkit_dbg_puts(s: *const u8);
}

const TELEMETRY_ADDR: usize = 0x4000000;
const RESET_CHANNEL: u64 = 10;

fn print(s: &[u8]) {
    unsafe { microkit_dbg_puts(s.as_ptr()); }
}

#[no_mangle]
pub extern "C" fn init() {
    print(b"MUSCLE: Core Ignition. Setting heartbeat...\n\0");
    
    unsafe {
        // 1. Set healthy heartbeat
        core::ptr::write_volatile(TELEMETRY_ADDR as *mut u8, 0x31);
        
        // 2. Simulate a delay, then a CRASH by clearing the heartbeat
        for _ in 0..5000000 { core::hint::spin_loop(); }
        
        core::ptr::write_volatile(TELEMETRY_ADDR as *mut u8, 0x00);
        print(b"MUSCLE: [CRITICAL ERROR] State Corrupted. Heartbeat Lost.\n\0");
    }
}

#[no_mangle]
pub extern "C" fn notified(ch: u64) {
    if ch == RESET_CHANNEL {
        print(b"MUSCLE: Reset Signal Received! Performing Software Recovery...\n\0");
        init(); // Jump back to start
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }
