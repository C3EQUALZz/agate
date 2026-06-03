use agate_crypto::Digest;

use crate::domain::merkle::InclusionProof;

/// Read model for an inclusion-proof query: a self-contained result carrying
/// everything a verifier needs (the proof, the leaf hash, and the root it
/// proves against).
pub struct InclusionProofView {
    pub proof: InclusionProof,
    pub leaf_hash: Digest,
    pub root: Digest,
}
