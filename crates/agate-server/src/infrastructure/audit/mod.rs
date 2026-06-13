//! The audit bridge.
//!
//! The proxy emits each inspected `(event, verdict)` pair to an `AuditSink`.
//! Here that sink ([`AuditLogSink`]) encodes the pair and enqueues it on a
//! bounded channel; a single background [`AuditOutbox`] drains the channel,
//! appending each record to the audit context's transparency log through the
//! [`RecordAppender`] port (implemented at the composition root).
//!
//! - **One writer** (the outbox) keeps the log ordered: the channel's FIFO order
//!   becomes the log's append order.
//! - **Bounded channel** applies backpressure rather than blocking the
//!   forwarding path on the database, and bounds memory under load.

pub mod appender;
pub mod issuer;
pub mod outbox;
pub mod record;
pub mod scheduler;
pub mod sink;

pub use appender::{AppendError, RecordAppender};
pub use issuer::{CheckpointIssuer, IssueError};
pub use outbox::AuditOutbox;
pub use scheduler::CheckpointScheduler;
pub use sink::AuditLogSink;
