use super::argument_inspector::ArgumentInspector;
use super::result_inspector::ResultInspector;
use super::text_redactor::TextRedactor;
use super::tool_authorizer::ToolAuthorizer;
use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{DenyReason, InspectedAction, PolicyDecision, PolicyRuleset};

/// Applies a [`PolicyRuleset`] to an [`InspectedAction`], routing it to the
/// service that governs its kind: tool calls to authorization (name, then
/// arguments), tool results to deny rules then redaction, emitted text to
/// redaction, state mutations to secret detection, everything else allowed.
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
            // A tool result is first checked against the result deny rules (a
            // forbidden result is blocked outright); if it clears, secrets in it
            // are redacted in place.
            InspectedAction::ToolResult { name, content } => {
                match ResultInspector::inspect(ruleset.result_rules(), name.as_deref(), content) {
                    PolicyDecision::Allow => TextRedactor::redact(ruleset.secrets(), content),
                    denied => denied,
                }
            }
            // Emitted assistant text is redacted in place.
            InspectedAction::Message { text } => TextRedactor::redact(ruleset.secrets(), text),
            // A state payload cannot be rewritten in place, so a secret found in
            // it is denied rather than leaked.
            InspectedAction::StateMutation { content } => {
                if TextRedactor::detects(ruleset.secrets(), content) {
                    PolicyDecision::Deny(DenyReason::new(
                        "state mutation contains a secret pattern that cannot be masked in place",
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
