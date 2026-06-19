//! Structural behaviour of the `Run` inspection state machine: tool-call
//! assembly, lifecycle ordering, and resource budgets.

use agate_proxy::domain::inspection::values::{
    AgentEvent, Budgets, Fragment, LifecyclePhase, OpaqueKind, PatchBudget, StateMutation,
    StructuralOutcome, ToolCallId,
};
use agate_proxy::domain::inspection::{Run, RunId};
use uuid::Uuid;

fn run() -> Run {
    Run::new(RunId::new(Uuid::nil()), Budgets::default())
}

fn started_run() -> Run {
    let mut run = run();
    let outcome = run.inspect(Fragment::Lifecycle(LifecyclePhase::RunStarted));
    assert_eq!(
        outcome,
        StructuralOutcome::Ready(AgentEvent::Lifecycle(LifecyclePhase::RunStarted))
    );
    run
}

fn tool(id: &str) -> ToolCallId {
    ToolCallId::new(id).expect("valid id")
}

#[test]
fn drain_open_assembles_a_tool_call_left_unclosed() {
    // The agent streamed START/ARGS but never sent TOOL_CALL_END; the call is
    // still recoverable so the run-end sweep can judge it rather than relay it
    // unjudged.
    let mut run = started_run();
    run.inspect(Fragment::ToolCallStarted {
        id: tool("t1"),
        name: "delete_file".to_string(),
    });
    run.inspect(Fragment::ToolCallArgs {
        id: tool("t1"),
        delta: "{\"path\":\"/etc\"}".to_string(),
    });

    let open = run.drain_open();
    assert_eq!(
        open,
        vec![(
            tool("t1"),
            AgentEvent::ToolCall {
                id: tool("t1"),
                name: "delete_file".to_string(),
                arguments: "{\"path\":\"/etc\"}".to_string(),
            }
        )]
    );
    // Draining empties the open set — a second sweep finds nothing to re-judge.
    assert!(run.drain_open().is_empty());
}

#[test]
fn assembles_a_tool_call_from_its_fragments() {
    let mut run = started_run();

    assert_eq!(
        run.inspect(Fragment::ToolCallStarted {
            id: tool("t1"),
            name: "search".to_string(),
        }),
        StructuralOutcome::Buffering(tool("t1"))
    );
    assert_eq!(
        run.inspect(Fragment::ToolCallArgs {
            id: tool("t1"),
            delta: "{\"q\":".to_string(),
        }),
        StructuralOutcome::Buffering(tool("t1"))
    );
    assert_eq!(
        run.inspect(Fragment::ToolCallArgs {
            id: tool("t1"),
            delta: "\"hi\"}".to_string(),
        }),
        StructuralOutcome::Buffering(tool("t1"))
    );

    assert_eq!(
        run.inspect(Fragment::ToolCallEnded { id: tool("t1") }),
        StructuralOutcome::ResolvedCall {
            id: tool("t1"),
            event: AgentEvent::ToolCall {
                id: tool("t1"),
                name: "search".to_string(),
                arguments: "{\"q\":\"hi\"}".to_string(),
            },
        }
    );
}

#[test]
fn rejects_content_before_the_run_starts() {
    let mut run = run();
    let outcome = run.inspect(Fragment::Opaque(OpaqueKind::Custom));
    assert!(matches!(outcome, StructuralOutcome::Reject(_)));
}

#[test]
fn rejects_events_after_the_run_finishes() {
    let mut run = started_run();
    assert!(matches!(
        run.inspect(Fragment::Lifecycle(LifecyclePhase::RunFinished)),
        StructuralOutcome::Ready(_)
    ));
    assert!(matches!(
        run.inspect(Fragment::Opaque(OpaqueKind::Raw)),
        StructuralOutcome::Reject(_)
    ));
}

#[test]
fn rejects_arguments_for_an_unknown_tool_call() {
    let mut run = started_run();
    assert!(matches!(
        run.inspect(Fragment::ToolCallArgs {
            id: tool("ghost"),
            delta: "x".to_string(),
        }),
        StructuralOutcome::Reject(_)
    ));
}

#[test]
fn enforces_the_tool_argument_budget() {
    let mut run = Run::new(RunId::new(Uuid::nil()), Budgets::new(8, 1024, 4));
    run.inspect(Fragment::Lifecycle(LifecyclePhase::RunStarted));
    run.inspect(Fragment::ToolCallStarted {
        id: tool("t1"),
        name: "x".to_string(),
    });
    let outcome = run.inspect(Fragment::ToolCallArgs {
        id: tool("t1"),
        delta: "0123456789".to_string(),
    });
    assert!(matches!(outcome, StructuralOutcome::Reject(_)));
}

#[test]
fn enforces_the_state_mutation_budget() {
    let mut run = Run::new(RunId::new(Uuid::nil()), Budgets::new(1024, 4, 4));
    run.inspect(Fragment::Lifecycle(LifecyclePhase::RunStarted));
    let outcome = run.inspect(Fragment::StateMutation(StateMutation::Snapshot {
        byte_size: 16,
        payload: "{\"big\":\"value\"}".to_string(),
    }));
    assert!(matches!(outcome, StructuralOutcome::Reject(_)));
}

#[test]
fn enforces_the_patch_bounds() {
    let tight = Budgets::default().with_patch(PatchBudget {
        max_ops: 4,
        max_path_depth: 4,
        max_value_bytes: 64,
    });
    // Each delta on its own would pass byte_size, but one bound is exceeded.
    let cases = [
        (5, 1, 0, "too many ops"),
        (1, 9, 0, "path too deep"),
        (1, 1, 256, "value too big"),
    ];
    for (op_count, max_path_depth, max_value_bytes, label) in cases {
        let mut run = Run::new(RunId::new(Uuid::nil()), tight);
        run.inspect(Fragment::Lifecycle(LifecyclePhase::RunStarted));
        let outcome = run.inspect(Fragment::StateMutation(StateMutation::Delta {
            op_count,
            byte_size: 8,
            max_path_depth,
            max_value_bytes,
            payload: "[]".to_string(),
        }));
        assert!(
            matches!(outcome, StructuralOutcome::Reject(_)),
            "expected reject: {label}"
        );
    }
}
