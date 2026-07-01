#![allow(clippy::unit_arg)]
//! criterion benchmarks for the kernel-side ledger consultation
//! latency budget. Master 4b deliverable from
//! `~/Foundry/clones/project-system/.claude/inbox-archive.md`.
//!
//! Run with: `cargo bench -p system-ledger`. Numbers surface in
//! `target/criterion/<bench>/report/`.
#![allow(clippy::unit_arg)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ed25519_dalek::{Signer, SigningKey};
use system_core::{
    rfc9162_internal_hash, rfc9162_leaf_hash, Capability, CapabilityType, Checkpoint,
    ConsistencyProof, InclusionProof, LedgerAnchor, NoteSignature, Right, SignedCheckpoint,
    WitnessRecord,
};
use system_ledger::{InMemoryLedger, LedgerConsumer};

fn keypair(seed: u8) -> (SigningKey, [u8; 32]) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let pk = sk.verifying_key().to_bytes();
    (sk, pk)
}

fn fixture_capability() -> Capability {
    Capability {
        cap_type: CapabilityType::Endpoint,
        rights: vec![Right::Invoke, Right::Read],
        expiry_t: None,
        witness_pubkey: None,
        ledger_anchor: LedgerAnchor {
            origin: "foundry.bench.cap-ledger".to_string(),
            tree_size: 1,
            root_hash: [0xAA; 32],
        },
    }
}

fn signed_checkpoint(
    tree_size: u64,
    root_byte: u8,
    signers: &[(&str, &SigningKey)],
) -> SignedCheckpoint {
    let cp = Checkpoint {
        origin: "foundry.bench.cap-ledger".to_string(),
        tree_size,
        root_hash: [root_byte; 32],
        extensions: vec![],
    };
    let body = cp.body_bytes();
    let signatures = signers
        .iter()
        .map(|(name, sk)| {
            let pk = sk.verifying_key().to_bytes();
            let key_hash = NoteSignature::derive_key_hash(name, &pk);
            let sig = sk.sign(&body).to_bytes();
            NoteSignature {
                signer_name: name.to_string(),
                key_hash,
                signature: sig,
            }
        })
        .collect();
    SignedCheckpoint {
        checkpoint: cp,
        signatures,
    }
}

fn bench_capability_hash(c: &mut Criterion) {
    let cap = fixture_capability();
    c.bench_function("Capability::hash", |b| b.iter(|| black_box(cap.hash())));
}

fn bench_verify_signer_single(c: &mut Criterion) {
    let (sk, pk) = keypair(0x11);
    let signed = signed_checkpoint(100, 0xAA, &[("apex", &sk)]);
    c.bench_function("SignedCheckpoint::verify_signer (1-sig)", |b| {
        b.iter(|| black_box(signed.verify_signer("apex", &pk).unwrap()))
    });
}

fn bench_verify_apex_handover(c: &mut Criterion) {
    let (sk_old, pk_old) = keypair(0x11);
    let (sk_new, pk_new) = keypair(0x22);
    let signed = signed_checkpoint(100, 0xCD, &[("apex-old", &sk_old), ("apex-new", &sk_new)]);
    c.bench_function("SignedCheckpoint::verify_apex_handover (2-sig)", |b| {
        b.iter(|| {
            black_box(
                signed
                    .verify_apex_handover("apex-old", &pk_old, "apex-new", &pk_new)
                    .unwrap(),
            )
        })
    });
}

fn bench_cache_hit(c: &mut Criterion) {
    let (sk, _pk) = keypair(0x11);
    let mut ledger = InMemoryLedger::new();
    // Fill cache with 64 entries; the target lookup is the LAST one
    // inserted (most-recent — best cache-hit case).
    for h in 0..64u64 {
        ledger
            .cache
            .insert(signed_checkpoint(h, h as u8, &[("apex", &sk)]));
    }
    c.bench_function("cache lookup_by_tree_size (hit, most-recent)", |b| {
        b.iter(|| {
            black_box(
                ledger
                    .cache
                    .lookup_by_tree_size("foundry.bench.cap-ledger", 63),
            )
        })
    });
}

fn bench_cache_miss(c: &mut Criterion) {
    let (sk, _pk) = keypair(0x11);
    let mut ledger = InMemoryLedger::new();
    for h in 0..64u64 {
        ledger
            .cache
            .insert(signed_checkpoint(h, h as u8, &[("apex", &sk)]));
    }
    c.bench_function("cache lookup_by_tree_size (miss, full scan)", |b| {
        b.iter(|| {
            black_box(
                ledger
                    .cache
                    .lookup_by_tree_size("foundry.bench.cap-ledger", 99999),
            )
        })
    });
}

fn bench_consult_capability_allow(c: &mut Criterion) {
    let (sk, pk) = keypair(0x11);
    let mut ledger = InMemoryLedger::new();
    ledger.apex.record_genesis("apex", pk, 0).unwrap();
    let cap = fixture_capability();
    let root = signed_checkpoint(5, 0xAA, &[("apex", &sk)]);
    c.bench_function("consult_capability (Allow path; 1-sig apex verify)", |b| {
        b.iter(|| black_box(ledger.consult_capability(&cap, &root, 1000, None).unwrap()))
    });
}

