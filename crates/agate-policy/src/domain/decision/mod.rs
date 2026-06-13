//! Decision subdomain: the rules that turn an inspected agent action into a
//! content/authorization verdict.
//!
//! An [`InspectedAction`] (a tool call, an emitted text chunk, or anything else)
//! is judged against a [`PolicyRuleset`] by the [`PolicyEvaluator`], yielding a
//! [`PolicyDecision`] — allow, deny, or redact. All pure: no async, no I/O.

pub mod services;
pub mod values;

pub use services::{ArgumentInspector, PolicyEvaluator, TextRedactor, ToolAuthorizer};
pub use values::{
    ArgumentRule, DenyReason, InspectedAction, Pattern, PolicyDecision, PolicyRuleset, ToolMatcher,
    ToolName, ToolPolicy,
};
