use std::fmt;

use async_trait::async_trait;

use agate_audit::application::errors::AuditError;
use agate_audit::domain::merkle::{LeafIndex, LogId};

/// Appends one record to a transparency log, each append in its own audit
/// request scope (one transaction, the audit context's commit boundary).
///
/// The scope lifecycle is a composition concern, so the implementation lives
/// at the composition root (`setup`); the outbox stays container-agnostic.
#[async_trait]
pub trait RecordAppender: Send + Sync {
    async fn append(&self, log: LogId, record: Vec<u8>) -> Result<LeafIndex, AppendError>;
}

/// Why an append produced no leaf index.
#[derive(Debug)]
pub enum AppendError {
    /// The request scope could not be opened: the record never reached the
    /// dispatch pipeline, so the audit MetricsBehavior never saw it.
    ScopeUnavailable(String),
    /// The append failed inside the pipeline, where the MetricsBehavior has
    /// already counted it.
    Pipeline(AuditError),
}

impl fmt::Display for AppendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeUnavailable(error) => write!(f, "request scope unavailable: {error}"),
            Self::Pipeline(error) => write!(f, "append failed in the pipeline: {error}"),
        }
    }
}

impl std::error::Error for AppendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::ScopeUnavailable(_) => None,
        }
    }
}
