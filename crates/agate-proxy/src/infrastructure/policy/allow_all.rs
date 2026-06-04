use async_trait::async_trait;

use crate::application::common::ports::PolicyPort;
use crate::application::inspection::InspectionContext;
use crate::domain::inspection::{AgentEvent, Verdict};

/// The default policy until `agate-policy` exists: allow every event unchanged.
pub struct AllowAllPolicy;

#[async_trait]
impl PolicyPort for AllowAllPolicy {
    async fn decide(
        &self,
        _context: &InspectionContext,
        _event: &AgentEvent,
    ) -> Verdict<AgentEvent> {
        Verdict::Allow
    }
}
