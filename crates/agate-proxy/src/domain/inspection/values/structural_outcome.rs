use super::agent_event::AgentEvent;
use super::deny_reason::DenyReason;
use super::identifiers::ToolCallId;
use crate::domain::common::values::ValueObject;

/// What feeding one [`Fragment`](super::fragment::Fragment) to a
/// [`Run`](crate::domain::inspection::Run) yields — the **structural** decision
/// the pure domain can make on its own, before any content policy runs.
///
/// `Ready` hands a complete standalone event to the async policy port.
/// `Buffering(id)` means the fragment is a held part of tool call `id` (its
/// `START`/`ARGS`): the proxy buffers the wire frame **keyed by that call** and
/// forwards nothing yet. `ResolvedCall` hands the fully assembled tool call to
/// the policy *and* names the `id` whose buffered frames the verdict governs, so
/// one call's verdict never flushes or drops another's. `Reject` is a
/// pure-domain denial (ordering or budget violation) that needs no policy call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuralOutcome {
    Ready(AgentEvent),
    ResolvedCall { id: ToolCallId, event: AgentEvent },
    Buffering(ToolCallId),
    Reject(DenyReason),
}

impl ValueObject for StructuralOutcome {}
