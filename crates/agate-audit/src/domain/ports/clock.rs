use crate::domain::common::values::Timestamp;

/// Domain port: the current instant. Reading the clock is I/O, so the
/// implementation lives in an adapter; the domain depends only on this trait.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
