//! Signing subdomain: the [`Signer`] / [`Verifier`] strategies and their values.

pub mod services;
pub mod values;

pub use services::{Signer, Verifier};
pub use values::{KeyId, SignAlgo, Signature};
