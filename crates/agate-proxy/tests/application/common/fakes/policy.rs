use async_trait::async_trait;

use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, Verdict};

/// Policy test double that always returns a configured verdict.
pub struct FixedPolicy(pub Verdict<AgentEvent>);

#[async_trait]
impl PolicyPort for FixedPolicy {
    async fn decide(&self, _: &InspectionContext, _: &AgentEvent) -> Verdict<AgentEvent> {
        self.0.clone()
    }
}
