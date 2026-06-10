use std::collections::BTreeSet;
use std::time::Duration;

use agate_audit::infrastructure::persistence::postgres::PoolConfig;
use agate_audit::setup::configs::{PostgresConfig, StorageConfig};
use agate_policy::domain::common::errors::DomainError;
use agate_policy::domain::decision::{PolicyRuleset, SecretPattern, ToolName, ToolPolicy};
use agate_proxy::application::inspection::MalformedEventMode;
use agate_proxy::infrastructure::FailMode;
use agate_proxy::setup::configs::ProxyConfig;
use serde::{Deserialize, Serialize};

use super::observability::ObservabilityConfig;
use super::tls::TlsConfig;

/// The full application configuration.
///
/// Deserialized from `agate.toml` layered with environment overrides (see
/// [`load`](super::loader::load)). The composition root reads this and maps each
/// section onto the bounded contexts' own config types — the server owns the
/// on-disk config format, the contexts stay free of it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub proxy: ProxySection,
    pub audit: AuditSection,
    pub policy: PolicySection,
    pub observability: ObservabilityConfig,
    /// Optional TLS termination at the listener (off by default).
    pub tls: TlsConfig,
}

impl AppConfig {
    /// Fail fast on missing required values (no sensible default exists for
    /// them). Each section owns its invariants; this only composes them.
    pub fn validate(&self) -> Result<(), String> {
        self.proxy.validate()?;
        self.audit.validate()?;
        self.policy.validate()?;
        self.tls.validate()
    }

    /// The TLS config when enabled, else `None` (serve plain HTTP).
    #[must_use]
    pub fn tls_config(&self) -> Option<&TlsConfig> {
        self.tls.enabled.then_some(&self.tls)
    }

    #[must_use]
    pub fn proxy_config(&self) -> ProxyConfig {
        ProxyConfig::new(self.proxy.agent_endpoint.clone(), self.proxy.bind.clone())
            .with_ingress(
                Duration::from_secs(self.proxy.connect_timeout_secs),
                Duration::from_secs(self.proxy.read_timeout_secs),
                self.proxy.max_body_bytes,
                self.accepted_api_keys(),
            )
            .with_concurrency_limit(self.proxy.max_concurrent_requests)
            .with_malformed_event_mode(self.malformed_event_mode())
    }

    /// How the response leg treats a recognized-but-malformed event.
    fn malformed_event_mode(&self) -> MalformedEventMode {
        match self.policy.on_malformed_event {
            MalformedMode::Forward => MalformedEventMode::Forward,
            MalformedMode::Drop => MalformedEventMode::Drop,
            MalformedMode::Terminate => MalformedEventMode::Terminate,
        }
    }

    /// The set of accepted API keys: the `api_keys` array plus the single
    /// `api_key` shorthand, trimmed and de-blanked. Empty means auth is off.
    fn accepted_api_keys(&self) -> Vec<String> {
        self.proxy
            .api_key
            .iter()
            .chain(self.proxy.api_keys.iter())
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .collect()
    }

    /// The connected-store descriptor for the configured backend (Postgres).
    #[must_use]
    pub fn storage_config(&self) -> StorageConfig {
        match self.audit.backend {
            AuditBackend::Postgres => StorageConfig::Postgres(self.postgres_config()),
        }
    }

    #[must_use]
    pub fn postgres_config(&self) -> PostgresConfig {
        PostgresConfig::new(self.audit.database_url.clone()).with_pool(PoolConfig {
            max_connections: self.audit.max_connections,
            acquire_timeout: Duration::from_secs(self.audit.acquire_timeout_secs),
            connect_max_retries: self.audit.connect_max_retries,
            connect_backoff: Duration::from_secs(self.audit.connect_backoff_secs),
        })
    }

