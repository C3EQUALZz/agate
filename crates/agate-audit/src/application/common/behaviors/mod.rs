//! Pipeline behaviors composed around handlers (chain of responsibility),
//! registered conditionally at the composition root.

pub mod transaction;

pub use transaction::TransactionBehavior;
