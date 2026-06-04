//! The AG-UI adapter: wire events map onto protocol-agnostic domain fragments,
//! unknown/marker events pass through, and malformed events are rejected.

use serde_json::json;

use agate_proxy::domain::inspection::{
    AgentEvent, Fragment, LifecyclePhase, MessageId, OpaqueKind, StateMutation, ToolCallId,
};
use agate_proxy::infrastructure::ag_ui::{AgUiError, to_event, to_fragment};

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
    let fragment = to_fragment(&json!({
        "type": "TEXT_MESSAGE_CONTENT",
        "messageId": "m1",
        "delta": "hello",
    }))
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
        to_fragment(&json!({ "type": "TOOL_CALL_ARGS", "toolCallId": "c1", "delta": "{\"q\":" }))
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
    // A framing marker we do not inspect, and an unknown/evolving type.
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
        Some(json!({
            "type": "TEXT_MESSAGE_CONTENT",
            "messageId": "m1",
            "delta": "[redacted]",
        }))
    );
    // A lifecycle event has no single-frame transformed form.
    assert_eq!(
        to_event(&AgentEvent::Lifecycle(LifecyclePhase::RunStarted)),
        None
    );
}
