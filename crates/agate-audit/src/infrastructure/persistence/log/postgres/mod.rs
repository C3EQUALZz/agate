//! PostgreSQL adapters for the transparency-log gateways.
//!
//! The write side ([`PostgresLogCommandGateway`]) runs on the request-scoped
//! transaction from [`crate::infrastructure::persistence::postgres`]; the read
//! side ([`PostgresLogQueryGateway`]) reads through the pool directly.

pub mod command_gateway;
pub mod query_gateway;

pub use command_gateway::PostgresLogCommandGateway;
pub use query_gateway::PostgresLogQueryGateway;
