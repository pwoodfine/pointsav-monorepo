#![no_std]
#![no_main]

use core::panic::PanicInfo;

extern "C" {
    fn microkit_dbg_puts(s: *const u8);
    // Link to our new C wrapper
    fn do_notify(ch: u64); 
}

const TELEMETRY_ADDR: usize = 0x4000000;
const RESET_CHANNEL: u64 = 10;

#[no_mangle]
pub extern "C" fn init() {
    loop {
        for _ in 0..15000000 { core::hint::spin_loop(); }
        
        unsafe {
            let heartbeat = core::ptr::read_volatile(TELEMETRY_ADDR as *const u8);
            if heartbeat != 0x31 {
                microkit_dbg_puts(b"WATCHDOG: Heartbeat Flatlined! Sending Reset...\n\0".as_ptr());
                
                // Use the official SDK capability math
                do_notify(RESET_CHANNEL); 
            }
        }
    }
}

#[no_mangle] pub extern "C" fn notified(_ch: u64) {}
#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
