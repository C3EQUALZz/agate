use async_trait::async_trait;
use tracing::{info, instrument};

use crate::application::common::ports::CheckpointAnchor;
use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, SignedTreeHead};
use crate::infrastructure::persistence::postgres::{SharedTransaction, storage_error};

/// Durable checkpoint anchor: persists each signed tree head to
/// `audit_checkpoint` on the request transaction — committed atomically with the
/// checkpoint's issue — and logs it for operators.
///
/// This is the durable record of the log's signed roots over time; an external
/// **independent witness** (the full defense against split-view) can later be
/// layered behind the same [`CheckpointAnchor`] port.
pub struct PostgresCheckpointAnchor {
    transaction: SharedTransaction,
}

impl PostgresCheckpointAnchor {
    #[must_use]
    pub fn new(transaction: SharedTransaction) -> Self {
        Self { transaction }
    }
}

#[async_trait]
impl CheckpointAnchor for PostgresCheckpointAnchor {
    #[instrument(name = "db.checkpoint.anchor", skip_all, fields(log = %log.0, size = sth.head.size.value()))]
    async fn anchor(&self, log: LogId, sth: &SignedTreeHead) -> Result<(), AuditError> {
        let mut slot = self.transaction.lock().await;
        let connection = slot.as_mut().ok_or_else(|| {
            AuditError::Storage("anchor without an active transaction".to_string())
        })?;

        sqlx::query(
            "INSERT INTO audit_checkpoint
                 (log_id, tree_size, root_hash, root_algo, issued_at, sig_algo, key_id, signature)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (log_id, tree_size) DO NOTHING",
        )
        .bind(log.0)
        .bind(sth.head.size.value() as i64)
        .bind(sth.head.root.bytes.as_slice())
        .bind(i16::from(sth.head.root.algo.code()))
        .bind(sth.head.at.as_millis())
        .bind(i16::from(sth.signature.algo.code()))
        .bind(&sth.signature.key_id.0)
        .bind(sth.signature.bytes.as_slice())
        .execute(&mut **connection)
        .await
        .map_err(storage_error)?;

        info!(
            size = sth.head.size.value(),
            root = %sth.head.root.to_hex(),
            key_id = %sth.signature.key_id.0,
            "anchored signed tree head (checkpoint)",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agate_crypto::{Digest, HashAlgo, KeyId, SignAlgo, Signature};
    use uuid::Uuid;

    use super::PostgresCheckpointAnchor;
    use crate::application::common::ports::CheckpointAnchor;
    use crate::application::errors::AuditError;
    use crate::domain::common::values::Timestamp;
    use crate::domain::merkle::{LogId, SignedTreeHead, TreeHead, TreeSize};
    use crate::infrastructure::persistence::postgres::TxSlot;

    fn sample_sth() -> SignedTreeHead {
        SignedTreeHead {
            head: TreeHead {
                size: TreeSize(1),
                root: Digest {
                    algo: HashAlgo::Sha256,
                    bytes: vec![0xab],
                },
                at: Timestamp::from_millis(0).expect("valid timestamp"),
            },
            signature: Signature {
                algo: SignAlgo::Ed25519,
                key_id: KeyId("k".to_owned()),
                bytes: vec![0; 64],
            },
        }
    }

    // The persistence/happy path is covered by the integration suite (a real
    // transaction). Here, with no active transaction in the slot, anchoring
    // surfaces a storage error rather than panicking.
    #[tokio::test]
    async fn anchoring_without_an_active_transaction_errors() {
        let anchor = PostgresCheckpointAnchor::new(Arc::new(TxSlot::new(None)));
        let result = anchor.anchor(LogId(Uuid::new_v4()), &sample_sth()).await;
        assert!(matches!(result, Err(AuditError::Storage(_))));
    }
}
