//! system-ledger-server — Unix socket transport shell for the Capability Ledger.
//!
//! Binds to `LEDGER_SOCK` (default `/run/system-ledger/ledger.sock`), accepts
//! sequential connections, and for each connection reads length-prefixed
//! postcard `ConsultRequest` frames, delegates to `InMemoryLedger`, and writes
//! `ConsultResponse` frames back. Zero business logic lives here — all decisions
//! are made by `system-ledger`.
//!
//! Single-threaded by design: `InMemoryLedger` is not `Sync` and the
//! single-writer invariant is the correct model for the kernel-side substrate
//! (per `system-substrate-doctrine.md` §3.1).
//!
//! # Frame format
//! Each message is prefixed by a 4-byte little-endian length (u32) followed by
//! the postcard payload. The seL4 PD (`system-ledger-pd`) uses the same 4-byte
//! header in its shared-memory ring, so the wire format is transport-agnostic.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use ciborium::from_reader as cbor_from;
use postcard::{from_bytes, to_allocvec};
use system_core::{Capability, SignedCheckpoint, WitnessRecord};
use system_ledger::{InMemoryLedger, LedgerConsumer, RefuseReason, Verdict};
use system_ledger_proto::{error_code, reason_code, ConsultRequest, ConsultResponse};

fn ledger_sock_path() -> PathBuf {
    std::env::var("LEDGER_SOCK")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/run/system-ledger/ledger.sock"))
}

/// Read exactly n bytes from a stream, blocking until available.
fn read_exact(stream: &mut UnixStream, buf: &mut [u8]) -> std::io::Result<()> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = stream.read(&mut buf[offset..])?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }
        offset += n;
    }
    Ok(())
}

/// Read one length-prefixed postcard frame from the stream.
fn read_frame(stream: &mut UnixStream) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    read_exact(stream, &mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > 64 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut payload = vec![0u8; len];
    read_exact(stream, &mut payload)?;
    Ok(payload)
}

/// Write one length-prefixed postcard frame to the stream.
fn write_frame(stream: &mut UnixStream, payload: &[u8]) -> std::io::Result<()> {
    let len = payload.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(payload)
}

