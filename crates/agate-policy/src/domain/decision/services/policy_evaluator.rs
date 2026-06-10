use super::argument_inspector::ArgumentInspector;
use super::text_redactor::TextRedactor;
use super::tool_authorizer::ToolAuthorizer;
use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{DenyReason, InspectedAction, PolicyDecision, PolicyRuleset};

/// Applies a [`PolicyRuleset`] to an [`InspectedAction`], routing it to the
/// service that governs its kind: tool calls to authorization (name, then
/// arguments), emitted text and tool results to redaction, state mutations to
/// secret detection, everything else allowed.
pub struct PolicyEvaluator;

impl PolicyEvaluator {
    #[must_use]
    pub fn evaluate(ruleset: &PolicyRuleset, action: &InspectedAction) -> PolicyDecision {
        match action {
            // A tool call clears name authorization first; a permitted tool is
            // then checked against the argument rules (what it was asked to do).
            InspectedAction::ToolCall { name, arguments } => {
                match ToolAuthorizer::authorize(ruleset.tools(), name) {
                    PolicyDecision::Allow => {
                        ArgumentInspector::inspect(ruleset.argument_rules(), name, arguments)
                    }
                    denied => denied,
                }
            }
            // Emitted text and tool results are both redacted in place.
            InspectedAction::Message { text } | InspectedAction::ToolResult { content: text } => {
                TextRedactor::redact(ruleset.secrets(), text)
            }
            // A state payload cannot be rewritten in place, so a secret found in
            // it is denied rather than leaked.
            InspectedAction::StateMutation { content } => {
                if TextRedactor::detects(ruleset.secrets(), content) {
                    PolicyDecision::Deny(DenyReason::new(
                        "state mutation contains a redacted marker and cannot be masked",
                    ))
                } else {
                    PolicyDecision::Allow
                }
            }
            InspectedAction::Other => PolicyDecision::Allow,
        }
    }
}

impl DomainService for PolicyEvaluator {}
