use agate_policy::domain::common::errors::DomainError;
use agate_policy::domain::decision::{ArgumentRule, JsonPath, Pattern, ResultRule, ToolName};
use serde::{Deserialize, Serialize};

/// `[policy]` — content/authorization rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicySection {
    pub tools: ToolsSection,
    /// Literal markers redacted from emitted text (case-insensitive).
    pub redact: Vec<String>,
    /// Regex markers redacted from emitted text (full `regex` syntax; add `(?i)`
    /// for case-insensitivity). An invalid expression aborts startup.
    pub redact_regex: Vec<String>,
    /// What to do when a policy decision cannot be made in time: `open`
    /// (forward) or `closed` (block). Defaults to the secure `closed`.
    pub fail_mode: PolicyFailMode,
    /// Deadline for a single policy decision, in milliseconds.
    pub decision_timeout_ms: u64,
    /// What to do with a recognized response event that is malformed (a known
    /// type with a missing/blank required field, so it cannot be inspected):
    /// `forward`, `drop`, or `terminate`. Defaults to the secure `terminate`.
    pub on_malformed_event: MalformedMode,
    /// Cross-run replay memory: once a tool is denied in a run, refuse it for
    /// the rest of the session (across runs). Off by default — the policy is
    /// otherwise stateless.
    pub session_memory: SessionMemorySection,
    /// Which engine decides verdicts: the built-in static `ruleset` (the default
    /// — tool allow/deny rules + redaction above), the `cel` plugin engine
    /// (operator CEL rules from `[policy.cel]`), or the `rego` plugin engine
    /// (operator Rego/OPA policy from `[policy.rego]`). A plugin engine then fully
    /// owns the decision and requires a build with its feature (`policy-cel` /
    /// `policy-rego`).
    pub backend: PolicyBackendKind,
    /// `[policy.cel]` — the CEL plugin engine (used when `backend = "cel"`).
    pub cel: CelSection,
    /// `[policy.rego]` — the Rego plugin engine (used when `backend = "rego"`).
    pub rego: RegoSection,
}

impl PolicySection {
    /// Fail fast on a zeroed decision deadline or, when session memory is on, a
    /// zeroed TTL.
    pub fn validate(&self) -> Result<(), String> {
        if self.decision_timeout_ms == 0 {
            return Err("policy.decision_timeout_ms must be greater than 0".into());
        }
        if self.backend == PolicyBackendKind::Cel {
            // Reject `backend = "cel"` in a build without the engine here, at
            // config time, rather than letting the process boot and panic when
            // the engine is constructed.
            #[cfg(not(feature = "policy-cel"))]
            {
                return Err(
                    "policy.backend = \"cel\" requires building agate-server with the \
                     `policy-cel` feature"
                        .into(),
                );
            }
            #[cfg(feature = "policy-cel")]
            if self
                .cel
                .policy_path
                .as_deref()
                .is_none_or(|path| path.trim().is_empty())
            {
                return Err("policy.cel.policy_path is required when backend = \"cel\"".into());
            }
            #[cfg(feature = "policy-cel")]
            if self.cel.max_rules == 0 {
                return Err("policy.cel.max_rules must be greater than 0".into());
            }
        }
        if self.backend == PolicyBackendKind::Rego {
            #[cfg(not(feature = "policy-rego"))]
            {
                return Err(
                    "policy.backend = \"rego\" requires building agate-server with the \
                     `policy-rego` feature"
                        .into(),
                );
            }
            #[cfg(feature = "policy-rego")]
            if self
                .rego
                .policy_path
                .as_deref()
                .is_none_or(|path| path.trim().is_empty())
            {
                return Err("policy.rego.policy_path is required when backend = \"rego\"".into());
            }
        }
        if self.session_memory.enabled && self.session_memory.ttl_secs == 0 {
            return Err(
                "policy.session_memory.ttl_secs must be greater than 0 when enabled".into(),
            );
        }
        if self.session_memory.enabled
            && self.session_memory.backend == SessionMemoryBackendKind::Redis
            && self
                .session_memory
                .redis_url
                .as_deref()
                .is_none_or(|url| url.trim().is_empty())
        {
            return Err(
                "policy.session_memory.redis_url is required when backend = \"redis\"".into(),
            );
        }
        Ok(())
    }
}

