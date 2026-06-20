//! The CEL policy engine: a [`PolicyPort`] backend that decides each event by
//! evaluating operator-authored CEL rules.
//!
//! A policy file is a TOML list of `[[rule]]` entries, each with a `when` CEL
//! boolean expression and an `effect` (`deny` / `redact` / `allow`). Rules are
//! evaluated in order; the first whose `when` is `true` wins. No match → allow.
//!
//! CEL is non-Turing-complete (no loops or recursion), so evaluation always
//! terminates — the per-decision timeout in `FailModePolicy` then reliably
//! bounds it, unlike a general interpreter that could spin a worker forever.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use arc_swap::ArcSwap;
use async_trait::async_trait;
use cel::{Context, Program, Value};
use serde::Deserialize;
use serde_json::{Map, json};
use tracing::warn;

use agate_policy::domain::decision::{DenyReason as PolicyDenyReason, PolicyDecision};
use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, DenyReason, Verdict};

use super::projection::lift_decision;

/// What a matched rule does. Mirrors [`PolicyDecision`] minus the data, which
/// comes from the rule's `reason` / `replacement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Effect {
    /// Block the event with the rule's `reason`.
    Deny,
    /// Replace the event's text with the rule's `replacement` (message / tool
    /// result only; other events pass through).
    Redact,
    /// Explicitly allow and stop evaluating further rules.
    Allow,
}

/// One `[[rule]]` from the policy file (before compilation).
#[derive(Debug, Deserialize)]
struct RuleConfig {
    /// CEL boolean expression over `action` and `context`.
    when: String,
    /// What to do when `when` is true.
    effect: Effect,
    /// Deny message (for `effect = "deny"`); defaults to a generic reason.
    #[serde(default)]
    reason: Option<String>,
    /// CEL string expression producing the replacement text (for
    /// `effect = "redact"`); defaults to `"[REDACTED]"`.
    #[serde(default)]
    replacement: Option<String>,
}

/// The policy file: a list of `[[rule]]` tables.
#[derive(Debug, Deserialize)]
struct PolicyFile {
    #[serde(default)]
    rule: Vec<RuleConfig>,
}

/// A compiled rule ready to evaluate per event.
struct CompiledRule {
    when: Program,
    effect: Effect,
    reason: String,
    replacement: Option<Program>,
}

const DEFAULT_DENY_REASON: &str = "denied by policy";
const DEFAULT_REDACTION: &str = "[REDACTED]";

/// A [`PolicyPort`] backend evaluating compiled CEL rules. The rule set is held
/// behind an [`ArcSwap`] so a future hot-reload can swap it without a lock on
/// the hot path.
pub struct CelPolicyAdapter {
    rules: Arc<ArcSwap<Vec<CompiledRule>>>,
}

impl CelPolicyAdapter {
    /// Read and compile the policy at `path`. Every rule's CEL is compiled now,
    /// so a parse error aborts startup with a message naming the offending rule.
    pub fn load(path: &str) -> Result<Self, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read CEL policy file '{path}': {error}"))?;
        Self::from_source(&source).map_err(|error| format!("in CEL policy file '{path}': {error}"))
    }

    /// Compile a policy from its TOML source (a list of `[[rule]]` tables).
    fn from_source(source: &str) -> Result<Self, String> {
        let file: PolicyFile =
            toml::from_str(source).map_err(|error| format!("cannot parse CEL policy: {error}"))?;
        let rules = file
            .rule
            .into_iter()
            .enumerate()
            .map(|(index, rule)| Self::compile(index, rule))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            rules: Arc::new(ArcSwap::from_pointee(rules)),
        })
    }

    fn compile(index: usize, rule: RuleConfig) -> Result<CompiledRule, String> {
        let when = Program::compile(&rule.when)
            .map_err(|error| format!("CEL rule #{index} `when` does not compile: {error}"))?;
        let replacement = rule
            .replacement
            .map(|source| Program::compile(&source))
            .transpose()
            .map_err(|error| {
                format!("CEL rule #{index} `replacement` does not compile: {error}")
            })?;
        Ok(CompiledRule {
            when,
            effect: rule.effect,
            reason: rule
                .reason
                .unwrap_or_else(|| DEFAULT_DENY_REASON.to_owned()),
            replacement,
        })
    }
}

