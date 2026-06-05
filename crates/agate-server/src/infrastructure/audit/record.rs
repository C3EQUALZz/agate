//! Canonical encoding of an inspected event into the bytes appended to the
//! transparency log. The proxy's domain types carry no `serde` derives (the
//! domain stays free of the JSON dependency), so the mapping is explicit here —
//! and being explicit keeps the on-log format stable as the domain evolves.

use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{
    AgentEvent, LifecyclePhase, OpaqueKind, StateMutation, Verdict,
};
use serde_json::{Value, json};

/// Encode one inspected event as the record bytes for the transparency log: the
/// run/session it belonged to, the verdict reached, and the event itself.
#[must_use]
pub fn encode_record(
    context: &InspectionContext,
    event: &AgentEvent,
    verdict: &Verdict<AgentEvent>,
) -> Vec<u8> {
    json!({
        "session": context.session.0.to_string(),
        "run": context.run.0.to_string(),
        "verdict": verdict_label(verdict),
        "event": encode_event(event),
    })
    .to_string()
    .into_bytes()
}

fn verdict_label(verdict: &Verdict<AgentEvent>) -> &'static str {
    match verdict {
        Verdict::Allow => "allow",
        Verdict::Deny(_) => "deny",
        Verdict::Transform(_) => "transform",
        Verdict::Buffer => "buffer",
        Verdict::Terminate(_) => "terminate",
    }
}

fn encode_event(event: &AgentEvent) -> Value {
    match event {
        AgentEvent::MessageChunk { message, text } => json!({
            "kind": "message_chunk",
            "message": message.0,
            "text": text,
        }),
        AgentEvent::ToolCall {
            id,
            name,
            arguments,
        } => json!({
            "kind": "tool_call",
            "id": id.0,
            "name": name,
            "arguments": arguments,
        }),
        AgentEvent::ToolResult { id, content } => json!({
            "kind": "tool_result",
            "id": id.0,
            "content": content,
        }),
        AgentEvent::StateMutation(mutation) => encode_state_mutation(mutation),
        AgentEvent::Lifecycle(phase) => json!({
            "kind": "lifecycle",
            "phase": encode_phase(phase),
        }),
        AgentEvent::Opaque(kind) => json!({
            "kind": "opaque",
            "opaque": encode_opaque(*kind),
        }),
    }
}

fn encode_state_mutation(mutation: &StateMutation) -> Value {
    match mutation {
        StateMutation::Snapshot { byte_size, payload } => json!({
            "kind": "state_snapshot",
            "byte_size": byte_size,
            "payload": payload,
        }),
        StateMutation::Delta {
            op_count,
            byte_size,
            payload,
        } => json!({
            "kind": "state_delta",
            "op_count": op_count,
            "byte_size": byte_size,
            "payload": payload,
        }),
    }
}

fn encode_phase(phase: &LifecyclePhase) -> Value {
    match phase {
        LifecyclePhase::RunStarted => json!("run_started"),
        LifecyclePhase::RunFinished => json!("run_finished"),
        LifecyclePhase::RunError => json!("run_error"),
        LifecyclePhase::StepStarted(name) => json!({ "step_started": name }),
        LifecyclePhase::StepFinished(name) => json!({ "step_finished": name }),
    }
}

fn encode_opaque(kind: OpaqueKind) -> &'static str {
    match kind {
        OpaqueKind::Raw => "raw",
        OpaqueKind::Custom => "custom",
        OpaqueKind::Encrypted => "encrypted",
    }
}
