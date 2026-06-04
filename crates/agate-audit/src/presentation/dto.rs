use agate_crypto::Digest;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::common::query_models::{ConsistencyProofView, InclusionProofView};

#[derive(Serialize)]
pub struct CreateLogResponse {
    pub id: Uuid,
}

#[derive(Deserialize)]
pub struct AppendRecordRequest {
    /// The record to append; its UTF-8 bytes are hashed into a leaf.
    pub record: String,
}

#[derive(Serialize)]
pub struct AppendRecordResponse {
    pub index: u64,
}

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

#[derive(Serialize)]
pub struct ConsistencyProofResponse {
    pub first_size: u64,
    pub second_size: u64,
    pub old_root: String,
    pub new_root: String,
    pub path: Vec<String>,
}

impl From<ConsistencyProofView> for ConsistencyProofResponse {
    fn from(view: ConsistencyProofView) -> Self {
        Self {
            first_size: view.proof.first_size.0,
            second_size: view.proof.second_size.0,
            old_root: view.old_root.to_hex(),
            new_root: view.new_root.to_hex(),
            path: view.proof.path.iter().map(Digest::to_hex).collect(),
        }
    }
}
