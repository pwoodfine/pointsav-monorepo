//! system-substrate — transport-agnostic capability invocation substrate.
//!
//! Defines the [`CapabilityInvoker`] trait and [`VerdictWire`] wire type.
//! Implementations carry a [`ConsultRequest`] over their transport — Unix socket
//! for the NetBSD compat bottom, Microkit PPC for the seL4 native bottom — and
//! return a [`VerdictWire`] without exposing transport details to callers.
//!
//! No codec or system-core dependencies are imported here. All encoded inputs
//! arrive as pre-serialized opaque byte slices, keeping this crate dependency-free.

#![cfg_attr(not(feature = "std"), no_std)]

/// Outcome of a capability consultation, transport-agnostic.
///
/// Mirrors `ConsultResponse` in `system-ledger-proto` without pulling that crate
/// into callers that only need the verdict type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictWire {
    Allow,
    Refuse { reason_code: u8 },
    ExtendThenAllow { new_expiry_t: u64 },
    Error { code: u8 },
}

/// Error returned when the substrate transport fails before a verdict is issued.
///
/// These errors are distinct from `VerdictWire::Refuse` and `VerdictWire::Error`,
/// which are ledger-level responses. `SubstrateError` means the request never
/// reached the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstrateError {
    /// Frame encode or decode failure (postcard or CBOR error).
    Codec,
    /// Transport I/O failure (socket closed, PPC error, shared-memory fault).
    Transport,
}

/// Transport-agnostic capability consultation interface.
///
/// Two implementations are planned:
/// - `UnixSocketInvoker` — NetBSD compat bottom (Unix domain socket to `system-ledger-server`).
/// - `PpcInvoker` — seL4 native bottom (Microkit PPC to `system-ledger-pd`).
///
/// Callers are parameterised over `impl CapabilityInvoker` and depend only on this
/// crate, not on any transport-specific crate.
///
/// All encoded inputs are pre-serialized opaque bytes so that `system-substrate`
/// itself needs no codec dependencies. See `system-ledger-proto` for the encoding:
/// - `cap_cbor`: CBOR-encoded `Capability` (via `ciborium::into_writer`).
/// - `ckpt_wire`: C2SP signed-note bytes (`SignedCheckpoint::to_wire().into_bytes()`).
/// - `witness_cbor`: CBOR-encoded `WitnessRecord`, if any.
pub trait CapabilityInvoker {
    /// Submit a capability consultation and return the verdict.
    ///
    /// # Arguments
    /// * `cap_cbor` — CBOR-encoded `Capability`.
    /// * `ckpt_wire` — C2SP signed-note bytes (`SignedCheckpoint::to_wire().into_bytes()`).
    /// * `now_unix` — current POSIX time in seconds.
    /// * `witness_cbor` — optional CBOR-encoded `WitnessRecord`.
    fn consult(
        &self,
        cap_cbor: &[u8],
        ckpt_wire: &[u8],
        now_unix: u64,
        witness_cbor: Option<&[u8]>,
    ) -> Result<VerdictWire, SubstrateError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_wire_debug() {
        assert_eq!(format!("{:?}", VerdictWire::Allow), "Allow");
        assert_eq!(
            format!("{:?}", VerdictWire::Refuse { reason_code: 1 }),
            "Refuse { reason_code: 1 }"
        );
    }

    #[test]
    fn substrate_error_debug() {
        assert_eq!(format!("{:?}", SubstrateError::Codec), "Codec");
        assert_eq!(format!("{:?}", SubstrateError::Transport), "Transport");
    }

    struct StubInvoker;
    impl CapabilityInvoker for StubInvoker {
        fn consult(
            &self,
            _cap_cbor: &[u8],
            _ckpt_wire: &[u8],
            _now_unix: u64,
            _witness_cbor: Option<&[u8]>,
        ) -> Result<VerdictWire, SubstrateError> {
            Ok(VerdictWire::Allow)
        }
    }

    #[test]
    fn trait_object_works() {
        let invoker: &dyn CapabilityInvoker = &StubInvoker;
        let verdict = invoker.consult(b"cap", b"ckpt", 1_700_000_000, None);
        assert_eq!(verdict, Ok(VerdictWire::Allow));
    }
}
