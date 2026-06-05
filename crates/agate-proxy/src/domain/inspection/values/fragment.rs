use super::identifiers::{MessageId, ToolCallId};
use super::lifecycle_phase::LifecyclePhase;
use super::opaque_kind::OpaqueKind;
use super::state_mutation::StateMutation;
use crate::domain::common::values::ValueObject;

/// A wire-grained event as it arrives, before the proxy assembles it. This is
/// the **input** to [`Run`](crate::domain::inspection::Run): an adapter (AG-UI
/// first) translates each wire event into one `Fragment`, doing no buffering of
/// its own — assembling a tool call from its fragments is the domain's job.
///
/// Tool calls arrive as three fragment kinds (`Started` → `Args*` → `Ended`);
/// the other categories map one-to-one onto a complete
/// [`AgentEvent`](crate::domain::inspection::AgentEvent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fragment {
    ToolCallStarted { id: ToolCallId, name: String },
    ToolCallArgs { id: ToolCallId, delta: String },
    ToolCallEnded { id: ToolCallId },
    ToolResult { id: ToolCallId, content: String },
    MessageChunk { message: MessageId, text: String },
    StateMutation(StateMutation),
    Lifecycle(LifecyclePhase),
    Opaque(OpaqueKind),
}

impl ValueObject for Fragment {}
