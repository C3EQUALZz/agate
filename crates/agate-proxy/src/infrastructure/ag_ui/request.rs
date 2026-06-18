//! Parse a `RunAgentInput` body into the facts the request leg inspects.

use serde_json::{Map, Value};

use super::error::AgUiError;
use crate::application::inspection::RequestContent;

/// Extract the offered tool names and the user message texts from a
/// `RunAgentInput` JSON body.
///
/// **Fail-closed:** security-relevant fields are validated strictly — a present
/// `tools`/`messages` that is not an array, a tool without a string `name`, or a
/// user message without a string `content` is a [`AgUiError::MalformedRequest`]
/// (HTTP 400), not a silently-skipped item. A malformed request must never slip
/// past inspection by parsing to empty facts. (Absent fields are fine — they
/// simply contribute nothing to inspect.)
pub fn parse_request(body: &[u8]) -> Result<RequestContent, AgUiError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| AgUiError::MalformedRequest)?;
    let object = value.as_object().ok_or(AgUiError::MalformedRequest)?;

    let (user_messages, system_messages) = parse_messages(object)?;
    let mut hidden_fields = system_messages;
    // The structured fields an injection can hide in. Collect their string
    // *leaves* (not the compact JSON blob) so a nested URL becomes a token the
    // SSRF guard can parse — `{"url":"http://h/x"}` would not. Absent fields add
    // nothing.
    for key in ["context", "forwardedProps", "state"] {
        if let Some(value) = object.get(key) {
            collect_string_leaves(value, &mut hidden_fields);
        }
    }

    Ok(RequestContent {
        thread_id: parse_identifier(object, "threadId"),
        run_id: parse_identifier(object, "runId"),
        offered_tools: parse_offered_tools(object)?,
        user_messages,
        hidden_fields,
    })
}

/// Read a string identifier (`threadId` / `runId`) from the input. A missing,
/// non-string, or blank value yields `None` — the run is then treated as a
/// one-off session rather than failing the request (these ids scope state, they
/// are not a security gate).
fn parse_identifier(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

fn parse_offered_tools(object: &Map<String, Value>) -> Result<Vec<String>, AgUiError> {
    let Some(tools) = object.get("tools") else {
        return Ok(Vec::new());
    };
    let tools = tools.as_array().ok_or(AgUiError::MalformedRequest)?;
    tools
        .iter()
        .map(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(AgUiError::MalformedRequest)
        })
        .collect()
}

