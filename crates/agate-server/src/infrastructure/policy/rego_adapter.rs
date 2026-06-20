//! The Rego policy engine: a [`PolicyPort`] backend that decides each event by
//! evaluating an operator's [Rego](https://www.openpolicyagent.org/docs/latest/policy-language/)
//! (the OPA policy language) policy through the pure-Rust `regorus` interpreter.
//!
//! The policy lives under package `agate.policy` and defines a `decision` rule.
//! For each event the adapter sets `input` to `{ "action": …, "context": … }`
//! (the same `event_view` projection the CEL backend sees) and evaluates
//! `data.agate.policy.decision`, expecting an object:
//!
//! ```rego
//! package agate.policy
//! import rego.v1
//! decision := {"effect": "deny", "reason": "…"} if { input.action.name == "rm" }
//! ```
//!
//! `effect` is `allow` / `deny` / `redact`. An **undefined** decision (the policy
//! matched nothing) allows the event — the operator's rules enumerate what is
//! blocked. An evaluation error or a malformed decision **fails closed** (deny).
//! Rego is non-Turing-complete, so evaluation always terminates.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use regorus::{Engine, Value};
use tracing::warn;

use agate_policy::domain::decision::{DenyReason as PolicyDenyReason, PolicyDecision};
use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, DenyReason, Verdict};

use super::ReloadablePolicy;
use super::event_view;
use super::projection::lift_decision;

/// The query evaluated for every event; the operator's policy must define it.
const DECISION_QUERY: &str = "data.agate.policy.decision";
/// The package the policy must declare, as `add_policy` reports it (the `data.`
/// prefix is how regorus namespaces a Rego `package`). A policy under any other
/// package would make [`DECISION_QUERY`] resolve to *undefined* — i.e. allow-all
/// — so a mismatch is rejected at startup rather than silently weakening the gate.
const EXPECTED_PACKAGE: &str = "data.agate.policy";
const POLICY_NAME: &str = "policy.rego";
const DEFAULT_DENY_REASON: &str = "denied by policy";
const DEFAULT_REDACTION: &str = "[REDACTED]";

/// A [`PolicyPort`] backend evaluating a compiled Rego policy. The engine is held
/// behind an [`ArcSwap`] so [`reload`](Self::reload) swaps it atomically without
/// a lock on the hot path; each decision clones the engine (regorus evaluation
/// takes `&mut Engine`, and a clone is cheap and shares nothing mutable, so
/// concurrent decisions never contend).
pub struct RegoPolicyAdapter {
    engine: Arc<ArcSwap<Engine>>,
    /// The source file, kept so the policy can be reloaded in place. `None` when
    /// built from an in-memory source (tests); reloading then errors.
    path: Option<PathBuf>,
}

impl RegoPolicyAdapter {
    /// Read and compile the policy at `path`. A parse error aborts startup with a
    /// message naming the file.
    pub fn load(path: &str) -> Result<Self, String> {
        let engine = Self::build_engine(Path::new(path))?;
        Ok(Self {
            engine: Arc::new(ArcSwap::from_pointee(engine)),
            path: Some(PathBuf::from(path)),
        })
    }

    /// Re-read and recompile the policy file, swapping the live engine on success.
    /// **Fail-safe:** a missing, empty, or uncompilable file leaves the current
    /// engine untouched (the gateway keeps enforcing the last known-good policy)
    /// and returns the error for the caller to log. Lock-free: a decision already
    /// in flight keeps the engine snapshot it cloned.
    pub fn reload(&self) -> Result<(), String> {
        let path = self
            .path
            .as_deref()
            .ok_or("Rego policy has no source file to reload")?;
        let engine = Self::build_engine(path)?;
        self.engine.store(Arc::new(engine));
        Ok(())
    }

    /// Read `path` and compile it into an engine, prefixing errors with the file
    /// name. An empty file is rejected (an empty policy decides nothing, i.e.
    /// allow-all — almost always a truncated write, never the intent on reload).
    fn build_engine(path: &Path) -> Result<Engine, String> {
        let source = std::fs::read_to_string(path).map_err(|error| {
            format!("cannot read Rego policy file '{}': {error}", path.display())
        })?;
        if source.trim().is_empty() {
            return Err(format!("Rego policy file '{}' is empty", path.display()));
        }
        Self::compile(&source)
            .map_err(|error| format!("in Rego policy file '{}': {error}", path.display()))
    }

