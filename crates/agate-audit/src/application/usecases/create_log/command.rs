use crate::application::common::messaging::{Command, Request};
use crate::application::errors::AuditError;
use crate::domain::merkle::LogId;

/// Create a new (empty) transparency log; returns its generated id.
pub struct CreateLog;

impl Request for CreateLog {
    type Response = Result<LogId, AuditError>;
}

impl Command for CreateLog {}
