//! Infrastructure layer: concrete adapters implementing domain/application
//! ports (system clock, id generation, persistence, ...).

pub mod clock;
pub mod di;
pub mod id_generator;
pub mod persistence;

pub use clock::SystemClock;
pub use id_generator::UuidLogIdGenerator;
