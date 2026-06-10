use std::sync::Arc;

use super::action::InspectionAction;
use super::context::InspectionContext;
use super::request::{RequestContent, RequestDecision, first_disallowed_url};
use crate::application::common::ports::{AuditSink, PolicyPort};
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
}

impl Inspector {
    pub fn new(policy: Arc<dyn PolicyPort>, audit: Arc<dyn AuditSink>) -> Self {
        Self { policy, audit }
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
                let verdict = self.policy.decide(context, &event).await;
                self.audit.record(context, &event, &verdict).await;
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
            if let Some(reason) = self.reject_reason(context, &event).await {
                return RequestDecision::Reject(reason);
            }
        }

        for text in &request.user_messages {
            let event = AgentEvent::MessageChunk {
                message: MessageId::new(REQUEST_ORIGIN).expect("the synthetic origin is not blank"),
                text: text.clone(),
            };
            if let Some(reason) = first_disallowed_url(text) {
                self.audit
                    .record(context, &event, &Verdict::Deny(reason.clone()))
                    .await;
                return RequestDecision::Reject(reason);
            }
            if let Some(reason) = self.reject_reason(context, &event).await {
                return RequestDecision::Reject(reason);
            }
        }

        RequestDecision::Allow
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
        self.audit
            .record(context, event, &Verdict::Deny(reason.clone()))
            .await;
        Some(reason)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use uuid::Uuid;

    use super::{Inspector, RequestContent, RequestDecision};
    use crate::application::common::ports::{AuditSink, PolicyPort};
    use crate::application::inspection::InspectionContext;
    use crate::domain::inspection::{AgentEvent, DenyReason, RunId, SessionId, Verdict};

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
        Inspector::new(policy, Arc::new(NoopAudit))
    }

    #[tokio::test]
    async fn allows_a_clean_request() {
        let content = RequestContent {
            offered_tools: vec!["search".to_owned()],
            user_messages: vec!["find the readme please".to_owned()],
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
            user_messages: Vec::new(),
        };
        let decision = inspector(Arc::new(DenyDelete))
            .inspect_request(&context(), &content)
            .await;
        assert!(matches!(decision, RequestDecision::Reject(_)));
    }

    #[tokio::test]
    async fn rejects_an_ssrf_url_even_under_allow_all() {
        let content = RequestContent {
            offered_tools: Vec::new(),
            user_messages: vec!["fetch http://169.254.169.254/latest/meta-data".to_owned()],
        };
        let decision = inspector(Arc::new(AllowAll))
            .inspect_request(&context(), &content)
            .await;
        assert!(matches!(decision, RequestDecision::Reject(_)));
    }
}
