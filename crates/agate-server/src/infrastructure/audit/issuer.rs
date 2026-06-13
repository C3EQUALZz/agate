use std::fmt;

use async_trait::async_trait;

use agate_audit::application::errors::AuditError;
use agate_audit::domain::merkle::{LogId, SignedTreeHead, TreeSize};

/// Issues one signed checkpoint (STH) for a log, in its own audit request scope
/// (one transaction). `previous_size` lets the caller skip re-anchoring a tree
/// that has not grown since its last checkpoint.
///
/// The scope lifecycle is a composition concern, so the implementation lives at
/// the composition root (`setup`); the scheduler stays container-agnostic — the
/// same split as [`RecordAppender`](super::RecordAppender).
#[async_trait]
pub trait CheckpointIssuer: Send + Sync {
    async fn issue(
        &self,
        log: LogId,
        previous_size: Option<TreeSize>,
    ) -> Result<SignedTreeHead, IssueError>;
}

/// Why issuing a checkpoint failed.
#[derive(Debug)]
pub enum IssueError {
    /// The request scope could not be opened: the command never reached the
    /// dispatch pipeline.
    ScopeUnavailable(String),
    /// The issue failed inside the pipeline (e.g. the signing key is not
    /// configured, or the store errored).
    Pipeline(AuditError),
}

impl fmt::Display for IssueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeUnavailable(error) => write!(f, "request scope unavailable: {error}"),
            Self::Pipeline(error) => write!(f, "checkpoint issue failed: {error}"),
        }
    }
}

impl std::error::Error for IssueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::ScopeUnavailable(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IssueError;

    #[test]
    fn display_distinguishes_the_failure_modes() {
        let scope = IssueError::ScopeUnavailable("closed".into());
        assert!(scope.to_string().contains("request scope unavailable"));
    }
}
