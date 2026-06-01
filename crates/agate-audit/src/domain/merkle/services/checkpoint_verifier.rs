use agate_crypto::Verifier;

use super::super::values::SignedTreeHead;
use crate::domain::common::services::DomainService;

/// Verifies a `SignedTreeHead` against an injected verifying strategy.
pub struct CheckpointVerifier;

impl CheckpointVerifier {
    pub fn verify(verifier: &dyn Verifier, sth: &SignedTreeHead) -> bool {
        verifier.verify(&sth.head.to_canonical_bytes(), &sth.signature)
    }
}

impl DomainService for CheckpointVerifier {}
