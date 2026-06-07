//! Pipeline behaviors composed around handlers (chain of responsibility),
//! registered conditionally at the composition root.

pub mod metrics;
pub mod tracing;
pub mod transaction;

pub use metrics::MetricsBehavior;
pub use tracing::TracingBehavior;
pub use transaction::TransactionBehavior;
