use std::fmt;

use crate::domain::merkle::{LeafIndex, LogId};

/// Errors returned by audit application use cases.
#[derive(Debug)]
pub enum AuditError {
    LogNotFound(LogId),
    LeafOutOfRange { index: LeafIndex, size: u64 },
    SizeOutOfRange { requested: u64, current: u64 },
    Storage(String),
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditError::LogNotFound(id) => write!(f, "log not found: {id:?}"),
            AuditError::LeafOutOfRange { index, size } => {
                write!(f, "leaf {index:?} is out of range (tree size {size})")
            }
            AuditError::SizeOutOfRange { requested, current } => {
                write!(
                    f,
                    "size {requested} is out of range (current size {current})"
                )
            }
            AuditError::Storage(msg) => write!(f, "storage error: {msg}"),
        }
    }
}

impl std::error::Error for AuditError {}
