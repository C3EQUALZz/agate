//! Domain error hierarchy.

pub mod base;
pub mod time_errors;

pub use base::DomainError;
pub use time_errors::TimeError;