    /// Build the policy ruleset, failing on any invalid tool name or pattern.
    pub fn policy_ruleset(&self) -> Result<PolicyRuleset, DomainError> {
        let names = || tool_set(&self.policy.tools.names);
        let tools = match self.policy.tools.mode {
            ToolMode::AllowAll => ToolPolicy::AllowAll,
            ToolMode::Allowlist => ToolPolicy::Allowlist(names()?),
            ToolMode::Denylist => ToolPolicy::Denylist(names()?),
        };
        let secrets = self
            .policy
            .redact
            .iter()
            .map(|pattern| SecretPattern::new(pattern.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PolicyRuleset::new(tools, secrets))
    }

    /// The proxy's fail mode for a policy-decision timeout.
    #[must_use]
    pub fn policy_fail_mode(&self) -> FailMode {
        match self.policy.fail_mode {
            PolicyFailMode::Open => FailMode::Open,
            PolicyFailMode::Closed => FailMode::Closed,
        }
    }

    /// The deadline for a single policy decision.
    #[must_use]
    pub fn policy_decision_timeout(&self) -> Duration {
        Duration::from_millis(self.policy.decision_timeout_ms)
    }
}

/// `[proxy]` — the reverse-proxy data plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxySection {
    /// Upstream agent run endpoint the proxy forwards to (required).
    pub agent_endpoint: String,
    /// Address the proxy listens on.
    pub bind: String,
    /// Connect timeout to the upstream agent, in seconds (fail fast when
    /// unreachable). Not an overall deadline — a healthy SSE stream runs on.
    pub connect_timeout_secs: u64,
    /// Idle read timeout between upstream response chunks, in seconds.
    pub read_timeout_secs: u64,
    /// Maximum accepted request body size, in bytes.
    pub max_body_bytes: usize,
    /// Single API key required on the `X-API-Key` header — a shorthand for one
    /// key. Merged with `api_keys`. Absent/blank (and `api_keys` empty) disables
    /// authentication (open proxy) — set one, or front the proxy with a guard.
    pub api_key: Option<String>,
    /// Accepted API keys: a request matching **any** is authenticated. Holding
    /// several at once is how rotation works (add the new, migrate, drop the old).
    pub api_keys: Vec<String>,
    /// Maximum concurrently in-flight proxied runs; excess is shed with `503`.
    pub max_concurrent_requests: usize,
}

impl ProxySection {
    /// Fail fast on a missing endpoint or zeroed ingress knobs.
    pub fn validate(&self) -> Result<(), String> {
        if self.agent_endpoint.trim().is_empty() {
            return Err(
                "proxy.agent_endpoint is required (set [proxy].agent_endpoint or \
                 AGATE__PROXY__AGENT_ENDPOINT)"
                    .into(),
            );
        }
        // Zero is a footgun, not a sensible "disable": a 0-byte body limit
        // rejects every request, and a 0s timeout fails the connection at once.
        if self.max_body_bytes == 0 {
            return Err("proxy.max_body_bytes must be greater than 0".into());
        }
        if self.connect_timeout_secs == 0 || self.read_timeout_secs == 0 {
            return Err(
                "proxy.connect_timeout_secs and proxy.read_timeout_secs must be greater than 0"
                    .into(),
            );
        }
        if self.max_concurrent_requests == 0 {
            return Err("proxy.max_concurrent_requests must be greater than 0".into());
        }
        Ok(())
    }
}

impl Default for ProxySection {
    fn default() -> Self {
        Self {
            agent_endpoint: String::new(),
            bind: "0.0.0.0:8080".into(),
            connect_timeout_secs: 5,
            read_timeout_secs: 60,
            max_body_bytes: 1 << 20,
            api_key: None,
            api_keys: Vec::new(),
            max_concurrent_requests: 256,
        }
    }
}

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
}

impl AuditSection {
    /// Fail fast on missing or zeroed store settings. The checks are keyed to
    /// the configured backend: `database_url` and the pool knobs are Postgres
    /// requirements, not generic audit ones — a future backend validates its
    /// own variant here.
    pub fn validate(&self) -> Result<(), String> {
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
        }
    }
}

/// Which persistence backend the transparency log uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditBackend {
    #[default]
    Postgres,
}

/// `[policy]` — content/authorization rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicySection {
    pub tools: ToolsSection,
    /// Literal markers redacted from emitted text.
    pub redact: Vec<String>,
    /// What to do when a policy decision cannot be made in time: `open`
    /// (forward) or `closed` (block). Defaults to the secure `closed`.
    pub fail_mode: PolicyFailMode,
    /// Deadline for a single policy decision, in milliseconds.
    pub decision_timeout_ms: u64,
    /// What to do with a recognized response event that is malformed (a known
    /// type with a missing/blank required field, so it cannot be inspected):
    /// `forward`, `drop`, or `terminate`. Defaults to the secure `terminate`.
    pub on_malformed_event: MalformedMode,
}

impl PolicySection {
    /// Fail fast on a zeroed decision deadline.
    pub fn validate(&self) -> Result<(), String> {
        if self.decision_timeout_ms == 0 {
            return Err("policy.decision_timeout_ms must be greater than 0".into());
        }
        Ok(())
    }
}

