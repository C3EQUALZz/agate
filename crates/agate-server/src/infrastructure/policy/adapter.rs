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
        AgentEvent::ToolResult { name, content, .. } => InspectedAction::ToolResult {
            name: name.clone(),
            content: content.clone(),
        },
        AgentEvent::StateMutation(mutation) => InspectedAction::StateMutation {
            content: mutation.payload().to_owned(),
        },
        AgentEvent::Lifecycle(_) | AgentEvent::Opaque(_) => InspectedAction::Other,
    }
}

/// Rebuild the event with the redacted text. Redaction is produced for emitted
/// messages and tool results (both carry rewritable text); any other event is
/// forwarded unchanged (defensive).
fn redacted_verdict(event: &AgentEvent, text: String) -> Verdict<AgentEvent> {
    match event {
        AgentEvent::MessageChunk { message, .. } => Verdict::Transform(AgentEvent::MessageChunk {
            message: message.clone(),
            text,
        }),
        AgentEvent::ToolResult { id, name, .. } => Verdict::Transform(AgentEvent::ToolResult {
            id: id.clone(),
            name: name.clone(),
            content: text,
        }),
        _ => Verdict::Allow,
    }
}

#[cfg(test)]
mod tests {
    use agate_policy::application::PolicyService;
    use agate_policy::domain::decision::{Pattern, PolicyRuleset, ToolMatcher, ToolPolicy};
    use agate_proxy::application::common::ports::PolicyPort;
    use agate_proxy::application::inspection::InspectionContext;
    use agate_proxy::domain::inspection::{
        AgentEvent, MessageId, RunId, SessionId, StateMutation, ToolCallId, Verdict,
    };
    use uuid::Uuid;

    use super::PolicyAdapter;

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()))
    }

    fn adapter(ruleset: PolicyRuleset) -> PolicyAdapter {
        PolicyAdapter::new(PolicyService::new(ruleset))
    }

    fn allowlist(names: &[&str]) -> ToolPolicy {
        let matchers = names
            .iter()
            .map(|name| ToolMatcher::exact(*name).expect("valid tool"))
            .collect();
        ToolPolicy::Allowlist(matchers)
    }

    fn tool(name: &str) -> AgentEvent {
        AgentEvent::ToolCall {
            id: ToolCallId::new("c").expect("valid id"),
            name: name.into(),
            arguments: "{}".into(),
        }
    }

    #[tokio::test]
    async fn allows_a_listed_tool() {
        let adapter = adapter(PolicyRuleset::new(allowlist(&["ok"]), vec![], vec![]));
        assert_eq!(
            adapter.decide(&context(), &tool("ok")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn denies_an_unlisted_tool() {
        let adapter = adapter(PolicyRuleset::new(allowlist(&["ok"]), vec![], vec![]));
        assert!(matches!(
            adapter.decide(&context(), &tool("rm")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn redacts_a_message_into_a_transform_keeping_the_id() {
        let adapter = adapter(PolicyRuleset::new(
            ToolPolicy::AllowAll,
            vec![],
            vec![Pattern::literal("sk").expect("valid pattern")],
        ));
        let event = AgentEvent::MessageChunk {
            message: MessageId::new("m1").expect("valid id"),
            text: "a sk b".into(),
        };
        match adapter.decide(&context(), &event).await {
            Verdict::Transform(AgentEvent::MessageChunk { message, text }) => {
                assert_eq!(message, MessageId::new("m1").expect("valid id"));
                assert!(text.contains("[REDACTED]") && !text.contains("sk"));
            }
            other => panic!("expected a message transform, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ungoverned_events_pass_through() {
        let adapter = adapter(PolicyRuleset::allow_all());
        let event = AgentEvent::ToolResult {
            id: ToolCallId::new("c").expect("valid id"),
            name: Some("fetch".into()),
            content: "result".into(),
        };
        assert_eq!(adapter.decide(&context(), &event).await, Verdict::Allow);
    }

    #[tokio::test]
    async fn redacts_a_secret_in_a_tool_result_keeping_the_id() {
        let adapter = adapter(PolicyRuleset::new(
            ToolPolicy::AllowAll,
            vec![],
            vec![Pattern::literal("sk").expect("valid pattern")],
        ));
        let event = AgentEvent::ToolResult {
            id: ToolCallId::new("c1").expect("valid id"),
            name: Some("fetch".into()),
            content: "token sk here".into(),
        };
        match adapter.decide(&context(), &event).await {
            Verdict::Transform(AgentEvent::ToolResult { id, name, content }) => {
                assert_eq!(id, ToolCallId::new("c1").expect("valid id"));
                assert_eq!(name, Some("fetch".into()));
                assert!(content.contains("[REDACTED]") && !content.contains("sk"));
            }
            other => panic!("expected a tool-result transform, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn denies_a_state_mutation_carrying_a_secret() {
        let adapter = adapter(PolicyRuleset::new(
            ToolPolicy::AllowAll,
            vec![],
            vec![Pattern::literal("sk").expect("valid pattern")],
        ));
        let event = AgentEvent::StateMutation(StateMutation::Snapshot {
            byte_size: 16,
            payload: r#"{"k":"sk-x"}"#.into(),
        });
        assert!(matches!(
            adapter.decide(&context(), &event).await,
            Verdict::Deny(_)
        ));
    }
}
