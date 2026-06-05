use std::collections::BTreeSet;

use super::tool_name::ToolName;
use crate::domain::common::values::ValueObject;

/// How tool invocations are authorized.
///
/// - `AllowAll` — no tool restriction (the permissive default).
/// - `Allowlist` — only the listed tools may run; everything else is denied.
/// - `Denylist` — every tool may run except the listed ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolPolicy {
    AllowAll,
    Allowlist(BTreeSet<ToolName>),
    Denylist(BTreeSet<ToolName>),
}

impl ToolPolicy {
    /// Whether `name` is permitted under this policy.
    #[must_use]
    pub fn permits(&self, name: &str) -> bool {
        match self {
            ToolPolicy::AllowAll => true,
            ToolPolicy::Allowlist(set) => contains(set, name),
            ToolPolicy::Denylist(set) => !contains(set, name),
        }
    }
}

fn contains(set: &BTreeSet<ToolName>, name: &str) -> bool {
    set.iter().any(|tool| tool.as_str() == name)
}

impl ValueObject for ToolPolicy {}