/// Gather every string leaf of a JSON value into `out` (recursing through
/// arrays and objects). Numbers, booleans, and null carry no injectable text,
/// so they are skipped. Screening leaves individually lets the SSRF guard parse
/// a URL that would be unreachable inside a compact JSON blob.
fn collect_string_leaves(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_string_leaves(item, out);
            }
        }
        Value::Object(fields) => {
            for field in fields.values() {
                collect_string_leaves(field, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Split message content by role into `(user, system)` texts — both are
/// screened (system prompts are an injection surface too); other roles are
/// skipped. Fail-closed: a present `messages` that is not an array, or a
/// `user`/`system` message without string `content`, is a malformed request.
fn parse_messages(object: &Map<String, Value>) -> Result<(Vec<String>, Vec<String>), AgUiError> {
    let Some(messages) = object.get("messages") else {
        return Ok((Vec::new(), Vec::new()));
    };
    let messages = messages.as_array().ok_or(AgUiError::MalformedRequest)?;
    let mut user = Vec::new();
    let mut system = Vec::new();
    for message in messages {
        let fields = message.as_object().ok_or(AgUiError::MalformedRequest)?;
        let role = fields
            .get("role")
            .and_then(Value::as_str)
            .ok_or(AgUiError::MalformedRequest)?;
        let bucket = match role {
            "user" => &mut user,
            "system" => &mut system,
            _ => continue, // only user and system messages are inspected
        };
        let content = fields
            .get("content")
            .and_then(Value::as_str)
            .ok_or(AgUiError::MalformedRequest)?;
        bucket.push(content.to_owned());
    }
    Ok((user, system))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_offered_tools_and_user_messages() {
        let body = br#"{
            "threadId": "t", "runId": "r",
            "messages": [
                {"id": "m1", "role": "user", "content": "hello"},
                {"id": "m2", "role": "assistant", "content": "hi"}
            ],
            "tools": [{"name": "search"}, {"name": "delete_file"}]
        }"#;
        let content = parse_request(body).expect("valid body");
        assert_eq!(content.offered_tools, vec!["search", "delete_file"]);
        assert_eq!(content.user_messages, vec!["hello"]); // assistant message skipped
        // The run identity is surfaced so the context can be scoped to it.
        assert_eq!(content.thread_id.as_deref(), Some("t"));
        assert_eq!(content.run_id.as_deref(), Some("r"));
    }

    #[test]
    fn empty_object_yields_no_facts() {
        let content = parse_request(b"{}").expect("valid object");
        assert!(content.offered_tools.is_empty());
        assert!(content.user_messages.is_empty());
        assert!(content.hidden_fields.is_empty());
        // Absent identity is None — the run is treated as its own session.
        assert!(content.thread_id.is_none());
        assert!(content.run_id.is_none());
    }

    #[test]
    fn a_blank_thread_id_is_treated_as_absent() {
        let content = parse_request(br#"{"threadId": "  ", "runId": ""}"#).expect("valid object");
        assert!(content.thread_id.is_none());
        assert!(content.run_id.is_none());
    }

    #[test]
    fn collects_system_message_and_structured_field_leaves_as_hidden() {
        let body = br#"{
            "messages": [
                {"id": "s1", "role": "system", "content": "you are a helpful agent"},
                {"id": "m1", "role": "user", "content": "hi"}
            ],
            "context": [{"description": "env", "value": "prod"}],
            "forwardedProps": {"trace": "abc"},
            "state": {"counter": 1, "note": "xyz"}
        }"#;
        let content = parse_request(body).expect("valid body");
        assert_eq!(content.user_messages, vec!["hi"]);
        // System content plus every string leaf of context/forwardedProps/state;
        // the numeric `counter` carries no text and is dropped.
        assert_eq!(content.hidden_fields[0], "you are a helpful agent");
        for leaf in ["env", "prod", "abc", "xyz"] {
            assert!(
                content.hidden_fields.iter().any(|f| f == leaf),
                "expected leaf {leaf:?} in {:?}",
                content.hidden_fields
            );
        }
    }

    #[test]
    fn extracts_a_nested_url_leaf_so_the_ssrf_guard_can_parse_it() {
        // A URL nested in a compact object must surface as its own token.
        let body = br#"{"forwardedProps": {"callback": "http://127.0.0.1/x"}}"#;
        let content = parse_request(body).expect("valid body");
        assert!(
            content
                .hidden_fields
                .contains(&"http://127.0.0.1/x".to_owned())
        );
    }

    #[test]
    fn fails_closed_on_a_system_message_without_string_content() {
        assert!(parse_request(br#"{"messages": [{"role": "system", "content": 7}]}"#).is_err());
    }

    #[test]
    fn rejects_non_object_or_malformed_json() {
        assert!(parse_request(b"[1, 2, 3]").is_err());
        assert!(parse_request(b"not json").is_err());
    }

    #[test]
    fn fails_closed_on_malformed_security_fields() {
        // `tools` present but not an array.
        assert!(parse_request(br#"{"tools": "search"}"#).is_err());
        // A tool without a string name.
        assert!(parse_request(br#"{"tools": [{"description": "x"}]}"#).is_err());
        // `messages` present but not an array.
        assert!(parse_request(br#"{"messages": {}}"#).is_err());
        // A user message without string content.
        assert!(parse_request(br#"{"messages": [{"role": "user", "content": 42}]}"#).is_err());
        // A message that is not an object.
        assert!(parse_request(br#"{"messages": ["hi"]}"#).is_err());
    }
}
