//! Infrastructure layer: concrete adapters implementing domain/application
//! ports (system clock, id generation, persistence, ...).

pub mod audit_metrics;
pub mod clock;
pub mod id_generator;
pub mod persistence;

pub use audit_metrics::AuditMetricsRecorder;
pub use clock::SystemClock;
pub use id_generator::UuidLogIdGenerator;
