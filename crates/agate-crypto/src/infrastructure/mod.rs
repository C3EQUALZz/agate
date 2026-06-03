//! Infrastructure: concrete RustCrypto-backed strategies and the factories that
//! build them. The only layer that depends on third-party crypto crates.

pub mod encryption;
pub mod hashing;
pub mod signing;
