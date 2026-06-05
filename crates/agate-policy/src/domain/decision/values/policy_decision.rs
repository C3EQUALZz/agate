use super::deny_reason::DenyReason;
use crate::domain::common::values::ValueObject;

/// The content/authorization verdict for one action. Deliberately narrower than
/// the proxy's structural verdict: this context only allows, denies, or rewrites
/// emitted text — it never reasons about buffering or stream termination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Permit the action unchanged.
    Allow,
    /// Block the action.
    Deny(DenyReason),
    /// Permit the action, but with the emitted text replaced by this redacted
    /// form (only produced for [`InspectedAction::Message`]).
    ///
    /// [`InspectedAction::Message`]: super::InspectedAction::Message
    RedactText(String),
}

impl ValueObject for PolicyDecision {}
