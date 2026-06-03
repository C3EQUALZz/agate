use uuid::Uuid;

use crate::domain::merkle::LogId;
use crate::domain::ports::IdGenerator;

/// Generates `LogId`s from random UUIDv4.
pub struct UuidLogIdGenerator;

impl IdGenerator<LogId> for UuidLogIdGenerator {
    fn generate(&self) -> LogId {
        LogId(Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_distinct_ids() {
        let generator = UuidLogIdGenerator;
        assert_ne!(generator.generate(), generator.generate());
    }
}