#[async_trait]
impl PolicyPort for CelPolicyAdapter {
    async fn decide(&self, context: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
        // CEL evaluation is synchronous and expected to be panic-free (the
        // interpreter returns `Err` on bad input). A bug there must still never
        // unwind the proxy's response stream, so the whole evaluation runs inside
        // a panic boundary and a panic fails *closed* — the `FailModePolicy`
        // decorator only guards the timeout, not panics.
        catch_unwind(AssertUnwindSafe(|| self.evaluate(context, event))).unwrap_or_else(|_| {
            warn!("CEL policy evaluation panicked; failing closed");
            Verdict::Deny(DenyReason::new("policy evaluation failed"))
        })
    }
}

impl CelPolicyAdapter {
    /// Evaluate the rules against `event` (synchronous; the panic boundary lives
    /// in [`PolicyPort::decide`]). The **first** rule whose `when` is `true` wins.
    /// A rule that cannot be reduced to a boolean — an evaluation error for this
    /// event (e.g. a field absent for its kind) or a non-boolean result (an
    /// operator authoring mistake) — is **logged and skipped**, never a hard
    /// failure and never a wrong allow of something a *later* rule denies. No rule
    /// matching means the operator did not block this event, so it is allowed.
    fn evaluate(&self, context: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
        let Some(ctx) = build_context(context, event) else {
            // build_context already logged the binding failure.
            return Verdict::Deny(DenyReason::new("policy input could not be evaluated"));
        };

        for (index, rule) in self.rules.load().iter().enumerate() {
            match rule.when.execute(&ctx) {
                Ok(Value::Bool(true)) => return lift_decision(event, effect_of(index, rule, &ctx)),
                Ok(Value::Bool(false)) => {}
                Ok(_) => warn!(
                    rule = index,
                    "CEL rule `when` did not evaluate to a boolean; treating the rule as not matched"
                ),
                Err(error) => warn!(
                    rule = index,
                    %error,
                    "CEL rule evaluation error; treating the rule as not matched"
                ),
            }
        }
        Verdict::Allow
    }
}

/// Turn a matched rule into a [`PolicyDecision`].
fn effect_of(index: usize, rule: &CompiledRule, ctx: &Context<'_>) -> PolicyDecision {
    match rule.effect {
        Effect::Allow => PolicyDecision::Allow,
        Effect::Deny => PolicyDecision::Deny(PolicyDenyReason::new(rule.reason.clone())),
        Effect::Redact => PolicyDecision::RedactText(replacement_text(index, rule, ctx)),
    }
}

/// Evaluate a redact rule's `replacement` expression to its string result. With
/// no replacement, or when it errors or yields a non-string, fall back to the
/// default marker — and log the misconfiguration so it is never silent. The
/// replacement sees the full context, so an operator who writes `action.text` as
/// the replacement masks a secret to itself; that is trusted-operator footgun,
/// documented in the configuration reference.
fn replacement_text(index: usize, rule: &CompiledRule, ctx: &Context<'_>) -> String {
    let Some(program) = rule.replacement.as_ref() else {
        return DEFAULT_REDACTION.to_owned();
    };
    match program.execute(ctx) {
        Ok(Value::String(text)) => text.to_string(),
        Ok(_) => {
            warn!(
                rule = index,
                "CEL `replacement` did not return a string; using the default marker"
            );
            DEFAULT_REDACTION.to_owned()
        }
        Err(error) => {
            warn!(
                rule = index,
                %error,
                "CEL `replacement` evaluation error; using the default marker"
            );
            DEFAULT_REDACTION.to_owned()
        }
    }
}

