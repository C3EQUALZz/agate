use std::collections::BTreeSet;

use agate_policy::domain::common::errors::DomainError;
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

    /// Assemble the ruleset. Exactly one tool list may be set; with neither, every
    /// tool is permitted.
    ///
    /// Fails on the first invalid entry rather than dropping it: a typo in a
    /// denylist or redaction marker would otherwise silently weaken enforcement,
    /// so it must abort startup instead. Likewise, an allowlist and a denylist
    /// together are contradictory (which wins?) and rejected rather than silently
    /// resolved.
    pub fn ruleset(&self) -> Result<PolicyRuleset, DomainError> {
        if !self.tool_allowlist.is_empty() && !self.tool_denylist.is_empty() {
            return Err(DomainError::Field(
                "POLICY_TOOL_ALLOWLIST and POLICY_TOOL_DENYLIST are mutually exclusive".into(),
            ));
        }

        let tools = if !self.tool_allowlist.is_empty() {
            ToolPolicy::Allowlist(tool_set(&self.tool_allowlist)?)
        } else if !self.tool_denylist.is_empty() {
            ToolPolicy::Denylist(tool_set(&self.tool_denylist)?)
        } else {
            ToolPolicy::AllowAll
        };
        let secrets = self
            .redact_patterns
            .iter()
            .map(|pattern| SecretPattern::new(pattern.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PolicyRuleset::new(tools, secrets))
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

fn tool_set(names: &[String]) -> Result<BTreeSet<ToolName>, DomainError> {
    names
        .iter()
        .map(|name| ToolName::new(name.clone()))
        .collect()
}
