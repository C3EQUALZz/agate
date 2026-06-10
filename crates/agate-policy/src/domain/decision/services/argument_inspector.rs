use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{ArgumentRule, DenyReason, PolicyDecision};

/// Inspects a tool call's arguments against the configured [`ArgumentRule`]s,
/// denying the first rule that fires. Runs after name authorization, so a
/// permitted tool can still be blocked on what it was asked to do.
pub struct ArgumentInspector;

impl ArgumentInspector {
    /// `Deny` if any rule matches the call, otherwise `Allow`. The reason names
    /// the tool but not the marker — the pattern is operator config, kept out of
    /// the client-facing `RUN_ERROR`.
    #[must_use]
    pub fn inspect(rules: &[ArgumentRule], name: &str, arguments: &str) -> PolicyDecision {
        if rules.iter().any(|rule| rule.matches(name, arguments)) {
            PolicyDecision::Deny(DenyReason::new(format!(
                "tool '{name}' arguments matched a blocked pattern"
            )))
        } else {
            PolicyDecision::Allow
        }
    }
}

impl DomainService for ArgumentInspector {}

#[cfg(test)]
mod tests {
    use super::{ArgumentInspector, ArgumentRule};
    use crate::domain::decision::values::{Pattern, PolicyDecision};

    fn rule(needle: &str) -> ArgumentRule {
        ArgumentRule::new(None, Pattern::literal(needle).expect("valid pattern"))
    }

    #[test]
    fn denies_on_a_matching_argument_rule() {
        let rules = [rule("rm -rf")];
        assert!(matches!(
            ArgumentInspector::inspect(&rules, "shell", r#"{"cmd":"rm -rf /"}"#),
            PolicyDecision::Deny(_)
        ));
    }

    #[test]
    fn allows_when_no_rule_fires() {
        let rules = [rule("rm -rf")];
        assert_eq!(
            ArgumentInspector::inspect(&rules, "shell", r#"{"cmd":"ls"}"#),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn allows_with_no_rules() {
        assert_eq!(
            ArgumentInspector::inspect(&[], "anything", "whatever"),
            PolicyDecision::Allow
        );
    }
}
