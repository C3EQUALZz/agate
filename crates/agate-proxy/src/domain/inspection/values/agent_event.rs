use super::identifiers::{MessageId, ToolCallId};
use super::lifecycle_phase::LifecyclePhase;
use super::opaque_kind::OpaqueKind;
use super::state_mutation::StateMutation;
use crate::domain::common::values::ValueObject;

/// The protocol-agnostic event the inspection core reasons about — the semantic
/// projection of a wire event. An adapter (AG-UI first) folds the protocol's
/// many event types into these few security-relevant categories, while keeping
/// the original raw frame for byte-faithful forwarding on `Allow`.
///
/// `ToolCall` is the *assembled* call (id + name + complete arguments), produced
/// only after the proxy has buffered the streamed argument fragments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// A chunk of streamed assistant text (the DLP / redaction target).
    MessageChunk { message: MessageId, text: String },
    /// A complete tool invocation (the authorization target).
    ToolCall {
        id: ToolCallId,
        name: String,
        arguments: String,
    },
    /// A tool result fed back into the conversation.
    ToolResult { id: ToolCallId, content: String },
    /// A change to the shared state (snapshot or delta).
    StateMutation(StateMutation),
    /// A run/step lifecycle transition.
    Lifecycle(LifecyclePhase),
    /// An event the proxy cannot inspect; pass-through-or-drop.
    Opaque(OpaqueKind),
}

impl ValueObject for AgentEvent {}
