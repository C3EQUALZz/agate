use agate_crypto::KeyId;

use crate::application::common::messaging::{Command, Request};
use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, SignedTreeHead, TreeSize};

/// Issue and sign a checkpoint (Signed Tree Head) for a log, then anchor it.
///
/// `previous_size` lets a periodic issuer skip redundant work on an idle log:
/// when it equals the log's current size, the head is signed and returned but
/// **not** re-recorded or re-anchored (the checkpoint at that size already
/// exists). `None` always issues — the manual/API path.
pub struct IssueCheckpoint {
    pub log: LogId,
    pub key: KeyId,
    pub previous_size: Option<TreeSize>,
}

impl Request for IssueCheckpoint {
    type Response = Result<SignedTreeHead, AuditError>;
}

impl Command for IssueCheckpoint {}
