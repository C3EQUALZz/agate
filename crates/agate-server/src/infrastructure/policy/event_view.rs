//! The event projection both policy engines evaluate against: a flat `action`
//! map describing the inspected event, plus a `context` map carrying the run
//! identity. Shared by the CEL and Rego backends so they see an *identical* view
//! and their semantics cannot drift.

use serde_json::{Map, json};

use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::AgentEvent;

/// Project an event onto the flat `action` map the policies see. Every key is
/// always present (`null` when not applicable), so a policy may reference any
/// field without erroring on a missing key. Strings that hold JSON (tool
/// arguments, results, state) are also offered **parsed** under a `*_json` key so
/// a policy can address fields (`action.arguments_json.url`).
pub(crate) fn action_value(event: &AgentEvent) -> serde_json::Value {
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

/// The run identity a policy can branch on: `session_id` and `run_id`.
pub(crate) fn run_context(context: &InspectionContext) -> serde_json::Value {
    json!({
        "session_id": context.session.to_string(),
        "run_id": context.run.to_string(),
    })
}

/// The combined `{ "action": …, "context": … }` object — the Rego `input`. The
/// CEL backend binds the two halves as separate variables instead.
#[cfg(feature = "policy-rego")]
pub(crate) fn input_value(context: &InspectionContext, event: &AgentEvent) -> serde_json::Value {
    json!({
        "action": action_value(event),
        "context": run_context(context),
    })
}

/// Parse `raw` as JSON, or `null` if it is not valid JSON.
fn parsed(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or(serde_json::Value::Null)
}
