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

    Ok(RequestContent {
        offered_tools: parse_offered_tools(object)?,
        user_messages: parse_user_messages(object)?,
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

fn parse_user_messages(object: &Map<String, Value>) -> Result<Vec<String>, AgUiError> {
    let Some(messages) = object.get("messages") else {
        return Ok(Vec::new());
    };
    let messages = messages.as_array().ok_or(AgUiError::MalformedRequest)?;
    let mut texts = Vec::new();
    for message in messages {
        let fields = message.as_object().ok_or(AgUiError::MalformedRequest)?;
        let role = fields
            .get("role")
            .and_then(Value::as_str)
            .ok_or(AgUiError::MalformedRequest)?;
        if role != "user" {
            continue; // only user messages are inspected
        }
        let content = fields
            .get("content")
            .and_then(Value::as_str)
            .ok_or(AgUiError::MalformedRequest)?;
        texts.push(content.to_owned());
    }
    Ok(texts)
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
