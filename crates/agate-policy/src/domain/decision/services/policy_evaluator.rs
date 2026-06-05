use super::text_redactor::TextRedactor;
use super::tool_authorizer::ToolAuthorizer;
use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{InspectedAction, PolicyDecision, PolicyRuleset};

/// Applies a [`PolicyRuleset`] to an [`InspectedAction`], routing it to the
/// service that governs its kind: tool calls to authorization, emitted text to
/// redaction, everything else allowed.
pub struct PolicyEvaluator;

impl PolicyEvaluator {
    #[must_use]
    pub fn evaluate(ruleset: &PolicyRuleset, action: &InspectedAction) -> PolicyDecision {
        match action {
            InspectedAction::ToolCall { name, .. } => {
                ToolAuthorizer::authorize(ruleset.tools(), name)
            }
            InspectedAction::Message { text } => TextRedactor::redact(ruleset.secrets(), text),
            InspectedAction::Other => PolicyDecision::Allow,
        }
    }
}

impl DomainService for PolicyEvaluator {}
