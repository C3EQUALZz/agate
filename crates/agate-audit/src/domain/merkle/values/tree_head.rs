use agate_crypto::Digest;

use super::tree_size::TreeSize;
use crate::domain::common::values::{Timestamp, ValueObject};

/// Unsigned snapshot of the log: size + Merkle root at an instant. The
/// application layer signs it into a `SignedTreeHead`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeHead {
    pub size: TreeSize,
    pub root: Digest,
    pub at: Timestamp,
}

impl TreeHead {
    /// Deterministic byte encoding used as the signing/anchoring input:
    /// `size (BE u64) ‖ hash-algo code ‖ root bytes ‖ timestamp (BE i64)`.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + 1 + self.root.bytes.len() + 8);
        buf.extend_from_slice(&self.size.value().to_be_bytes());
        buf.push(self.root.algo.code());
        buf.extend_from_slice(&self.root.bytes);
        buf.extend_from_slice(&self.at.as_millis().to_be_bytes());
        buf
    }
}

impl ValueObject for TreeHead {}