/// Build the CEL evaluation context: bind `action` (a flat map describing the
/// event, every key present — `null` when not applicable, so a rule referencing
/// any field never errors on a missing key) and `context` (the run identity).
fn build_context(context: &InspectionContext, event: &AgentEvent) -> Option<Context<'static>> {
    let action = action_value(event);
    let run_context = json!({
        "session_id": context.session.to_string(),
        "run_id": context.run.to_string(),
    });
    let mut ctx = Context::default();
    ctx.add_variable_from_value("action", bind(&action, "action")?);
    ctx.add_variable_from_value("context", bind(&run_context, "context")?);
    Some(ctx)
}

/// Convert a JSON value to a CEL value. On failure, log which variable could not
/// be bound and yield `None` so the caller fails closed (denies the event).
fn bind(value: &serde_json::Value, name: &str) -> Option<Value> {
    match cel::to_value(value) {
        Ok(value) => Some(value),
        Err(error) => {
            warn!(variable = name, %error, "failed to bind CEL variable; failing closed");
            None
        }
    }
}

/// Project an event onto the flat `action` map the rules see. Strings that hold
/// JSON (tool arguments, results, state) are also offered parsed under a
/// `*_json` key so rules can address fields (`action.arguments_json.url`).
fn action_value(event: &AgentEvent) -> serde_json::Value {
    let mut map = Map::new();
    map.insert("name".into(), serde_json::Value::Null);
    map.insert("arguments".into(), serde_json::Value::Null);
    map.insert("arguments_json".into(), serde_json::Value::Null);
    map.insert("text".into(), serde_json::Value::Null);
    map.insert("content".into(), serde_json::Value::Null);
    map.insert("content_json".into(), serde_json::Value::Null);
    map.insert("state_json".into(), serde_json::Value::Null);

    let kind = match event {
        AgentEvent::ToolCall {
            name, arguments, ..
        } => {
            map.insert("name".into(), json!(name));
            map.insert("arguments".into(), json!(arguments));
            map.insert("arguments_json".into(), parsed(arguments));
            "tool_call"
        }
        AgentEvent::MessageChunk { text, .. } => {
            map.insert("text".into(), json!(text));
            "message"
        }
        AgentEvent::ToolResult { name, content, .. } => {
            map.insert("name".into(), json!(name));
            map.insert("content".into(), json!(content));
            map.insert("content_json".into(), parsed(content));
            "tool_result"
        }
        AgentEvent::StateMutation(mutation) => {
            map.insert("state_json".into(), parsed(mutation.payload()));
            "state"
        }
        AgentEvent::Lifecycle(_) | AgentEvent::Opaque(_) => "other",
    };
    map.insert("kind".into(), json!(kind));
    serde_json::Value::Object(map)
}

