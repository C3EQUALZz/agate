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
    /// Bounds on a single RFC 6902 state-delta patch (anti-poisoning).
    pub patch: PatchBudget,
}

/// Per-patch bounds on a `STATE_DELTA` (an RFC 6902 JSON Patch): caps that stop
/// one delta from poisoning shared state with an unbounded number of ops, a
/// pathologically deep pointer, or an oversized value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatchBudget {
    /// Cap on the number of operations in one patch.
    pub max_ops: usize,
    /// Cap on the depth (reference-token count) of any op's JSON Pointer path.
    pub max_path_depth: usize,
    /// Cap on the encoded byte size of any single op's `value`.
    pub max_value_bytes: usize,
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
            patch: PatchBudget::default(),
        }
    }

    /// Override the per-patch bounds.
    #[must_use]
    pub fn with_patch(mut self, patch: PatchBudget) -> Self {
        self.patch = patch;
        self
    }
}

impl Default for Budgets {
    /// Conservative defaults: 64 KiB of tool arguments, 256 KiB per state
    /// mutation, 16 concurrent tool calls.
    fn default() -> Self {
        Self::new(64 * 1024, 256 * 1024, 16)
    }
}

impl Default for PatchBudget {
    /// Conservative defaults: 256 ops, pointer depth 32, 64 KiB per value.
    fn default() -> Self {
        Self {
            max_ops: 256,
            max_path_depth: 32,
            max_value_bytes: 64 * 1024,
        }
    }
}

impl ValueObject for Budgets {}
impl ValueObject for PatchBudget {}
