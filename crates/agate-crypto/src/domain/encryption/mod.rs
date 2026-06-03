//! Encryption subdomain: the [`Aead`] strategy and its self-describing values.

pub mod services;
pub mod values;

pub use services::Aead;
pub use values::{AeadAlgo, AssociatedData, Ciphertext, Nonce};
