use crate::domain::decision::{InspectedAction, PolicyDecision, PolicyEvaluator, PolicyRuleset};

/// Decides the policy verdict for an inspected action against a fixed ruleset.
///
/// The ruleset is captured at construction (assembled from configuration by the
/// composition root), so deciding is a pure, synchronous lookup — the async
/// boundary the proxy needs lives in the adapter that wraps this service.
pub struct PolicyService {
    ruleset: PolicyRuleset,
}

impl PolicyService {
    #[must_use]
    pub fn new(ruleset: PolicyRuleset) -> Self {
        Self { ruleset }
    }

    #[must_use]
    pub fn decide(&self, action: &InspectedAction) -> PolicyDecision {
        PolicyEvaluator::evaluate(&self.ruleset, action)
    }
}
