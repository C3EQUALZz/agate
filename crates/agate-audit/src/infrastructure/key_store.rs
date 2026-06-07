//! In-memory Ed25519 key store holding the configured checkpoint signing key.
//!
//! The key is loaded once from the environment: a 32-byte seed (`AUDIT_CHECKPOINT_SEED`,
//! 64 hex chars) under id `AUDIT_CHECKPOINT_KEY_ID` (default `checkpoint-ed25519`).
//! A missing or malformed seed leaves the store **unconfigured** — every lookup
//! then fails with [`AuditError::KeyNotFound`], so a checkpoint is never silently
//! signed by an ephemeral key that no verifier could trust across restarts.

use std::sync::Arc;

use agate_crypto::ed25519::{Ed25519Signer, Ed25519Verifier};
use agate_crypto::{KeyId, Signer, Verifier};
use async_trait::async_trait;
use tracing::warn;

use crate::application::common::ports::KeyStore;
use crate::application::errors::AuditError;

const SEED_ENV: &str = "AUDIT_CHECKPOINT_SEED";
const KEY_ID_ENV: &str = "AUDIT_CHECKPOINT_KEY_ID";
const DEFAULT_KEY_ID: &str = "checkpoint-ed25519";
const SEED_HEX_LEN: usize = 64;

struct ConfiguredKey {
    id: KeyId,
    signer: Arc<Ed25519Signer>,
    verifier: Arc<Ed25519Verifier>,
}

/// Holds at most one Ed25519 checkpoint key.
pub struct Ed25519KeyStore {
    key: Option<ConfiguredKey>,
}

impl Ed25519KeyStore {
    /// Load the checkpoint key from the environment. A missing/invalid seed is
    /// logged and leaves the store unconfigured (checkpoint requests then fail
    /// cleanly).
    #[must_use]
    pub fn from_env() -> Self {
        let key_id = KeyId(std::env::var(KEY_ID_ENV).unwrap_or_else(|_| DEFAULT_KEY_ID.to_owned()));
        let Some(seed) = std::env::var(SEED_ENV)
            .ok()
            .and_then(|hex| decode_seed(&hex))
        else {
            warn!("{SEED_ENV} unset or not 32 hex-encoded bytes; checkpoint signing is disabled");
            return Self { key: None };
        };
        Self::from_seed(key_id, &seed)
    }

    /// Build a store holding the single Ed25519 key derived from `seed`.
    #[must_use]
    pub fn from_seed(key_id: KeyId, seed: &[u8; 32]) -> Self {
        let signer = Ed25519Signer::from_seed(seed, key_id.clone());
        match Ed25519Verifier::from_public_bytes(&signer.verifying_key_bytes()) {
            Ok(verifier) => Self {
                key: Some(ConfiguredKey {
                    id: key_id,
                    signer: Arc::new(signer),
                    verifier: Arc::new(verifier),
                }),
            },
            Err(error) => {
                warn!(%error, "failed to derive the checkpoint verifying key; signing disabled");
                Self { key: None }
            }
        }
    }

    fn lookup(&self, key: &KeyId) -> Option<&ConfiguredKey> {
        self.key.as_ref().filter(|configured| &configured.id == key)
    }
}

#[async_trait]
impl KeyStore for Ed25519KeyStore {
    async fn signer(&self, key: &KeyId) -> Result<Arc<dyn Signer>, AuditError> {
        self.lookup(key)
            .map(|configured| configured.signer.clone() as Arc<dyn Signer>)
            .ok_or_else(|| AuditError::KeyNotFound(key.clone()))
    }

    async fn verifier(&self, key: &KeyId) -> Result<Arc<dyn Verifier>, AuditError> {
        self.lookup(key)
            .map(|configured| configured.verifier.clone() as Arc<dyn Verifier>)
            .ok_or_else(|| AuditError::KeyNotFound(key.clone()))
    }
}

/// Decode exactly 32 hex-encoded bytes (64 hex chars) into a seed; `None` on any
/// wrong length or non-hex character.
fn decode_seed(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != SEED_HEX_LEN {
        return None;
    }
    let mut seed = [0_u8; 32];
    for (index, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let high = char::from(chunk[0]).to_digit(16)?;
        let low = char::from(chunk[1]).to_digit(16)?;
        seed[index] = u8::try_from((high << 4) | low).ok()?;
    }
    Some(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_seed_accepts_64_hex_chars() {
        let hex = "00".repeat(32);
        assert_eq!(decode_seed(&hex), Some([0_u8; 32]));

        let mut expected = [0_u8; 32];
        expected[0] = 0xAB;
        expected[31] = 0x0F;
        let hex = format!("ab{}0f", "00".repeat(30));
        assert_eq!(decode_seed(&hex), Some(expected));
    }

    #[test]
    fn decode_seed_rejects_wrong_length_or_non_hex() {
        assert_eq!(decode_seed("abcd"), None);
        assert_eq!(decode_seed(&"zz".repeat(32)), None);
        assert_eq!(decode_seed(&"00".repeat(33)), None);
    }

    #[tokio::test]
    async fn unconfigured_store_reports_key_not_found() {
        let store = Ed25519KeyStore { key: None };
        let result = store.signer(&KeyId("any".to_owned())).await;
        assert!(matches!(result, Err(AuditError::KeyNotFound(_))));
    }

    #[tokio::test]
    async fn configured_store_yields_the_matching_key_only() {
        let store = Ed25519KeyStore::from_seed(KeyId("k".to_owned()), &[7_u8; 32]);
        assert!(store.signer(&KeyId("k".to_owned())).await.is_ok());
        assert!(store.verifier(&KeyId("k".to_owned())).await.is_ok());
        assert!(matches!(
            store.signer(&KeyId("other".to_owned())).await,
            Err(AuditError::KeyNotFound(_)),
        ));
    }
}
