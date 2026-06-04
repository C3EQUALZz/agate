//! Infrastructure layer: concrete adapters implementing the application ports.

pub mod policy;

pub use policy::AllowAllPolicy;
