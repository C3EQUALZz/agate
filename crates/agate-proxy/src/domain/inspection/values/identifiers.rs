use std::fmt;

use uuid::Uuid;

use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// One agent execution (an AG-UI `runId`): the unit a verdict applies within.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RunId(Uuid);

impl RunId {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ValueObject for RunId {}

/// A conversation spanning multiple runs (an AG-UI `threadId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ValueObject for SessionId {}

/// Correlates the start, streamed arguments, and result of one tool call.
///
/// Carries the protocol's own opaque id (a string, not a UUID) so the proxy can
/// match `TOOL_CALL_START` / `TOOL_CALL_ARGS` / `TOOL_CALL_END` frames. Opaque,
/// but never blank — a blank id could never match its sibling frames.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolCallId(String);

impl ToolCallId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        non_blank(value.into(), "a tool call id must not be blank").map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for ToolCallId {}

/// Correlates the streamed fragments of one assistant text message. Opaque,
/// but never blank — a blank id could never correlate the fragments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MessageId(String);

impl MessageId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        non_blank(value.into(), "a message id must not be blank").map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for MessageId {}

fn non_blank(value: String, message: &str) -> Result<String, DomainError> {
    if value.trim().is_empty() {
        return Err(DomainError::Field(message.to_owned()));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{MessageId, RunId, ToolCallId};

    #[test]
    fn blank_protocol_ids_are_rejected() {
        assert!(ToolCallId::new("  ").is_err());
        assert!(MessageId::new("").is_err());
    }

    #[test]
    fn valid_protocol_ids_round_trip() {
        assert_eq!(ToolCallId::new("c1").expect("valid").as_str(), "c1");
        assert_eq!(MessageId::new("m1").expect("valid").as_str(), "m1");
    }

    #[test]
    fn uuid_ids_display_as_the_uuid() {
        let id = Uuid::nil();
        assert_eq!(RunId::new(id).to_string(), id.to_string());
        assert_eq!(RunId::new(id).value(), id);
    }
}
