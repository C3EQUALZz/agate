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
            message: MessageId(string(value, kind, "messageId")?),
            text: string(value, kind, "delta")?,
        },
        et::TOOL_CALL_START => Fragment::ToolCallStarted {
            id: ToolCallId(string(value, kind, "toolCallId")?),
            name: string(value, kind, "toolCallName")?,
        },
        et::TOOL_CALL_ARGS => Fragment::ToolCallArgs {
            id: ToolCallId(string(value, kind, "toolCallId")?),
            delta: string(value, kind, "delta")?,
        },
        et::TOOL_CALL_END => Fragment::ToolCallEnded {
            id: ToolCallId(string(value, kind, "toolCallId")?),
        },
        et::TOOL_CALL_RESULT => Fragment::ToolResult {
            id: ToolCallId(string(value, kind, "toolCallId")?),
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
            let op_count = delta.as_array().map_or(0, Vec::len);
            let payload = delta.to_string();
            Fragment::StateMutation(StateMutation::Delta {
                op_count,
                byte_size: payload.len(),
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
            "messageId": message.0,
            "delta": text,
        })),
        AgentEvent::ToolResult { id, content } => Some(json!({
            "type": et::TOOL_CALL_RESULT,
            "toolCallId": id.0,
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

fn missing(kind: &str, field: &'static str) -> AgUiError {
    AgUiError::MissingField {
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
                message: MessageId("m1".to_string()),
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
                id: ToolCallId("c1".to_string()),
                name: "search".to_string(),
            })
        );
        assert_eq!(
            to_fragment(
                &json!({ "type": "TOOL_CALL_ARGS", "toolCallId": "c1", "delta": "{\"q\":" })
            )
            .unwrap(),
            Some(Fragment::ToolCallArgs {
                id: ToolCallId("c1".to_string()),
                delta: "{\"q\":".to_string(),
            })
        );
        assert_eq!(
            to_fragment(&json!({ "type": "TOOL_CALL_END", "toolCallId": "c1" })).unwrap(),
            Some(Fragment::ToolCallEnded {
                id: ToolCallId("c1".to_string()),
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
            message: MessageId("m1".to_string()),
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
