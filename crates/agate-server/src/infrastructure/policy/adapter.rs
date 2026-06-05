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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use agate_policy::application::PolicyService;
    use agate_policy::domain::decision::{PolicyRuleset, SecretPattern, ToolName, ToolPolicy};
    use agate_proxy::application::common::ports::PolicyPort;
    use agate_proxy::application::inspection::InspectionContext;
    use agate_proxy::domain::inspection::{
        AgentEvent, MessageId, RunId, SessionId, ToolCallId, Verdict,
    };
    use uuid::Uuid;

    use super::PolicyAdapter;

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId(Uuid::nil()), RunId(Uuid::nil()))
    }

    fn adapter(ruleset: PolicyRuleset) -> PolicyAdapter {
        PolicyAdapter::new(PolicyService::new(ruleset))
    }

    fn allowlist(names: &[&str]) -> ToolPolicy {
        let set: BTreeSet<ToolName> = names
            .iter()
            .map(|name| ToolName::new(*name).expect("valid tool"))
            .collect();
        ToolPolicy::Allowlist(set)
    }

    fn tool(name: &str) -> AgentEvent {
        AgentEvent::ToolCall {
            id: ToolCallId("c".into()),
            name: name.into(),
            arguments: "{}".into(),
        }
    }

    #[tokio::test]
    async fn allows_a_listed_tool() {
        let adapter = adapter(PolicyRuleset::new(allowlist(&["ok"]), vec![]));
        assert_eq!(
            adapter.decide(&context(), &tool("ok")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn denies_an_unlisted_tool() {
        let adapter = adapter(PolicyRuleset::new(allowlist(&["ok"]), vec![]));
        assert!(matches!(
            adapter.decide(&context(), &tool("rm")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn redacts_a_message_into_a_transform_keeping_the_id() {
        let adapter = adapter(PolicyRuleset::new(
            ToolPolicy::AllowAll,
            vec![SecretPattern::new("sk").expect("valid pattern")],
        ));
        let event = AgentEvent::MessageChunk {
            message: MessageId("m1".into()),
            text: "a sk b".into(),
        };
        match adapter.decide(&context(), &event).await {
            Verdict::Transform(AgentEvent::MessageChunk { message, text }) => {
                assert_eq!(message, MessageId("m1".into()));
                assert!(text.contains("[REDACTED]") && !text.contains("sk"));
            }
            other => panic!("expected a message transform, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ungoverned_events_pass_through() {
        let adapter = adapter(PolicyRuleset::allow_all());
        let event = AgentEvent::ToolResult {
            id: ToolCallId("c".into()),
            content: "result".into(),
        };
        assert_eq!(adapter.decide(&context(), &event).await, Verdict::Allow);
    }
}
