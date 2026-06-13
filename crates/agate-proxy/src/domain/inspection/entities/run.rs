use std::collections::HashMap;

use crate::domain::common::entities::Entity;
use crate::domain::inspection::values::{
    AgentEvent, Budgets, DenyReason, Fragment, LifecyclePhase, RunId, StructuralOutcome, ToolCallId,
};

/// Where a run is in its lifecycle. Used to reject out-of-order events (the
/// protocol gives no ordering guarantee, so the proxy enforces one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Before `RUN_STARTED`.
    Pending,
    /// Between `RUN_STARTED` and `RUN_FINISHED`/`RUN_ERROR`.
    Running,
    /// After the run ended; no further events are valid.
    Finished,
}

/// Arguments accumulated for a tool call still being streamed.
struct ToolCallBuffer {
    name: String,
    arguments: String,
}

/// The inspection aggregate for one agent run (consistency boundary, identified
/// by [`RunId`]). It is a transient per-request state machine — not persisted,
/// no domain-event recording — that assembles wire [`Fragment`]s into complete
/// [`AgentEvent`]s and enforces the structural invariants the pure domain owns:
/// lifecycle ordering, tool-call assembly, and resource budgets.
pub struct Run {
    id: RunId,
    phase: Phase,
    open_tool_calls: HashMap<ToolCallId, ToolCallBuffer>,
    /// Tool name by call id, recorded at the call's start and kept for the run
    /// so a later `TOOL_CALL_RESULT` (which carries only the id) can be
    /// attributed to its tool.
    tool_names: HashMap<ToolCallId, String>,
    budgets: Budgets,
}

impl Run {
    pub fn new(id: RunId, budgets: Budgets) -> Self {
        Self {
            id,
            phase: Phase::Pending,
            open_tool_calls: HashMap::new(),
            tool_names: HashMap::new(),
            budgets,
        }
    }

    /// Feed one fragment through the structural state machine.
    pub fn inspect(&mut self, fragment: Fragment) -> StructuralOutcome {
        match fragment {
            Fragment::Lifecycle(phase) => self.inspect_lifecycle(phase),
            other => {
                // Every non-lifecycle event must fall inside a running run.
                if self.phase != Phase::Running {
                    return reject("event outside an active run");
                }
                self.inspect_content(other)
            }
        }
    }

    fn inspect_lifecycle(&mut self, phase: LifecyclePhase) -> StructuralOutcome {
        match &phase {
            LifecyclePhase::RunStarted => {
                if self.phase != Phase::Pending {
                    return reject("run started more than once");
                }
                self.phase = Phase::Running;
            }
            LifecyclePhase::RunFinished | LifecyclePhase::RunError => {
                if self.phase != Phase::Running {
                    return reject("run ended without being active");
                }
                self.phase = Phase::Finished;
            }
            LifecyclePhase::StepStarted(_) | LifecyclePhase::StepFinished(_) => {
                if self.phase != Phase::Running {
                    return reject("step outside an active run");
                }
            }
        }
        StructuralOutcome::Ready(AgentEvent::Lifecycle(phase))
    }

    fn inspect_content(&mut self, fragment: Fragment) -> StructuralOutcome {
        match fragment {
            Fragment::ToolCallStarted { id, name } => self.tool_call_started(id, name),
            Fragment::ToolCallArgs { id, delta } => self.tool_call_args(&id, &delta),
            Fragment::ToolCallEnded { id } => self.tool_call_ended(&id),
            Fragment::ToolResult { id, content } => {
                let name = self.tool_names.get(&id).cloned();
                StructuralOutcome::Ready(AgentEvent::ToolResult { id, name, content })
            }
            Fragment::MessageChunk { message, text } => {
                StructuralOutcome::Ready(AgentEvent::MessageChunk { message, text })
            }
            Fragment::StateMutation(mutation) => {
                if mutation.byte_size() > self.budgets.max_state_bytes {
                    return reject("state mutation exceeds budget");
                }
                StructuralOutcome::Ready(AgentEvent::StateMutation(mutation))
            }
            Fragment::Opaque(kind) => StructuralOutcome::Ready(AgentEvent::Opaque(kind)),
            Fragment::Lifecycle(_) => unreachable!("lifecycle handled before content"),
        }
    }

    fn tool_call_started(&mut self, id: ToolCallId, name: String) -> StructuralOutcome {
        if self.open_tool_calls.contains_key(&id) {
            return reject("tool call started twice");
        }
        if self.open_tool_calls.len() >= self.budgets.max_open_tool_calls {
            return reject("too many concurrent tool calls");
        }
        self.tool_names.insert(id.clone(), name.clone());
        self.open_tool_calls.insert(
            id,
            ToolCallBuffer {
                name,
                arguments: String::new(),
            },
        );
        StructuralOutcome::Buffering
    }

    fn tool_call_args(&mut self, id: &ToolCallId, delta: &str) -> StructuralOutcome {
        let max = self.budgets.max_tool_args_bytes;
        let Some(buffer) = self.open_tool_calls.get_mut(id) else {
            return reject("arguments for an unknown tool call");
        };
        if buffer.arguments.len() + delta.len() > max {
            return reject("tool call arguments exceed budget");
        }
        buffer.arguments.push_str(delta);
        StructuralOutcome::Buffering
    }

    fn tool_call_ended(&mut self, id: &ToolCallId) -> StructuralOutcome {
        match self.open_tool_calls.remove(id) {
            Some(buffer) => StructuralOutcome::Ready(AgentEvent::ToolCall {
                id: id.clone(),
                name: buffer.name,
                arguments: buffer.arguments,
            }),
            None => reject("end of an unknown tool call"),
        }
    }
}

impl Entity for Run {
    type Id = RunId;

    fn id(&self) -> &RunId {
        &self.id
    }
}

fn reject(reason: &str) -> StructuralOutcome {
    StructuralOutcome::Reject(DenyReason::new(reason))
}