    /// Compile a policy from its Rego source into a ready-to-evaluate engine, and
    /// verify it declares `package agate.policy` — otherwise the decision query
    /// would resolve to undefined (allow-all), so a wrong/typo'd package is a
    /// startup error, not a silent open gate.
    fn compile(source: &str) -> Result<Engine, String> {
        let mut engine = Engine::new();
        let package = engine
            .add_policy(POLICY_NAME.to_string(), source.to_string())
            .map_err(|error| format!("Rego policy does not compile: {error}"))?;
        if package != EXPECTED_PACKAGE {
            let found = package.strip_prefix("data.").unwrap_or(&package);
            return Err(format!(
                "Rego policy must declare `package agate.policy` (found `{found}`); otherwise \
                 `{DECISION_QUERY}` is undefined and every event would be allowed"
            ));
        }
        Ok(engine)
    }

    /// Evaluate the policy against `event` (synchronous; the panic boundary lives
    /// in [`PolicyPort::decide`]).
    fn evaluate(&self, context: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
        let input = event_view::input_value(context, event);
        let Some(input) = to_rego_value(&input) else {
            return Verdict::Deny(DenyReason::new("policy input could not be evaluated"));
        };

        // Clone the prepared engine for this decision: regorus eval needs `&mut`,
        // and the clone shares no mutable state, so concurrent decisions are safe.
        let mut engine = (*self.engine.load_full()).clone();
        engine.set_input(input);

        let results = match engine.eval_query(DECISION_QUERY.to_string(), false) {
            Ok(results) => results,
            Err(error) => {
                warn!(%error, "Rego policy evaluation error; failing closed");
                return Verdict::Deny(DenyReason::new("policy evaluation failed"));
            }
        };

        // An undefined `decision` yields no result — either the operator's rules
        // did not match, or (the package guard aside) no `decision` rule was
        // defined at all. Both mean the operator did not block this event, so it
        // is allowed (mirrors the CEL no-match default). Add a catch-all
        // `decision` for default-deny.
        let Some(value) = results
            .result
            .first()
            .and_then(|result| result.expressions.first())
            .map(|expression| &expression.value)
        else {
            return Verdict::Allow;
        };

        lift_decision(event, to_decision(value))
    }
}

#[async_trait]
impl PolicyPort for RegoPolicyAdapter {
    async fn decide(&self, context: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
        // Rego evaluation is synchronous and expected to be panic-free; a bug in
        // the interpreter must still never unwind the proxy stream, so the whole
        // evaluation runs inside a panic boundary that fails closed. `AssertUnwindSafe`
        // is sound: evaluate() only mutates a local engine clone, so a panic cannot
        // corrupt `self.engine` or any other in-flight decision.
        catch_unwind(AssertUnwindSafe(|| self.evaluate(context, event))).unwrap_or_else(|_| {
            warn!("Rego policy evaluation panicked; failing closed");
            Verdict::Deny(DenyReason::new("policy evaluation failed"))
        })
    }
}

impl ReloadablePolicy for RegoPolicyAdapter {
    fn reload_policy(&self) -> Result<(), String> {
        self.reload()
    }
}

/// Map the policy's `decision` object onto a [`PolicyDecision`]. A non-object
/// result, or one whose `effect` is missing or unknown, **fails closed** (deny) —
/// a malformed policy output must never silently allow.
fn to_decision(value: &Value) -> PolicyDecision {
    let Ok(object) = value.as_object() else {
        warn!("Rego `decision` is not an object; failing closed");
        return PolicyDecision::Deny(PolicyDenyReason::new(
            "policy returned a non-object decision",
        ));
    };
    // Read a string field from the decision object (`None` if absent/not a string).
    let field = |key: &str| {
        object
            .get(&Value::from(key))
            .and_then(|value| value.as_string().ok())
            .map(ToString::to_string)
    };
    match field("effect").as_deref() {
        Some("allow") => PolicyDecision::Allow,
        Some("deny") => PolicyDecision::Deny(PolicyDenyReason::new(
            field("reason").unwrap_or_else(|| DEFAULT_DENY_REASON.to_owned()),
        )),
        Some("redact") => PolicyDecision::RedactText(
            field("replacement").unwrap_or_else(|| DEFAULT_REDACTION.to_owned()),
        ),
        other => {
            warn!(effect = ?other, "Rego `decision.effect` missing or unknown; failing closed");
            PolicyDecision::Deny(PolicyDenyReason::new(
                "policy decision had no recognized effect",
            ))
        }
    }
}

