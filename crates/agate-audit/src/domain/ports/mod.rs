//! Domain ports: interfaces the domain defines and adapters implement.

pub mod clock;
pub mod id_generator;

pub use clock::Clock;
pub use id_generator::IdGenerator;
