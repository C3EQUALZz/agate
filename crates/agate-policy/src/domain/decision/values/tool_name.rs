use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// The name of a tool an agent may invoke — the unit a [`ToolPolicy`] allows or
/// denies. Validated non-empty (after trimming) so a ruleset cannot be built
/// around a meaningless entry.
///
/// [`ToolPolicy`]: super::ToolPolicy
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolName(String);

impl ToolName {
    /// Build a tool name, rejecting blank input.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DomainError::Field("tool name must not be blank".into()));
        }
        Ok(Self(name))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for ToolName {}
