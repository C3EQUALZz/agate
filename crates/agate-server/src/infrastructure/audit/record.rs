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
        "session": context.session.to_string(),
        "run": context.run.to_string(),
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
            "message": message.as_str(),
            "text": text,
        }),
        AgentEvent::ToolCall {
            id,
            name,
            arguments,
        } => json!({
            "kind": "tool_call",
            "id": id.as_str(),
            "name": name,
            "arguments": arguments,
        }),
        AgentEvent::ToolResult { id, name, content } => json!({
            "kind": "tool_result",
            "id": id.as_str(),
            "name": name,
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

#[cfg(test)]
mod tests {
    use agate_proxy::application::inspection::InspectionContext;
    use agate_proxy::domain::inspection::{
        AgentEvent, DenyReason, LifecyclePhase, MessageId, OpaqueKind, RunId, SessionId,
        StateMutation, ToolCallId, Verdict,
    };
    use serde_json::Value;
    use uuid::Uuid;

    use super::encode_record;

    fn decode(event: &AgentEvent, verdict: &Verdict<AgentEvent>) -> Value {
        let context = InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()));
        serde_json::from_slice(&encode_record(&context, event, verdict)).expect("valid JSON")
    }

    fn lifecycle() -> AgentEvent {
        AgentEvent::Lifecycle(LifecyclePhase::RunStarted)
    }

    #[test]
    fn encodes_context_and_verdict() {
        let record = decode(&lifecycle(), &Verdict::Allow);
        assert_eq!(record["session"], Uuid::nil().to_string());
        assert_eq!(record["run"], Uuid::nil().to_string());
        assert_eq!(record["verdict"], "allow");
    }

    #[test]
    fn encodes_every_verdict_label() {
        let e = lifecycle();
        assert_eq!(
            decode(&e, &Verdict::Deny(DenyReason::new("x")))["verdict"],
            "deny"
        );
        assert_eq!(
            decode(&e, &Verdict::Transform(e.clone()))["verdict"],
            "transform"
        );
        assert_eq!(decode(&e, &Verdict::Buffer)["verdict"], "buffer");
        assert_eq!(
            decode(&e, &Verdict::Terminate(DenyReason::new("x")))["verdict"],
            "terminate"
        );
    }

    #[test]
    fn encodes_every_event_kind() {
        let allow = Verdict::Allow;
        let cases = [
            (
                AgentEvent::MessageChunk {
                    message: MessageId::new("m").expect("valid id"),
                    text: "hi".into(),
                },
                "message_chunk",
            ),
            (
                AgentEvent::ToolCall {
                    id: ToolCallId::new("c").expect("valid id"),
                    name: "t".into(),
                    arguments: "{}".into(),
                },
                "tool_call",
            ),
            (
                AgentEvent::ToolResult {
                    id: ToolCallId::new("c").expect("valid id"),
                    name: Some("t".into()),
                    content: "r".into(),
                },
                "tool_result",
            ),
            (
                AgentEvent::StateMutation(StateMutation::Snapshot {
                    byte_size: 2,
                    payload: "{}".into(),
                }),
                "state_snapshot",
            ),
            (
                AgentEvent::StateMutation(StateMutation::Delta {
                    op_count: 1,
                    byte_size: 2,
                    payload: "[]".into(),
                }),
                "state_delta",
            ),
        ];
        for (event, kind) in cases {
            assert_eq!(decode(&event, &allow)["event"]["kind"], kind);
        }
    }

    #[test]
    fn encodes_every_lifecycle_phase() {
        let allow = Verdict::Allow;
        let phase = |p| decode(&AgentEvent::Lifecycle(p), &allow)["event"]["phase"].clone();
        assert_eq!(phase(LifecyclePhase::RunStarted), "run_started");
        assert_eq!(phase(LifecyclePhase::RunFinished), "run_finished");
        assert_eq!(phase(LifecyclePhase::RunError), "run_error");
        assert_eq!(
            phase(LifecyclePhase::StepStarted("s".into()))["step_started"],
            "s"
        );
        assert_eq!(
            phase(LifecyclePhase::StepFinished("s".into()))["step_finished"],
            "s"
        );
    }

    #[test]
    fn encodes_every_opaque_kind() {
        let allow = Verdict::Allow;
        let opaque = |k| decode(&AgentEvent::Opaque(k), &allow)["event"]["opaque"].clone();
        assert_eq!(opaque(OpaqueKind::Raw), "raw");
        assert_eq!(opaque(OpaqueKind::Custom), "custom");
        assert_eq!(opaque(OpaqueKind::Encrypted), "encrypted");
    }
}
