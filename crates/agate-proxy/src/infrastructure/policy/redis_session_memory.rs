//! Redis-backed [`SessionMemory`]: a cross-run replay ledger shared across
//! proxy instances, so a tool denied on one replica stays quarantined for the
//! session on every replica (and across restarts).

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use tokio::sync::OnceCell;
use tracing::warn;

use crate::application::common::ports::SessionMemory;
use crate::domain::inspection::{DenyReason, SessionId};

/// Bound how long a Redis operation may block the data plane. Kept short so a
/// slow or unreachable Redis degrades to "no memory" quickly rather than
/// stalling a run on the hot path — the fail-open contract must be *fast*.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECT_RETRIES: usize = 1;
/// After a failed connect, fast-fail (no connect attempt) for this long, so a
/// Redis that is down does not make every event on the hot path pay the connect
/// timeout. One attempt per window probes for recovery.
const CONNECT_COOLDOWN: Duration = Duration::from_secs(5);

/// A Redis-backed replay ledger. Each session is one hash
/// (`agate:session:{uuid}`) whose fields map a denied tool name to the first
/// reason it was denied; a sliding TTL on the hash lets Redis evict idle
/// sessions natively (no background pruner).
///
/// **Fail-open by the [`SessionMemory`] contract.** Any Redis error degrades to
/// "no memory" — `recall` returns `None`, `remember` is a no-op — both logged,
/// never a wrong allow nor a panic. The stateless policy still judges every
/// event, so an unreachable Redis loses only the cross-run quarantine, not the
/// base authorization.
pub struct RedisSessionMemory {
    client: redis::Client,
    /// Lazily established, auto-reconnecting multiplexed connection. Once built
    /// it is cached and reused (and reconnects itself on a blip); building it is
    /// the only slow step, gated by `cooldown_until`.
    connection: OnceCell<ConnectionManager>,
    /// When a failed connect attempt's cooldown ends. While set in the future,
    /// `connection` fast-fails (returns `None`) without a connect attempt, so a
    /// down Redis costs at most one connect timeout per [`CONNECT_COOLDOWN`].
    cooldown_until: Mutex<Option<Instant>>,
    /// Inactivity TTL in whole seconds (Redis `EXPIRE` granularity), at least 1.
    ttl_secs: i64,
}

impl RedisSessionMemory {
    /// Build the ledger for `url` with an inactivity `ttl`. Only the URL is
    /// parsed here (no connection yet), so a malformed URL fails fast at startup.
    pub fn new(url: &str, ttl: Duration) -> redis::RedisResult<Self> {
        Ok(Self {
            client: redis::Client::open(url)?,
            connection: OnceCell::new(),
            cooldown_until: Mutex::new(None),
            ttl_secs: i64::try_from(ttl.as_secs().max(1)).unwrap_or(i64::MAX),
        })
    }

    /// The shared connection, established and cached on first use; `None` when
    /// Redis is currently unreachable (the caller then degrades to no memory).
    /// A failed connect starts a cooldown so repeated calls on a down Redis
    /// fast-fail instead of each paying the connect timeout.
    async fn connection(&self) -> Option<ConnectionManager> {
        if let Some(conn) = self.connection.get() {
            return Some(conn.clone());
        }
        // Not connected yet: skip the attempt while a recent failure's cooldown
        // is still in effect (lock not held across the await below).
        if self.cooldown(|until| (*until).is_some_and(|deadline| Instant::now() < deadline)) {
            return None;
        }
        match self
            .connection
            .get_or_try_init(|| {
                let config = ConnectionManagerConfig::new()
                    .set_number_of_retries(CONNECT_RETRIES)
                    .set_connection_timeout(CONNECT_TIMEOUT)
                    .set_response_timeout(RESPONSE_TIMEOUT);
                ConnectionManager::new_with_config(self.client.clone(), config)
            })
            .await
        {
            Ok(conn) => Some(conn.clone()),
            Err(error) => {
                warn!(%error, "session-memory: cannot reach Redis; treating as no memory");
                self.cooldown(|until| {
                    *until = Some(Instant::now() + CONNECT_COOLDOWN);
                    true
                });
                None
            }
        }
    }

    /// Run `f` against the cooldown deadline under its lock (never held across an
    /// await), recovering a poisoned lock. Returns whatever `f` returns.
    fn cooldown(&self, f: impl FnOnce(&mut Option<Instant>) -> bool) -> bool {
        let mut guard = self
            .cooldown_until
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }

    fn key(session: SessionId) -> String {
        format!("agate:session:{}", session.value())
    }
}

#[async_trait]
impl SessionMemory for RedisSessionMemory {
    async fn recall(&self, session: SessionId, tool: &str) -> Option<DenyReason> {
        let mut conn = self.connection().await?;
        let key = Self::key(session);
        let reason: Option<String> = conn
            .hget(&key, tool)
            .await
            .map_err(|error| warn!(%error, "session-memory recall failed; treating as no memory"))
            .ok()
            .flatten();
        if reason.is_some() {
            // Sliding TTL: activity keeps an in-use session warm (best-effort).
            let _: Result<(), redis::RedisError> = conn.expire(&key, self.ttl_secs).await;
        }
        reason.map(DenyReason::new)
    }

    async fn remember(&self, session: SessionId, tool: &str, reason: &DenyReason) {
        let Some(mut conn) = self.connection().await else {
            return;
        };
        let key = Self::key(session);
        // Keep the first reason (HSETNX), and refresh the session TTL regardless
        // (sliding). Best-effort: a Redis error just means this denial isn't
        // recorded — never a failure surfaced to the data plane.
        if let Err(error) = conn
            .hset_nx::<_, _, _, ()>(&key, tool, reason.as_str())
            .await
        {
            warn!(%error, "session-memory remember failed; denial not recorded");
            return;
        }
        let _: Result<(), redis::RedisError> = conn.expire(&key, self.ttl_secs).await;
    }
}
