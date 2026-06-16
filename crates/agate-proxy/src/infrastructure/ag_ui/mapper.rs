use serde_json::{Value, json};

use super::error::AgUiError;
use super::event_type as et;
use crate::domain::inspection::{
    AgentEvent, Fragment, LifecyclePhase, MessageId, OpaqueKind, StateMutation, ToolCallId,
};

/// Translate one parsed AG-UI event into a domain [`Fragment`] for inspection.
///
/// `Ok(None)` means the event carries nothing the proxy inspects (message-frame
/// markers, snapshots not yet modeled, and any unknown/evolving type) — the
/// proxy forwards its raw frame unchanged. AG-UI is a `.passthrough()` schema
/// that drifts between versions, so the mapper extracts the security-relevant
/// fields loosely rather than committing to a 34-variant typed enum.
pub fn to_fragment(value: &Value) -> Result<Option<Fragment>, AgUiError> {
    let object = value.as_object().ok_or(AgUiError::NotAnObject)?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or(AgUiError::MissingType)?;

    let fragment = match kind {
        et::RUN_STARTED => Fragment::Lifecycle(LifecyclePhase::RunStarted),
        et::RUN_FINISHED => Fragment::Lifecycle(LifecyclePhase::RunFinished),
        et::RUN_ERROR => Fragment::Lifecycle(LifecyclePhase::RunError),
        et::STEP_STARTED => Fragment::Lifecycle(LifecyclePhase::StepStarted(string(
            value, kind, "stepName",
        )?)),
        et::STEP_FINISHED => Fragment::Lifecycle(LifecyclePhase::StepFinished(string(
            value, kind, "stepName",
        )?)),
        et::TEXT_MESSAGE_CONTENT => Fragment::MessageChunk {
            message: message_id(value, kind)?,
            text: string(value, kind, "delta")?,
        },
        et::TOOL_CALL_START => Fragment::ToolCallStarted {
            id: tool_call_id(value, kind)?,
            name: string(value, kind, "toolCallName")?,
        },
        et::TOOL_CALL_ARGS => Fragment::ToolCallArgs {
            id: tool_call_id(value, kind)?,
            delta: string(value, kind, "delta")?,
        },
        et::TOOL_CALL_END => Fragment::ToolCallEnded {
            id: tool_call_id(value, kind)?,
        },
        et::TOOL_CALL_RESULT => Fragment::ToolResult {
            id: tool_call_id(value, kind)?,
            content: string(value, kind, "content")?,
        },
        et::STATE_SNAPSHOT => {
            let payload = value
                .get("snapshot")
                .ok_or(missing(kind, "snapshot"))?
                .to_string();
            Fragment::StateMutation(StateMutation::Snapshot {
                byte_size: payload.len(),
                payload,
            })
        }
        et::STATE_DELTA => {
            let delta = value.get("delta").ok_or(missing(kind, "delta"))?;
            let measured = measure_delta(delta, kind)?;
            let payload = delta.to_string();
            Fragment::StateMutation(StateMutation::Delta {
                op_count: measured.op_count,
                byte_size: payload.len(),
                max_path_depth: measured.max_path_depth,
                max_value_bytes: measured.max_value_bytes,
                payload,
            })
        }
        et::RAW => Fragment::Opaque(OpaqueKind::Raw),
        et::CUSTOM => Fragment::Opaque(OpaqueKind::Custom),
        et::REASONING_ENCRYPTED_VALUE => Fragment::Opaque(OpaqueKind::Encrypted),
        _ => return Ok(None),
    };
    Ok(Some(fragment))
}

/// Re-encode a (possibly transformed) semantic event back to AG-UI wire JSON,
/// for the `ForwardTransformed` path. Supports the events a content policy
/// realistically rewrites; others have no single-frame wire form yet.
pub fn to_event(event: &AgentEvent) -> Option<Value> {
    match event {
        AgentEvent::MessageChunk { message, text } => Some(json!({
            "type": et::TEXT_MESSAGE_CONTENT,
            "messageId": message.as_str(),
            "delta": text,
        })),
        AgentEvent::ToolResult { id, content, .. } => Some(json!({
            "type": et::TOOL_CALL_RESULT,
            "toolCallId": id.as_str(),
            "content": content,
        })),
        AgentEvent::ToolCall { .. }
        | AgentEvent::StateMutation(_)
        | AgentEvent::Lifecycle(_)
        | AgentEvent::Opaque(_) => None,
    }
}

fn string(value: &Value, kind: &str, field: &'static str) -> Result<String, AgUiError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| missing(kind, field))
}

/// Extract a validated `messageId` — present and non-blank.
fn message_id(value: &Value, kind: &str) -> Result<MessageId, AgUiError> {
    MessageId::new(string(value, kind, "messageId")?).map_err(|_| blank(kind, "messageId"))
}

/// Extract a validated `toolCallId` — present and non-blank.
fn tool_call_id(value: &Value, kind: &str) -> Result<ToolCallId, AgUiError> {
    ToolCallId::new(string(value, kind, "toolCallId")?).map_err(|_| blank(kind, "toolCallId"))
}

