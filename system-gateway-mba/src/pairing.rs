use rand::RngExt;
use serde::{Deserialize, Serialize};

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn new_code() -> String {
    let mut bytes = [0u8; 5];
    rand::rng().fill(&mut bytes);
    let mut acc: u64 = 0;
    for b in bytes {
        acc = (acc << 8) | b as u64;
    }
    let mut out = String::with_capacity(9);
    for i in (0..8u64).rev() {
        let idx = ((acc >> (i * 5)) & 0x1f) as usize;
        out.push(CROCKFORD[idx] as char);
        if i == 4 {
            out.push('-');
        }
    }
    out
}

/// Normalise user input: strip dashes/whitespace, fix common confusable chars.
pub fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        })
        .collect()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairRequestBody {
    pub username: String,
    pub tenant: String,
    pub public_key: String,
    pub fingerprint: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairResponseBody {
    pub request_id: String,
    pub code: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponseBody {
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveBody {
    pub code: String,
}
