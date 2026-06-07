//! Outbound application ports (implemented by infrastructure adapters).

pub mod checkpoint_anchor;
pub mod event_outbox;
pub mod health;
pub mod key_store;
pub mod log;
pub mod metrics;
pub mod transaction_manager;

pub use checkpoint_anchor::CheckpointAnchor;
pub use event_outbox::EventOutbox;
pub use health::HealthCheck;
pub use key_store::KeyStore;
pub use log::{LogCommandGateway, LogQueryGateway};
pub use metrics::AuditMetrics;
pub use transaction_manager::TransactionManager;