impl Default for PolicySection {
    fn default() -> Self {
        Self {
            tools: ToolsSection::default(),
            redact: Vec::new(),
            redact_regex: Vec::new(),
            fail_mode: PolicyFailMode::default(),
            decision_timeout_ms: 5000,
            on_malformed_event: MalformedMode::default(),
            session_memory: SessionMemorySection::default(),
            backend: PolicyBackendKind::default(),
            cel: CelSection::default(),
            rego: RegoSection::default(),
        }
    }
}

/// Which engine decides verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyBackendKind {
    /// The built-in static ruleset (tool allow/deny + redaction) — the default.
    #[default]
    Ruleset,
    /// The CEL plugin engine (operator rules from `[policy.cel]`); requires a
    /// build with the `policy-cel` feature.
    Cel,
    /// The Rego/OPA plugin engine (operator policy from `[policy.rego]`); requires
    /// a build with the `policy-rego` feature.
    Rego,
}

/// `[policy.cel]` — the CEL plugin engine. When `backend = "cel"`, the operator's
/// CEL rules at `policy_path` fully own the verdict (the static ruleset above is
/// not consulted).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CelSection {
    /// Path to the CEL policy file (a TOML list of `[[rule]]` entries). Required
    /// when `backend = "cel"`; the file is read and every rule is compiled at
    /// startup, so a parse error aborts the process.
    pub policy_path: Option<String>,
    /// Auto-reload the policy file when it changes on disk (in addition to the
    /// always-on `SIGHUP` reload). Off by default — file-watching has more moving
    /// parts (inotify limits, network filesystems that emit no events), so it is
    /// opt-in. A reload triggered by a watch is the same fail-safe reload as
    /// `SIGHUP`: a bad or truncated file keeps the running policy.
    pub watch: bool,
    /// Upper bound on the number of `[[rule]]` entries a policy may contain.
    /// Evaluation is synchronous and runs inline on a worker per event (see
    /// ADR-0001), so its cost is linear in the rule count; this caps that cost so
    /// no operator-authored policy can stall a worker. A policy exceeding it is
    /// rejected at load and at reload (the running policy is kept). Must be > 0.
    /// Generous by default — legitimate policies have tens to hundreds of rules.
    pub max_rules: usize,
}

impl Default for CelSection {
    fn default() -> Self {
        Self {
            policy_path: None,
            watch: false,
            max_rules: 1000,
        }
    }
}

/// `[policy.rego]` — the Rego (OPA) plugin engine. When `backend = "rego"`, the
/// operator's Rego policy at `policy_path` (package `agate.policy`, rule
/// `decision`) fully owns the verdict (the static ruleset above is not consulted).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RegoSection {
    /// Path to the Rego policy file. Required when `backend = "rego"`; it is read
    /// and compiled at startup, so a parse error aborts the process.
    pub policy_path: Option<String>,
    /// Auto-reload the policy file when it changes on disk (in addition to the
    /// always-on `SIGHUP` reload). Off by default; the same fail-safe reload.
    pub watch: bool,
}

/// `[policy.session_memory]` — cross-run replay protection. When enabled, a tool
/// denied in one run is quarantined (by name) for the rest of the session, so
/// the agent cannot retry it with varied arguments in a later run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionMemorySection {
    /// Enable the per-session replay ledger. Off by default (stateless policy).
    pub enabled: bool,
    /// How long a session's quarantine survives without activity, in seconds.
    /// A session idle longer than this is forgotten. Must be > 0 when enabled.
    pub ttl_secs: u64,
    /// Where the ledger lives: `memory` (process-local, single instance) or
    /// `redis` (shared across replicas and restarts).
    pub backend: SessionMemoryBackendKind,
    /// Redis connection URL (e.g. `redis://127.0.0.1:6379`). Required when
    /// `backend = "redis"`; ignored otherwise.
    pub redis_url: Option<String>,
}

/// Where the session-replay ledger is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionMemoryBackendKind {
    /// Process-local, lost on restart, not shared across replicas (the default).
    #[default]
    Memory,
    /// A shared Redis store (multi-replica, survives restarts).
    Redis,
}

impl Default for SessionMemorySection {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl_secs: 3600,
            backend: SessionMemoryBackendKind::default(),
            redis_url: None,
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
    /// Argument-level deny rules: a permitted tool call is still blocked when
    /// its arguments match one of these markers. Configured as
    /// `[[policy.tools.deny_arguments]]` tables.
    pub deny_arguments: Vec<ArgumentRuleConfig>,
    /// Result-level deny rules: a tool result is blocked when its content
    /// matches one of these markers. Configured as
    /// `[[policy.tools.deny_results]]` tables.
    pub deny_results: Vec<ResultRuleConfig>,
}

