use agate_crypto::Digest;

use super::tree_size::TreeSize;
use crate::domain::common::values::ValueObject;

/// RFC 6962 consistency proof: the tree of `first_size` is an append-only
/// prefix of the tree of `second_size`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyProof {
    pub first_size: TreeSize,
    pub second_size: TreeSize,
    pub path: Vec<Digest>,
}

impl ValueObject for ConsistencyProof {}
