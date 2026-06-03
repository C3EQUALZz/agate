use agate_crypto::KeyId;

use crate::application::common::messaging::{Command, Request};
use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, SignedTreeHead};

/// Issue and sign a checkpoint (Signed Tree Head) for a log, then anchor it.
pub struct IssueCheckpoint {
    pub log: LogId,
    pub key: KeyId,
}

impl Request for IssueCheckpoint {
    type Response = Result<SignedTreeHead, AuditError>;
}

impl Command for IssueCheckpoint {}
