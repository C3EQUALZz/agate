//! Hashing subdomain: the [`Hasher`] strategy and its self-describing values.

pub mod services;
pub mod values;

pub use services::Hasher;
pub use values::{Digest, HashAlgo};
