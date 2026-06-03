use std::sync::Arc;

use agate_audit::domain::merkle::MerkleHasher;
use agate_crypto::{CryptoRegistry, HashAlgo, Hasher};

pub fn sha256() -> Arc<dyn Hasher> {
    CryptoRegistry::hasher(HashAlgo::Sha256).unwrap()
}

pub fn merkle_hasher() -> MerkleHasher {
    MerkleHasher::new(sha256())
}
