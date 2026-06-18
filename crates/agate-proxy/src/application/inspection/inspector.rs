use std::sync::Arc;

use super::action::InspectionAction;
use super::context::InspectionContext;
use super::request::{RequestContent, RequestDecision, first_disallowed_url};
use crate::application::common::ports::{AuditSink, HostResolver, PolicyPort, SessionMemory};
use crate::domain::inspection::{
    AgentEvent, DenyReason, Fragment, MessageId, Run, StructuralOutcome, ToolCallId, Verdict,
};

/// Synthetic event origin for actions inspected on the request leg (before any
/// real per-event ids exist).
const REQUEST_ORIGIN: &str = "request";

/// Drives one fragment through the inspection seam: the pure-domain [`Run`]
/// state machine decides the structural outcome, then a complete semantic event
/// is judged by the (async) [`PolicyPort`] and recorded to the [`AuditSink`].
/// Transport concerns (the raw-frame buffer) stay in the presentation layer —
/// the [`InspectionAction`] only says *what* to do.
///
/// A structural reject is a protocol-integrity violation, so it terminates the
/// run (fail-closed); content denials from the policy only drop the one event.
pub struct Inspector {
    policy: Arc<dyn PolicyPort>,
    audit: Arc<dyn AuditSink>,
    resolver: Arc<dyn HostResolver>,
    memory: Arc<dyn SessionMemory>,
}

impl Inspector {
    pub fn new(
        policy: Arc<dyn PolicyPort>,
        audit: Arc<dyn AuditSink>,
        resolver: Arc<dyn HostResolver>,
        memory: Arc<dyn SessionMemory>,
    ) -> Self {
        Self {
            policy,
            audit,
            resolver,
            memory,
        }
    }

    pub async fn inspect(
        &self,
        run: &mut Run,
        context: &InspectionContext,
        fragment: Fragment,
    ) -> InspectionAction {
        match run.inspect(fragment) {
            StructuralOutcome::Buffering => InspectionAction::Hold,
            StructuralOutcome::Reject(reason) => InspectionAction::Terminate(reason),
            StructuralOutcome::Ready(event) => {
                // Replay guard: a tool quarantined by an earlier denial in this
                // session is refused across runs, before any fresh evaluation.
                if let Some(name) = tool_name(&event)
                    && let Some(reason) = self.memory.recall(context.session, name).await
                {
                    self.record_deny(context, &event, &reason).await;
                    return InspectionAction::Drop(reason);
                }
                // SSRF screen on any URL the event carries (a tool-call argument,
                // an emitted message, or a tool result), resolving domain hosts —
                // the response-leg counterpart to the request-leg guard. A hit
                // drops the one event rather than terminating the run.
                if let Some(text) = url_bearing_text(&event)
                    && let Some(reason) = first_disallowed_url(text, self.resolver.as_ref()).await
                {
                    self.record_deny(context, &event, &reason).await;
                    self.remember_tool_denial(context, &event, &reason).await;
                    return InspectionAction::Drop(reason);
                }
                let verdict = self.policy.decide(context, &event).await;
                self.audit.record(context, &event, &verdict).await;
                if let Verdict::Deny(reason) = &verdict {
                    self.remember_tool_denial(context, &event, reason).await;
                }
                match verdict {
                    Verdict::Allow => InspectionAction::Forward,
                    Verdict::Transform(replacement) => {
                        InspectionAction::ForwardTransformed(replacement)
                    }
                    Verdict::Deny(reason) => InspectionAction::Drop(reason),
                    Verdict::Terminate(reason) => InspectionAction::Terminate(reason),
                    Verdict::Buffer => InspectionAction::Hold,
                }
            }
        }
    }

    /// Inspect a request **before** forwarding (the preventive request leg).
    ///
    /// Every offered tool and user message is judged by the same [`PolicyPort`]
    /// as the response leg, and message text is scanned for disallowed URLs
    /// (the SSRF guard). The first rejection is recorded to the [`AuditSink`]
    /// and returned; an empty/permitted request yields [`RequestDecision::Allow`].
    pub async fn inspect_request(
        &self,
        context: &InspectionContext,
        request: &RequestContent,
    ) -> RequestDecision {
        for name in &request.offered_tools {
            let event = AgentEvent::ToolCall {
                id: ToolCallId::new(REQUEST_ORIGIN).expect("the synthetic origin is not blank"),
                name: name.clone(),
                arguments: String::new(),
            };
            // Replay guard first: a tool quarantined earlier in this session is
            // refused without re-evaluating it.
            if let Some(reason) = self.memory.recall(context.session, name).await {
                self.record_deny(context, &event, &reason).await;
                return RequestDecision::Reject(reason);
            }
            if let Some(reason) = self.reject_reason(context, &event).await {
                self.memory.remember(context.session, name, &reason).await;
                return RequestDecision::Reject(reason);
            }
        }

        // User messages and the otherwise-hidden fields (system prompt,
        // context, forwardedProps, inbound state) get the same text screen.
        for text in request.user_messages.iter().chain(&request.hidden_fields) {
            if let Some(reason) = self.screen_text(context, text).await {
                return RequestDecision::Reject(reason);
            }
        }

        RequestDecision::Allow
    }

