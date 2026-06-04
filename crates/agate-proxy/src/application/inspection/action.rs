use crate::domain::inspection::{AgentEvent, DenyReason};

/// What the proxy should do with the wire frame(s) after inspecting a fragment.
///
/// The presentation layer owns the raw frames and maps these onto the
/// transport: it buffers the frame while [`Hold`](Self::Hold), flushes the held
/// frames on [`Forward`](Self::Forward), discards them and emits a re-encoded
/// event on [`ForwardTransformed`](Self::ForwardTransformed), discards them on
/// [`Drop`](Self::Drop) (optionally surfacing a `RUN_ERROR`), and closes the
/// stream on [`Terminate`](Self::Terminate).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InspectionAction {
    Forward,
    ForwardTransformed(AgentEvent),
    Hold,
    Drop(DenyReason),
    Terminate(DenyReason),
}
