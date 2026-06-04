use super::agent_event::AgentEvent;
use super::deny_reason::DenyReason;
use crate::domain::common::values::ValueObject;

/// What feeding one [`Fragment`](super::fragment::Fragment) to a
/// [`Run`](crate::domain::inspection::Run) yields — the **structural** decision
/// the pure domain can make on its own, before any content policy runs.
///
/// `Ready` hands a complete semantic event to the async policy port; `Buffering`
/// means the proxy holds the fragment (e.g. mid tool-call) and forwards nothing
/// yet; `Reject` is a pure-domain denial (ordering or budget violation) that
/// needs no policy call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuralOutcome {
    Ready(AgentEvent),
    Buffering,
    Reject(DenyReason),
}

impl ValueObject for StructuralOutcome {}
