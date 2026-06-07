use std::collections::BTreeSet;
use std::time::Duration;

use agate_audit::setup::configs::PostgresConfig;
use agate_policy::domain::common::errors::DomainError;
use agate_policy::domain::decision::{PolicyRuleset, SecretPattern, ToolName, ToolPolicy};
use agate_proxy::setup::configs::ProxyConfig;
use serde::{Deserialize, Serialize};

use super::observability::ObservabilityConfig;

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
}

impl AppConfig {
    /// Fail fast on missing required values (no sensible default exists for them).
    pub fn validate(&self) -> Result<(), String> {
        if self.proxy.agent_endpoint.trim().is_empty() {
            return Err(
                "proxy.agent_endpoint is required (set [proxy].agent_endpoint or \
                 AGATE__PROXY__AGENT_ENDPOINT)"
                    .into(),
            );
        }
        if self.audit.database_url.trim().is_empty() {
            return Err(
                "audit.database_url is required (set [audit].database_url or \
                 AGATE__AUDIT__DATABASE_URL)"
                    .into(),
            );
        }
        Ok(())
    }

    #[must_use]
    pub fn proxy_config(&self) -> ProxyConfig {
        ProxyConfig::new(self.proxy.agent_endpoint.clone(), self.proxy.bind.clone()).with_ingress(
            Duration::from_secs(self.proxy.connect_timeout_secs),
            Duration::from_secs(self.proxy.read_timeout_secs),
            self.proxy.max_body_bytes,
            // Treat an empty key as "no auth", so a blank TOML value disables it.
            self.proxy
                .api_key
                .as_ref()
                .map(|key| key.trim().to_owned())
                .filter(|key| !key.is_empty()),
        )
    }

    #[must_use]
    pub fn postgres_config(&self) -> PostgresConfig {
        PostgresConfig::new(self.audit.database_url.clone())
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
    /// API key required on the `X-API-Key` header. Absent or blank disables
    /// authentication (open proxy) — set it, or front the proxy with another guard.
    pub api_key: Option<String>,
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
        }
    }
}

/// `[audit]` — the transparency-log store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuditSection {
    /// PostgreSQL connection URL (required; prefer the env override for secrets).
    pub database_url: String,
}

/// `[policy]` — content/authorization rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicySection {
    pub tools: ToolsSection,
    /// Literal markers redacted from emitted text.
    pub redact: Vec<String>,
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

    use super::{AppConfig, PolicySection, ProxySection, ToolMode, ToolsSection};

    fn with_policy(mode: ToolMode, names: &[&str], redact: &[&str]) -> AppConfig {
        AppConfig {
            policy: PolicySection {
                tools: ToolsSection {
                    mode,
                    names: names.iter().map(|s| (*s).to_owned()).collect(),
                },
                redact: redact.iter().map(|s| (*s).to_owned()).collect(),
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
    fn proxy_section_defaults_to_the_standard_bind() {
        assert_eq!(ProxySection::default().bind, "0.0.0.0:8080");
    }
}
