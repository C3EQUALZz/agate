use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use agate_audit::application::common::ports::TransactionManager;
use agate_audit::application::errors::AuditError;

/// Counts begin/commit/rollback calls (test double for the transaction boundary).
#[derive(Default)]
pub struct RecordingTransactionManager {
    pub begins: AtomicUsize,
    pub commits: AtomicUsize,
    pub rollbacks: AtomicUsize,
}

impl RecordingTransactionManager {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl TransactionManager for RecordingTransactionManager {
    async fn begin(&self) -> Result<(), AuditError> {
        self.begins.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn commit(&self) -> Result<(), AuditError> {
        self.commits.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn rollback(&self) -> Result<(), AuditError> {
        self.rollbacks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
