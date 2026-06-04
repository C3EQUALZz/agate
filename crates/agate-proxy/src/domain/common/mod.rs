//! Base building blocks (DDD seedwork) for the proxy context's domain model.
//!
//! Kept per-crate on purpose — there is no cross-context shared kernel.

pub mod entities;
pub mod errors;
pub mod factories;
pub mod services;
pub mod values;
