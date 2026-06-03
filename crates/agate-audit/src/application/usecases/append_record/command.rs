use crate::application::common::messaging::{Command, Request};
use crate::application::errors::AuditError;
use crate::domain::merkle::{LeafIndex, LogId};

/// Append a record to a transparency log.
pub struct AppendRecord {
    pub log: LogId,
    pub record: Vec<u8>,
}

impl Request for AppendRecord {
    type Response = Result<LeafIndex, AuditError>;
}

impl Command for AppendRecord {}
