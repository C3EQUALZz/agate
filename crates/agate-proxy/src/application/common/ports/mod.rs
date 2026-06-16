//! Outbound application ports (implemented by infrastructure adapters).

pub mod audit_sink;
pub mod host_resolver;
pub mod metrics;
pub mod policy;
pub mod upstream;

pub use audit_sink::AuditSink;
pub use host_resolver::HostResolver;
pub use metrics::{InspectionOutcome, ProxyMetrics};
pub use policy::PolicyPort;
pub use upstream::{AgentResponseStream, RunRequest, UpstreamAgentClient, UpstreamError};
