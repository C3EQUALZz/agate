use agate_audit::domain::merkle::LogId;
use agate_audit::domain::ports::IdGenerator;

pub struct FixedId(pub LogId);

impl IdGenerator<LogId> for FixedId {
    fn generate(&self) -> LogId {
        self.0
    }
}
