use serde_json::Value;

use super::json_path::JsonPath;
use super::pattern::Pattern;
use super::tool_name::ToolName;
use crate::domain::common::values::ValueObject;

/// A rule that denies a tool *result* when its content matches a forbidden
/// [`Pattern`] — the result-side counterpart to an [`ArgumentRule`]. Where an
/// argument rule guards what a tool is asked to do, a result rule guards what it
/// returns (the indirect-injection / exfiltration surface).
///
/// An optional [`ToolName`] scopes the rule to one tool (absent = every tool);
/// an optional [`JsonPath`] scopes the match to one field of the parsed result
/// JSON (absent = the whole result string). Unlike an argument, a result is not
/// always attributable to a tool by name, so the scope only fires when the
/// result's tool name is known and matches.
///
/// [`ArgumentRule`]: super::ArgumentRule
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResultRule {
    tool: Option<ToolName>,
    path: Option<JsonPath>,
    marker: Pattern,
}

impl ResultRule {
    /// Build a whole-result rule. `tool` scopes it to a single tool
    /// (`None` = any tool); `marker` is the forbidden content matcher.
    #[must_use]
    pub fn new(tool: Option<ToolName>, marker: Pattern) -> Self {
        Self {
            tool,
            path: None,
            marker,
        }
    }

    /// Scope the match to one field of the parsed result JSON.
    #[must_use]
    pub fn with_path(mut self, path: JsonPath) -> Self {
        self.path = Some(path);
        self
    }

    /// Whether this rule fires for a result from tool `name` (`None` when the
    /// proxy could not attribute the result to a tool) with `content`: the tool
    /// scope matches **and** the marker occurs in the targeted text. A
    /// tool-scoped rule does not fire on an unattributed result — the scope
    /// cannot be confirmed.
    ///
    /// `parsed` is the content deserialized as JSON, supplied by the caller so a
    /// result is parsed once across rules; `None` means it was not valid JSON.
    #[must_use]
    pub fn matches(&self, name: Option<&str>, content: &str, parsed: Option<&Value>) -> bool {
        if let Some(tool) = &self.tool
            && name != Some(tool.as_str())
        {
            return false;
        }
        match &self.path {
            None => self.marker.matches(content),
            Some(path) => parsed
                .and_then(|value| path.get_text(value))
                .is_some_and(|text| self.marker.matches(&text)),
        }
    }

    /// The tool this rule is scoped to, if any.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_ref().map(ToolName::as_str)
    }

    /// The result path this rule is scoped to, if any.
    #[must_use]
    pub fn path(&self) -> Option<&JsonPath> {
        self.path.as_ref()
    }

    /// The forbidden-content matcher this rule fires on.
    #[must_use]
    pub fn marker(&self) -> &Pattern {
        &self.marker
    }
}

impl ValueObject for ResultRule {}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{JsonPath, Pattern, ResultRule, ToolName};

    fn literal(needle: &str) -> ResultRule {
        ResultRule::new(None, Pattern::literal(needle).expect("valid pattern"))
    }

    fn fires(rule: &ResultRule, name: Option<&str>, content: &str) -> bool {
        let parsed = serde_json::from_str::<Value>(content).ok();
        rule.matches(name, content, parsed.as_ref())
    }

    #[test]
    fn an_unscoped_rule_matches_any_result_by_marker() {
        let rule = literal("BEGIN RSA PRIVATE KEY");
        assert!(fires(
            &rule,
            Some("fetch"),
            "leaked BEGIN RSA PRIVATE KEY material"
        ));
        assert!(fires(&rule, None, "begin rsa private key")); // ci, no tool
        assert!(!fires(&rule, Some("fetch"), "all good"));
    }

    #[test]
    fn a_tool_scoped_rule_fires_only_for_its_tool() {
        let rule = ResultRule::new(
            Some(ToolName::new("fetch").expect("valid")),
            Pattern::literal("secret").expect("valid"),
        );
        assert!(fires(&rule, Some("fetch"), "a secret here"));
        assert!(!fires(&rule, Some("search"), "a secret here")); // wrong tool
        assert!(!fires(&rule, None, "a secret here")); // unattributed → not confirmed
    }

    #[test]
    fn a_path_rule_targets_one_field_of_the_result() {
        let rule = literal("evil").with_path(JsonPath::parse("body").expect("valid"));
        assert!(fires(&rule, Some("fetch"), r#"{"body":"evil payload"}"#));
        assert!(!fires(
            &rule,
            Some("fetch"),
            r#"{"meta":"evil","body":"ok"}"#
        ));
        assert!(!fires(&rule, Some("fetch"), "not json")); // unparsable → no fire
    }
}
