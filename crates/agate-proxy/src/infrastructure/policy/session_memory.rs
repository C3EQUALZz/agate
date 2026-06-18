//! Replay-memory adapters for the [`SessionMemory`] port: the disabled default
//! and a process-local ledger with a sliding time-to-live.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::application::common::ports::SessionMemory;
use crate::domain::inspection::{DenyReason, SessionId};

/// The disabled default: remembers nothing, so the policy judges every run
/// afresh (the behavior before per-session memory existed).
pub struct NoopSessionMemory;

#[async_trait]
impl SessionMemory for NoopSessionMemory {
    async fn recall(&self, _session: SessionId, _tool: &str) -> Option<DenyReason> {
        None
    }

    async fn remember(&self, _session: SessionId, _tool: &str, _reason: &DenyReason) {}
}

/// How often the background task evicts sessions whose TTL has lapsed, bounding
/// the map so a long-lived proxy does not accumulate ledgers for sessions that
/// will never return.
const PRUNE_INTERVAL: Duration = Duration::from_mins(1);

/// A tool quarantined within a session, and when the session's ledger expires.
struct SessionLedger {
    /// When this session's quarantine lapses if it sees no further activity.
    expires_at: Instant,
    /// Denied tool name → the reason it was first denied (cited on replay).
    denied_tools: HashMap<String, DenyReason>,
}

/// A process-local replay ledger with a sliding time-to-live per session.
///
/// Each recall or denial refreshes its session's expiry; a session idle for
/// longer than the TTL is forgotten (its tools are no longer quarantined).
/// State lives only in this process — front several replicas with a shared
/// backend (e.g. Redis) when a session may span instances.
pub struct InMemorySessionMemory {
    ttl: Duration,
    sessions: Arc<Mutex<HashMap<SessionId, SessionLedger>>>,
}

impl InMemorySessionMemory {
    /// A ledger that forgets a session after `ttl` of inactivity. Spawns a
    /// background pruner, so it must be constructed within a Tokio runtime.
    #[must_use]
    pub fn new(ttl: Duration) -> Self {
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        spawn_pruner(&sessions);
        Self { ttl, sessions }
    }

    /// Lock the map, recovering the inner guard if a previous holder panicked —
    /// a poisoned lock must not take down the data plane (the worst case is a
    /// stale ledger entry, never a wrong allow).
    fn lock(&self) -> MutexGuard<'_, HashMap<SessionId, SessionLedger>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl SessionMemory for InMemorySessionMemory {
    async fn recall(&self, session: SessionId, tool: &str) -> Option<DenyReason> {
        let now = Instant::now();
        let mut sessions = self.lock();
        let ledger = sessions.get_mut(&session)?;
        if ledger.expires_at <= now {
            // Lapsed since its last touch — forget it rather than honoring it.
            sessions.remove(&session);
            return None;
        }
        // Sliding TTL: activity keeps an in-use session warm.
        ledger.expires_at = now + self.ttl;
        ledger.denied_tools.get(tool).cloned()
    }

    async fn remember(&self, session: SessionId, tool: &str, reason: &DenyReason) {
        let now = Instant::now();
        let mut sessions = self.lock();
        let ledger = sessions.entry(session).or_insert_with(|| SessionLedger {
            expires_at: now + self.ttl,
            denied_tools: HashMap::new(),
        });
        ledger.expires_at = now + self.ttl;
        // Keep the first reason a tool was denied for; later denials don't
        // overwrite the original cause (and don't re-allocate the key).
        if !ledger.denied_tools.contains_key(tool) {
            ledger.denied_tools.insert(tool.to_owned(), reason.clone());
        }
    }
}

/// Periodically drop ledgers for sessions whose TTL has lapsed, so the map
/// tracks only currently-active sessions. Holds a [`Weak`] so the task ends once
/// the ledger is dropped.
fn spawn_pruner(sessions: &Arc<Mutex<HashMap<SessionId, SessionLedger>>>) {
    let weak = Arc::downgrade(sessions);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(PRUNE_INTERVAL);
        loop {
            tick.tick().await;
            let Some(sessions) = Weak::upgrade(&weak) else {
                return;
            };
            let now = Instant::now();
            let mut guard = sessions.lock().unwrap_or_else(PoisonError::into_inner);
            guard.retain(|_, ledger| ledger.expires_at > now);
            guard.shrink_to_fit();
        }
    });
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{Duration, InMemorySessionMemory, NoopSessionMemory, SessionMemory};
    use crate::domain::inspection::{DenyReason, SessionId};

    fn session() -> SessionId {
        SessionId::new(Uuid::nil())
    }

    fn reason() -> DenyReason {
        DenyReason::new("tool not allowed")
    }

    #[tokio::test]
    async fn the_noop_ledger_never_remembers() {
        let memory = NoopSessionMemory;
        memory.remember(session(), "delete_file", &reason()).await;
        assert!(memory.recall(session(), "delete_file").await.is_none());
    }

    #[tokio::test]
    async fn a_remembered_tool_is_recalled_within_the_session() {
        let memory = InMemorySessionMemory::new(Duration::from_hours(1));
        memory.remember(session(), "delete_file", &reason()).await;
        assert_eq!(
            memory.recall(session(), "delete_file").await,
            Some(reason())
        );
        // A different tool in the same session is not quarantined.
        assert!(memory.recall(session(), "search").await.is_none());
    }

    #[tokio::test]
    async fn a_different_session_is_not_quarantined() {
        let memory = InMemorySessionMemory::new(Duration::from_hours(1));
        memory.remember(session(), "delete_file", &reason()).await;
        let other = SessionId::new(Uuid::from_u128(1));
        assert!(memory.recall(other, "delete_file").await.is_none());
    }

    #[tokio::test]
    async fn a_lapsed_session_is_forgotten() {
        // A zero TTL expires the entry the instant it is written, so the next
        // recall (a later `Instant::now`) finds it lapsed and evicts it.
        let memory = InMemorySessionMemory::new(Duration::ZERO);
        memory.remember(session(), "delete_file", &reason()).await;
        assert!(memory.recall(session(), "delete_file").await.is_none());
    }

    #[tokio::test]
    async fn the_first_denial_reason_is_kept() {
        let memory = InMemorySessionMemory::new(Duration::from_hours(1));
        memory
            .remember(session(), "fetch", &DenyReason::new("first"))
            .await;
        memory
            .remember(session(), "fetch", &DenyReason::new("second"))
            .await;
        assert_eq!(
            memory.recall(session(), "fetch").await,
            Some(DenyReason::new("first"))
        );
    }
}