impl Default for PolicySection {
    fn default() -> Self {
        Self {
            tools: ToolsSection::default(),
            redact: Vec::new(),
            fail_mode: PolicyFailMode::default(),
            decision_timeout_ms: 5000,
            on_malformed_event: MalformedMode::default(),
        }
    }
}

/// What to do with a recognized-but-malformed response event — the data plane's
/// fail-open / fail-closed knob for events it cannot inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MalformedMode {
    /// Forward the raw frame (availability over safety).
    Forward,
    /// Drop the frame, continue the run.
    Drop,
    /// End the run with a `RUN_ERROR` — the secure default.
    #[default]
    Terminate,
}

/// Behavior when a policy decision times out — the fail-open / fail-closed knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyFailMode {
    /// Forward the event (availability over safety).
    Open,
    /// Block the run (safety over availability) — the secure default.
    #[default]
    Closed,
}

/// `[policy.tools]` — tool-call authorization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsSection {
    pub mode: ToolMode,
    /// Tool names governed by `mode` (ignored when `mode = "allow-all"`).
    pub names: Vec<String>,
}

/// How tool invocations are authorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolMode {
    #[default]
    AllowAll,
    Allowlist,
    Denylist,
}

fn tool_set(names: &[String]) -> Result<BTreeSet<ToolName>, DomainError> {
    names
        .iter()
        .map(|name| ToolName::new(name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use agate_policy::domain::decision::ToolPolicy;

    use super::{AppConfig, FailMode, PolicySection, ProxySection, ToolMode, ToolsSection};

    fn with_policy(mode: ToolMode, names: &[&str], redact: &[&str]) -> AppConfig {
        AppConfig {
            policy: PolicySection {
                tools: ToolsSection {
                    mode,
                    names: names.iter().map(|s| (*s).to_owned()).collect(),
                },
                redact: redact.iter().map(|s| (*s).to_owned()).collect(),
                ..PolicySection::default()
            },
            ..AppConfig::default()
        }
    }

    #[test]
    fn allow_all_is_the_default_tool_policy() {
        let ruleset = AppConfig::default().policy_ruleset().expect("valid");
        assert_eq!(*ruleset.tools(), ToolPolicy::AllowAll);
    }

    #[test]
    fn proxy_config_maps_the_ingress_settings() {
        let config = AppConfig {
            proxy: ProxySection {
                agent_endpoint: "http://agent/run".to_owned(),
                bind: "0.0.0.0:9000".to_owned(),
                connect_timeout_secs: 3,
                read_timeout_secs: 120,
                max_body_bytes: 2048,
                api_key: Some("  k  ".to_owned()),
                api_keys: vec!["k2".to_owned(), "  ".to_owned()],
                max_concurrent_requests: 64,
            },
            ..AppConfig::default()
        };

        let proxy = config.proxy_config();
        assert_eq!(proxy.agent_endpoint, "http://agent/run");
        assert_eq!(proxy.bind_addr, "0.0.0.0:9000");
        assert_eq!(proxy.connect_timeout, std::time::Duration::from_secs(3));
        assert_eq!(proxy.read_timeout, std::time::Duration::from_mins(2));
        assert_eq!(proxy.max_body_bytes, 2048);
        // `api_key` shorthand + `api_keys` array merged, trimmed, blanks dropped.
        assert_eq!(proxy.api_keys, vec!["k".to_owned(), "k2".to_owned()]);
        assert_eq!(proxy.max_concurrent_requests, 64);
    }

    #[test]
    fn proxy_config_treats_blank_api_keys_as_disabled() {
        let config = AppConfig {
            proxy: ProxySection {
                api_key: Some("   ".to_owned()),
                api_keys: vec![String::new(), "  ".to_owned()],
                ..ProxySection::default()
            },
            ..AppConfig::default()
        };
        assert!(
            config.proxy_config().api_keys.is_empty(),
            "all-blank keys leave auth disabled"
        );
    }

    #[test]
    fn validate_rejects_zero_ingress_knobs() {
        let mut config = AppConfig::default();
        config.proxy.agent_endpoint = "http://agent/run".to_owned();
        config.audit.database_url = "postgres://agate@db/agate".to_owned();
        assert!(config.validate().is_ok(), "the sane defaults validate");

        let mut zero_body = config.clone();
        zero_body.proxy.max_body_bytes = 0;
        assert!(
            zero_body.validate().is_err(),
            "a 0-byte body limit is rejected"
        );

        let mut zero_timeout = config.clone();
        zero_timeout.proxy.connect_timeout_secs = 0;
        assert!(zero_timeout.validate().is_err(), "a 0s timeout is rejected");

        let mut zero_decision = config.clone();
        zero_decision.policy.decision_timeout_ms = 0;
        assert!(
            zero_decision.validate().is_err(),
            "a 0ms policy decision timeout is rejected"
        );
    }

    #[test]
    fn tls_is_off_by_default_and_validated_when_enabled() {
        let mut config = AppConfig::default();
        config.proxy.agent_endpoint = "http://agent/run".to_owned();
        config.audit.database_url = "postgres://agate@db/agate".to_owned();

        // Off by default: no TLS config, validates fine.
        assert!(config.tls_config().is_none());
        assert!(config.validate().is_ok());

        // Enabled but missing cert/key → rejected.
        let mut missing = config.clone();
        missing.tls.enabled = true;
        assert!(
            missing.validate().is_err(),
            "enabling TLS without cert/key is rejected"
        );

        // Enabled with both paths → validates, and tls_config() exposes it.
        config.tls.enabled = true;
        config.tls.cert = "/etc/agate/tls/cert.pem".to_owned();
        config.tls.key = "/etc/agate/tls/key.pem".to_owned();
        assert!(config.validate().is_ok());
        let tls = config.tls_config().expect("TLS is enabled");
        assert_eq!(tls.cert, "/etc/agate/tls/cert.pem");
    }

    #[test]
    fn policy_fail_mode_defaults_to_closed() {
        let config = AppConfig::default();
        assert_eq!(config.policy_fail_mode(), FailMode::Closed);
        assert_eq!(
            config.policy_decision_timeout(),
            std::time::Duration::from_secs(5)
        );
    }

    #[test]
    fn malformed_event_mode_defaults_to_terminate_and_maps() {
        use super::{MalformedEventMode, MalformedMode};

        // Secure default: a malformed known event terminates the run.
        assert_eq!(
            AppConfig::default().proxy_config().malformed_event_mode,
            MalformedEventMode::Terminate
        );

        // Every TOML variant maps onto the matching proxy inspection setting.
        for (toml, expected) in [
            (MalformedMode::Forward, MalformedEventMode::Forward),
            (MalformedMode::Drop, MalformedEventMode::Drop),
            (MalformedMode::Terminate, MalformedEventMode::Terminate),
        ] {
            let mut config = AppConfig::default();
            config.policy.on_malformed_event = toml;
            assert_eq!(config.proxy_config().malformed_event_mode, expected);
        }
    }

    #[test]
    fn allowlist_and_denylist_modes_build() {
        let allow = with_policy(ToolMode::Allowlist, &["search"], &[]);
        assert!(matches!(
            allow.policy_ruleset().expect("valid").tools(),
            ToolPolicy::Allowlist(_)
        ));
        let deny = with_policy(ToolMode::Denylist, &["rm"], &[]);
        assert!(matches!(
            deny.policy_ruleset().expect("valid").tools(),
            ToolPolicy::Denylist(_)
        ));
    }

    #[test]
    fn redaction_patterns_build() {
        let config = with_policy(ToolMode::AllowAll, &[], &["sk-", "AKIA"]);
        assert_eq!(config.policy_ruleset().expect("valid").secrets().len(), 2);
    }

    #[test]
    fn a_blank_tool_name_is_rejected() {
        let config = with_policy(ToolMode::Allowlist, &["  "], &[]);
        assert!(config.policy_ruleset().is_err());
    }

    #[test]
    fn validate_requires_endpoint_and_database_url() {
        assert!(AppConfig::default().validate().is_err());

        let mut config = AppConfig::default();
        config.proxy.agent_endpoint = "http://agent/run".into();
        assert!(config.validate().is_err(), "still missing database_url");

        config.audit.database_url = "postgres://db".into();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn the_database_url_requirement_is_keyed_to_the_postgres_backend() {
        // Pins the multi-backend seam: `database_url` (and the pool knobs) are
        // Postgres rules — a future backend must validate its own variant, not
        // inherit these.
        let section = super::AuditSection::default();
        assert_eq!(section.backend, super::AuditBackend::Postgres);
        assert!(
            section
                .validate()
                .err()
                .is_some_and(|message| { message.contains("audit.database_url is required") }),
            "the Postgres arm owns the database_url requirement"
        );
    }

    #[test]
    fn proxy_section_defaults_to_the_standard_bind() {
        assert_eq!(ProxySection::default().bind, "0.0.0.0:8080");
    }
}
