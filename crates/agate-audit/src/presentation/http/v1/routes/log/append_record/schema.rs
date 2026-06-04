use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct AppendRecordRequest {
    /// The record to append; its UTF-8 bytes are hashed into a leaf.
    pub record: String,
}

#[derive(Serialize)]
pub struct AppendRecordResponse {
    pub index: u64,
}
