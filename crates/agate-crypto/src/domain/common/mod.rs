//! Seedwork shared across the crypto subdomains: the [`ValueObject`] marker,
//! the [`SecretKey`] wrapper, and the [`CryptoError`] hierarchy.

pub mod errors;
pub mod values;

pub use errors::CryptoError;
pub use values::{SecretKey, ValueObject};
