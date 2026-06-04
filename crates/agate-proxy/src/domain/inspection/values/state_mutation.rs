use crate::domain::common::values::ValueObject;

/// A change to the shared agent/client state. The adapter measures the
/// structural facts the **domain** needs for budget/DoS checks (byte size, op
/// count) and carries the raw JSON `payload` for the **policy** to inspect —
/// the domain never parses it (stays pure, no JSON dependency).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateMutation {
    /// Full state replacement (AG-UI `STATE_SNAPSHOT`).
    Snapshot { byte_size: usize, payload: String },
    /// Incremental change as an RFC 6902 JSON Patch (AG-UI `STATE_DELTA`).
    Delta {
        op_count: usize,
        byte_size: usize,
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
}

impl ValueObject for StateMutation {}
