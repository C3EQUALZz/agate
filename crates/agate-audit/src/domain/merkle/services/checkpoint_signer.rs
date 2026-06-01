use agate_crypto::Signer;

use super::super::values::{SignedTreeHead, TreeHead};
use crate::domain::common::services::DomainService;

/// Signs a `TreeHead` into a `SignedTreeHead` using an injected signing strategy.
pub struct CheckpointSigner;

impl CheckpointSigner {
    pub fn sign(signer: &dyn Signer, head: &TreeHead) -> SignedTreeHead {
        SignedTreeHead {
            head: head.clone(),
            signature: signer.sign(&head.to_canonical_bytes()),
        }
    }
}

impl DomainService for CheckpointSigner {}
