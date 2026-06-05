use super::secret_pattern::SecretPattern;
use super::tool_policy::ToolPolicy;
use crate::domain::common::values::ValueObject;

/// The complete set of rules a [`PolicyEvaluator`] applies: which tools may run
/// and which text markers must be redacted. Immutable once built; the
/// composition root assembles it from configuration.
///
/// [`PolicyEvaluator`]: super::super::services::PolicyEvaluator
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRuleset {
    tools: ToolPolicy,
    secrets: Vec<SecretPattern>,
}

impl PolicyRuleset {
    #[must_use]
    pub fn new(tools: ToolPolicy, secrets: Vec<SecretPattern>) -> Self {
        Self { tools, secrets }
    }

    /// A ruleset that permits every tool and redacts nothing.
    #[must_use]
    pub fn allow_all() -> Self {
        Self::new(ToolPolicy::AllowAll, Vec::new())
    }

    #[must_use]
    pub fn tools(&self) -> &ToolPolicy {
        &self.tools
    }

    #[must_use]
    pub fn secrets(&self) -> &[SecretPattern] {
        &self.secrets
    }
}

impl ValueObject for PolicyRuleset {}
