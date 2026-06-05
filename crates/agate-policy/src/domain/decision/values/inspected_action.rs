use crate::domain::common::values::ValueObject;

/// What an agent is attempting, described in the policy context's own terms —
/// only the facts a content/authorization decision needs. The composition root
/// projects the proxy's events onto this; the policy never sees wire formats.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InspectedAction {
    /// A complete tool invocation (the authorization target).
    ToolCall { name: String, arguments: String },
    /// A chunk of emitted assistant text (the redaction target).
    Message { text: String },
    /// An action this context does not govern (lifecycle, opaque, state); it is
    /// allowed without a content decision.
    Other,
}

impl ValueObject for InspectedAction {}
