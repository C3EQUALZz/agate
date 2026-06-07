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