// ---------- Phase 1A.4 inclusion-proof benchmarks ----------

fn build_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    let mut layer = leaves.to_vec();
    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        let mut i = 0;
        while i < layer.len() {
            if i + 1 < layer.len() {
                next.push(rfc9162_internal_hash(&layer[i], &layer[i + 1]));
            } else {
                next.push(layer[i]);
            }
            i += 2;
        }
        layer = next;
    }
    layer.into_iter().next().unwrap()
}

fn make_inclusion_proof(leaves: &[[u8; 32]], leaf_index: u64) -> InclusionProof {
    let mut path = Vec::new();
    let mut layer = leaves.to_vec();
    let mut idx = leaf_index as usize;
    while layer.len() > 1 {
        let sibling_idx = idx ^ 1;
        if sibling_idx < layer.len() {
            path.push(layer[sibling_idx]);
        }
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        let mut i = 0;
        while i < layer.len() {
            if i + 1 < layer.len() {
                next.push(rfc9162_internal_hash(&layer[i], &layer[i + 1]));
            } else {
                next.push(layer[i]);
            }
            i += 2;
        }
        idx /= 2;
        layer = next;
    }
    InclusionProof {
        leaf_index,
        tree_size: leaves.len() as u64,
        sibling_hashes: path,
    }
}

fn bench_verify_inclusion_proof_raw_8_leaves(c: &mut Criterion) {
    let leaves: Vec<[u8; 32]> = (0..8u64)
        .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
        .collect();
    let root = build_merkle_root(&leaves);
    let proof = make_inclusion_proof(&leaves, 4);
    c.bench_function(
        "InclusionProof::verify (raw, tree-size 8 — 3-hash path)",
        |b| b.iter(|| proof.verify(&leaves[4], &root).unwrap()),
    );
}

fn bench_verify_inclusion_proof_raw_1024_leaves(c: &mut Criterion) {
    let leaves: Vec<[u8; 32]> = (0..1024u64)
        .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
        .collect();
    let root = build_merkle_root(&leaves);
    let proof = make_inclusion_proof(&leaves, 512);
    c.bench_function(
        "InclusionProof::verify (raw, tree-size 1024 — 10-hash path)",
        |b| b.iter(|| proof.verify(&leaves[512], &root).unwrap()),
    );
}

fn bench_signed_checkpoint_verify_inclusion_proof(c: &mut Criterion) {
    let (sk, pk) = keypair(0x55);
    let leaves: Vec<[u8; 32]> = (0..1024u64)
        .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
        .collect();
    let root = build_merkle_root(&leaves);
    let proof = make_inclusion_proof(&leaves, 512);
    let cp = Checkpoint {
        origin: "foundry.bench.cap-ledger".to_string(),
        tree_size: 1024,
        root_hash: root,
        extensions: vec![],
    };
    let body = cp.body_bytes();
    let key_hash = NoteSignature::derive_key_hash("apex", &pk);
    let sig = sk.sign(&body).to_bytes();
    let signed = SignedCheckpoint {
        checkpoint: cp,
        signatures: vec![NoteSignature {
            signer_name: "apex".to_string(),
            key_hash,
            signature: sig,
        }],
    };
    c.bench_function(
        "SignedCheckpoint::verify_inclusion_proof (composed, 1024-leaf tree)",
        |b| {
            b.iter(|| {
                signed
                    .verify_inclusion_proof(&proof, &leaves[512], "apex", &pk)
                    .unwrap()
            })
        },
    );
}

