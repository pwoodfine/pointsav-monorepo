//! system-ledger-pd — seL4 Microkit Protection Domain wrapping InMemoryLedger.
//!
//! # Transport
//! Two shared memory regions carry the request/response:
//!
//! - `CAP_REQUEST_MR`  (16 KiB at 0x4001000): client writes length-prefixed postcard
//!   `ConsultRequest` before issuing PPC on channel `CONSULT_CH`.
//! - `CAP_RESPONSE_MR` (4 KiB  at 0x4005000): this PD writes length-prefixed postcard
//!   `ConsultResponse` before returning from `protected()`.
//!
//! The 4-byte length prefix is LE u32, identical to the Unix socket transport in
//! `system-ledger-server`, making the wire format transport-agnostic.
//!
//! # Execution model
//! Microkit PDs are single-threaded. `InMemoryLedger` requires no synchronisation.
//! The ledger is stored in a static `Option` initialised in `init()`.
//!
//! # Heap
//! A 512 KiB static array is managed by `linked_list_allocator`. It is initialised
//! at the top of `init()`, before the first allocation.

#![no_std]
#![no_main]
// Static mutable references are safe here: seL4 Microkit PDs are single-threaded;
// no concurrent access to HEAP_MEM, ALLOCATOR, or LEDGER is possible.
#![allow(static_mut_refs)]

extern crate alloc;

use linked_list_allocator::LockedHeap;
use postcard::{from_bytes, to_slice};
use system_core::{Capability, SignedCheckpoint, WitnessRecord};
use system_ledger::{InMemoryLedger, LedgerConsumer, RefuseReason, Verdict};
use system_ledger_proto::{error_code, reason_code, ConsultRequest, ConsultResponse};

// ── Global heap ──────────────────────────────────────────────────────────────

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// 512 KiB: holds InMemoryLedger's BTreeMaps plus ConsultRequest decode buffers.
static mut HEAP_MEM: [u8; 512 * 1024] = [0u8; 512 * 1024];

// ── Shared memory layout ──────────────────────────────────────────────────────

/// PPC channel ID: `client_pd` issues `microkit_ppcall(CONSULT_CH, ...)` to reach us.
const CONSULT_CH: u64 = 1;

/// Shared memory region addresses (must match `ledger.system` XML).
const CAP_REQUEST_ADDR: usize  = 0x4001_000;
const CAP_REQUEST_SIZE: usize  = 16 * 1024;
const CAP_RESPONSE_ADDR: usize = 0x4005_000;
#[allow(dead_code)]
const CAP_RESPONSE_SIZE: usize = 4 * 1024;   // used by write_response bounds check (planned Phase 1C)

// ── Static ledger ─────────────────────────────────────────────────────────────

// Safety: single-threaded Microkit PD; no concurrent access.
static mut LEDGER: Option<InMemoryLedger> = None;

// ── microkit ABI (declared in shim.c) ────────────────────────────────────────

extern "C" {
    fn microkit_dbg_puts(s: *const u8);
}

fn print(s: &[u8]) {
    // SAFETY: shim.c exposes the microkit inline function; s must be null-terminated.
    unsafe { microkit_dbg_puts(s.as_ptr()); }
}

// ── Microkit entry points ─────────────────────────────────────────────────────

/// Called once by the Microkit runtime after seL4 initialises the PD.
#[no_mangle]
pub extern "C" fn init() {
    // Initialise heap before any allocation.
    unsafe { ALLOCATOR.lock().init(HEAP_MEM.as_mut_ptr(), HEAP_MEM.len()); }

    unsafe { LEDGER = Some(InMemoryLedger::new()); }

    print(b"LEDGER PD: online\n\0");
}

/// Called when `client_pd` issues `microkit_ppcall(CONSULT_CH, ...)`.
///
/// Reads `ConsultRequest` from `CAP_REQUEST_MR`, delegates to `InMemoryLedger`,
/// writes `ConsultResponse` to `CAP_RESPONSE_MR`, and returns. The return value
/// is the Microkit msginfo for the PPC reply (we return 0 — no reply registers used;
/// the response is in the shared MR).
#[no_mangle]
pub extern "C" fn protected(ch: u64, _msginfo: u64) -> u64 {
    if ch != CONSULT_CH {
        write_response(&ConsultResponse::Error { code: error_code::INTERNAL });
        return 0;
    }

    let req = match read_request() {
        Some(r) => r,
        None => {
            write_response(&ConsultResponse::Error { code: error_code::DECODE_CAP });
            return 0;
        }
    };

    let ledger = unsafe {
        match LEDGER.as_ref() {
            Some(l) => l,
            None => {
                write_response(&ConsultResponse::Error { code: error_code::INTERNAL });
                return 0;
            }
        }
    };

    let resp = handle_request(ledger, &req);
    write_response(&resp);
    0
}

