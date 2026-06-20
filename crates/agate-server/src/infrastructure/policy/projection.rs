//! Shared lifting of a policy-context [`PolicyDecision`] onto the proxy's
//! [`Verdict`], used by every `PolicyPort` backend (the static ruleset adapter
//! and the CEL adapter) so the redaction invariants cannot drift between them.

use agate_policy::domain::decision::PolicyDecision;
use agate_proxy::domain::inspection::{AgentEvent, DenyReason, Verdict};

/// Lift a [`PolicyDecision`] onto the proxy's [`Verdict`] for `event`: allow
/// passes it, deny carries the reason, and a redaction rebuilds the event with
/// the new text — but only for the events that carry rewritable text (a message
/// chunk or a tool result).
///
/// A redaction on any other event kind **fails closed** (deny), not open: the
/// static engine never asks to redact a non-rewritable event (it denies a
/// secret-bearing state mutation instead), but a hand-written CEL rule can put
/// `effect = "redact"` on a tool call or state mutation — and silently *allowing*
/// it would defeat the operator's intent to scrub it. Blocking surfaces the
/// misapplied rule safely.
pub(crate) fn lift_decision(event: &AgentEvent, decision: PolicyDecision) -> Verdict<AgentEvent> {
    match decision {
        PolicyDecision::Allow => Verdict::Allow,
        PolicyDecision::Deny(reason) => Verdict::Deny(DenyReason::new(reason.as_str())),
        PolicyDecision::RedactText(text) => match event {
            AgentEvent::MessageChunk { message, .. } => {
                Verdict::Transform(AgentEvent::MessageChunk {
                    message: message.clone(),
                    text,
                })
            }
            AgentEvent::ToolResult { id, name, .. } => Verdict::Transform(AgentEvent::ToolResult {
                id: id.clone(),
                name: name.clone(),
                content: text,
            }),
            _ => Verdict::Deny(DenyReason::new(
                "redaction not applicable to this event kind",
            )),
        },
    }
}
