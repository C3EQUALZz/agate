use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct IssueCheckpointRequest {
    /// Id of the signing key to use; must match the server's configured key
    /// (`AUDIT_CHECKPOINT_KEY_ID`).
    pub key_id: String,
}

/// The signed tree head (checkpoint): the snapshot, the signature, and the key
/// and algorithm needed to verify it.
#[derive(Serialize)]
pub struct IssueCheckpointResponse {
    pub size: u64,
    /// Merkle root, hex-encoded.
    pub root: String,
    pub at_ms: i64,
    pub key_id: String,
    pub algorithm: String,
    /// Signature over the canonical tree-head bytes, hex-encoded.
    pub signature: String,
}
