use std::fmt;

use agate_audit::application::errors::AuditError;

/// Why a scoped audit dispatch (one request scope = one transaction) failed.
/// Shared by every port that runs an audit command in its own scope
/// ([`RecordAppender`](super::RecordAppender),
/// [`CheckpointIssuer`](super::CheckpointIssuer)): the failure modes are the
/// same, so the error is too.
#[derive(Debug)]
pub enum ScopeError {
    /// The request scope could not be opened: the command never reached the
    /// dispatch pipeline (so the audit MetricsBehavior never saw it).
    Unavailable(String),
    /// The command failed inside the pipeline, where the MetricsBehavior has
    /// already counted it.
    Pipeline(AuditError),
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(error) => write!(f, "request scope unavailable: {error}"),
            Self::Pipeline(error) => write!(f, "audit command failed: {error}"),
        }
    }
}

impl std::error::Error for ScopeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::Unavailable(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use agate_audit::application::errors::AuditError;
    use agate_audit::domain::merkle::LogId;
    use uuid::Uuid;

    use super::ScopeError;

    #[test]
    fn unavailable_has_no_source_and_a_clear_message() {
        let error = ScopeError::Unavailable("closed".into());
        assert!(error.to_string().contains("request scope unavailable"));
        assert!(error.source().is_none());
    }

    #[test]
    fn pipeline_exposes_the_inner_audit_error() {
        let error = ScopeError::Pipeline(AuditError::LogNotFound(LogId(Uuid::nil())));
        assert!(error.to_string().contains("audit command failed"));
        assert!(error.source().is_some());
    }
}
