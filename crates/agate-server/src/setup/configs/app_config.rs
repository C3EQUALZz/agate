use std::time::Duration;

use agate_audit::infrastructure::persistence::postgres::PoolConfig;
use agate_audit::setup::configs::{PostgresConfig, StorageConfig};
use agate_crypto::KeyId;
use agate_policy::domain::common::errors::DomainError;
use agate_policy::domain::decision::{Pattern, PolicyRuleset, ToolMatcher, ToolPolicy};
use agate_proxy::application::inspection::{MalformedEventMode, ResponseBudget};
use agate_proxy::infrastructure::FailMode;
use agate_proxy::setup::configs::ProxyConfig;
use serde::{Deserialize, Serialize};

use super::audit_section::{AuditBackend, AuditSection};
use super::observability::ObservabilityConfig;
use super::policy_section::{
    ArgumentRuleConfig, MalformedMode, PolicyFailMode, PolicySection, ResultRuleConfig, ToolMode,
};
use super::proxy_section::ProxySection;
use super::tls::TlsConfig;
use crate::setup::bootstrap::CheckpointSettings;

/// The full application configuration — the server's composition root for
/// on-disk config.
///
/// Deserialized from `agate.toml` layered with environment overrides (see
/// [`load`](super::loader::load)). It reads this and maps each section onto the
/// bounded contexts' own config types — the server owns the on-disk config
/// format, the contexts stay free of it. Each `[section]` lives in its own
/// module (`proxy_section`, `audit_section`, `policy_section`); this file
/// composes them and the context mappings.
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
            .with_response_budget(ResponseBudget {
                max_events: self.proxy.max_response_events,
                max_bytes: self.proxy.max_response_bytes,
            })
            .with_rate_limit(
                self.proxy.rate_limit_per_second,
                self.proxy.rate_limit_burst,
            )
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
        let matchers = || tool_matchers(&self.policy.tools.names);
        let tools = match self.policy.tools.mode {
            ToolMode::AllowAll => ToolPolicy::AllowAll,
            ToolMode::Allowlist => ToolPolicy::Allowlist(matchers()?),
            ToolMode::Denylist => ToolPolicy::Denylist(matchers()?),
        };
        let argument_rules = self
            .policy
            .tools
            .deny_arguments
            .iter()
            .map(ArgumentRuleConfig::to_rule)
            .collect::<Result<Vec<_>, _>>()?;
        let result_rules = self
            .policy
            .tools
            .deny_results
            .iter()
            .map(ResultRuleConfig::to_rule)
            .collect::<Result<Vec<_>, _>>()?;
        // Literal markers first, then regex markers — both join the one secret
        // list the redactor applies in order.
        let mut secrets = self
            .policy
            .redact
            .iter()
            .map(Pattern::literal)
            .collect::<Result<Vec<_>, _>>()?;
        for source in &self.policy.redact_regex {
            secrets.push(Pattern::regex(source)?);
        }
        Ok(PolicyRuleset::new(tools, argument_rules, secrets).with_result_rules(result_rules))
    }

    /// How the periodic checkpoint issuer should run, or `None` when disabled
    /// (`checkpoint_interval_secs = 0`). The signing key itself is loaded from
    /// the environment by the key store; here we only carry its id.
    #[must_use]
    pub fn checkpoint_settings(&self) -> Option<CheckpointSettings> {
        (self.audit.checkpoint_interval_secs > 0).then(|| CheckpointSettings {
            period: Duration::from_secs(self.audit.checkpoint_interval_secs),
            key: KeyId(self.audit.checkpoint_key_id.clone()),
        })
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

/// Build the tool matchers from the configured entries. An entry is classified
/// by an optional prefix: `glob:<pat>` (shell-style `*`/`?`), `regex:<expr>`
/// (anchored to the whole name), or a bare name (an exact match — the default,
/// so existing configs are unchanged).
fn tool_matchers(names: &[String]) -> Result<Vec<ToolMatcher>, DomainError> {
    names
        .iter()
        .map(|entry| parse_tool_matcher(entry))
        .collect()
}

fn parse_tool_matcher(entry: &str) -> Result<ToolMatcher, DomainError> {
    let entry = entry.trim();
    if let Some(glob) = entry.strip_prefix("glob:") {
        ToolMatcher::glob(glob)
    } else if let Some(regex) = entry.strip_prefix("regex:") {
        ToolMatcher::regex(regex)
    } else {
        ToolMatcher::exact(entry)
    }
}

#[cfg(test)]
mod tests {
    use agate_policy::application::PolicyService;
    use agate_policy::domain::decision::{InspectedAction, PolicyDecision, ToolPolicy};

    use super::super::policy_section::{ArgumentRuleConfig, ToolsSection};
    use super::{AppConfig, FailMode, PolicySection, ProxySection, ToolMode};

    fn with_policy(mode: ToolMode, names: &[&str], redact: &[&str]) -> AppConfig {
        AppConfig {
            policy: PolicySection {
                tools: ToolsSection {
                    mode,
                    names: names.iter().map(|s| (*s).to_owned()).collect(),
                    ..ToolsSection::default()
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
    fn checkpoints_are_disabled_by_default_and_enabled_by_a_positive_interval() {
        assert!(AppConfig::default().checkpoint_settings().is_none());

        let mut config = AppConfig::default();
        config.audit.checkpoint_interval_secs = 3600;
        config.audit.checkpoint_key_id = "k".to_owned();
        let settings = config.checkpoint_settings().expect("enabled");
        assert_eq!(settings.period.as_secs(), 3600);
        assert_eq!(settings.key.0, "k");
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
                rate_limit_per_second: 10,
                rate_limit_burst: 20,
                ..ProxySection::default()
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
        assert_eq!(proxy.rate_limit_per_second, 10);
        assert_eq!(proxy.rate_limit_burst, 20);
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
        use super::super::policy_section::MalformedMode;
        use super::MalformedEventMode;

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
    fn response_budget_maps_from_config() {
        let mut config = AppConfig::default();
        config.proxy.max_response_events = 42;
        config.proxy.max_response_bytes = 4096;
        let budget = config.proxy_config().response_budget;
        assert_eq!(budget.max_events, 42);
        assert_eq!(budget.max_bytes, 4096);
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
    fn tool_entries_are_classified_by_prefix() {
        // Bare = exact, `glob:` = glob, `regex:` = anchored regex; the resulting
        // allowlist permits exactly the names each kind should match.
        let config = with_policy(
            ToolMode::Allowlist,
            &["search", "glob:fs.*", "regex:db_.*"],
            &[],
        );
        let ruleset = config.policy_ruleset().expect("valid");
        let tools = ruleset.tools();

        assert!(tools.permits("search"));
        assert!(!tools.permits("research")); // exact is anchored
        assert!(tools.permits("fs.read")); // glob family
        assert!(tools.permits("db_query")); // regex, anchored
        assert!(!tools.permits("mydb_query"));
        assert!(!tools.permits("rm"));
    }

    #[test]
    fn an_invalid_tool_regex_is_rejected() {
        let config = with_policy(ToolMode::Allowlist, &["regex:(unclosed"], &[]);
        assert!(config.policy_ruleset().is_err());
    }

    #[test]
    fn literal_and_regex_redaction_patterns_build_together() {
        let mut config = with_policy(ToolMode::AllowAll, &[], &["sk-"]);
        config.policy.redact_regex = vec![r"AKIA[0-9A-Z]{16}".to_owned()];
        let ruleset = config.policy_ruleset().expect("valid");
        assert_eq!(ruleset.secrets().len(), 2);
    }

    #[test]
    fn an_invalid_redaction_regex_aborts_the_ruleset() {
        let mut config = AppConfig::default();
        config.policy.redact_regex = vec!["(unclosed".to_owned()];
        assert!(config.policy_ruleset().is_err());
    }

    #[test]
    fn deny_argument_rules_build_from_config() {
        let mut config = AppConfig::default();
        config.policy.tools.deny_arguments = vec![
            ArgumentRuleConfig {
                tool: Some("shell".to_owned()),
                contains: Some("rm -rf".to_owned()),
                ..ArgumentRuleConfig::default()
            },
            // A regex-marker rule alongside a literal one.
            ArgumentRuleConfig {
                matches: Some(r"AKIA[0-9A-Z]{16}".to_owned()),
                ..ArgumentRuleConfig::default()
            },
        ];
        let ruleset = config.policy_ruleset().expect("valid");
        assert_eq!(ruleset.argument_rules().len(), 2);
    }

    #[test]
    fn a_blank_argument_marker_is_rejected() {
        let mut config = AppConfig::default();
        config.policy.tools.deny_arguments = vec![ArgumentRuleConfig {
            contains: Some("   ".to_owned()),
            ..ArgumentRuleConfig::default()
        }];
        assert!(config.policy_ruleset().is_err());
    }

    #[test]
    fn a_deny_argument_rule_needs_exactly_one_marker() {
        let mut config = AppConfig::default();
        // Neither set → error.
        config.policy.tools.deny_arguments = vec![ArgumentRuleConfig::default()];
        assert!(config.policy_ruleset().is_err(), "no marker is rejected");

        // Both set → error.
        config.policy.tools.deny_arguments = vec![ArgumentRuleConfig {
            contains: Some("x".to_owned()),
            matches: Some("y".to_owned()),
            ..ArgumentRuleConfig::default()
        }];
        assert!(config.policy_ruleset().is_err(), "two markers are rejected");
    }

    #[test]
    fn a_path_scoped_argument_rule_builds_and_targets_one_field() {
        let mut config = AppConfig::default();
        config.policy.tools.deny_arguments = vec![ArgumentRuleConfig {
            path: Some("url".to_owned()),
            matches: Some(r"^https?://169\.254".to_owned()),
            ..ArgumentRuleConfig::default()
        }];
        let service = PolicyService::new(config.policy_ruleset().expect("valid"));

        let blocked = service.decide(&InspectedAction::ToolCall {
            name: "fetch".to_owned(),
            arguments: r#"{"url":"http://169.254.169.254/"}"#.to_owned(),
        });
        assert!(matches!(blocked, PolicyDecision::Deny(_)));

        // Same marker text in a different field does not fire the path rule.
        let allowed = service.decide(&InspectedAction::ToolCall {
            name: "fetch".to_owned(),
            arguments: r#"{"note":"http://169.254.0.1","url":"https://ok"}"#.to_owned(),
        });
        assert_eq!(allowed, PolicyDecision::Allow);

        // A bad path aborts the build.
        config.policy.tools.deny_arguments = vec![ArgumentRuleConfig {
            path: Some("a..b".to_owned()),
            contains: Some("x".to_owned()),
            ..ArgumentRuleConfig::default()
        }];
        assert!(
            config.policy_ruleset().is_err(),
            "an invalid path is rejected"
        );
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
        use super::super::audit_section::{AuditBackend, AuditSection};

        // Pins the multi-backend seam: `database_url` (and the pool knobs) are
        // Postgres rules — a future backend must validate its own variant, not
        // inherit these.
        let section = AuditSection::default();
        assert_eq!(section.backend, AuditBackend::Postgres);
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
