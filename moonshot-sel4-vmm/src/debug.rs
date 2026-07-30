// Serial output for seL4 Protection Domains via DebugPutChar.
//
// Requires KernelPrinting=ON in the seL4 kernel cmake config.
// In verification builds this path is absent; replace with VirtIO serial.

use crate::syscall;

/// Write a single byte to the seL4 debug serial channel.
#[inline]
pub fn putchar(c: u8) {
    // Safety: single-byte debug write, valid in any seL4 PD context with KernelPrinting=ON.
    unsafe { syscall::debug_put_char(c) }
}

/// Write a byte slice to the seL4 debug serial channel.
pub fn write_bytes(s: &[u8]) {
    for &b in s {
        putchar(b);
    }
}

/// Write a UTF-8 string to the seL4 debug serial channel.
pub fn puts(s: &str) {
    write_bytes(s.as_bytes());
}

/// Write a line (string + CRLF) to the seL4 debug serial channel.
pub fn puts_line(s: &str) {
    puts(s);
    putchar(b'\r');
    putchar(b'\n');
}

/// Enter an idle spin loop, yielding the CPU timeslice on each iteration.
///
/// Call when the PD has completed its work and has nothing left to do.
/// The seL4 scheduler will context-switch to other runnable PDs.
#[inline]
pub fn spin() -> ! {
    loop {
        // Safety: SysYield is always valid in a seL4 PD context.
        unsafe { syscall::yield_cpu() }
    }
}
