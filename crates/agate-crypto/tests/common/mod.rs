#![allow(dead_code)]

use agate_crypto::SecretKey;

/// Deterministic key material of the requested length (test fixtures must never
/// touch the RNG — AGENTS testing rules).
pub fn secret_key(len: usize) -> SecretKey {
    SecretKey::new((0..len).map(|i| (i % 251) as u8).collect())
}
