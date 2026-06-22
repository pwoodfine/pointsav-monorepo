#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
extern "C" {
    fn microkit_dbg_puts(s: *const u8);
}

#[cfg(not(test))]
const TELEMETRY_ADDR: usize = 0x4000000;
#[cfg(not(test))]
const RESET_CHANNEL: u64 = 10;

#[cfg(not(test))]
fn print(s: &[u8]) {
    unsafe {
        microkit_dbg_puts(s.as_ptr());
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn init() {
    print(b"MUSCLE: Core Ignition. Setting heartbeat...\n\0");

    unsafe {
        // 1. Set healthy heartbeat
        core::ptr::write_volatile(TELEMETRY_ADDR as *mut u8, 0x31);

        // 2. Simulate a delay, then a CRASH by clearing the heartbeat
        for _ in 0..5000000 {
            core::hint::spin_loop();
        }

        core::ptr::write_volatile(TELEMETRY_ADDR as *mut u8, 0x00);
        print(b"MUSCLE: [CRITICAL ERROR] State Corrupted. Heartbeat Lost.\n\0");
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn notified(ch: u64) {
    if ch == RESET_CHANNEL {
        print(b"MUSCLE: Reset Signal Received! Performing Software Recovery...\n\0");
        init(); // Jump back to start
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

