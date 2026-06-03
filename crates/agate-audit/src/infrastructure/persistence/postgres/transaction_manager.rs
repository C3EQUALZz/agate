use async_trait::async_trait;
use sqlx::PgPool;

use super::{SharedTransaction, storage_error};
use crate::application::common::ports::TransactionManager;
use crate::application::errors::AuditError;

/// Owns the transaction boundary for one request scope. `begin` opens a
/// transaction on the pool and stores it in the shared slot the gateways read
/// from; `commit`/`rollback` take it back out. Gateways never commit.
pub struct PgTransactionManager {
    pool: PgPool,
    transaction: SharedTransaction,
}

impl PgTransactionManager {
    pub fn new(pool: PgPool, transaction: SharedTransaction) -> Self {
        Self { pool, transaction }
    }
}

#[async_trait]
impl TransactionManager for PgTransactionManager {
    async fn begin(&self) -> Result<(), AuditError> {
        let mut slot = self.transaction.lock().await;
        if slot.is_some() {
            return Err(AuditError::Storage("transaction already begun".to_string()));
        }
        *slot = Some(self.pool.begin().await.map_err(storage_error)?);
        Ok(())
    }

    async fn commit(&self) -> Result<(), AuditError> {
        let transaction = self.transaction.lock().await.take().ok_or_else(|| {
            AuditError::Storage("commit without an active transaction".to_string())
        })?;
        transaction.commit().await.map_err(storage_error)
    }

    async fn rollback(&self) -> Result<(), AuditError> {
        match self.transaction.lock().await.take() {
            Some(transaction) => transaction.rollback().await.map_err(storage_error),
            None => Ok(()),
        }
    }
}
