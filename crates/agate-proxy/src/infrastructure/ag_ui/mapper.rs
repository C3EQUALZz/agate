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
