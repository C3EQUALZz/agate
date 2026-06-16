use crate::domain::common::values::ValueObject;

/// A change to the shared agent/client state. The adapter measures the
/// structural facts the **domain** needs for budget/DoS checks (byte size, op
/// count) and carries the raw JSON `payload` for the **policy** to inspect —
/// the domain never parses it (stays pure, no JSON dependency).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateMutation {
    /// Full state replacement (AG-UI `STATE_SNAPSHOT`).
    Snapshot { byte_size: usize, payload: String },
    /// Incremental change as an RFC 6902 JSON Patch (AG-UI `STATE_DELTA`). The
    /// adapter validates each op is well-formed (a known op kind with a path)
    /// and measures the bounds the domain budgets: op count, deepest pointer,
    /// and the largest single op `value`.
    Delta {
        op_count: usize,
        byte_size: usize,
        max_path_depth: usize,
        max_value_bytes: usize,
        payload: String,
    },
}

impl StateMutation {
    /// Encoded size in bytes — the figure budget checks bound.
    pub fn byte_size(&self) -> usize {
        match self {
            StateMutation::Snapshot { byte_size, .. } | StateMutation::Delta { byte_size, .. } => {
                *byte_size
            }
        }
    }

    /// The per-patch bounds the domain checks, for a delta: `(op_count,
    /// max_path_depth, max_value_bytes)`. `None` for a snapshot (no ops).
    #[must_use]
    pub fn patch_bounds(&self) -> Option<(usize, usize, usize)> {
        match self {
            StateMutation::Delta {
                op_count,
                max_path_depth,
                max_value_bytes,
                ..
            } => Some((*op_count, *max_path_depth, *max_value_bytes)),
            StateMutation::Snapshot { .. } => None,
        }
    }

    /// The raw JSON payload — what the policy inspects (the domain never parses
    /// it).
    #[must_use]
    pub fn payload(&self) -> &str {
        match self {
            StateMutation::Snapshot { payload, .. } | StateMutation::Delta { payload, .. } => {
                payload
            }
        }
    }
}

impl ValueObject for StateMutation {}
