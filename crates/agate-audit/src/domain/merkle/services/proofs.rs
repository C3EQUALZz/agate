use agate_crypto::Digest;

use super::hasher::MerkleHasher;
use super::tree::{split_point, MerkleTree};
use crate::domain::common::services::DomainService;
use crate::domain::merkle::values::{ConsistencyProof, InclusionProof, LeafIndex, TreeSize};

/// RFC 6962 inclusion and consistency proofs: generation (needs the leaves) and
/// verification (needs only the roots + proof).
pub struct MerkleProofs;

impl MerkleProofs {
    pub fn inclusion(
        hasher: &MerkleHasher,
        leaves: &[Digest],
        index: usize,
    ) -> Option<InclusionProof> {
        let n = leaves.len();
        if index >= n {
            return None;
        }
        Some(InclusionProof {
            leaf_index: LeafIndex(index as u64),
            tree_size: TreeSize(n as u64),
            path: audit_path(hasher, index, leaves),
        })
    }

    pub fn verify_inclusion(
        hasher: &MerkleHasher,
        proof: &InclusionProof,
        leaf: &Digest,
        root: &Digest,
    ) -> bool {
        let m = proof.leaf_index.value() as usize;
        let n = proof.tree_size.value() as usize;
        if m >= n {
            return false;
        }
        match rebuild_inclusion(hasher, m, n, leaf, &proof.path) {
            Some(computed) => &computed == root,
            None => false,
        }
    }

    pub fn consistency(
        hasher: &MerkleHasher,
        leaves: &[Digest],
        first_size: usize,
    ) -> Option<ConsistencyProof> {
        let n = leaves.len();
        if first_size == 0 || first_size > n {
            return None;
        }
        let path = if first_size == n {
            Vec::new()
        } else {
            subproof(hasher, first_size, leaves, true)
        };
        Some(ConsistencyProof {
            first_size: TreeSize(first_size as u64),
            second_size: TreeSize(n as u64),
            path,
        })
    }

    pub fn verify_consistency(
        hasher: &MerkleHasher,
        proof: &ConsistencyProof,
        old_root: &Digest,
        new_root: &Digest,
    ) -> bool {
        let m = proof.first_size.value() as usize;
        let n = proof.second_size.value() as usize;
        if m > n {
            return false;
        }
        if m == n {
            return proof.path.is_empty() && old_root == new_root;
        }
        if m == 0 {
            return proof.path.is_empty();
        }
        match rebuild_consistency(hasher, m, n, true, old_root, &proof.path) {
            Some((old, new)) => &old == old_root && &new == new_root,
            None => false,
        }
    }
}

impl DomainService for MerkleProofs {}

/// PATH(m, D[n]) — audit path, ordered deepest sibling first.
fn audit_path(hasher: &MerkleHasher, m: usize, leaves: &[Digest]) -> Vec<Digest> {
    let n = leaves.len();
    if n <= 1 {
        return Vec::new();
    }
    let k = split_point(n);
    if m < k {
        let mut path = audit_path(hasher, m, &leaves[..k]);
        path.push(MerkleTree::root(hasher, &leaves[k..]));
        path
    } else {
        let mut path = audit_path(hasher, m - k, &leaves[k..]);
        path.push(MerkleTree::root(hasher, &leaves[..k]));
        path
    }
}

/// Recompute the root from a leaf + audit path (mirrors `audit_path`).
fn rebuild_inclusion(
    hasher: &MerkleHasher,
    m: usize,
    n: usize,
    leaf: &Digest,
    path: &[Digest],
) -> Option<Digest> {
    if n == 0 {
        return None;
    }
    if n == 1 {
        return if path.is_empty() {
            Some(leaf.clone())
        } else {
            None
        };
    }
    let (last, rest) = path.split_last()?;
    let k = split_point(n);
    if m < k {
        let left = rebuild_inclusion(hasher, m, k, leaf, rest)?;
        Some(hasher.node(&left, last))
    } else {
        let right = rebuild_inclusion(hasher, m - k, n - k, leaf, rest)?;
        Some(hasher.node(last, &right))
    }
}

