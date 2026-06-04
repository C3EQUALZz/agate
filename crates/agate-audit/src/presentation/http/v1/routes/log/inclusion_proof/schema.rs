use agate_crypto::Digest;
use serde::Serialize;

use crate::application::common::query_models::InclusionProofView;

/// A self-contained inclusion proof: hex-encoded hashes plus the position it
/// proves, so a client can verify without server state.
#[derive(Serialize)]
pub struct InclusionProofResponse {
    pub leaf_index: u64,
    pub tree_size: u64,
    pub leaf_hash: String,
    pub root: String,
    pub path: Vec<String>,
}

impl From<InclusionProofView> for InclusionProofResponse {
    fn from(view: InclusionProofView) -> Self {
        Self {
            leaf_index: view.proof.leaf_index.0,
            tree_size: view.proof.tree_size.0,
            leaf_hash: view.leaf_hash.to_hex(),
            root: view.root.to_hex(),
            path: view.proof.path.iter().map(Digest::to_hex).collect(),
        }
    }
}
