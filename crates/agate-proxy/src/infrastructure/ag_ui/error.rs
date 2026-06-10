use std::fmt;

/// A malformed AG-UI event — the wire JSON is not an object, lacks a `type`, or
/// a recognized event is missing a field the proxy needs to inspect it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgUiError {
    NotAnObject,
    MissingType,
    MissingField {
        event: String,
        field: &'static str,
    },
    /// A correlating id field is present but blank, so it could never match
    /// its sibling frames.
    BlankField {
        event: String,
        field: &'static str,
    },
    /// The request body is not valid `RunAgentInput` JSON.
    MalformedRequest,
}

impl AgUiError {
    /// Whether this is a **recognized** event type with a missing or blank
    /// required field — i.e. the proxy knows the `type` but cannot inspect the
    /// event. A non-object, a missing/unknown `type`, or a malformed request
    /// body is *not* a malformed known event (it carries nothing to inspect),
    /// so this returns `false` for them.
    #[must_use]
    pub fn is_malformed_known(&self) -> bool {
        matches!(self, Self::MissingField { .. } | Self::BlankField { .. })
    }
}

impl fmt::Display for AgUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgUiError::NotAnObject => write!(f, "AG-UI event is not a JSON object"),
            AgUiError::MissingType => write!(f, "AG-UI event has no `type`"),
            AgUiError::MissingField { event, field } => {
                write!(f, "AG-UI {event} event is missing `{field}`")
            }
            AgUiError::BlankField { event, field } => {
                write!(f, "AG-UI {event} event has a blank `{field}`")
            }
            AgUiError::MalformedRequest => {
                write!(f, "request body is not valid RunAgentInput JSON")
            }
        }
    }
}

impl std::error::Error for AgUiError {}
