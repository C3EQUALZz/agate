use super::super::values::{KeyId, SignAlgo, Signature};

/// Signing strategy. Pure given the key material; *loading* the key is I/O and
/// belongs to a `KeyStore` port in the consuming bounded context.
pub trait Signer: Send + Sync {
    fn algo(&self) -> SignAlgo;
    fn key_id(&self) -> KeyId;
    fn sign(&self, data: &[u8]) -> Signature;
}
