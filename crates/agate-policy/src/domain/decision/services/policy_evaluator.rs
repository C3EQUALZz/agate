use super::argument_inspector::ArgumentInspector;
use super::text_redactor::TextRedactor;
use super::tool_authorizer::ToolAuthorizer;
use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{InspectedAction, PolicyDecision, PolicyRuleset};

/// Applies a [`PolicyRuleset`] to an [`InspectedAction`], routing it to the
/// service that governs its kind: tool calls to authorization (name, then
/// arguments), emitted text to redaction, everything else allowed.
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
            InspectedAction::Message { text } => TextRedactor::redact(ruleset.secrets(), text),
            InspectedAction::Other => PolicyDecision::Allow,
        }
    }
}

impl DomainService for PolicyEvaluator {}
