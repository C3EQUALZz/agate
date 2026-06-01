use agate_crypto::Digest;

use super::values::{LeafIndex, TreeHead};
use crate::domain::common::events::DomainEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuditEvent {
    RecordAppended { index: LeafIndex, leaf: Digest },
    CheckpointIssued { head: TreeHead },
}

impl DomainEvent for AuditEvent {
    fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::RecordAppended { .. } => "audit.record_appended",
            AuditEvent::CheckpointIssued { .. } => "audit.checkpoint_issued",
        }
    }
}
