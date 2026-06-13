use super::argument_rule::ArgumentRule;
use super::pattern::Pattern;
use super::result_rule::ResultRule;
use super::tool_policy::ToolPolicy;
use crate::domain::common::values::ValueObject;

/// The complete set of rules a [`PolicyEvaluator`] applies: which tools may run,
/// which tool arguments are forbidden, which tool results are forbidden, and
/// which text markers must be redacted. Immutable once built; the composition
/// root assembles it from configuration.
///
/// [`PolicyEvaluator`]: super::super::services::PolicyEvaluator
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PolicyRuleset {
    tools: ToolPolicy,
    argument_rules: Vec<ArgumentRule>,
    result_rules: Vec<ResultRule>,
    secrets: Vec<Pattern>,
}

impl PolicyRuleset {
    #[must_use]
    pub fn new(
        tools: ToolPolicy,
        argument_rules: Vec<ArgumentRule>,
        secrets: Vec<Pattern>,
    ) -> Self {
        Self {
            tools,
            argument_rules,
            result_rules: Vec::new(),
            secrets,
        }
    }

    /// Add the result deny rules (builder-style, so call sites that predate
    /// result rules stay unchanged).
    #[must_use]
    pub fn with_result_rules(mut self, result_rules: Vec<ResultRule>) -> Self {
        self.result_rules = result_rules;
        self
    }

    /// A ruleset that permits every tool and redacts nothing.
    #[must_use]
    pub fn allow_all() -> Self {
        Self::new(ToolPolicy::AllowAll, Vec::new(), Vec::new())
    }

    #[must_use]
    pub fn tools(&self) -> &ToolPolicy {
        &self.tools
    }

    /// The argument-level rules applied to a permitted tool call.
    #[must_use]
    pub fn argument_rules(&self) -> &[ArgumentRule] {
        &self.argument_rules
    }

    /// The result-level deny rules applied to a tool result.
    #[must_use]
    pub fn result_rules(&self) -> &[ResultRule] {
        &self.result_rules
    }

    #[must_use]
    pub fn secrets(&self) -> &[Pattern] {
        &self.secrets
    }
}

impl ValueObject for PolicyRuleset {}
