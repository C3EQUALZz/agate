use serde_json::Value;

use super::content_match::ContentMatch;
use super::json_path::JsonPath;
use super::pattern::Pattern;
use super::tool_name::ToolName;
use crate::domain::common::values::ValueObject;

/// A rule that denies a tool call when its arguments match a forbidden
/// [`Pattern`] — the argument-level counterpart to a [`ToolPolicy`] name check.
///
/// The marker is a literal or regex (the shared content matcher). An optional
/// [`ToolName`] scopes the rule to one tool (absent = every tool). An optional
/// [`JsonPath`] scopes the *match* to one field of the parsed arguments
/// (absent = the whole raw argument string): a path rule parses the arguments
/// as JSON and matches the marker against the value at that path, so
/// `{ path = "url", matches = "169\.254" }` screens `args.url` without firing on
/// an unrelated field. A tool call always has a name, so the scope behaves like
/// a plain equality check.
///
/// [`ToolPolicy`]: super::ToolPolicy
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentRule(ContentMatch);

impl ArgumentRule {
    /// Build a whole-arguments rule. `tool` scopes it to a single tool
    /// (`None` = any tool); `marker` is the forbidden content matcher, matched
    /// against the raw argument string.
    #[must_use]
    pub fn new(tool: Option<ToolName>, marker: Pattern) -> Self {
        Self(ContentMatch::new(tool, marker))
    }

    /// Scope the match to one field of the parsed arguments. The marker is then
    /// matched against the value at `path` rather than the whole argument blob.
    #[must_use]
    pub fn with_path(self, path: JsonPath) -> Self {
        Self(self.0.with_path(path))
    }

    /// Whether this rule fires for a call to `name` with `arguments`. `parsed`
    /// is the arguments deserialized as JSON, supplied by the caller so a tool
    /// call is parsed once across rules; `None` means they were not valid JSON.
    #[must_use]
    pub fn matches(&self, name: &str, arguments: &str, parsed: Option<&Value>) -> bool {
        self.0.matches(Some(name), arguments, parsed)
    }

    /// The tool this rule is scoped to, if any.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.0.tool()
    }

    /// The argument path this rule is scoped to, if any.
    #[must_use]
    pub fn path(&self) -> Option<&JsonPath> {
        self.0.path()
    }

    /// The forbidden-content matcher this rule fires on.
    #[must_use]
    pub fn marker(&self) -> &Pattern {
        self.0.marker()
    }
}

impl ValueObject for ArgumentRule {}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{ArgumentRule, JsonPath, Pattern, ToolName};

    fn literal(needle: &str) -> Pattern {
        Pattern::literal(needle).expect("valid pattern")
    }

    fn path(source: &str) -> JsonPath {
        JsonPath::parse(source).expect("valid path")
    }

    /// Match as the inspector would: parse the arguments once, then check.
    fn fires(rule: &ArgumentRule, name: &str, arguments: &str) -> bool {
        let parsed = serde_json::from_str::<Value>(arguments).ok();
        rule.matches(name, arguments, parsed.as_ref())
    }

    #[test]
    fn an_unscoped_rule_matches_any_tool_by_marker() {
        let rule = ArgumentRule::new(None, literal("rm -rf"));
        assert!(fires(&rule, "shell", r#"{"cmd":"rm -rf /"}"#));
        assert!(fires(&rule, "exec", "RM -RF everything")); // case-insensitive
        assert!(!fires(&rule, "shell", r#"{"cmd":"ls"}"#));
    }

    #[test]
    fn a_scoped_rule_only_fires_for_its_tool() {
        let rule = ArgumentRule::new(
            Some(ToolName::new("shell").expect("valid")),
            literal("curl"),
        );
        assert!(fires(&rule, "shell", r#"{"cmd":"curl evil"}"#));
        assert!(!fires(&rule, "search", r#"{"q":"curl"}"#)); // wrong tool, no match
    }

    #[test]
    fn a_path_rule_matches_only_the_targeted_field() {
        let rule = ArgumentRule::new(None, Pattern::regex(r"^https?://169\.254").expect("valid"))
            .with_path(path("url"));
        // The marker is anchored at the start of the `url` value.
        assert!(fires(
            &rule,
            "fetch",
            r#"{"url":"http://169.254.169.254/"}"#
        ));
        // Same text in a different field does not fire the path rule.
        assert!(!fires(
            &rule,
            "fetch",
            r#"{"note":"http://169.254.0.1","url":"https://ok"}"#
        ));
    }

    #[test]
    fn a_path_rule_does_not_fire_on_missing_path_or_non_json() {
        let rule = ArgumentRule::new(None, literal("x")).with_path(path("url"));
        assert!(!fires(&rule, "fetch", r#"{"other":"x"}"#)); // path absent
        assert!(!fires(&rule, "fetch", "not json at all")); // unparsable
    }
}
