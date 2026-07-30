//! system-ledger-proto — transport-agnostic wire types for Capability Ledger consultation.
//!
//! [`ConsultRequest`] and [`ConsultResponse`] are postcard-serialized identically
//! whether the transport is a Unix socket (NetBSD compat bottom daemon) or a
//! seL4 Microkit PPC shared-memory ring (seL4 native bottom PD). Transport changes;
//! protocol and business logic do not.
//!
//! Substrate types are carried as opaque bytes in `ConsultRequest`:
//! - `cap_cbor` — CBOR-encoded `Capability` (derives `serde::Deserialize`).
//! - `ckpt_wire` — C2SP signed-note wire format for `SignedCheckpoint` (does NOT
//!   derive serde; decode via `SignedCheckpoint::parse()`).
//! - `witness_cbor` — CBOR-encoded `WitnessRecord` (derives `serde::Deserialize`).

#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// Sent from a ledger client to `system-ledger-server` or `system-ledger-pd`.
///
/// Substrate types are encoded as opaque byte vectors to avoid importing the
/// full codec chain into this `no_std` crate:
/// - `cap_cbor` / `witness_cbor`: CBOR-encoded (decode with `ciborium::from_reader`).
/// - `ckpt_wire`: C2SP signed-note text (decode with `SignedCheckpoint::parse()`).
///   `SignedCheckpoint` does not implement `serde::Deserialize`; it uses a custom
///   text wire format. Clients encode via `signed_checkpoint.to_wire().into_bytes()`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConsultRequest {
    /// CBOR-encoded `system_core::Capability`.
    pub cap_cbor: Vec<u8>,
    /// C2SP signed-note wire bytes (`SignedCheckpoint::to_wire().into_bytes()`).
    /// Decoded server-side via `SignedCheckpoint::parse(std::str::from_utf8(...))`.
    pub ckpt_wire: Vec<u8>,
    /// Current POSIX time (seconds). Used for `expiry_t` comparison.
    pub now_unix: u64,
    /// Optional CBOR-encoded `system_core::WitnessRecord` for temporal extension.
    pub witness_cbor: Option<Vec<u8>>,
}

/// Sent back from `system-ledger-server` or `system-ledger-pd`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum ConsultResponse {
    Allow,
    Refuse { reason_code: u8 },
    ExtendThenAllow { new_expiry_t: u64 },
    Error { code: u8 },
}

/// Error codes for `ConsultResponse::Error`.
pub mod error_code {
    /// Failed to CBOR-decode the `Capability` field.
    pub const DECODE_CAP: u8 = 1;
    /// Failed to decode the `SignedCheckpoint` wire field (UTF-8 or parse error).
    pub const DECODE_CKPT: u8 = 2;
    /// Failed to CBOR-decode the `WitnessRecord` field.
    pub const DECODE_WITNESS: u8 = 3;
    /// Internal state machine error (should not occur; indicates a bug).
    pub const INTERNAL: u8 = 255;
}

/// Reason codes for `ConsultResponse::Refuse`.
pub mod reason_code {
    /// Capability `expiry_t` is in the past and no valid witness extends it.
    pub const EXPIRED: u8 = 1;
    /// Capability hash appears in the revocation set.
    pub const REVOKED: u8 = 2;
    /// Inclusion proof verification failed against the current checkpoint.
    pub const INVALID_PROOF: u8 = 3;
    /// Witness signature verification failed.
    pub const INVALID_WITNESS: u8 = 4;
    /// Apex history post-handover invariant violated (N+3+ rule).
    pub const POST_HANDOVER: u8 = 5;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consult_request_roundtrip() {
        let req = ConsultRequest {
            cap_cbor: vec![0x01, 0x02, 0x03],
            ckpt_wire: b"test.ledger/1\n\nhash+1234=\n".to_vec(),
            now_unix: 1_700_000_000,
            witness_cbor: None,
        };
        let encoded = postcard::to_allocvec(&req).expect("encode");
        let decoded: ConsultRequest = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(decoded.now_unix, req.now_unix);
        assert_eq!(decoded.cap_cbor, req.cap_cbor);
        assert_eq!(decoded.ckpt_wire, req.ckpt_wire);
        assert!(decoded.witness_cbor.is_none());
    }

    #[test]
    fn consult_request_with_witness_roundtrip() {
        let req = ConsultRequest {
            cap_cbor: vec![0x01],
            ckpt_wire: b"test.ledger/1\n\nhash+1234=\n".to_vec(),
            now_unix: 1_800_000_000,
            witness_cbor: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        };
        let encoded = postcard::to_allocvec(&req).expect("encode");
        let decoded: ConsultRequest = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(decoded.witness_cbor, req.witness_cbor);
    }

    #[test]
    fn consult_response_allow_roundtrip() {
        let resp = ConsultResponse::Allow;
        let encoded = postcard::to_allocvec(&resp).expect("encode");
        let decoded: ConsultResponse = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(decoded, ConsultResponse::Allow);
    }

    #[test]
    fn consult_response_refuse_roundtrip() {
        let resp = ConsultResponse::Refuse {
            reason_code: reason_code::EXPIRED,
        };
        let encoded = postcard::to_allocvec(&resp).expect("encode");
        let decoded: ConsultResponse = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(
            decoded,
            ConsultResponse::Refuse {
                reason_code: reason_code::EXPIRED
            }
        );
    }

    #[test]
    fn consult_response_extend_roundtrip() {
        let resp = ConsultResponse::ExtendThenAllow {
            new_expiry_t: 1_900_000_000,
        };
        let encoded = postcard::to_allocvec(&resp).expect("encode");
        let decoded: ConsultResponse = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(
            decoded,
            ConsultResponse::ExtendThenAllow {
                new_expiry_t: 1_900_000_000
            }
        );
    }

    #[test]
    fn consult_response_error_roundtrip() {
        let resp = ConsultResponse::Error {
            code: error_code::INTERNAL,
        };
        let encoded = postcard::to_allocvec(&resp).expect("encode");
        let decoded: ConsultResponse = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(
            decoded,
            ConsultResponse::Error {
                code: error_code::INTERNAL
            }
        );
    }
}
