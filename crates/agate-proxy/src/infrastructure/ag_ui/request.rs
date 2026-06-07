//! Parse a `RunAgentInput` body into the facts the request leg inspects.

use serde_json::Value;

use super::error::AgUiError;
use crate::application::inspection::RequestContent;

/// Extract the offered tool names and the user message texts from a
/// `RunAgentInput` JSON body. Rejects a body that is not a JSON object.
///
/// AG-UI is a permissive (`.passthrough()`) schema, so fields are read loosely
/// rather than against a fixed struct — only the security-relevant parts are
/// pulled out.
pub fn parse_request(body: &[u8]) -> Result<RequestContent, AgUiError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| AgUiError::MalformedRequest)?;
    let object = value.as_object().ok_or(AgUiError::MalformedRequest)?;

    let offered_tools = object
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let user_messages = object
        .get("messages")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter(|message| message.get("role").and_then(Value::as_str) == Some("user"))
                .filter_map(|message| message.get("content").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    Ok(RequestContent {
        offered_tools,
        user_messages,
    })
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
}
