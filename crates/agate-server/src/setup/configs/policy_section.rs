use agate_policy::domain::common::errors::DomainError;
use agate_policy::domain::decision::{ArgumentRule, JsonPath, Pattern, ToolName};
use serde::{Deserialize, Serialize};

/// `[policy]` — content/authorization rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicySection {
    pub tools: ToolsSection,
    /// Literal markers redacted from emitted text (case-insensitive).
    pub redact: Vec<String>,
    /// Regex markers redacted from emitted text (full `regex` syntax; add `(?i)`
    /// for case-insensitivity). An invalid expression aborts startup.
    pub redact_regex: Vec<String>,
    /// What to do when a policy decision cannot be made in time: `open`
    /// (forward) or `closed` (block). Defaults to the secure `closed`.
    pub fail_mode: PolicyFailMode,
    /// Deadline for a single policy decision, in milliseconds.
    pub decision_timeout_ms: u64,
    /// What to do with a recognized response event that is malformed (a known
    /// type with a missing/blank required field, so it cannot be inspected):
    /// `forward`, `drop`, or `terminate`. Defaults to the secure `terminate`.
    pub on_malformed_event: MalformedMode,
}

impl PolicySection {
    /// Fail fast on a zeroed decision deadline.
    pub fn validate(&self) -> Result<(), String> {
        if self.decision_timeout_ms == 0 {
            return Err("policy.decision_timeout_ms must be greater than 0".into());
        }
        Ok(())
    }
}

impl Default for PolicySection {
    fn default() -> Self {
        Self {
            tools: ToolsSection::default(),
            redact: Vec::new(),
            redact_regex: Vec::new(),
            fail_mode: PolicyFailMode::default(),
            decision_timeout_ms: 5000,
            on_malformed_event: MalformedMode::default(),
        }
    }
}

/// What to do with a recognized-but-malformed response event — the data plane's
/// fail-open / fail-closed knob for events it cannot inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MalformedMode {
    /// Forward the raw frame (availability over safety).
    Forward,
    /// Drop the frame, continue the run.
    Drop,
    /// End the run with a `RUN_ERROR` — the secure default.
    #[default]
    Terminate,
}

/// Behavior when a policy decision times out — the fail-open / fail-closed knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyFailMode {
    /// Forward the event (availability over safety).
    Open,
    /// Block the run (safety over availability) — the secure default.
    #[default]
    Closed,
}

/// `[policy.tools]` — tool-call authorization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsSection {
    pub mode: ToolMode,
    /// Tool names governed by `mode` (ignored when `mode = "allow-all"`).
    pub names: Vec<String>,
    /// Argument-level deny rules: a permitted tool call is still blocked when
    /// its arguments match one of these markers. Configured as
    /// `[[policy.tools.deny_arguments]]` tables.
    pub deny_arguments: Vec<ArgumentRuleConfig>,
}

/// One `[[policy.tools.deny_arguments]]` entry: a marker forbidden in tool
/// arguments, optionally scoped to a single tool. Provide exactly one of
/// `contains` (a case-insensitive literal) or `matches` (a regex).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArgumentRuleConfig {
    /// The tool this rule applies to; omit (or leave blank) to apply to any tool.
    pub tool: Option<String>,
    /// A dotted path into the parsed arguments (`url`, `config.endpoint`) to
    /// match against. Omit to match the whole raw argument string. With a path,
    /// the marker is matched against just that field's value, so it cannot fire
    /// on an unrelated field carrying the same text.
    pub path: Option<String>,
    /// A literal forbidden in the arguments, folded ASCII-case-insensitively
    /// (not Unicode).
    pub contains: Option<String>,
    /// A regex forbidden in the arguments (full `regex` syntax; prefix `(?i)`
    /// for case-insensitivity). Matched against the raw argument JSON string,
    /// or against the `path` field's value when `path` is set.
    pub matches: Option<String>,
}

impl ArgumentRuleConfig {
    pub(super) fn to_rule(&self) -> Result<ArgumentRule, DomainError> {
        let tool = match self.tool.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => Some(ToolName::new(name)?),
            _ => None,
        };
        let marker = match (&self.contains, &self.matches) {
            (Some(literal), None) => Pattern::literal(literal)?,
            (None, Some(regex)) => Pattern::regex(regex)?,
            (Some(_), Some(_)) => {
                return Err(DomainError::Field(
                    "a deny_arguments rule sets exactly one of `contains` or `matches`, not both"
                        .into(),
                ));
            }
            (None, None) => {
                return Err(DomainError::Field(
                    "a deny_arguments rule needs `contains` or `matches`".into(),
                ));
            }
        };
        let rule = ArgumentRule::new(tool, marker);
        match self.path.as_deref().map(str::trim) {
            Some(path) if !path.is_empty() => Ok(rule.with_path(JsonPath::parse(path)?)),
            _ => Ok(rule),
        }
    }
}

/// How tool invocations are authorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolMode {
    /// No tool restriction (the permissive default).
    #[default]
    AllowAll,
    /// Only the tools in `names` may run; everything else is denied.
    Allowlist,
    /// Every tool may run except the ones in `names`.
    Denylist,
}
