//! Random secrets, and how they are kept: only their hash is ever stored.

use base64::Engine as _;
use sha2::{Digest, Sha256};

/// 256 bits of randomness, URL-safe: a session, a link, a code, a token.
///
/// # Panics
///
/// If the system has no source of randomness.
pub fn token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the system has randomness");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// 128 bits as hex: an id nothing derives and nobody guesses.
///
/// # Panics
///
/// If the system has no source of randomness.
pub fn id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the system has randomness");
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// What the database keeps instead of the secret.
pub fn hash(secret: &str) -> String {
    hex(&Sha256::digest(secret.as_bytes()))
}

/// The PKCE check: the verifier the client kept hashes to the challenge it
/// sent first (S256, the only method accepted).
pub fn pkce_matches(verifier: &str, challenge: &str) -> bool {
    // RFC 7636: 43 to 128 characters of the unreserved set.
    let shaped = (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b));
    if !shaped {
        return false;
    }
    let digest = Sha256::digest(verifier.as_bytes());
    let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    same(&computed, challenge)
}

/// Compares two secrets in time that does not depend on where they differ.
pub fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |seen, (x, y)| seen | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_long_and_different() {
        let (a, b) = (token(), token());
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert_eq!(id().len(), 32);
        assert_eq!(hash("x").len(), 64);
        assert_ne!(hash(&a), a);
    }

    /// The RFC 7636 appendix B example.
    #[test]
    fn pkce_follows_the_rfc() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(pkce_matches(verifier, challenge));
        assert!(!pkce_matches(
            verifier,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cX"
        ));
        assert!(
            !pkce_matches("short", challenge),
            "a verifier is 43 to 128 characters"
        );
    }
}