    /// Screen one request-leg text blob: the SSRF URL guard first, then the same
    /// policy as the response leg (projected onto a message). Returns the reason
    /// on rejection, recording it as a denial.
    async fn screen_text(&self, context: &InspectionContext, text: &str) -> Option<DenyReason> {
        let event = AgentEvent::MessageChunk {
            message: MessageId::new(REQUEST_ORIGIN).expect("the synthetic origin is not blank"),
            text: text.to_owned(),
        };
        if let Some(reason) = first_disallowed_url(text, self.resolver.as_ref()).await {
            self.record_deny(context, &event, &reason).await;
            return Some(reason);
        }
        self.reject_reason(context, &event).await
    }

    /// Ask the policy about `event`; on any non-`Allow` verdict, record it as a
    /// denial and return the reason. (`Buffer` is meaningless on the request leg
    /// and treated as allow.)
    async fn reject_reason(
        &self,
        context: &InspectionContext,
        event: &AgentEvent,
    ) -> Option<DenyReason> {
        let verdict = self.policy.decide(context, event).await;
        let reason = match &verdict {
            Verdict::Allow | Verdict::Buffer => return None,
            Verdict::Deny(reason) | Verdict::Terminate(reason) => reason.clone(),
            Verdict::Transform(_) => DenyReason::new("request content matched a redaction rule"),
        };
        self.record_deny(context, event, &reason).await;
        Some(reason)
    }

    /// Record `event` as denied for `reason` — the audit-trail entry behind
    /// every drop/reject the inspector emits (a replay refusal, an SSRF hit, or
    /// a policy denial), so all of them are logged through one path.
    async fn record_deny(
        &self,
        context: &InspectionContext,
        event: &AgentEvent,
        reason: &DenyReason,
    ) {
        self.audit
            .record(context, event, &Verdict::Deny(reason.clone()))
            .await;
    }

    /// Quarantine the tool behind `event` for the rest of the session when it is
    /// denied, so the agent cannot replay it (with varied arguments) in a later
    /// run. A no-op for non-tool events and when memory is disabled.
    async fn remember_tool_denial(
        &self,
        context: &InspectionContext,
        event: &AgentEvent,
        reason: &DenyReason,
    ) {
        if let Some(name) = tool_name(event) {
            self.memory.remember(context.session, name, reason).await;
        }
    }
}

/// The text of an event that may carry a URL worth SSRF-screening: a tool call's
/// arguments, an emitted message chunk, or a tool result. Lifecycle, state, and
/// opaque events carry none.
///
/// Screening is per-event, so a URL split across streamed message chunks is not
/// reassembled — best-effort, like the per-chunk redaction.
fn url_bearing_text(event: &AgentEvent) -> Option<&str> {
    match event {
        AgentEvent::ToolCall { arguments, .. } => Some(arguments),
        AgentEvent::MessageChunk { text, .. } => Some(text),
        AgentEvent::ToolResult { content, .. } => Some(content),
        AgentEvent::StateMutation(_) | AgentEvent::Lifecycle(_) | AgentEvent::Opaque(_) => None,
    }
}

