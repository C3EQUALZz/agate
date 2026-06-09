use crate::domain::common::values::ValueObject;

/// Why an event was denied or a run terminated. Carried in the verdict so it can
/// be audited and surfaced to the client (e.g. as a `RUN_ERROR` message).
#[derive(Clone, Debug, PartialEq, Eq)]
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
