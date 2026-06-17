use serde::{Deserialize, Serialize};

/// `[audit]` — the transparency-log store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuditSection {
    /// Which persistence backend to assemble at startup.
    pub backend: AuditBackend,
    /// PostgreSQL connection URL (required; prefer the env override for secrets).
    pub database_url: String,
    /// Maximum pooled database connections.
    pub max_connections: u32,
    /// How long to wait for a free pooled connection before erroring, in seconds.
    pub acquire_timeout_secs: u64,
    /// Initial-connect retries before giving up (`0` = try once, no retry).
    pub connect_max_retries: u32,
    /// Base backoff between connect attempts, in seconds (doubled each retry).
    pub connect_backoff_secs: u64,
    /// How often a signed checkpoint (STH) is issued for the log, in seconds
    /// (`0` = disabled, the default). Requires a signing key in
    /// `AUDIT_CHECKPOINT_SEED`; an idle log between ticks is not re-anchored.
    pub checkpoint_interval_secs: u64,
    /// The signing key id the periodic issuer asks for — must match the key the
    /// store loaded (`AUDIT_CHECKPOINT_KEY_ID`, same default).
    pub checkpoint_key_id: String,
    /// How many inspected records may queue for the audit log before the outbox
    /// is full. Bounded so a slow database cannot grow memory without limit.
    pub outbox_capacity: usize,
    /// What the proxy does when the outbox is full: `block` (apply backpressure,
    /// never lose a record — the default) or `shed` (drop with a loud alert so
    /// the proxy keeps serving, accepting a transparency-log gap).
    pub outbox_on_full: OnFull,
}

/// The full-outbox policy — completeness (`block`) vs availability (`shed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnFull {
    /// Apply backpressure and never drop — the secure default for a log.
    #[default]
    Block,
    /// Drop the record (loudly logged + counted) to keep the proxy serving.
    Shed,
}

impl AuditSection {
    /// Fail fast on missing or zeroed store settings. The checks are keyed to
    /// the configured backend: `database_url` and the pool knobs are Postgres
    /// requirements, not generic audit ones — a future backend validates its
    /// own variant here.
    pub fn validate(&self) -> Result<(), String> {
        // A zero-capacity outbox would reject every record (or, unbounded, defeat
        // the backpressure that protects memory) — require a real bound.
        if self.outbox_capacity == 0 {
            return Err("audit.outbox_capacity must be greater than 0".into());
        }
        match self.backend {
            AuditBackend::Postgres => self.validate_postgres(),
        }
    }

    fn validate_postgres(&self) -> Result<(), String> {
        if self.database_url.trim().is_empty() {
            return Err(
                "audit.database_url is required (set [audit].database_url or \
                 AGATE__AUDIT__DATABASE_URL)"
                    .into(),
            );
        }
        if self.max_connections == 0 {
            return Err("audit.max_connections must be greater than 0".into());
        }
        if self.acquire_timeout_secs == 0 {
            return Err("audit.acquire_timeout_secs must be greater than 0".into());
        }
        // A zero backoff would busy-loop the connect retries; require a real pause.
        if self.connect_backoff_secs == 0 {
            return Err("audit.connect_backoff_secs must be greater than 0".into());
        }
        Ok(())
    }
}

impl Default for AuditSection {
    fn default() -> Self {
        Self {
            backend: AuditBackend::Postgres,
            database_url: String::new(),
            max_connections: 10,
            acquire_timeout_secs: 30,
            connect_max_retries: 10,
            connect_backoff_secs: 1,
            // Off by default: periodic checkpoints need a signing key, so opt in
            // once `AUDIT_CHECKPOINT_SEED` is set.
            checkpoint_interval_secs: 0,
            checkpoint_key_id: "checkpoint-ed25519".into(),
            outbox_capacity: 1024,
            outbox_on_full: OnFull::Block,
        }
    }
}

/// Which persistence backend the transparency log uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditBackend {
    /// PostgreSQL — the only implemented backend (and the default).
    #[default]
    Postgres,
}
