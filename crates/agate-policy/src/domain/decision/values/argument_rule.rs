use serde_json::Value;

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
/// an unrelated field.
///
/// [`ToolPolicy`]: super::ToolPolicy
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgumentRule {
    tool: Option<ToolName>,
    path: Option<JsonPath>,
    marker: Pattern,
}

impl ArgumentRule {
    /// Build a whole-arguments rule. `tool` scopes it to a single tool
    /// (`None` = any tool); `marker` is the forbidden content matcher, matched
    /// against the raw argument string.
    #[must_use]
    pub fn new(tool: Option<ToolName>, marker: Pattern) -> Self {
        Self {
            tool,
            path: None,
            marker,
        }
    }

    /// Scope the match to one field of the parsed arguments. The marker is then
    /// matched against the value at `path` rather than the whole argument blob.
    #[must_use]
    pub fn with_path(mut self, path: JsonPath) -> Self {
        self.path = Some(path);
        self
    }

    /// Whether this rule fires for a call to `name` with `arguments`: the tool
    /// scope matches (or is unscoped) **and** the marker occurs in the targeted
    /// text — the value at the rule's path, or the whole argument string when
    /// the rule has no path.
    ///
    /// `parsed` is the arguments deserialized as JSON, supplied by the caller so
    /// a tool call is parsed once and shared across rules; `None` means the
    /// arguments were not valid JSON. A path rule on absent/unparsable arguments
    /// does not fire — there is nothing at that path to match.
    #[must_use]
    pub fn matches(&self, name: &str, arguments: &str, parsed: Option<&Value>) -> bool {
        if let Some(tool) = &self.tool
            && tool.as_str() != name
        {
            return false;
        }
        match &self.path {
            None => self.marker.matches(arguments),
            Some(path) => parsed
                .and_then(|value| path.get(value))
                .is_some_and(|node| self.marker.matches(&node_text(node))),
        }
    }

    /// The tool this rule is scoped to, if any.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_ref().map(ToolName::as_str)
    }

    /// The argument path this rule is scoped to, if any.
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

impl ValueObject for ArgumentRule {}

/// The text a marker matches against for a path node: the inner string for a
/// JSON string (so `"http://x"` matches as `http://x`), or the node's compact
/// JSON otherwise (so a number/object/array can still be screened).
fn node_text(node: &Value) -> String {
    match node {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

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
    fn a_regex_marker_matches_structured_arguments() {
        let rule = ArgumentRule::new(
            None,
            Pattern::regex(r#""url"\s*:\s*"https?://169\.254"#).expect("valid"),
        );
        assert!(fires(
            &rule,
            "fetch",
            r#"{"url":"http://169.254.169.254/"}"#
        ));
        assert!(!fires(&rule, "fetch", r#"{"url":"https://example.com"}"#));
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
    fn a_path_rule_on_a_nested_field_resolves_the_path() {
        let rule = ArgumentRule::new(None, literal("evil")).with_path(path("config.endpoint"));
        assert!(fires(
            &rule,
            "fetch",
            r#"{"config":{"endpoint":"evil.example"}}"#
        ));
        assert!(!fires(
            &rule,
            "fetch",
            r#"{"config":{"endpoint":"ok.example"}}"#
        ));
    }

    #[test]
    fn a_path_rule_does_not_fire_on_missing_path_or_non_json() {
        let rule = ArgumentRule::new(None, literal("x")).with_path(path("url"));
        assert!(!fires(&rule, "fetch", r#"{"other":"x"}"#)); // path absent
        assert!(!fires(&rule, "fetch", "not json at all")); // unparsable
    }

    #[test]
    fn a_path_rule_can_match_a_non_string_node_by_its_json() {
        let rule = ArgumentRule::new(None, literal("true")).with_path(path("danger"));
        assert!(fires(&rule, "tool", r#"{"danger":true}"#));
    }
}
