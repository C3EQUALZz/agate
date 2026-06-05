//! Infrastructure adapters owned by the composition root — chiefly the bridge
//! that turns the proxy's `AuditSink` port into appends on the audit log.

pub mod audit;
pub mod policy;

pub use audit::{AuditLogSink, AuditOutbox};
pub use policy::PolicyAdapter;
