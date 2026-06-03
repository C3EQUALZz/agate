//! Outbound ports: abstract factories that resolve a self-describing algorithm
//! to a concrete strategy. Concrete factories live in `infrastructure`.

mod aead_factory;
mod hasher_factory;
mod signature_factory;

pub use aead_factory::AeadFactory;
pub use hasher_factory::HasherFactory;
pub use signature_factory::SignatureFactory;
