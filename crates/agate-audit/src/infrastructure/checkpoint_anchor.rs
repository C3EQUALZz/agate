//! A logging checkpoint anchor: emits each signed tree head at INFO so it can be
//! shipped to an external collector.
//!
//! This is a placeholder for a real **independent witness** (the defense against
//! split-view / equivocation by the log operator). The [`CheckpointAnchor`] port
//! is the seam where a witness HTTP client or a separate datastore plugs in
//! without touching the use case.

use async_trait::async_trait;
use tracing::info;

use crate::application::common::ports::CheckpointAnchor;
use crate::application::errors::AuditError;
use crate::domain::merkle::SignedTreeHead;

/// Records a signed tree head by logging it.
#[derive(Debug, Default, Clone, Copy)]
pub struct LoggingCheckpointAnchor;

#[async_trait]
impl CheckpointAnchor for LoggingCheckpointAnchor {
    async fn anchor(&self, sth: &SignedTreeHead) -> Result<(), AuditError> {
        info!(
            size = sth.head.size.value(),
            root = %sth.head.root.to_hex(),
            at_ms = sth.head.at.as_millis(),
            key_id = %sth.signature.key_id.0,
            "anchored signed tree head (checkpoint)",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use agate_crypto::{Digest, HashAlgo, KeyId, SignAlgo, Signature};

    use super::*;
    use crate::domain::common::values::Timestamp;
    use crate::domain::merkle::{TreeHead, TreeSize};

    #[tokio::test]
    async fn anchor_accepts_a_signed_tree_head() {
        let sth = SignedTreeHead {
            head: TreeHead {
                size: TreeSize(3),
                root: Digest {
                    algo: HashAlgo::Sha256,
                    bytes: vec![1, 2, 3],
                },
                at: Timestamp::from_millis(0).expect("valid timestamp"),
            },
            signature: Signature {
                algo: SignAlgo::Ed25519,
                key_id: KeyId("k".to_owned()),
                bytes: vec![9; 64],
            },
        };
        assert!(LoggingCheckpointAnchor.anchor(&sth).await.is_ok());
    }
}
