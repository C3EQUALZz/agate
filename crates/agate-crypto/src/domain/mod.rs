//! Pure domain layer: self-describing algorithm values and the strategy traits
//! ([`Hasher`](hashing::Hasher), [`Signer`](signing::Signer),
//! [`Verifier`](signing::Verifier), [`Aead`](encryption::Aead)). No I/O, no
//! third-party crypto crates — concrete backends live in `infrastructure`.

pub mod common;
pub mod encryption;
pub mod hashing;
pub mod signing;