/// The RFC 6902 operation kinds — a closed set; any other `op` is not valid
/// JSON Patch and is rejected as malformed.
const PATCH_OPS: [&str; 6] = ["add", "remove", "replace", "move", "copy", "test"];

/// The bounds the domain budgets, measured over a delta's ops.
struct DeltaMeasure {
    op_count: usize,
    max_path_depth: usize,
    max_value_bytes: usize,
}

/// Validate a `STATE_DELTA` is a well-formed RFC 6902 patch and measure the
/// per-patch bounds. Each op must be an object with a known `op` kind and a
/// string `path`; anything else is a malformed (recognized) event so the
/// configured fail-closed mode applies. The domain enforces the bounds.
fn measure_delta(delta: &Value, kind: &str) -> Result<DeltaMeasure, AgUiError> {
    let ops = delta.as_array().ok_or_else(|| missing(kind, "delta"))?;
    let mut max_path_depth = 0;
    let mut max_value_bytes = 0;
    for op in ops {
        let fields = op.as_object().ok_or_else(|| missing(kind, "delta"))?;
        let op_kind = fields
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| missing(kind, "op"))?;
        if !PATCH_OPS.contains(&op_kind) {
            return Err(missing(kind, "op"));
        }
        let path = fields
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| missing(kind, "path"))?;
        max_path_depth = max_path_depth.max(pointer_depth(path));
        // RFC 6902 requires the per-op operand: `value` for add/replace/test,
        // a `from` pointer for move/copy. A missing operand is malformed.
        match op_kind {
            "add" | "replace" | "test" => {
                let value = fields.get("value").ok_or_else(|| missing(kind, "value"))?;
                max_value_bytes = max_value_bytes.max(value.to_string().len());
            }
            "move" | "copy" => {
                // `from` is itself a JSON Pointer into the document, so it
                // counts toward the depth budget too — a shallow `path` with a
                // deep `from` must not slip past.
                let from = fields
                    .get("from")
                    .and_then(Value::as_str)
                    .ok_or_else(|| missing(kind, "from"))?;
                max_path_depth = max_path_depth.max(pointer_depth(from));
            }
            _ => {} // remove: path only
        }
    }
    Ok(DeltaMeasure {
        op_count: ops.len(),
        max_path_depth,
        max_value_bytes,
    })
}

/// Depth of a JSON Pointer: its number of reference tokens, i.e. the count of
/// `/` separators (`/a/b/c` = 3, `/` = 1 the empty-named key, `` = 0 the whole
/// document). An escaped slash (`~1`) lives *inside* a token, so it does not
/// add depth.
fn pointer_depth(path: &str) -> usize {
    path.bytes().filter(|&byte| byte == b'/').count()
}

fn missing(kind: &str, field: &'static str) -> AgUiError {
    AgUiError::MissingField {
        event: kind.to_owned(),
        field,
    }
}

