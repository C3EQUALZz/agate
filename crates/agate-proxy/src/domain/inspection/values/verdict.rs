use super::deny_reason::DenyReason;
use crate::domain::common::values::ValueObject;

/// The decision the proxy reaches for one inspected event.
///
/// Generic over the event payload `E` so the inspection core stays
/// protocol-agnostic: `Transform` carries a replacement of whatever the
/// adapter's event type happens to be, never a wire-specific one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict<E> {
    /// Forward the event unchanged.
    Allow,
    /// Block this event; on the response leg, surface it as a `RUN_ERROR`.
    Deny(DenyReason),
    /// Forward a modified event in place of the original (e.g. redacted text).
    Transform(E),
    /// Not enough has arrived to decide yet (e.g. mid tool-call); keep buffering.
    Buffer,
    /// End the whole run/stream.
    Terminate(DenyReason),
}

impl<E> Verdict<E> {
    /// Whether the event is forwarded (possibly modified) rather than stopped.
    pub fn forwards(&self) -> bool {
        matches!(self, Verdict::Allow | Verdict::Transform(_))
    }

    /// Whether this verdict stops the flow (block or terminate).
    pub fn stops(&self) -> bool {
        matches!(self, Verdict::Deny(_) | Verdict::Terminate(_))
    }
}

impl<E: Clone + PartialEq + Eq> ValueObject for Verdict<E> {}