/// The tool name behind `event` if it is a tool call — the unit the session
/// ledger quarantines. `None` for every other event kind.
fn tool_name(event: &AgentEvent) -> Option<&str> {
    match event {
        AgentEvent::ToolCall { name, .. } => Some(name),
        AgentEvent::MessageChunk { .. }
        | AgentEvent::ToolResult { .. }
        | AgentEvent::StateMutation(_)
        | AgentEvent::Lifecycle(_)
        | AgentEvent::Opaque(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use uuid::Uuid;

    use super::{Inspector, RequestContent, RequestDecision};
    use crate::application::common::ports::{AuditSink, PolicyPort, SessionMemory};
    use crate::application::inspection::InspectionContext;
    use crate::domain::inspection::{AgentEvent, DenyReason, RunId, SessionId, Verdict};
    use crate::infrastructure::{InMemorySessionMemory, NoopHostResolver, NoopSessionMemory};

    struct NoopAudit;
    #[async_trait]
    impl AuditSink for NoopAudit {
        async fn record(&self, _: &InspectionContext, _: &AgentEvent, _: &Verdict<AgentEvent>) {}
    }

    struct AllowAll;
    #[async_trait]
    impl PolicyPort for AllowAll {
        async fn decide(&self, _: &InspectionContext, _: &AgentEvent) -> Verdict<AgentEvent> {
            Verdict::Allow
        }
    }

    /// Denies a tool named `delete_file`; allows everything else.
    struct DenyDelete;
    #[async_trait]
    impl PolicyPort for DenyDelete {
        async fn decide(&self, _: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
            match event {
                AgentEvent::ToolCall { name, .. } if name == "delete_file" => {
                    Verdict::Deny(DenyReason::new("tool not allowed"))
                }
                _ => Verdict::Allow,
            }
        }
    }

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()))
    }

    fn inspector(policy: Arc<dyn PolicyPort>) -> Inspector {
        inspector_with_memory(policy, Arc::new(NoopSessionMemory))
    }

    fn inspector_with_memory(
        policy: Arc<dyn PolicyPort>,
        memory: Arc<dyn SessionMemory>,
    ) -> Inspector {
        Inspector::new(
            policy,
            Arc::new(NoopAudit),
            Arc::new(NoopHostResolver),
            memory,
        )
    }

    #[tokio::test]
    async fn allows_a_clean_request() {
        let content = RequestContent {
            offered_tools: vec!["search".to_owned()],
            user_messages: vec!["find the readme please".to_owned()],
            ..RequestContent::default()
        };
        let decision = inspector(Arc::new(AllowAll))
            .inspect_request(&context(), &content)
            .await;
        assert_eq!(decision, RequestDecision::Allow);
    }

    #[tokio::test]
    async fn rejects_a_denied_offered_tool() {
        let content = RequestContent {
            offered_tools: vec!["search".to_owned(), "delete_file".to_owned()],
            ..RequestContent::default()
        };
        let decision = inspector(Arc::new(DenyDelete))
            .inspect_request(&context(), &content)
            .await;
        assert!(matches!(decision, RequestDecision::Reject(_)));
    }

    #[tokio::test]
    async fn rejects_an_ssrf_url_even_under_allow_all() {
        let content = RequestContent {
            user_messages: vec!["fetch http://169.254.169.254/latest/meta-data".to_owned()],
            ..RequestContent::default()
        };
        let decision = inspector(Arc::new(AllowAll))
            .inspect_request(&context(), &content)
            .await;
        assert!(matches!(decision, RequestDecision::Reject(_)));
    }

    #[tokio::test]
    async fn rejects_an_ssrf_url_hidden_in_a_request_field() {
        // No user message — the SSRF URL is buried in a hidden field (e.g. the
        // JSON of `state`/`context`), which is now screened too.
        let content = RequestContent {
            hidden_fields: vec!["fetch http://127.0.0.1/secret".to_owned()],
            ..RequestContent::default()
        };
        let decision = inspector(Arc::new(AllowAll))
            .inspect_request(&context(), &content)
            .await;
        assert!(matches!(decision, RequestDecision::Reject(_)));
    }

    #[tokio::test]
    async fn a_tool_denied_in_one_run_is_quarantined_for_the_whole_session() {
        let memory: Arc<dyn SessionMemory> =
            Arc::new(InMemorySessionMemory::new(Duration::from_hours(1)));
        let offered = RequestContent {
            offered_tools: vec!["delete_file".to_owned()],
            ..RequestContent::default()
        };

        // Run 1: the policy denies `delete_file`, which the session remembers.
        let run_one = inspector_with_memory(Arc::new(DenyDelete), memory.clone());
        assert!(matches!(
            run_one.inspect_request(&context(), &offered).await,
            RequestDecision::Reject(_)
        ));

        // Run 2 (same session): an allow-all policy would permit the tool, but
        // the session ledger refuses the replay regardless.
        let run_two = inspector_with_memory(Arc::new(AllowAll), memory);
        assert!(matches!(
            run_two.inspect_request(&context(), &offered).await,
            RequestDecision::Reject(_)
        ));
    }
}
