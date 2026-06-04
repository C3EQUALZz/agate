//! Inspection subdomain: the protocol-agnostic decisions a proxy makes about
//! the events flowing through it.
//!
//! Two levels (see the threat-model doc): wire-grained [`Fragment`]s are fed
//! into the [`Run`] aggregate, which assembles them and enforces structural
//! invariants, yielding a [`StructuralOutcome`]; a complete [`AgentEvent`] then
//! goes to the (async) policy port, which returns the final [`Verdict`].

pub mod entities;
pub mod values;

pub use entities::Run;
pub use values::{
    AgentEvent, Budgets, DenyReason, Fragment, LifecyclePhase, MessageId, OpaqueKind, RunId,
    SessionId, StateMutation, StructuralOutcome, ToolCallId, Verdict,
};
