use agate_crypto::Digest;
use serde::Serialize;

use crate::application::common::query_models::ConsistencyProofView;

/// A consistency proof between two tree sizes, with both roots hex-encoded.
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
