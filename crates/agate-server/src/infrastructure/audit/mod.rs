//! The audit bridge.
//!
//! The proxy emits each inspected `(event, verdict)` pair to an `AuditSink`.
//! Here that sink ([`AuditLogSink`]) encodes the pair and enqueues it on a
//! bounded channel; a single background [`AuditOutbox`] drains the channel,
//! appending each record to the audit context's transparency log.
//!
//! - **One writer** (the outbox) keeps the log ordered: the channel's FIFO order
//!   becomes the log's append order.
//! - **Bounded channel** applies backpressure rather than blocking the
//!   forwarding path on the database, and bounds memory under load.

pub mod outbox;
pub mod record;
pub mod sink;

pub use outbox::AuditOutbox;
pub use sink::AuditLogSink;
