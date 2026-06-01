use std::fmt;

use super::time_errors::TimeError;

/// Root of the audit domain error hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Field(String),
    Time(TimeError),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Field(msg) => write!(f, "domain field error: {msg}"),
            DomainError::Time(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for DomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DomainError::Time(err) => Some(err),
            DomainError::Field(_) => None,
        }
    }
}

impl From<TimeError> for DomainError {
    fn from(err: TimeError) -> Self {
        DomainError::Time(err)
    }
}
