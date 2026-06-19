use crate::domain::inspection::{AgentEvent, DenyReason, ToolCallId};

/// What the proxy should do with the wire frame(s) after inspecting a fragment.
///
/// The presentation layer owns the raw frames and maps these onto the transport.
/// Standalone events use [`Forward`](Self::Forward) (flush the loose buffer, then
/// relay this frame), [`ForwardTransformed`](Self::ForwardTransformed) (emit a
/// re-encoded event), [`Drop`](Self::Drop) (discard this frame), or
/// [`Terminate`](Self::Terminate) (close the stream with a `RUN_ERROR`).
///
/// Tool-call frames are buffered **per call id**, so a verdict on one call never
/// flushes or drops another's held frames: [`Hold`](Self::Hold) buffers a
/// `START`/`ARGS` frame under its id, [`FlushCall`](Self::FlushCall) relays that
/// id's buffered frames (the call was allowed), and [`DropCall`](Self::DropCall)
/// discards them (the call was denied).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InspectionAction {
    Forward,
    ForwardTransformed(AgentEvent),
    Hold(ToolCallId),
    FlushCall(ToolCallId),
    DropCall(ToolCallId, DenyReason),
    Drop(DenyReason),
    Terminate(DenyReason),
}
