use std::fmt;

/// Root of the proxy domain error hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    Field(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Field(msg) => write!(f, "domain field error: {msg}"),
        }
    }
}

impl std::error::Error for DomainError {}
