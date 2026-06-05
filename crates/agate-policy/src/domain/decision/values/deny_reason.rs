use crate::domain::common::values::ValueObject;

/// Why an action was denied. Surfaced to the proxy (which turns it into a
/// client-visible `RUN_ERROR` or drops the event) and recorded in the audit
/// trail.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DenyReason(String);

impl DenyReason {
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for DenyReason {}
