use async_trait::async_trait;
use tracing::instrument;

use agate_crypto::{Digest, HashAlgo};

use crate::application::common::ports::LogCommandGateway;
use crate::application::errors::AuditError;
use crate::domain::common::entities::Entity;
use crate::domain::common::values::{Timestamp, Timestamps};
use crate::domain::merkle::{LeafIndex, LogId, TransparencyLog, TransparencyLogFactory};
use crate::infrastructure::persistence::postgres::{SharedTransaction, storage_error};

/// Write-side gateway backed by PostgreSQL (append-only). Runs every statement
/// on the shared request-scoped transaction and never commits — the
/// `PgTransactionManager` owns the commit boundary.
pub struct PostgresLogCommandGateway {
    transaction: SharedTransaction,
    factory: TransparencyLogFactory,
}

impl PostgresLogCommandGateway {
    pub fn new(transaction: SharedTransaction, factory: TransparencyLogFactory) -> Self {
        Self {
            transaction,
            factory,
        }
    }
}

#[async_trait]
impl LogCommandGateway for PostgresLogCommandGateway {
    #[instrument(name = "db.log.load", skip_all, fields(log = %id.0))]
    async fn load(&self, id: LogId) -> Result<Option<TransparencyLog>, AuditError> {
        let mut slot = self.transaction.lock().await;
        let connection = slot
            .as_mut()
            .ok_or_else(|| AuditError::Storage("load without an active transaction".to_string()))?;

        let Some((created, updated, algo_code)) = sqlx::query_as::<_, (i64, i64, i16)>(
            "SELECT created_at, updated_at, hash_algo FROM audit_log WHERE id = $1",
        )
        .bind(id.0)
        .fetch_optional(&mut **connection)
        .await
        .map_err(storage_error)?
        else {
            return Ok(None);
        };

        let algo = HashAlgo::from_code(algo_code as u8)
            .ok_or_else(|| AuditError::Storage(format!("unknown hash algo code {algo_code}")))?;

        let leaf_rows = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT leaf_hash FROM audit_leaf WHERE log_id = $1 ORDER BY leaf_index",
        )
        .bind(id.0)
        .fetch_all(&mut **connection)
        .await
        .map_err(storage_error)?;

        let leaves = leaf_rows
            .into_iter()
            .map(|(bytes,)| Digest { algo, bytes })
            .collect();

        let timestamps = Timestamps::reconstitute(
            Timestamp::from_millis(created)?,
            Timestamp::from_millis(updated)?,
        )?;

        Ok(Some(self.factory.reconstitute(id, timestamps, leaves)))
    }

    #[instrument(name = "db.log.save", skip_all, fields(log = %log.id().0))]
    async fn save(&self, log: &TransparencyLog) -> Result<(), AuditError> {
        let id = log.id().0;

        let mut slot = self.transaction.lock().await;
        let connection = slot
            .as_mut()
            .ok_or_else(|| AuditError::Storage("save without an active transaction".to_string()))?;

        sqlx::query(
            "INSERT INTO audit_log (id, created_at, updated_at, hash_algo)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE SET updated_at = EXCLUDED.updated_at",
        )
        .bind(id)
        .bind(log.created_at().as_millis())
        .bind(log.updated_at().as_millis())
        .bind(i16::from(log.algo().code()))
        .execute(&mut **connection)
        .await
        .map_err(storage_error)?;

        for (index, leaf) in log.leaf_hashes().iter().enumerate() {
            sqlx::query(
                "INSERT INTO audit_leaf (log_id, leaf_index, leaf_hash)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (log_id, leaf_index) DO NOTHING",
            )
            .bind(id)
            .bind(index as i64)
            .bind(leaf.bytes.as_slice())
            .execute(&mut **connection)
            .await
            .map_err(storage_error)?;
        }

        Ok(())
    }

    #[instrument(name = "db.log.append", skip_all, fields(log = %id.0))]
    async fn append_record(
        &self,
        id: LogId,
        record: &[u8],
    ) -> Result<Option<LeafIndex>, AuditError> {
        let mut slot = self.transaction.lock().await;
        let connection = slot.as_mut().ok_or_else(|| {
            AuditError::Storage("append without an active transaction".to_string())
        })?;

        // The log must exist; absent → `None` (the handler maps it to LogNotFound).
        let exists = sqlx::query_scalar::<_, i32>("SELECT 1 FROM audit_log WHERE id = $1")
            .bind(id.0)
            .fetch_optional(&mut **connection)
            .await
            .map_err(storage_error)?;
        if exists.is_none() {
            return Ok(None);
        }

        // Next index = current size. Append-only + single-writer (the audit
        // outbox), so this read-then-insert is race-free.
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(leaf_index) + 1, 0) FROM audit_leaf WHERE log_id = $1",
        )
        .bind(id.0)
        .fetch_one(&mut **connection)
        .await
        .map_err(storage_error)?;

        // Hash the leaf exactly as the aggregate would, then insert just it —
        // O(1), no load/rewrite of the existing leaves. A plain INSERT on
        // purpose: the (log_id, leaf_index) unique constraint must REJECT a
        // duplicate index, not swallow it. Under single-writer append-only this
        // never conflicts; if it ever did (a second writer, a replay) the
        // constraint surfaces it as a loud storage error rather than reporting a
        // lost leaf as success. (`save` uses ON CONFLICT DO NOTHING because it
        // re-inserts the whole leaf set idempotently; an append must not.)
        let leaf = self.factory.merkle_hasher().leaf(record);
        sqlx::query("INSERT INTO audit_leaf (log_id, leaf_index, leaf_hash) VALUES ($1, $2, $3)")
            .bind(id.0)
            .bind(next)
            .bind(leaf.bytes.as_slice())
            .execute(&mut **connection)
            .await
            .map_err(storage_error)?;

        Ok(Some(LeafIndex(next as u64)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agate_crypto::{CryptoRegistry, HashAlgo};
    use uuid::Uuid;

    use super::PostgresLogCommandGateway;
    use crate::application::common::ports::LogCommandGateway;
    use crate::application::errors::AuditError;
    use crate::domain::merkle::{LogId, TransparencyLogFactory};
    use crate::infrastructure::persistence::postgres::TxSlot;

    fn factory() -> TransparencyLogFactory {
        TransparencyLogFactory::new(CryptoRegistry::hasher(HashAlgo::Sha256).expect("sha-256"))
    }

    // The persistence/happy path is covered by the integration suite (a real
    // transaction). Here, with no active transaction in the slot, appending
    // surfaces a storage error rather than panicking.
    #[tokio::test]
    async fn appending_without_an_active_transaction_errors() {
        let gateway = PostgresLogCommandGateway::new(Arc::new(TxSlot::new(None)), factory());
        let result = gateway.append_record(LogId(Uuid::new_v4()), b"x").await;
        assert!(matches!(result, Err(AuditError::Storage(_))));
    }
}
