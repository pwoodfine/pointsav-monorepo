use russh::keys::{HashAlg, PublicKey};

/// Compute the OpenSSH SHA-256 fingerprint of a public key.
/// Returns a string in the form "SHA256:<base64>" matching `ssh-keygen -lf`.
pub fn compute_fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}