/// SUBPROOF(m, D[n], b) — consistency proof, ordered deepest first.
fn subproof(hasher: &MerkleHasher, m: usize, leaves: &[Digest], b: bool) -> Vec<Digest> {
    let n = leaves.len();
    if m == n {
        return if b {
            Vec::new()
        } else {
            vec![MerkleTree::root(hasher, leaves)]
        };
    }
    let k = split_point(n);
    if m <= k {
        let mut path = subproof(hasher, m, &leaves[..k], b);
        path.push(MerkleTree::root(hasher, &leaves[k..]));
        path
    } else {
        let mut path = subproof(hasher, m - k, &leaves[k..], false);
        path.push(MerkleTree::root(hasher, &leaves[..k]));
        path
    }
}

/// Reconstruct (old_root, new_root) from a consistency proof (mirrors
/// `subproof`). `old_seed` is the trusted old root, used where the subtree is
/// fully covered by the old tree (`b == true`).
fn rebuild_consistency(
    hasher: &MerkleHasher,
    m: usize,
    n: usize,
    b: bool,
    old_seed: &Digest,
    path: &[Digest],
) -> Option<(Digest, Digest)> {
    if m == n {
        if b {
            return if path.is_empty() {
                Some((old_seed.clone(), old_seed.clone()))
            } else {
                None
            };
        }
        let (last, rest) = path.split_last()?;
        return if rest.is_empty() {
            Some((last.clone(), last.clone()))
        } else {
            None
        };
    }
    let k = split_point(n);
    let (last, rest) = path.split_last()?;
    if m <= k {
        let (old_left, new_left) = rebuild_consistency(hasher, m, k, b, old_seed, rest)?;
        Some((old_left, hasher.node(&new_left, last)))
    } else {
        let (old_right, new_right) =
            rebuild_consistency(hasher, m - k, n - k, false, old_seed, rest)?;
        Some((hasher.node(last, &old_right), hasher.node(last, &new_right)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agate_crypto::{CryptoRegistry, HashAlgo};
    use proptest::prelude::*;

    fn mh() -> MerkleHasher {
        MerkleHasher::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
    }

    fn leaf_hashes(h: &MerkleHasher, records: &[Vec<u8>]) -> Vec<Digest> {
        records.iter().map(|r| h.leaf(r)).collect()
    }

    fn records_strategy() -> impl Strategy<Value = Vec<Vec<u8>>> {
        prop::collection::vec(prop::collection::vec(any::<u8>(), 0..8), 1..40)
    }

    proptest! {
        #[test]
        fn inclusion_round_trip(records in records_strategy()) {
            let h = mh();
            let leaves = leaf_hashes(&h, &records);
            let root = MerkleTree::root(&h, &leaves);
            for m in 0..leaves.len() {
                let proof = MerkleProofs::inclusion(&h, &leaves, m).unwrap();
                prop_assert!(MerkleProofs::verify_inclusion(&h, &proof, &leaves[m], &root));
            }
        }

        #[test]
        fn inclusion_rejects_wrong_root(records in records_strategy()) {
            let h = mh();
            let leaves = leaf_hashes(&h, &records);
            let root = MerkleTree::root(&h, &leaves);
            let bad_root = h.leaf(b"not-the-root");
            prop_assume!(bad_root != root);
            for m in 0..leaves.len() {
                let proof = MerkleProofs::inclusion(&h, &leaves, m).unwrap();
                prop_assert!(!MerkleProofs::verify_inclusion(&h, &proof, &leaves[m], &bad_root));
            }
        }

        #[test]
        fn consistency_round_trip(records in records_strategy()) {
            let h = mh();
            let leaves = leaf_hashes(&h, &records);
            let n = leaves.len();
            let new_root = MerkleTree::root(&h, &leaves);
            for m in 1..=n {
                let old_root = MerkleTree::root(&h, &leaves[..m]);
                let proof = MerkleProofs::consistency(&h, &leaves, m).unwrap();
                prop_assert!(MerkleProofs::verify_consistency(&h, &proof, &old_root, &new_root));
            }
        }

        #[test]
        fn consistency_rejects_tampered_new_root(
            records in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..8), 2..40)
        ) {
            let h = mh();
            let leaves = leaf_hashes(&h, &records);
            let n = leaves.len();
            let new_root = MerkleTree::root(&h, &leaves);
            let wrong_new = MerkleTree::root(&h, &leaves[..n - 1]);
            prop_assume!(wrong_new != new_root);
            for m in 1..n {
                let old_root = MerkleTree::root(&h, &leaves[..m]);
                let proof = MerkleProofs::consistency(&h, &leaves, m).unwrap();
                prop_assert!(!MerkleProofs::verify_consistency(&h, &proof, &old_root, &wrong_new));
            }
        }
    }
}
