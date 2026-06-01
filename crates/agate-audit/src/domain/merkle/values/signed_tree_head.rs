use agate_crypto::Signature;

use super::tree_head::TreeHead;
use crate::domain::common::values::ValueObject;

/// A `TreeHead` together with a signature over its canonical bytes (STH).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedTreeHead {
    pub head: TreeHead,
    pub signature: Signature,
}

impl ValueObject for SignedTreeHead {}
