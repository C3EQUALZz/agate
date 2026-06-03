//! Use cases: thin orchestration that resolves a strategy through a factory
//! port and applies it. Handlers are plain structs (no mediator) since the
//! crypto operations carry no cross-cutting concerns (transactions, outbox).

pub mod decrypt;
pub mod encrypt;
pub mod hash_data;
pub mod sign_data;
pub mod verify_signature;
