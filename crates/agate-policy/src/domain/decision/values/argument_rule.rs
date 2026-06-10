use super::pattern::Pattern;
use super::tool_name::ToolName;
use crate::domain::common::values::ValueObject;

/// A rule that denies a tool call when its arguments match a forbidden
/// [`Pattern`] — the argument-level counterpart to a [`ToolPolicy`] name check.
///
/// The marker is a literal or regex (the shared content matcher); an optional
/// [`ToolName`] scopes the rule to one tool, and when absent it applies to
/// every tool call.
///
/// [`ToolPolicy`]: super::ToolPolicy
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentRule {
    tool: Option<ToolName>,
    marker: Pattern,
}

impl ArgumentRule {
    /// Build a rule. `tool` scopes it to a single tool (`None` = any tool);
    /// `marker` is the forbidden content matcher.
    #[must_use]
    pub fn new(tool: Option<ToolName>, marker: Pattern) -> Self {
        Self { tool, marker }
    }

    /// Whether this rule fires for a call to `name` with `arguments`: the tool
    /// scope matches (or is unscoped) **and** the marker occurs in the
    /// arguments.
    #[must_use]
    pub fn matches(&self, name: &str, arguments: &str) -> bool {
        if let Some(tool) = &self.tool
            && tool.as_str() != name
        {
            return false;
        }
        self.marker.matches(arguments)
    }

    /// The tool this rule is scoped to, if any.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_ref().map(ToolName::as_str)
    }

    /// The forbidden-content matcher this rule fires on.
    #[must_use]
    pub fn marker(&self) -> &Pattern {
        &self.marker
    }
}

impl ValueObject for ArgumentRule {}

#[cfg(test)]
mod tests {
    use super::{ArgumentRule, Pattern, ToolName};

    fn literal(needle: &str) -> Pattern {
        Pattern::literal(needle).expect("valid pattern")
    }

    #[test]
    fn an_unscoped_rule_matches_any_tool_by_marker() {
        let rule = ArgumentRule::new(None, literal("rm -rf"));
        assert!(rule.matches("shell", r#"{"cmd":"rm -rf /"}"#));
        assert!(rule.matches("exec", "RM -RF everything")); // case-insensitive
        assert!(!rule.matches("shell", r#"{"cmd":"ls"}"#));
    }

    #[test]
    fn a_scoped_rule_only_fires_for_its_tool() {
        let rule = ArgumentRule::new(
            Some(ToolName::new("shell").expect("valid")),
            literal("curl"),
        );
        assert!(rule.matches("shell", r#"{"cmd":"curl evil"}"#));
        assert!(!rule.matches("search", r#"{"q":"curl"}"#)); // wrong tool, no match
    }

    #[test]
    fn a_regex_marker_matches_structured_arguments() {
        let rule = ArgumentRule::new(
            None,
            Pattern::regex(r#""url"\s*:\s*"https?://169\.254"#).expect("valid"),
        );
        assert!(rule.matches("fetch", r#"{"url":"http://169.254.169.254/"}"#));
        assert!(!rule.matches("fetch", r#"{"url":"https://example.com"}"#));
    }
}
