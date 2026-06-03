use async_trait::async_trait;
use sqlx::PgPool;

use agate_crypto::{Digest, HashAlgo};

use super::storage_error;
use crate::application::common::ports::LogCommandGateway;
use crate::application::errors::AuditError;
use crate::domain::common::entities::Entity;
use crate::domain::common::values::{Timestamp, Timestamps};
use crate::domain::merkle::{LogId, TransparencyLog, TransparencyLogFactory};

/// Write-side gateway backed by PostgreSQL (append-only).
pub struct PostgresLogCommandGateway {
    pool: PgPool,
    factory: TransparencyLogFactory,
}

impl PostgresLogCommandGateway {
    pub fn new(pool: PgPool, factory: TransparencyLogFactory) -> Self {
        Self { pool, factory }
    }
}

#[async_trait]
impl LogCommandGateway for PostgresLogCommandGateway {
    async fn load(&self, id: LogId) -> Result<Option<TransparencyLog>, AuditError> {
        let Some((created, updated, algo_code)) = sqlx::query_as::<_, (i64, i64, i16)>(
            "SELECT created_at, updated_at, hash_algo FROM audit_log WHERE id = $1",
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
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
        .fetch_all(&self.pool)
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

    async fn save(&self, log: &TransparencyLog) -> Result<(), AuditError> {
        let id = log.id().0;

        sqlx::query(
            "INSERT INTO audit_log (id, created_at, updated_at, hash_algo)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE SET updated_at = EXCLUDED.updated_at",
        )
        .bind(id)
        .bind(log.created_at().as_millis())
        .bind(log.updated_at().as_millis())
        .bind(i16::from(log.algo().code()))
        .execute(&self.pool)
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
            .execute(&self.pool)
            .await
            .map_err(storage_error)?;
        }

        Ok(())
    }
}
