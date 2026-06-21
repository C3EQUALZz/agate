use crate::domain::common::errors::{DomainError, ToolNameError};
use crate::domain::common::values::ValueObject;

/// The name of a tool an agent may invoke — the unit a [`ToolPolicy`] allows or
/// denies. Validated non-empty (after trimming) so a ruleset cannot be built
/// around a meaningless entry.
///
/// [`ToolPolicy`]: super::ToolPolicy
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolName(String);

impl ToolName {
    /// Build a tool name, rejecting blank input. The name is trimmed so a padded
    /// config entry (`" search "`) still matches the tool it names — parse into
    /// the normalized value, don't merely validate.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        let normalized = name.trim();
        if normalized.is_empty() {
            return Err(ToolNameError::Blank.into());
        }
        Ok(Self(normalized.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for ToolName {}
