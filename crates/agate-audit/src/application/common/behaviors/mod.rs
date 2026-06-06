//! Pipeline behaviors composed around handlers (chain of responsibility),
//! registered conditionally at the composition root.

pub mod metrics;
pub mod transaction;

pub use metrics::MetricsBehavior;
pub use transaction::TransactionBehavior;
