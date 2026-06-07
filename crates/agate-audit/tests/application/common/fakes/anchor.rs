use std::sync::Mutex;

use async_trait::async_trait;

use agate_audit::application::common::ports::CheckpointAnchor;
use agate_audit::application::errors::AuditError;
use agate_audit::domain::merkle::{LogId, SignedTreeHead};

/// Records anchored checkpoints instead of publishing them externally.
pub struct RecordingAnchor {
    pub anchored: Mutex<Vec<SignedTreeHead>>,
}

impl RecordingAnchor {
    pub fn new() -> Self {
        Self {
            anchored: Mutex::new(Vec::new()),
        }
    }
}

impl Default for RecordingAnchor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CheckpointAnchor for RecordingAnchor {
    async fn anchor(&self, _log: LogId, sth: &SignedTreeHead) -> Result<(), AuditError> {
        self.anchored.lock().unwrap().push(sth.clone());
        Ok(())
    }
}