/// Called when another PD issues `microkit_notify(ch)` — reserved for future use.
#[no_mangle]
pub extern "C" fn notified(_ch: u64) {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    print(b"LEDGER PD: panic\n\0");
    loop {}
}

// ── Transport helpers ─────────────────────────────────────────────────────────

/// Read one length-prefixed postcard `ConsultRequest` from `CAP_REQUEST_MR`.
fn read_request() -> Option<ConsultRequest> {
    // SAFETY: the shared MR is mapped read-write into this PD's address space by Microkit.
    let base = CAP_REQUEST_ADDR as *const u8;
    let len_bytes = unsafe { core::slice::from_raw_parts(base, 4) };
    let len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
    if len == 0 || len > CAP_REQUEST_SIZE.saturating_sub(4) {
        return None;
    }
    let payload = unsafe { core::slice::from_raw_parts(base.add(4), len) };
    from_bytes(payload).ok()
}

/// Write one length-prefixed postcard `ConsultResponse` to `CAP_RESPONSE_MR`.
fn write_response(resp: &ConsultResponse) {
    let base = CAP_RESPONSE_ADDR as *mut u8;
    // Stack-allocate a buffer — ConsultResponse is at most ~16 bytes encoded.
    let mut buf = [0u8; 64];
    match to_slice(resp, &mut buf) {
        Ok(encoded) => {
            let len = encoded.len() as u32;
            // SAFETY: Microkit maps CAP_RESPONSE_MR writable into this PD.
            unsafe {
                core::ptr::copy_nonoverlapping(len.to_le_bytes().as_ptr(), base, 4);
                core::ptr::copy_nonoverlapping(encoded.as_ptr(), base.add(4), encoded.len());
            }
        }
        Err(_) => {
            // Write a minimal internal-error response without allocation.
            let fallback = [0u8, 0u8, 0u8, 2u8,  // len = 2 (LE)
                            3u8,                   // enum tag for ConsultResponse::Error
                            error_code::INTERNAL]; // code field
            unsafe { core::ptr::copy_nonoverlapping(fallback.as_ptr(), base, fallback.len()); }
        }
    }
}

// ── Ledger dispatch (mirrors system-ledger-server::handle_request) ─────────────

fn handle_request(ledger: &InMemoryLedger, req: &ConsultRequest) -> ConsultResponse {
    use ciborium::from_reader as cbor_from;

    let cap: Capability = match cbor_from(req.cap_cbor.as_slice()) {
        Ok(c) => c,
        Err(_) => return ConsultResponse::Error { code: error_code::DECODE_CAP },
    };

    let ckpt_str = match core::str::from_utf8(&req.ckpt_wire) {
        Ok(s) => s,
        Err(_) => return ConsultResponse::Error { code: error_code::DECODE_CKPT },
    };
    let ckpt: SignedCheckpoint = match SignedCheckpoint::parse(ckpt_str) {
        Ok(c) => c,
        Err(_) => return ConsultResponse::Error { code: error_code::DECODE_CKPT },
    };

    let witness: Option<WitnessRecord> = match req.witness_cbor {
        None => None,
        Some(ref b) => match cbor_from(b.as_slice()) {
            Ok(w) => Some(w),
            Err(_) => return ConsultResponse::Error { code: error_code::DECODE_WITNESS },
        },
    };

    match ledger.consult_capability(&cap, &ckpt, req.now_unix, witness.as_ref()) {
        Ok(Verdict::Allow) => ConsultResponse::Allow,
        Ok(Verdict::ExtendThenAllow { new_expiry_t }) => {
            ConsultResponse::ExtendThenAllow { new_expiry_t }
        }
        Ok(Verdict::Refuse(reason)) => {
            let code = match reason {
                RefuseReason::Expired => reason_code::EXPIRED,
                RefuseReason::Revoked => reason_code::REVOKED,
                RefuseReason::WitnessNotInLedger | RefuseReason::WitnessSignatureInvalid => {
                    reason_code::INVALID_WITNESS
                }
                RefuseReason::ApexInvalid | RefuseReason::StaleApex => reason_code::INVALID_PROOF,
                RefuseReason::NotExtensible => reason_code::EXPIRED,
            };
            ConsultResponse::Refuse { reason_code: code }
        }
        Err(_) => ConsultResponse::Error { code: error_code::INTERNAL },
    }
}
