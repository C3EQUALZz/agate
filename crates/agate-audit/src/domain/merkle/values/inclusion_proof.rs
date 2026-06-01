use agate_crypto::Digest;

use super::leaf_index::LeafIndex;
use super::tree_size::TreeSize;
use crate::domain::common::values::ValueObject;

/// RFC 6962 audit path: proves a leaf is included in a tree of a given size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InclusionProof {
    pub leaf_index: LeafIndex,
    pub tree_size: TreeSize,
    pub path: Vec<Digest>,
}

impl ValueObject for InclusionProof {}