/// Parse `raw` as JSON, or `null` if it is not valid JSON.
fn parsed(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use agate_proxy::application::common::ports::PolicyPort;
    use agate_proxy::application::inspection::InspectionContext;
    use agate_proxy::domain::inspection::{
        AgentEvent, MessageId, RunId, SessionId, StateMutation, ToolCallId, Verdict,
    };
    use uuid::Uuid;

    use super::CelPolicyAdapter;

    fn engine(source: &str) -> CelPolicyAdapter {
        CelPolicyAdapter::from_source(source).expect("policy compiles")
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

    #[tokio::test]
    async fn no_rules_allows_everything() {
        let engine = engine("");
        assert_eq!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn denies_a_tool_by_name() {
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.kind == "tool_call" && action.name == "rm"'
            effect = "deny"
            reason = "rm is forbidden"
        "#,
        );
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
    async fn the_first_matching_rule_wins() {
        // An explicit allow placed before a deny shadows it (first true wins).
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.name == "rm"'
            effect = "allow"
            [[rule]]
            when = 'action.name == "rm"'
            effect = "deny"
        "#,
        );
        assert_eq!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Allow
        );
    }

    #[tokio::test]
    async fn redacts_a_message_with_the_replacement_keeping_id() {
        // `replacement` is a CEL *string expression*, hence the inner quotes.
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.kind == "message"'
            effect = "redact"
            replacement = '"[GONE]"'
        "#,
        );
        match engine.decide(&context(), &message("secret stuff")).await {
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
            r#"
            [[rule]]
            when = 'action.kind == "message"'
            effect = "redact"
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
    async fn redacts_a_tool_result_keeping_id_and_name() {
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.kind == "tool_result"'
            effect = "redact"
            replacement = '"[SCRUBBED]"'
        "#,
        );
        let event = AgentEvent::ToolResult {
            id: ToolCallId::new("c1").expect("valid id"),
            name: Some("fetch".into()),
            content: "leak".into(),
        };
        match engine.decide(&context(), &event).await {
            Verdict::Transform(AgentEvent::ToolResult { id, name, content }) => {
                assert_eq!(id, ToolCallId::new("c1").expect("valid id"));
                assert_eq!(name, Some("fetch".into()));
                assert_eq!(content, "[SCRUBBED]");
            }
            other => panic!("expected a tool-result transform, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn matches_against_parsed_json_arguments() {
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.arguments_json.path == "/etc/passwd"'
            effect = "deny"
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
    async fn an_unguarded_rule_error_is_treated_as_not_matched() {
        // The rule reaches into parsed-JSON arguments, but the call's arguments
        // are not JSON, so `arguments_json` is null and the field access does not
        // produce a match. The rule is skipped (logged), never a hard failure —
        // and with no rule matching, the event is allowed. This documents the
        // per-rule fail-open operators must guard against (see the null-guard
        // note in the CEL docs): an unguarded deny rule that errors does not
        // block.
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.arguments_json.url == "http://x"'
            effect = "deny"
        "#,
        );
        assert_eq!(
            engine.decide(&context(), &tool("fetch", "not json")).await,
            Verdict::Allow
        );
        // The same rule still fires on a well-formed match.
        assert!(matches!(
            engine
                .decide(&context(), &tool("fetch", r#"{"url":"http://x"}"#))
                .await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn matches_against_parsed_state_json() {
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.kind == "state" && action.state_json.secret == true'
            effect = "deny"
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
            r#"
            [[rule]]
            when = 'context.run_id == "00000000-0000-0000-0000-000000000000"'
            effect = "deny"
        "#,
        );
        assert!(matches!(
            engine.decide(&context(), &message("hi")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn a_non_boolean_when_is_skipped_not_matched() {
        // A `when` that evaluates to a non-boolean (an operator authoring mistake)
        // is logged and skipped — it must NOT short-circuit as a match. Here the
        // broken `allow` rule precedes a valid `deny`, which still fires.
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.name'
            effect = "allow"
            [[rule]]
            when = 'action.name == "rm"'
            effect = "deny"
        "#,
        );
        assert!(matches!(
            engine.decide(&context(), &tool("rm", "{}")).await,
            Verdict::Deny(_)
        ));
    }

    #[tokio::test]
    async fn redacting_a_non_rewritable_event_fails_closed() {
        // A redact rule can be written against a tool call, but a tool call has no
        // rewritable text — the lift fails closed (deny), never leaking it.
        let engine = engine(
            r#"
            [[rule]]
            when = 'action.kind == "tool_call"'
            effect = "redact"
        "#,
        );
        assert!(matches!(
            engine.decide(&context(), &tool("fetch", "{}")).await,
            Verdict::Deny(_)
        ));
    }

    #[test]
    fn rejects_an_uncompilable_when_expression() {
        let Err(error) = CelPolicyAdapter::from_source(
            r#"
            [[rule]]
            when = "1 +"
            effect = "deny"
        "#,
        ) else {
            panic!("an incomplete CEL expression must not compile");
        };
        assert!(error.contains("does not compile"), "got: {error}");
    }

    #[test]
    fn rejects_an_uncompilable_replacement_expression() {
        let Err(error) = CelPolicyAdapter::from_source(
            r#"
            [[rule]]
            when = "true"
            effect = "redact"
            replacement = "1 +"
        "#,
        ) else {
            panic!("an incomplete replacement expression must not compile");
        };
        assert!(error.contains("replacement"), "got: {error}");
    }

    #[test]
    fn rejects_malformed_policy_toml() {
        let Err(error) = CelPolicyAdapter::from_source("this is = = not toml") else {
            panic!("malformed TOML must not parse");
        };
        assert!(error.contains("cannot parse"), "got: {error}");
    }
}
