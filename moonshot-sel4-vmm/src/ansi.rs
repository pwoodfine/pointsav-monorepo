// ANSI terminal escape sequences for seL4 no_std serial PD IPC.
// All sequences are raw byte slices, compatible with the send_str protocol
// (24-byte chunks via seL4 endpoint IPC).

pub const CLEAR_HOME: &[u8] = b"\x1b[2J\x1b[H";
pub const RESET:      &[u8] = b"\x1b[0m";
pub const BOLD:       &[u8] = b"\x1b[1m";
pub const DIM:        &[u8] = b"\x1b[2m";
pub const GREEN:      &[u8] = b"\x1b[32m";
pub const CYAN:       &[u8] = b"\x1b[36m";
pub const YELLOW:     &[u8] = b"\x1b[33m";
pub const WHITE:      &[u8] = b"\x1b[37m";
