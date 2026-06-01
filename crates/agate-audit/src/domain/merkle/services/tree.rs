use agate_crypto::Digest;

use super::hasher::MerkleHasher;
use crate::domain::common::services::DomainService;

/// Pure Merkle math over a slice of leaf hashes (RFC 6962).
pub struct MerkleTree;

impl MerkleTree {
    /// Merkle Tree Hash (RFC 6962 §2.1) over already leaf-hashed `leaves`:
    /// `MTH({})=H("")`, `MTH({d0})=d0`, `MTH(D)=node(MTH(left), MTH(right))`
    /// splitting at the largest power of two below `n`.
    pub fn root(hasher: &MerkleHasher, leaves: &[Digest]) -> Digest {
        match leaves.len() {
            0 => hasher.empty_root(),
            1 => leaves[0].clone(),
            n => {
                let k = split_point(n);
                let left = Self::root(hasher, &leaves[..k]);
                let right = Self::root(hasher, &leaves[k..]);
                hasher.node(&left, &right)
            }
        }
    }
}

impl DomainService for MerkleTree {}

/// Largest power of two strictly less than `n` (`n >= 2`).
pub(crate) fn split_point(n: usize) -> usize {
    debug_assert!(n >= 2);
    let mut k = 1usize;
    while k < n {
        k <<= 1;
    }
    k >> 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::merkle::services::hasher::MerkleHasher;
    use agate_crypto::{CryptoRegistry, Digest, HashAlgo};

    fn hasher() -> MerkleHasher {
        MerkleHasher::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
    }

    #[test]
    fn split_point_follows_rfc6962() {
        assert_eq!(split_point(2), 1);
        assert_eq!(split_point(3), 2);
        assert_eq!(split_point(4), 2);
        assert_eq!(split_point(5), 4);
        assert_eq!(split_point(7), 4);
        assert_eq!(split_point(8), 4);
    }

    #[test]
    fn empty_root_is_hash_of_empty_string() {
        let h = hasher();
        assert_eq!(MerkleTree::root(&h, &[]), h.empty_root());
    }

    #[test]
    fn single_leaf_root_is_the_leaf_hash() {
        let h = hasher();
        let leaf = h.leaf(b"a");
        assert_eq!(MerkleTree::root(&h, std::slice::from_ref(&leaf)), leaf);
    }

    #[test]
    fn two_leaf_root_is_node_of_leaves() {
        let h = hasher();
        let l0 = h.leaf(b"a");
        let l1 = h.leaf(b"b");
        let expected = h.node(&l0, &l1);
        assert_eq!(MerkleTree::root(&h, &[l0, l1]), expected);
    }

    #[test]
    fn three_leaf_root_splits_as_two_and_one() {
        let h = hasher();
        let inputs: [&[u8]; 3] = [b"a", b"b", b"c"];
        let leaves: Vec<Digest> = inputs.iter().map(|r| h.leaf(r)).collect();
        let expected = h.node(&h.node(&leaves[0], &leaves[1]), &leaves[2]);
        assert_eq!(MerkleTree::root(&h, &leaves), expected);
    }
}
