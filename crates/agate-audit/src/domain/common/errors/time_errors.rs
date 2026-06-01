use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeError {
    InconsistentTime { created_at: i64, updated_at: i64 },
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeError::InconsistentTime {
                created_at,
                updated_at,
            } => write!(
                f,
                "inconsistent time: updated_at ({updated_at}) earlier than created_at ({created_at})"
            ),
        }
    }
}

impl std::error::Error for TimeError {}