/// One `[[policy.tools.deny_arguments]]` entry: a marker forbidden in tool
/// arguments, optionally scoped to a single tool. Provide exactly one of
/// `contains` (a case-insensitive literal) or `matches` (a regex).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArgumentRuleConfig {
    /// The tool this rule applies to; omit (or leave blank) to apply to any tool.
    pub tool: Option<String>,
    /// A dotted path into the parsed arguments (`url`, `config.endpoint`) to
    /// match against. Omit to match the whole raw argument string. With a path,
    /// the marker is matched against just that field's value, so it cannot fire
    /// on an unrelated field carrying the same text.
    pub path: Option<String>,
    /// A literal forbidden in the arguments, folded ASCII-case-insensitively
    /// (not Unicode).
    pub contains: Option<String>,
    /// A regex forbidden in the arguments (full `regex` syntax; prefix `(?i)`
    /// for case-insensitivity). Matched against the raw argument JSON string,
    /// or against the `path` field's value when `path` is set.
    pub matches: Option<String>,
}

impl ArgumentRuleConfig {
    pub(super) fn to_rule(&self) -> Result<ArgumentRule, DomainError> {
        let (tool, path, marker) = rule_parts(
            self.tool.as_deref(),
            self.path.as_deref(),
            self.contains.as_ref(),
            self.matches.as_ref(),
            "deny_arguments",
        )?;
        let rule = ArgumentRule::new(tool, marker);
        Ok(match path {
            Some(path) => rule.with_path(path),
            None => rule,
        })
    }
}

/// One `[[policy.tools.deny_results]]` entry: a marker forbidden in a tool
/// *result*, optionally scoped to a single tool and/or one field of the parsed
/// result. Provide exactly one of `contains` or `matches` — same shape as
/// [`ArgumentRuleConfig`], applied to what a tool returns rather than its input.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResultRuleConfig {
    /// The tool this rule applies to; omit (or leave blank) to apply to any
    /// tool. Only fires when the result's tool is known and matches.
    pub tool: Option<String>,
    /// A dotted path into the parsed result JSON (`body`, `data.token`) to match
    /// against. Omit to match the whole raw result string.
    pub path: Option<String>,
    /// A literal forbidden in the result, folded ASCII-case-insensitively.
    pub contains: Option<String>,
    /// A regex forbidden in the result (full `regex` syntax; prefix `(?i)` for
    /// case-insensitivity).
    pub matches: Option<String>,
}

impl ResultRuleConfig {
    pub(super) fn to_rule(&self) -> Result<ResultRule, DomainError> {
        let (tool, path, marker) = rule_parts(
            self.tool.as_deref(),
            self.path.as_deref(),
            self.contains.as_ref(),
            self.matches.as_ref(),
            "deny_results",
        )?;
        let rule = ResultRule::new(tool, marker);
        Ok(match path {
            Some(path) => rule.with_path(path),
            None => rule,
        })
    }
}

/// Shared parsing for a deny rule's scope and marker: an optional tool scope, an
/// optional argument/result path, and exactly one of a literal (`contains`) or a
/// regex (`matches`) marker. `kind` names the config table in error messages.
fn rule_parts(
    tool: Option<&str>,
    path: Option<&str>,
    contains: Option<&String>,
    matches: Option<&String>,
    kind: &str,
) -> Result<(Option<ToolName>, Option<JsonPath>, Pattern), DomainError> {
    let tool = match tool.map(str::trim) {
        Some(name) if !name.is_empty() => Some(ToolName::new(name)?),
        _ => None,
    };
    let path = match path.map(str::trim) {
        Some(path) if !path.is_empty() => Some(JsonPath::parse(path)?),
        _ => None,
    };
    let marker = match (contains, matches) {
        (Some(literal), None) => Pattern::literal(literal)?,
        (None, Some(regex)) => Pattern::regex(regex)?,
        (Some(_), Some(_)) => {
            return Err(DomainError::Field(format!(
                "a {kind} rule sets exactly one of `contains` or `matches`, not both"
            )));
        }
        (None, None) => {
            return Err(DomainError::Field(format!(
                "a {kind} rule needs `contains` or `matches`"
            )));
        }
    };
    Ok((tool, path, marker))
}

/// How tool invocations are authorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolMode {
    /// No tool restriction (the permissive default).
    #[default]
    AllowAll,
    /// Only the tools in `names` may run; everything else is denied.
    Allowlist,
    /// Every tool may run except the ones in `names`.
    Denylist,
}
