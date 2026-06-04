//! Infrastructure layer: concrete adapters — the AG-UI protocol adapter and the
//! application-port implementations.

pub mod ag_ui;
pub mod agent;
pub mod policy;
pub mod sse;

pub use agent::ReqwestAgentClient;
pub use policy::AllowAllPolicy;
