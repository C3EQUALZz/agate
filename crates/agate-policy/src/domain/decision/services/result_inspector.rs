use serde_json::Value;

use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{DenyReason, PolicyDecision, ResultRule};

/// Inspects a tool result against the configured [`ResultRule`]s, denying the
/// first rule that fires. Runs before redaction, so a result carrying forbidden
/// content is blocked outright rather than masked and forwarded.
pub struct ResultInspector;

impl ResultInspector {
    /// `Deny` if any rule matches the result, otherwise `Allow`. The reason
    /// names the tool (when known) but not the marker — the pattern is operator
    /// config, kept out of the client-facing error.
    #[must_use]
    pub fn inspect(rules: &[ResultRule], name: Option<&str>, content: &str) -> PolicyDecision {
        // Parse the content once for the whole rule set (only when a path rule
        // needs it), so several path rules don't each re-deserialize.
        let parsed = rules
            .iter()
            .any(|rule| rule.path().is_some())
            .then(|| serde_json::from_str::<Value>(content).ok())
            .flatten();
        if rules
            .iter()
            .any(|rule| rule.matches(name, content, parsed.as_ref()))
        {
            let tool = name.unwrap_or("unknown");
            PolicyDecision::Deny(DenyReason::new(format!(
                "tool '{tool}' result matched a blocked pattern"
            )))
        } else {
            PolicyDecision::Allow
        }
    }
}

impl DomainService for ResultInspector {}

#[cfg(test)]
mod tests {
    use super::ResultInspector;
    use crate::domain::decision::values::{Pattern, PolicyDecision, ResultRule};

    fn rule(needle: &str) -> ResultRule {
        ResultRule::new(None, Pattern::literal(needle).expect("valid pattern"))
    }

    #[test]
    fn denies_on_a_matching_result_rule() {
        let rules = [rule("PRIVATE KEY")];
        assert!(matches!(
            ResultInspector::inspect(&rules, Some("fetch"), "leaked PRIVATE KEY material"),
            PolicyDecision::Deny(_)
        ));
    }

    #[test]
    fn allows_when_no_rule_fires() {
        let rules = [rule("PRIVATE KEY")];
        assert_eq!(
            ResultInspector::inspect(&rules, Some("fetch"), "ordinary output"),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn allows_with_no_rules() {
        assert_eq!(
            ResultInspector::inspect(&[], Some("fetch"), "anything"),
            PolicyDecision::Allow
        );
    }
}
