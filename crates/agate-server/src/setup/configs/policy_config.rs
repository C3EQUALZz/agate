use std::collections::BTreeSet;

use agate_policy::domain::decision::{PolicyRuleset, SecretPattern, ToolName, ToolPolicy};

/// Policy rules loaded from the environment, turned into a [`PolicyRuleset`].
///
/// - `POLICY_TOOL_ALLOWLIST` — comma-separated tool names; only these may run.
/// - `POLICY_TOOL_DENYLIST` — comma-separated tool names denied (used only when
///   no allowlist is set).
/// - `POLICY_REDACT_PATTERNS` — comma-separated literal markers redacted from
///   emitted text.
///
/// All optional: with none set the policy permits everything and redacts
/// nothing. Blank or invalid entries are dropped.
#[derive(Clone, Debug)]
pub struct PolicyConfig {
    tool_allowlist: Vec<String>,
    tool_denylist: Vec<String>,
    redact_patterns: Vec<String>,
}

impl PolicyConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            tool_allowlist: csv_env("POLICY_TOOL_ALLOWLIST"),
            tool_denylist: csv_env("POLICY_TOOL_DENYLIST"),
            redact_patterns: csv_env("POLICY_REDACT_PATTERNS"),
        }
    }

    /// Assemble the ruleset. An allowlist takes precedence over a denylist; with
    /// neither set, every tool is permitted.
    #[must_use]
    pub fn ruleset(&self) -> PolicyRuleset {
        let tools = if !self.tool_allowlist.is_empty() {
            ToolPolicy::Allowlist(tool_set(&self.tool_allowlist))
        } else if !self.tool_denylist.is_empty() {
            ToolPolicy::Denylist(tool_set(&self.tool_denylist))
        } else {
            ToolPolicy::AllowAll
        };
        let secrets = self
            .redact_patterns
            .iter()
            .filter_map(|pattern| SecretPattern::new(pattern.clone()).ok())
            .collect();
        PolicyRuleset::new(tools, secrets)
    }
}

/// Split a comma-separated env var into trimmed, non-empty entries.
fn csv_env(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn tool_set(names: &[String]) -> BTreeSet<ToolName> {
    names
        .iter()
        .filter_map(|name| ToolName::new(name.clone()).ok())
        .collect()
}
