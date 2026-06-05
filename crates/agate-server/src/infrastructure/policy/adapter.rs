use async_trait::async_trait;

use agate_policy::application::PolicyService;
use agate_policy::domain::decision::{InspectedAction, PolicyDecision};
use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, DenyReason, Verdict};

/// Bridges the proxy's [`PolicyPort`] to the policy context: it projects a proxy
/// [`AgentEvent`] onto the policy's [`InspectedAction`], asks the
/// [`PolicyService`] for a decision, and lifts that back into the proxy's
/// [`Verdict`]. This is the only place the two contexts' vocabularies meet —
/// neither depends on the other.
pub struct PolicyAdapter {
    service: PolicyService,
}

impl PolicyAdapter {
    #[must_use]
    pub fn new(service: PolicyService) -> Self {
        Self { service }
    }
}

#[async_trait]
impl PolicyPort for PolicyAdapter {
    async fn decide(
        &self,
        _context: &InspectionContext,
        event: &AgentEvent,
    ) -> Verdict<AgentEvent> {
        match self.service.decide(&to_action(event)) {
            PolicyDecision::Allow => Verdict::Allow,
            PolicyDecision::Deny(reason) => Verdict::Deny(DenyReason::new(reason.as_str())),
            PolicyDecision::RedactText(text) => redacted_verdict(event, text),
        }
    }
}

/// Project the proxy's semantic event onto the policy's input vocabulary.
fn to_action(event: &AgentEvent) -> InspectedAction {
    match event {
        AgentEvent::ToolCall {
            name, arguments, ..
        } => InspectedAction::ToolCall {
            name: name.clone(),
            arguments: arguments.clone(),
        },
        AgentEvent::MessageChunk { text, .. } => InspectedAction::Message { text: text.clone() },
        AgentEvent::ToolResult { .. }
        | AgentEvent::StateMutation(_)
        | AgentEvent::Lifecycle(_)
        | AgentEvent::Opaque(_) => InspectedAction::Other,
    }
}

/// Rebuild the event with the redacted text. Redaction is only produced for a
/// message, so any other event is forwarded unchanged (defensive).
fn redacted_verdict(event: &AgentEvent, text: String) -> Verdict<AgentEvent> {
    match event {
        AgentEvent::MessageChunk { message, .. } => Verdict::Transform(AgentEvent::MessageChunk {
            message: message.clone(),
            text,
        }),
        _ => Verdict::Allow,
    }
}
