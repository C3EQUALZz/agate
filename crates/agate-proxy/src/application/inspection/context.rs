use crate::domain::inspection::{RunId, SessionId};

/// Which run (and session) an inspected event belongs to — passed to the policy
/// and audit ports so they can scope their decisions and records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectionContext {
    pub session: SessionId,
    pub run: RunId,
}

impl InspectionContext {
    pub fn new(session: SessionId, run: RunId) -> Self {
        Self { session, run }
    }
}
