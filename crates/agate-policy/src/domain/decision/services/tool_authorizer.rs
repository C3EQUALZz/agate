use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{DenyReason, PolicyDecision, ToolPolicy};

/// Authorizes a tool invocation against a [`ToolPolicy`].
pub struct ToolAuthorizer;

impl ToolAuthorizer {
    /// `Allow` if the policy permits `name`, otherwise `Deny` with a reason
    /// naming the tool.
    #[must_use]
    pub fn authorize(policy: &ToolPolicy, name: &str) -> PolicyDecision {
        if policy.permits(name) {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny(DenyReason::new(format!("tool '{name}' is not permitted")))
        }
    }
}

impl DomainService for ToolAuthorizer {}
