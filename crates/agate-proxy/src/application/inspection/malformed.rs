/// What the proxy does with a **recognized but malformed** response event — an
/// AG-UI event whose `type` the proxy knows, but which is missing or has a blank
/// required field, so a verdict cannot be computed for it.
///
/// Such an event cannot be inspected, yet it is part of an active run, so
/// forwarding it raw lets it bypass the policy entirely. The secure default is
/// therefore [`Terminate`](Self::Terminate) — matching the fail-closed posture
/// the [`Run`](crate::domain::inspection::Run) state machine already takes on a
/// structural protocol violation. Operators who prefer availability over safety
/// can relax it per deployment.
///
/// This governs only malformed **known** events; an unrecognized type, a
/// non-object, or a non-JSON frame carries nothing the proxy inspects and is
/// always forwarded unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MalformedEventMode {
    /// Forward the raw frame unchanged (availability over safety).
    Forward,
    /// Drop the offending frame but let the run continue.
    Drop,
    /// End the run with a `RUN_ERROR` — the secure default.
    #[default]
    Terminate,
}