/// Decode a ConsultRequest and render a ConsultResponse via the ledger.
fn handle_request(ledger: &InMemoryLedger, req_bytes: &[u8]) -> ConsultResponse {
    let req: ConsultRequest = match from_bytes(req_bytes) {
        Ok(r) => r,
        Err(_) => {
            return ConsultResponse::Error {
                code: error_code::DECODE_CAP,
            }
        }
    };

    let cap: Capability = match cbor_from(req.cap_cbor.as_slice()) {
        Ok(c) => c,
        Err(_) => {
            return ConsultResponse::Error {
                code: error_code::DECODE_CAP,
            }
        }
    };

    let ckpt_str = match std::str::from_utf8(&req.ckpt_wire) {
        Ok(s) => s,
        Err(_) => {
            return ConsultResponse::Error {
                code: error_code::DECODE_CKPT,
            }
        }
    };
    let ckpt: SignedCheckpoint = match SignedCheckpoint::parse(ckpt_str) {
        Ok(c) => c,
        Err(_) => {
            return ConsultResponse::Error {
                code: error_code::DECODE_CKPT,
            }
        }
    };

    let witness: Option<WitnessRecord> = match req.witness_cbor {
        None => None,
        Some(ref b) => match cbor_from(b.as_slice()) {
            Ok(w) => Some(w),
            Err(_) => {
                return ConsultResponse::Error {
                    code: error_code::DECODE_WITNESS,
                }
            }
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
        Err(_) => ConsultResponse::Error {
            code: error_code::INTERNAL,
        },
    }
}

/// Serve one connection to completion (blocking, single-request-per-connection).
fn serve_connection(stream: &mut UnixStream, ledger: &InMemoryLedger) {
    loop {
        let frame = match read_frame(stream) {
            Ok(f) => f,
            Err(_) => break,
        };
        let resp = handle_request(ledger, &frame);
        let resp_bytes = match to_allocvec(&resp) {
            Ok(b) => b,
            Err(_) => break,
        };
        if write_frame(stream, &resp_bytes).is_err() {
            break;
        }
    }
}

fn main() -> std::io::Result<()> {
    let sock_path = ledger_sock_path();

    // Remove stale socket from a prior run.
    let _ = std::fs::remove_file(&sock_path);

    // Ensure the socket directory exists.
    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&sock_path)?;
    eprintln!("system-ledger-server: listening on {}", sock_path.display());

    let ledger = InMemoryLedger::new();

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => serve_connection(&mut s, &ledger),
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream as StdStream;
    use system_core::{Capability, CapabilityType, LedgerAnchor, Right};

    fn temp_sock_path() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("ledger-test-{}-{}.sock", std::process::id(), n));
        p
    }

    fn test_cap() -> Capability {
        Capability {
            cap_type: CapabilityType::Endpoint,
            rights: vec![Right::Invoke],
            expiry_t: None,
            witness_pubkey: None,
            ledger_anchor: LedgerAnchor {
                origin: "test.ledger".to_string(),
                tree_size: 1,
                root_hash: [0xAA; 32],
            },
        }
    }

    fn cbor_encode<T: serde::Serialize>(val: &T) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(val, &mut buf).expect("cbor encode");
        buf
    }

    fn send_request(stream: &mut StdStream, req: &ConsultRequest) -> ConsultResponse {
        let payload = to_allocvec(req).expect("encode request");
        let len = payload.len() as u32;
        stream.write_all(&len.to_le_bytes()).expect("write len");
        stream.write_all(&payload).expect("write payload");

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).expect("read resp len");
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf).expect("read resp payload");
        from_bytes(&resp_buf).expect("decode response")
    }

    /// Spawn a server thread; returns only after the socket is bound and ready.
    fn spawn_server(path: PathBuf) -> std::thread::JoinHandle<()> {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            let _ = std::fs::remove_file(&path);
            let listener = UnixListener::bind(&path).expect("bind");
            tx.send(()).ok(); // signal: socket is ready
            let ledger = InMemoryLedger::new();
            // Serve exactly one connection for tests.
            if let Ok(mut s) = listener.accept().map(|(s, _)| s) {
                serve_connection(&mut s, &ledger);
            }
        });
        rx.recv().expect("server ready signal"); // block until socket bound
        handle
    }

    #[test]
    fn error_on_empty_frame() {
        let path = temp_sock_path();
        let _h = spawn_server(path.clone());
        let mut stream = StdStream::connect(&path).expect("connect");

        // Send a zero-length frame — should get an error response.
        stream.write_all(&0u32.to_le_bytes()).expect("write len");
        let mut len_buf = [0u8; 4];
        // The server will try to parse a 0-byte postcard payload → decoding error.
        // It should still return ConsultResponse::Error rather than panicking.
        stream.read_exact(&mut len_buf).expect("read resp len");
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf).expect("read resp payload");
        let resp: ConsultResponse = from_bytes(&resp_buf).expect("decode");
        assert!(matches!(resp, ConsultResponse::Error { .. }));
    }

    #[test]
    fn no_apex_returns_refuse() {
        use ed25519_dalek::{Signer, SigningKey};
        use system_core::{Checkpoint, NoteSignature, SignedCheckpoint};

        let path = temp_sock_path();
        let _h = spawn_server(path.clone());
        let mut stream = StdStream::connect(&path).expect("connect");

        let cap = test_cap();
        let sk = SigningKey::from_bytes(&[0x42u8; 32]);
        let pk = sk.verifying_key().to_bytes();
        let cp = Checkpoint {
            origin: "test.ledger".to_string(),
            tree_size: 1,
            root_hash: [0xBB; 32],
            extensions: vec![],
        };
        let body = cp.body_bytes();
        let key_hash = NoteSignature::derive_key_hash("apex", &pk);
        let sig = sk.sign(&body).to_bytes();
        let ckpt = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![NoteSignature {
                signer_name: "apex".to_string(),
                key_hash,
                signature: sig,
            }],
        };

        let req = ConsultRequest {
            cap_cbor: cbor_encode(&cap),
            ckpt_wire: ckpt.to_wire().into_bytes(),
            now_unix: 1_700_000_000,
            witness_cbor: None,
        };
        let resp = send_request(&mut stream, &req);
        // No apex registered → ApexInvalid → Refuse
        assert!(
            matches!(resp, ConsultResponse::Refuse { .. }),
            "expected Refuse, got {resp:?}"
        );
    }

    #[test]
    fn malformed_cap_cbor_returns_error() {
        let path = temp_sock_path();
        let _h = spawn_server(path.clone());
        let mut stream = StdStream::connect(&path).expect("connect");

        let req = ConsultRequest {
            cap_cbor: vec![0xFF, 0xFF, 0xFF], // invalid CBOR
            ckpt_wire: vec![],
            now_unix: 0,
            witness_cbor: None,
        };
        let resp = send_request(&mut stream, &req);
        assert!(matches!(resp, ConsultResponse::Error { code } if code == error_code::DECODE_CAP));
    }

    #[test]
    fn malformed_ckpt_cbor_returns_error() {
        use system_core::{Capability, CapabilityType, LedgerAnchor, Right};

        let path = temp_sock_path();
        let _h = spawn_server(path.clone());
        let mut stream = StdStream::connect(&path).expect("connect");

        let cap = Capability {
            cap_type: CapabilityType::Memory,
            rights: vec![Right::Read],
            expiry_t: None,
            witness_pubkey: None,
            ledger_anchor: LedgerAnchor {
                origin: "t".to_string(),
                tree_size: 1,
                root_hash: [0; 32],
            },
        };
        let req = ConsultRequest {
            cap_cbor: cbor_encode(&cap),
            ckpt_wire: b"\xFF\xFE invalid utf-8".to_vec(), // invalid UTF-8 → parse fails
            now_unix: 0,
            witness_cbor: None,
        };
        let resp = send_request(&mut stream, &req);
        assert!(matches!(resp, ConsultResponse::Error { code } if code == error_code::DECODE_CKPT));
    }

    #[test]
    fn multiple_requests_on_one_connection() {
        use ed25519_dalek::{Signer, SigningKey};
        use system_core::{Checkpoint, NoteSignature, SignedCheckpoint};

        let path = temp_sock_path();
        let _h = spawn_server(path.clone());
        let mut stream = StdStream::connect(&path).expect("connect");

        let cap = test_cap();
        let sk = SigningKey::from_bytes(&[0x77u8; 32]);
        let pk = sk.verifying_key().to_bytes();
        let make_ckpt = |n: u64| {
            let cp = Checkpoint {
                origin: "test.ledger".to_string(),
                tree_size: n,
                root_hash: [0xCC; 32],
                extensions: vec![],
            };
            let body = cp.body_bytes();
            let key_hash = NoteSignature::derive_key_hash("apex", &pk);
            let sig = sk.sign(&body).to_bytes();
            SignedCheckpoint {
                checkpoint: cp,
                signatures: vec![NoteSignature {
                    signer_name: "apex".to_string(),
                    key_hash,
                    signature: sig,
                }],
            }
        };

        for n in 1..=3 {
            let req = ConsultRequest {
                cap_cbor: cbor_encode(&cap),
                ckpt_wire: make_ckpt(n).to_wire().into_bytes(),
                now_unix: 1_700_000_000,
                witness_cbor: None,
            };
            let resp = send_request(&mut stream, &req);
            // No apex → Refuse; 3 times in a row on the same connection.
            assert!(
                matches!(resp, ConsultResponse::Refuse { .. }),
                "request {n}: expected Refuse"
            );
        }
    }
}
