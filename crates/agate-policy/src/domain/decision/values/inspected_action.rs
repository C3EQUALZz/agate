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
    /// The content a tool returned — the indirect-injection / exfiltration
    /// surface. Redacted like emitted text (secrets masked before the client),
    /// and checked against result deny rules. `name` is the tool the result
    /// belongs to (`None` if the proxy never saw the call start), used to scope
    /// a result rule to one tool.
    ToolResult {
        name: Option<String>,
        content: String,
    },
    /// A mutation of shared agent/client state, carrying its raw JSON payload.
    /// Its structure cannot be rewritten in place, so a secret found here is
    /// denied rather than leaked.
    StateMutation { content: String },
    /// An action this context does not govern (lifecycle, opaque); it is allowed
    /// without a content decision.
    Other,
}

impl ValueObject for InspectedAction {}