fn blank(kind: &str, field: &'static str) -> AgUiError {
    AgUiError::BlankField {
        event: kind.to_owned(),
        field,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{AgUiError, to_event, to_fragment};
    use crate::domain::inspection::{
        AgentEvent, Fragment, LifecyclePhase, MessageId, OpaqueKind, StateMutation, ToolCallId,
    };

    #[test]
    fn maps_lifecycle_events() {
        assert_eq!(
            to_fragment(&json!({ "type": "RUN_STARTED", "threadId": "t", "runId": "r" })).unwrap(),
            Some(Fragment::Lifecycle(LifecyclePhase::RunStarted))
        );
        assert_eq!(
            to_fragment(&json!({ "type": "STEP_STARTED", "stepName": "plan" })).unwrap(),
            Some(Fragment::Lifecycle(LifecyclePhase::StepStarted(
                "plan".to_string()
            )))
        );
    }

    #[test]
    fn maps_text_message_content_to_a_chunk() {
        let fragment = to_fragment(
            &json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": "m1", "delta": "hello" }),
        )
        .unwrap();
        assert_eq!(
            fragment,
            Some(Fragment::MessageChunk {
                message: MessageId::new("m1").expect("valid id"),
                text: "hello".to_string(),
            })
        );
    }

    #[test]
    fn maps_the_tool_call_lifecycle() {
        assert_eq!(
            to_fragment(
                &json!({ "type": "TOOL_CALL_START", "toolCallId": "c1", "toolCallName": "search" })
            )
            .unwrap(),
            Some(Fragment::ToolCallStarted {
                id: ToolCallId::new("c1").expect("valid id"),
                name: "search".to_string(),
            })
        );
        assert_eq!(
            to_fragment(
                &json!({ "type": "TOOL_CALL_ARGS", "toolCallId": "c1", "delta": "{\"q\":" })
            )
            .unwrap(),
            Some(Fragment::ToolCallArgs {
                id: ToolCallId::new("c1").expect("valid id"),
                delta: "{\"q\":".to_string(),
            })
        );
        assert_eq!(
            to_fragment(&json!({ "type": "TOOL_CALL_END", "toolCallId": "c1" })).unwrap(),
            Some(Fragment::ToolCallEnded {
                id: ToolCallId::new("c1").expect("valid id"),
            })
        );
    }

    #[test]
    fn maps_state_delta_with_op_count() {
        let fragment = to_fragment(&json!({
            "type": "STATE_DELTA",
            "delta": [
                { "op": "add", "path": "/a", "value": 1 },
                { "op": "remove", "path": "/b" },
            ],
        }))
        .unwrap();
        match fragment {
            Some(Fragment::StateMutation(StateMutation::Delta {
                op_count,
                byte_size,
                ..
            })) => {
                assert_eq!(op_count, 2);
                assert!(byte_size > 0);
            }
            other => panic!("expected a state delta, got {other:?}"),
        }
    }

    #[test]
    fn measures_delta_path_depth_and_value_size() {
        let fragment = to_fragment(&json!({
            "type": "STATE_DELTA",
            "delta": [
                { "op": "replace", "path": "/a/b/c", "value": "xy" },
                { "op": "remove", "path": "/d" },
            ],
        }))
        .unwrap();
        match fragment {
            Some(Fragment::StateMutation(StateMutation::Delta {
                max_path_depth,
                max_value_bytes,
                ..
            })) => {
                assert_eq!(max_path_depth, 3); // /a/b/c
                assert_eq!(max_value_bytes, 4); // "xy" serialized incl. quotes
            }
            other => panic!("expected a state delta, got {other:?}"),
        }
    }

    #[test]
    fn rejects_a_malformed_json_patch() {
        // delta is not an array.
        assert!(to_fragment(&json!({ "type": "STATE_DELTA", "delta": {} })).is_err());
        // unknown op kind.
        assert!(
            to_fragment(&json!({
                "type": "STATE_DELTA", "delta": [{ "op": "explode", "path": "/a" }]
            }))
            .is_err()
        );
        // op missing its path.
        assert!(
            to_fragment(&json!({
                "type": "STATE_DELTA", "delta": [{ "op": "add", "value": 1 }]
            }))
            .is_err()
        );
        // add/replace/test without the required `value`.
        assert!(
            to_fragment(&json!({
                "type": "STATE_DELTA", "delta": [{ "op": "replace", "path": "/a" }]
            }))
            .is_err()
        );
        // move/copy without the required `from` pointer.
        assert!(
            to_fragment(&json!({
                "type": "STATE_DELTA", "delta": [{ "op": "move", "path": "/a" }]
            }))
            .is_err()
        );
    }

    #[test]
    fn pointer_depth_counts_tokens_including_empty_and_the_from_pointer() {
        // Empty reference tokens count (`/a//b` = 3), and a `move`'s deep `from`
        // is measured even when `path` is shallow.
        let fragment = to_fragment(&json!({
            "type": "STATE_DELTA",
            "delta": [{ "op": "move", "path": "/x", "from": "/a//b/c" }],
        }))
        .unwrap();
        match fragment {
            Some(Fragment::StateMutation(StateMutation::Delta { max_path_depth, .. })) => {
                assert_eq!(max_path_depth, 4); // /a//b/c → 4 slashes
            }
            other => panic!("expected a state delta, got {other:?}"),
        }
    }

    #[test]
    fn maps_opaque_events() {
        assert_eq!(
            to_fragment(&json!({ "type": "CUSTOM", "name": "x", "value": 1 })).unwrap(),
            Some(Fragment::Opaque(OpaqueKind::Custom))
        );
    }

    #[test]
    fn unknown_and_marker_events_pass_through() {
        assert_eq!(
            to_fragment(&json!({ "type": "TEXT_MESSAGE_START", "messageId": "m1" })).unwrap(),
            None
        );
        assert_eq!(
            to_fragment(&json!({ "type": "SOME_FUTURE_EVENT" })).unwrap(),
            None
        );
    }

    #[test]
    fn rejects_malformed_events() {
        assert_eq!(
            to_fragment(&json!([1, 2, 3])).unwrap_err(),
            AgUiError::NotAnObject
        );
        assert_eq!(
            to_fragment(&json!({ "noType": true })).unwrap_err(),
            AgUiError::MissingType
        );
        assert!(matches!(
            to_fragment(&json!({ "type": "TOOL_CALL_START", "toolCallId": "c1" })).unwrap_err(),
            AgUiError::MissingField {
                field: "toolCallName",
                ..
            }
        ));
    }

    #[test]
    fn re_encodes_a_transformed_message_chunk() {
        let event = AgentEvent::MessageChunk {
            message: MessageId::new("m1").expect("valid id"),
            text: "[redacted]".to_string(),
        };
        assert_eq!(
            to_event(&event),
            Some(
                json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": "m1", "delta": "[redacted]" })
            )
        );
        assert_eq!(
            to_event(&AgentEvent::Lifecycle(LifecyclePhase::RunStarted)),
            None
        );
    }
}
