/*
 * shim.c — supplementary C stubs for system-ledger-pd.
 *
 * microkit_dbg_puts, microkit_ppcall, and all Microkit protocol functions
 * are provided by libmicrokit.a — do not re-declare them here.
 *
 * Add stubs here only for symbols required by the Rust staticlib that are
 * absent from both libmicrokit.a and the standard freestanding headers.
 */
