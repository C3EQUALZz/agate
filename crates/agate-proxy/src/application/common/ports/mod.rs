//! Outbound application ports (implemented by infrastructure adapters).

pub mod audit_sink;
pub mod policy;

pub use audit_sink::AuditSink;
pub use policy::PolicyPort;
