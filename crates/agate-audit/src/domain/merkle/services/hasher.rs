use std::sync::Arc;

use agate_crypto::{Digest, HashAlgo, Hasher};

use crate::domain::common::services::DomainService;

const LEAF_PREFIX: u8 = 0x00;
const NODE_PREFIX: u8 = 0x01;

/// Wraps a configured [`Hasher`] with RFC 6962 domain separation:
/// leaf = `H(0x00 ‖ record)`, node = `H(0x01 ‖ left ‖ right)`.
#[derive(Clone)]
pub struct MerkleHasher {
    inner: Arc<dyn Hasher>,
}

impl MerkleHasher {
    pub fn new(inner: Arc<dyn Hasher>) -> Self {
        Self { inner }
    }

    pub fn algo(&self) -> HashAlgo {
        self.inner.algo()
    }

    pub fn empty_root(&self) -> Digest {
        self.inner.hash(&[])
    }

    pub fn leaf(&self, record: &[u8]) -> Digest {
        let mut buf = Vec::with_capacity(1 + record.len());
        buf.push(LEAF_PREFIX);
        buf.extend_from_slice(record);
        self.inner.hash(&buf)
    }

    pub fn node(&self, left: &Digest, right: &Digest) -> Digest {
        debug_assert_eq!(left.algo, self.algo());
        debug_assert_eq!(right.algo, self.algo());
        let mut buf = Vec::with_capacity(1 + left.bytes.len() + right.bytes.len());
        buf.push(NODE_PREFIX);
        buf.extend_from_slice(&left.bytes);
        buf.extend_from_slice(&right.bytes);
        self.inner.hash(&buf)
    }
}

impl DomainService for MerkleHasher {}

#[cfg(test)]
mod tests {
    use super::*;
    use agate_crypto::CryptoRegistry;

    fn hasher() -> MerkleHasher {
        MerkleHasher::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
    }

    #[test]
    fn leaf_and_node_are_domain_separated() {
        let h = hasher();
        let leaf = h.leaf(&[]);
        let node = h.node(&h.empty_root(), &h.empty_root());
        assert_ne!(leaf, node);
    }
}