/// Convert the JSON event projection into a Rego value, logging and yielding
/// `None` (so the caller fails closed) if the conversion fails.
fn to_rego_value(input: &serde_json::Value) -> Option<Value> {
    let encoded = serde_json::to_string(input)
        .inspect_err(|error| warn!(%error, "failed to serialize the Rego input; failing closed"))
        .ok()?;
    match Value::from_json_str(&encoded) {
        Ok(value) => Some(value),
        Err(error) => {
            warn!(%error, "failed to bind the Rego input; failing closed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use agate_proxy::application::common::ports::PolicyPort;
    use agate_proxy::application::inspection::InspectionContext;
    use agate_proxy::domain::inspection::{
        AgentEvent, MessageId, RunId, SessionId, StateMutation, ToolCallId, Verdict,
    };
    use uuid::Uuid;

    use super::RegoPolicyAdapter;

    impl RegoPolicyAdapter {
        /// Build from an in-memory Rego source (test-only; production loads from a
        /// file so it can be reloaded).
        fn from_source(source: &str) -> Result<Self, String> {
            let engine = Self::compile(source)?;
            Ok(Self {
                engine: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(engine)),
                path: None,
            })
        }
    }

    fn engine(source: &str) -> RegoPolicyAdapter {
        RegoPolicyAdapter::from_source(source).expect("policy compiles")
    }

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()))
    }

    fn tool(name: &str, arguments: &str) -> AgentEvent {
        AgentEvent::ToolCall {
            id: ToolCallId::new("c").expect("valid id"),
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    fn message(text: &str) -> AgentEvent {
        AgentEvent::MessageChunk {
            message: MessageId::new("m1").expect("valid id"),
            text: text.into(),
        }
    }

    const DENY_RM: &str = r#"package agate.policy
import rego.v1
decision := {"effect": "deny", "reason": "rm forbidden"} if {
    input.action.kind == "tool_call"
    input.action.name == "rm"
}
"#;

    #[tokio::test]
    async fn denies_a_tool_by_name() {
        let engine = engine(DENY_RM);
        assert!(matches!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Deny(_)
        ));
        assert_eq!(
            engine.decide(&context(), &tool("ls", "{}")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn an_undefined_decision_allows() {
        // A policy that defines the rule but never matches → undefined → allow.
        let engine = engine(DENY_RM);
        assert_eq!(
            engine.decide(&context(), &message("hello")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn redacts_a_message_keeping_id() {
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "redact", "replacement": "[GONE]"} if {
    input.action.kind == "message"
}
"#,
        );
        match engine.decide(&context(), &message("secret")).await {
            Verdict::Transform(AgentEvent::MessageChunk { message, text }) => {
                assert_eq!(message, MessageId::new("m1").expect("valid id"));
                assert_eq!(text, "[GONE]");
            }
            other => panic!("expected a message transform, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn redacts_with_the_default_marker_when_no_replacement() {
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "redact"} if { input.action.kind == "message" }
"#,
        );
        match engine.decide(&context(), &message("x")).await {
            Verdict::Transform(AgentEvent::MessageChunk { text, .. }) => {
                assert_eq!(text, "[REDACTED]");
            }
            other => panic!("expected a transform, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn matches_against_parsed_json_arguments() {
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "deny"} if {
    input.action.arguments_json.path == "/etc/passwd"
}
"#,
        );
        assert!(matches!(
            engine
                .decide(&context(), &tool("read", r#"{"path":"/etc/passwd"}"#))
                .await,
            Verdict::Deny(_)
        ));
        assert_eq!(
            engine
                .decide(&context(), &tool("read", r#"{"path":"/tmp/ok"}"#))
                .await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn matches_against_state_json() {
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "deny"} if {
    input.action.kind == "state"
    input.action.state_json.secret == true
}
"#,
        );
        let event = AgentEvent::StateMutation(StateMutation::Snapshot {
            byte_size: 16,
            payload: r#"{"secret":true}"#.into(),
        });
        assert!(matches!(
            engine.decide(&context(), &event).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn binds_the_run_context() {
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "deny"} if {
    input.context.run_id == "00000000-0000-0000-0000-000000000000"
}
"#,
        );
        assert!(matches!(
            engine.decide(&context(), &message("hi")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn a_non_object_decision_fails_closed() {
        // The policy resolves `decision` to a string, not the expected object.
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := "oops" if { input.action.kind == "message" }
"#,
        );
        assert!(matches!(
            engine.decide(&context(), &message("x")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn an_unknown_effect_fails_closed() {
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "maybe"} if { input.action.kind == "message" }
"#,
        );
        assert!(matches!(
            engine.decide(&context(), &message("x")).await,
            Verdict::Deny(_)
        ));
    }

    #[test]
    fn rejects_an_uncompilable_policy() {
        let Err(error) = RegoPolicyAdapter::from_source("package agate.policy\ndecision := {{{")
        else {
            panic!("an uncompilable Rego policy must not load");
        };
        assert!(error.contains("does not compile"), "got: {error}");
    }

    #[test]
    fn reloading_an_in_memory_policy_errors() {
        let Err(error) = engine(DENY_RM).reload() else {
            panic!("an in-memory policy has no file to reload");
        };
        assert!(error.contains("no source file"), "got: {error}");
    }

    #[test]
    fn rejects_a_policy_in_the_wrong_package() {
        // A policy under any other package would make the decision query resolve
        // to undefined (allow-all), so a wrong/typo'd package is rejected.
        let Err(error) = RegoPolicyAdapter::from_source(
            "package wrong.place\nimport rego.v1\ndecision := {\"effect\": \"deny\"} if { true }\n",
        ) else {
            panic!("a policy in the wrong package must not load");
        };
        assert!(error.contains("package agate.policy"), "got: {error}");
    }

    #[tokio::test]
    async fn a_policy_without_a_decision_rule_allows() {
        // Documented behavior: the right package but no `decision` rule resolves to
        // undefined → allow (the package guard catches the common typo, but an
        // absent rule is indistinguishable from a non-match at runtime).
        let engine = engine("package agate.policy\nimport rego.v1\n");
        assert_eq!(
            engine.decide(&context(), &message("x")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn redacting_a_non_rewritable_event_fails_closed() {
        // A redact decision on a tool call (no rewritable text) fails closed via
        // the shared lift, never leaking the call.
        let engine = engine(
            r#"package agate.policy
import rego.v1
decision := {"effect": "redact"} if { input.action.kind == "tool_call" }
"#,
        );
        assert!(matches!(
            engine.decide(&context(), &tool("fetch", "{}")).await,
            Verdict::Deny(_)
        ));
    }

    /// Write `source` to a fresh temp file and load an engine from it; the file is
    /// returned so the caller can rewrite it and keeps it alive for the test.
    fn loaded(source: &str) -> (tempfile::NamedTempFile, RegoPolicyAdapter) {
        let file = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(file.path(), source).expect("write policy");
        let engine =
            RegoPolicyAdapter::load(file.path().to_str().expect("utf-8 path")).expect("compiles");
        (file, engine)
    }

    const DENY_LS: &str = r#"package agate.policy
import rego.v1
decision := {"effect": "deny"} if {
    input.action.kind == "tool_call"
    input.action.name == "ls"
}
"#;

    #[tokio::test]
    async fn reload_swaps_the_engine() {
        let (file, engine) = loaded(DENY_RM);
        assert!(matches!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Deny(_)
        ));

        std::fs::write(file.path(), DENY_LS).expect("rewrite policy");
        engine.reload().expect("reload");
        assert_eq!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Allow
        );
        assert!(matches!(
            engine.decide(&context(), &tool("ls", "{}")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn a_failed_reload_keeps_the_current_engine() {
        let (file, engine) = loaded(DENY_RM);
        std::fs::write(file.path(), "package agate.policy\ndecision := {{{").expect("corrupt");
        let Err(error) = engine.reload() else {
            panic!("an uncompilable reload must fail");
        };
        assert!(error.contains("does not compile"), "got: {error}");
        // Still enforcing the original rule.
        assert!(matches!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn a_reload_to_an_empty_policy_is_refused() {
        let (file, engine) = loaded(DENY_RM);
        std::fs::write(file.path(), "").expect("truncate");
        let Err(error) = engine.reload() else {
            panic!("a reload to an empty file must be refused");
        };
        assert!(error.contains("empty"), "got: {error}");
        assert!(matches!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Deny(_)
        ));
    }
}
