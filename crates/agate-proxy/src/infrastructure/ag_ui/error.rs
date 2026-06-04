use std::fmt;

/// A malformed AG-UI event — the wire JSON is not an object, lacks a `type`, or
/// a recognized event is missing a field the proxy needs to inspect it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgUiError {
    NotAnObject,
    MissingType,
    MissingField { event: String, field: &'static str },
}

impl fmt::Display for AgUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgUiError::NotAnObject => write!(f, "AG-UI event is not a JSON object"),
            AgUiError::MissingType => write!(f, "AG-UI event has no `type`"),
            AgUiError::MissingField { event, field } => {
                write!(f, "AG-UI {event} event is missing `{field}`")
            }
        }
    }
}

impl std::error::Error for AgUiError {}
