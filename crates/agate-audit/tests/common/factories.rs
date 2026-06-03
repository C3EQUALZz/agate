use std::sync::Arc;

use agate_audit::domain::common::values::Timestamp;
use agate_audit::domain::merkle::{MerkleHasher, TransparencyLogFactory};
use agate_crypto::{CryptoRegistry, HashAlgo, Hasher};

pub fn sha256() -> Arc<dyn Hasher> {
    CryptoRegistry::hasher(HashAlgo::Sha256).unwrap()
}

pub fn merkle_hasher() -> MerkleHasher {
    MerkleHasher::new(sha256())
}

pub fn log_factory() -> TransparencyLogFactory {
    TransparencyLogFactory::new(sha256())
}

pub fn epoch() -> Timestamp {
    Timestamp::from_millis(0).unwrap()
}
