use rand::RngExt;
use serde::{Deserialize, Serialize};

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Generate a new 8-symbol Crockford base32 code from 5 random bytes (40 bits entropy).
/// Formatted as XXXX-XXXX for readability. Single-use; expires after 600 seconds.
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

/// Strip dashes/whitespace and fix common confusable characters (I→1, L→1, O→0).
/// Applied to operator input at approval time so codes can be read aloud without error.
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

/// Render `content` as a Unicode Dense1x2 QR code string (▀ ▄ █ space).
/// Works on any terminal without image protocol support. Returns empty string on encode failure.
pub fn qr_unicode(content: &str) -> String {
    use qrcode::{render::unicode, QrCode};
    match QrCode::new(content.as_bytes()) {
        Ok(code) => code.render::<unicode::Dense1x2>().quiet_zone(true).build(),
        Err(_) => String::new(),
    }
}

// --- PPN node-join request/response types ---

/// Body sent by a joining node. The node provides its machine identity and WireGuard pubkey.
/// The admin sees node_id + bottom + arch and approves or denies.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeJoinRequestBody {
    pub node_id: String,
    pub wireguard_pubkey: String,
    /// "seL4" (native AArch64 bottom) or "netbsd-compat" (x86-64 compat bottom)
    pub bottom: String,
    /// "aarch64" or "x86_64"
    pub arch: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeJoinResponseBody {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_code_format() {
        let code = new_code();
        // Must be 9 characters: XXXX-XXXX
        assert_eq!(code.len(), 9, "code must be 9 chars (XXXX-XXXX)");
        assert_eq!(&code[4..5], "-", "dash must be at position 4");
        // All characters (ex dash) must be valid Crockford symbols
        let valid: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        for c in code.chars().filter(|c| *c != '-') {
            assert!(valid.contains(c), "unexpected character: {c}");
        }
    }

    #[test]
    fn new_code_uniqueness() {
        let codes: std::collections::HashSet<String> = (0..100).map(|_| new_code()).collect();
        assert!(codes.len() > 95, "codes should be highly unique");
    }

    #[test]
    fn normalize_strips_dashes_and_spaces() {
        assert_eq!(normalize("K7Q2-9XMT"), "K7Q29XMT");
        assert_eq!(normalize("k7q2 9xmt"), "K7Q29XMT");
    }

    #[test]
    fn normalize_fixes_confusables() {
        assert_eq!(normalize("I"), "1");
        assert_eq!(normalize("L"), "1");
        assert_eq!(normalize("O"), "0");
        assert_eq!(normalize("ILO"), "110");
    }

    #[test]
    fn qr_unicode_returns_nonempty_for_short_ascii() {
        let qr = qr_unicode("NODE:K7Q29XMT");
        assert!(!qr.is_empty(), "QR output must not be empty");
        // Dense1x2 uses these block characters
        assert!(
            qr.contains('▀') || qr.contains('▄') || qr.contains('█') || qr.contains(' '),
            "QR output must contain block characters"
        );
    }
}
