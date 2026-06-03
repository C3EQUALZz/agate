use agate_crypto::Digest;

use crate::domain::merkle::ConsistencyProof;

/// Read model for a consistency-proof query: the proof plus both roots it
/// relates, so a verifier needs nothing else.
pub struct ConsistencyProofView {
    pub proof: ConsistencyProof,
    pub old_root: Digest,
    pub new_root: Digest,
}
