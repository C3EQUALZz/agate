//! The grouped stream-guard settings the composition root hands to the
//! inspection pipeline.

use crate::application::inspection::{MalformedEventMode, ResponseBudget};
use crate::domain::inspection::Budgets;

/// How a response stream is guarded during inspection — the configuration
/// knobs, grouped so they travel together from the composition root to
/// [`inspect_stream`](crate::presentation::inspect_stream) instead of as a
/// parameter list.
///
/// Per-request identity (the [`InspectionContext`]) deliberately stays
/// separate: these settings are fixed at startup, the context is minted per
/// run.
///
/// [`InspectionContext`]: super::InspectionContext
#[derive(Clone, Copy, Debug, Default)]
pub struct InspectionSettings {
    /// Structural budgets the `Run` state machine enforces (tool-call args,
    /// state size, open tool calls).
    pub budgets: Budgets,
    /// What to do with a recognized-but-malformed event.
    pub malformed_mode: MalformedEventMode,
    /// Per-run ceiling on the response stream (events / bytes).
    pub response_budget: ResponseBudget,
    /// Maximum bytes buffered for a single not-yet-complete SSE event (`0` =
    /// unlimited). Guards the gap the [`response_budget`] cannot see: that budget
    /// is charged per *decoded* event, so an upstream streaming a frame that
    /// never terminates would grow the decoder's buffer without bound. Crossing
    /// this ends the run with a `RUN_ERROR` (fail closed).
    ///
    /// [`response_budget`]: Self::response_budget
    pub max_frame_bytes: usize,
}
