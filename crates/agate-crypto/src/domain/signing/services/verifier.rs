use super::super::values::{SignAlgo, Signature};

/// Verification strategy (public-key side).
pub trait Verifier: Send + Sync {
    fn algo(&self) -> SignAlgo;
    fn verify(&self, data: &[u8], sig: &Signature) -> bool;
}