fn bench_apply_witness_record_with_proof(c: &mut Criterion) {
    let (sk, pk) = keypair(0x66);
    let witness = WitnessRecord {
        capability_hash: [0xBB; 32],
        new_expiry_t: 5000,
        signature: vec![],
    };
    // Compute leaf hash matching InMemoryLedger::witness_record_leaf_hash (ciborium CBOR).
    let mut bytes = Vec::new();
    ciborium::into_writer(&witness, &mut bytes).expect("serializable");
    let witness_leaf = rfc9162_leaf_hash(&bytes);

    // Build a 1024-leaf tree with our witness at index 512.
    let mut leaves: Vec<[u8; 32]> = (0..1024u64)
        .map(|i| rfc9162_leaf_hash(format!("filler-{i}").as_bytes()))
        .collect();
    leaves[512] = witness_leaf;
    let root = build_merkle_root(&leaves);
    let proof = make_inclusion_proof(&leaves, 512);

    // Build the signed checkpoint at this root.
    let cp = Checkpoint {
        origin: "foundry.bench.cap-ledger".to_string(),
        tree_size: 1024,
        root_hash: root,
        extensions: vec![],
    };
    let body = cp.body_bytes();
    let key_hash = NoteSignature::derive_key_hash("apex", &pk);
    let sig = sk.sign(&body).to_bytes();
    let signed_cp = SignedCheckpoint {
        checkpoint: cp,
        signatures: vec![NoteSignature {
            signer_name: "apex".to_string(),
            key_hash,
            signature: sig,
        }],
    };

    c.bench_function(
        "apply_witness_record (full path: verify_inclusion_proof + insert)",
        |b| {
            b.iter_batched(
                || {
                    let mut ledger = InMemoryLedger::new();
                    ledger.apex.record_genesis("apex", pk, 0).unwrap();
                    ledger.set_current_checkpoint(signed_cp.clone());
                    ledger
                },
                |mut ledger| {
                    ledger
                        .apply_witness_record(witness.clone(), proof.clone())
                        .unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        },
    );
}

// ---------- Phase 1A.5 consistency-proof benchmarks ----------

fn bench_verify_consistency_proof_raw(c: &mut Criterion) {
    // 4→8 transition. Proof per the accumulator verifier algorithm (not the
    // RFC generation function): anchor=leaves[3], then two BOTH-branch
    // siblings (leaves[2], internal(l0,l1)), then one ELSE-branch sibling
    // (right-half root internal(internal(l4,l5),internal(l6,l7))). 4 hashes.
    let leaves: Vec<[u8; 32]> = (0..8u64)
        .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
        .collect();
    let old_root = build_merkle_root(&leaves[..4]);
    let new_root = build_merkle_root(&leaves);
    let proof = ConsistencyProof {
        hashes: vec![
            leaves[3],
            leaves[2],
            rfc9162_internal_hash(&leaves[0], &leaves[1]),
            rfc9162_internal_hash(
                &rfc9162_internal_hash(&leaves[4], &leaves[5]),
                &rfc9162_internal_hash(&leaves[6], &leaves[7]),
            ),
        ],
    };
    c.bench_function(
        "ConsistencyProof::verify (raw, 4→8 transition — 4-hash proof)",
        |b| b.iter(|| black_box(proof.verify(old_root, 4, new_root, 8).unwrap())),
    );
}

fn bench_signed_checkpoint_verify_consistency_proof(c: &mut Criterion) {
    let (sk, pk) = keypair(0x77);
    let leaves: Vec<[u8; 32]> = (0..8u64)
        .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
        .collect();
    let old_root = build_merkle_root(&leaves[..4]);
    let new_root = build_merkle_root(&leaves);
    let proof = ConsistencyProof {
        hashes: vec![
            leaves[3],
            leaves[2],
            rfc9162_internal_hash(&leaves[0], &leaves[1]),
            rfc9162_internal_hash(
                &rfc9162_internal_hash(&leaves[4], &leaves[5]),
                &rfc9162_internal_hash(&leaves[6], &leaves[7]),
            ),
        ],
    };
    let old_cp = Checkpoint {
        origin: "foundry.bench.cap-ledger".to_string(),
        tree_size: 4,
        root_hash: old_root,
        extensions: vec![],
    };
    let body = old_cp.body_bytes();
    let key_hash = NoteSignature::derive_key_hash("apex", &pk);
    let sig = sk.sign(&body).to_bytes();
    let old_signed_exact = SignedCheckpoint {
        checkpoint: old_cp,
        signatures: vec![NoteSignature {
            signer_name: "apex".to_string(),
            key_hash,
            signature: sig,
        }],
    };
    let new_cp = Checkpoint {
        origin: "foundry.bench.cap-ledger".to_string(),
        tree_size: 8,
        root_hash: new_root,
        extensions: vec![],
    };
    let body = new_cp.body_bytes();
    let key_hash = NoteSignature::derive_key_hash("apex", &pk);
    let sig = sk.sign(&body).to_bytes();
    let new_signed = SignedCheckpoint {
        checkpoint: new_cp,
        signatures: vec![NoteSignature {
            signer_name: "apex".to_string(),
            key_hash,
            signature: sig,
        }],
    };
    c.bench_function(
        "SignedCheckpoint::verify_consistency_proof (composed, 4→8 tree)",
        |b| {
            b.iter(|| {
                black_box(
                    new_signed
                        .verify_consistency_proof(&proof, 4, 8, &old_signed_exact, "apex", &pk)
                        .unwrap(),
                )
            })
        },
    );
}

criterion_group!(
    benches,
    bench_capability_hash,
    bench_verify_signer_single,
    bench_verify_apex_handover,
    bench_cache_hit,
    bench_cache_miss,
    bench_consult_capability_allow,
    bench_verify_inclusion_proof_raw_8_leaves,
    bench_verify_inclusion_proof_raw_1024_leaves,
    bench_signed_checkpoint_verify_inclusion_proof,
    bench_apply_witness_record_with_proof,
    bench_verify_consistency_proof_raw,
    bench_signed_checkpoint_verify_consistency_proof,
);
criterion_main!(benches);
