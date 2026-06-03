//! Gateways for the transparency-log aggregate, split by CQRS side.

pub mod command_gateway;
pub mod query_gateway;

pub use command_gateway::LogCommandGateway;
pub use query_gateway::LogQueryGateway;
