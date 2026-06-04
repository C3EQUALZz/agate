//! Infrastructure layer: concrete adapters — the AG-UI protocol adapter and the
//! application-port implementations.

pub mod ag_ui;
pub mod policy;
pub mod sse;

pub use policy::AllowAllPolicy;
