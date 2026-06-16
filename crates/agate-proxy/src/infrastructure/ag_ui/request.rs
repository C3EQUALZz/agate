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
    // The structured fields an injection can hide in: screen each as its JSON
    // text (a secret marker or URL inside is still caught). Absent fields add
    // nothing.
    for key in ["context", "forwardedProps", "state"] {
        if let Some(value) = object.get(key) {
            hidden_fields.push(value.to_string());
        }
    }

    Ok(RequestContent {
        offered_tools: parse_offered_tools(object)?,
        user_messages,
        hidden_fields,
    })
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
    }

    #[test]
    fn empty_object_yields_no_facts() {
        let content = parse_request(b"{}").expect("valid object");
        assert!(content.offered_tools.is_empty());
        assert!(content.user_messages.is_empty());
        assert!(content.hidden_fields.is_empty());
    }

    #[test]
    fn collects_system_message_and_structured_fields_as_hidden() {
        let body = br#"{
            "messages": [
                {"id": "s1", "role": "system", "content": "you are a helpful agent"},
                {"id": "m1", "role": "user", "content": "hi"}
            ],
            "context": [{"description": "env", "value": "prod"}],
            "forwardedProps": {"trace": "abc"},
            "state": {"counter": 1}
        }"#;
        let content = parse_request(body).expect("valid body");
        assert_eq!(content.user_messages, vec!["hi"]);
        // system content + the JSON of context/forwardedProps/state, in order.
        assert_eq!(content.hidden_fields.len(), 4);
        assert_eq!(content.hidden_fields[0], "you are a helpful agent");
        assert!(content.hidden_fields[1].contains("prod"));
        assert!(content.hidden_fields[2].contains("abc"));
        assert!(content.hidden_fields[3].contains("counter"));
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
