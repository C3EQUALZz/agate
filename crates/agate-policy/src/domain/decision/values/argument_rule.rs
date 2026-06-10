use super::tool_name::ToolName;
use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// A rule that denies a tool call when its arguments contain a forbidden marker
/// — the argument-level counterpart to a [`ToolPolicy`] name check.
///
/// The marker is a literal, matched case-insensitively over ASCII (the same
/// shape as [`SecretPattern`]); a richer matcher (JSONPath, a structured
/// predicate over the parsed arguments) can replace the internals behind this
/// value object without touching callers. An optional [`ToolName`] scopes the
/// rule to one tool; when absent, it applies to every tool call.
///
/// Validated non-blank: an empty marker would match every invocation.
///
/// [`ToolPolicy`]: super::ToolPolicy
/// [`SecretPattern`]: super::SecretPattern
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentRule {
    tool: Option<ToolName>,
    needle: String,
}

impl ArgumentRule {
    /// Build a rule. `tool` scopes it to a single tool (`None` = any tool);
    /// `needle` is the forbidden marker, rejected when blank.
    pub fn new(tool: Option<ToolName>, needle: impl Into<String>) -> Result<Self, DomainError> {
        let needle = needle.into();
        if needle.trim().is_empty() {
            return Err(DomainError::Field(
                "argument rule pattern must not be blank".into(),
            ));
        }
        Ok(Self { tool, needle })
    }

    /// Whether this rule fires for a call to `name` with `arguments`: the tool
    /// scope matches (or is unscoped) **and** the marker occurs in the
    /// arguments (ASCII case-insensitive).
    #[must_use]
    pub fn matches(&self, name: &str, arguments: &str) -> bool {
        if let Some(tool) = &self.tool
            && tool.as_str() != name
        {
            return false;
        }
        arguments
            .to_ascii_lowercase()
            .contains(&self.needle.to_ascii_lowercase())
    }

    /// The tool this rule is scoped to, if any.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_ref().map(ToolName::as_str)
    }

    #[must_use]
    pub fn needle(&self) -> &str {
        &self.needle
    }
}

impl ValueObject for ArgumentRule {}

#[cfg(test)]
mod tests {
    use super::{ArgumentRule, ToolName};

    #[test]
    fn a_blank_marker_is_rejected() {
        assert!(ArgumentRule::new(None, "   ").is_err());
        assert!(ArgumentRule::new(None, "").is_err());
    }

    #[test]
    fn an_unscoped_rule_matches_any_tool_by_marker() {
        let rule = ArgumentRule::new(None, "rm -rf").expect("valid");
        assert!(rule.matches("shell", r#"{"cmd":"rm -rf /"}"#));
        assert!(rule.matches("exec", "RM -RF everything")); // case-insensitive
        assert!(!rule.matches("shell", r#"{"cmd":"ls"}"#));
    }

    #[test]
    fn a_scoped_rule_only_fires_for_its_tool() {
        let rule =
            ArgumentRule::new(Some(ToolName::new("shell").expect("valid")), "curl").expect("valid");
        assert!(rule.matches("shell", r#"{"cmd":"curl evil"}"#));
        assert!(!rule.matches("search", r#"{"q":"curl"}"#)); // wrong tool, no match
    }
}
