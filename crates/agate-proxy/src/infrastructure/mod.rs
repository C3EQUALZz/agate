//! Infrastructure layer: concrete adapters — the AG-UI protocol adapter and the
//! application-port implementations.

pub mod ag_ui;
pub mod agent;
pub mod audit;
pub mod dns;
pub mod fail_mode_policy;
pub mod policy;
pub mod proxy_metrics;
pub mod sse;

pub use agent::ReqwestAgentClient;
pub use audit::NoopAuditSink;
pub use dns::{NoopHostResolver, TokioHostResolver};
pub use fail_mode_policy::{FailMode, FailModePolicy};
pub use policy::{AllowAllPolicy, InMemorySessionMemory, NoopSessionMemory};
pub use proxy_metrics::ProxyMetricsRecorder;
