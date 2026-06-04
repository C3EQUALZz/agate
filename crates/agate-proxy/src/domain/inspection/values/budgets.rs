use crate::domain::common::values::ValueObject;

/// Per-run resource limits the domain enforces structurally (the DoS-facing
/// part of the threat model). Supplied at run construction — typically from
/// configuration at the composition root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budgets {
    /// Cap on the assembled argument bytes of a single tool call.
    pub max_tool_args_bytes: usize,
    /// Cap on the encoded size of a single state mutation.
    pub max_state_bytes: usize,
    /// Cap on tool calls open (started, not yet ended) at once.
    pub max_open_tool_calls: usize,
}

impl Budgets {
    pub fn new(
        max_tool_args_bytes: usize,
        max_state_bytes: usize,
        max_open_tool_calls: usize,
    ) -> Self {
        Self {
            max_tool_args_bytes,
            max_state_bytes,
            max_open_tool_calls,
        }
    }
}

impl Default for Budgets {
    /// Conservative defaults: 64 KiB of tool arguments, 256 KiB per state
    /// mutation, 16 concurrent tool calls.
    fn default() -> Self {
        Self::new(64 * 1024, 256 * 1024, 16)
    }
}

impl ValueObject for Budgets {}
