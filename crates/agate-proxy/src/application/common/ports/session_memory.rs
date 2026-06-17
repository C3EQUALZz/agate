use async_trait::async_trait;

use crate::domain::inspection::{DenyReason, SessionId};

/// Cross-run replay memory for a conversation (an AG-UI session).
///
/// When a tool is denied in one run, the ledger remembers it so the same tool
/// cannot be retried — with whatever arguments — in a later run of the same
/// session. The unit quarantined is the tool *name*: a tool that triggered any
/// denial (name, arguments, SSRF, or result) is refused for the rest of the
/// session regardless of how the agent varies the call.
///
/// This is defense-in-depth layered *over* the stateless policy: it only ever
/// *adds* a denial. A backend that is unavailable (or the no-op default)
/// degrades to "no memory" — the base policy still judges every event — never to
/// allowing something the policy would deny.
#[async_trait]
pub trait SessionMemory: Send + Sync {
    /// The reason `tool` was denied earlier in `session`, if it is quarantined;
    /// `None` otherwise (including when memory is disabled).
    async fn recall(&self, session: SessionId, tool: &str) -> Option<DenyReason>;

    /// Quarantine `tool` for the rest of `session` after a denial, keeping the
    /// `reason` so a later replay can cite the original cause.
    async fn remember(&self, session: SessionId, tool: &str, reason: &DenyReason);
}
