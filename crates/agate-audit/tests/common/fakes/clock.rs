use agate_audit::domain::common::values::Timestamp;
use agate_audit::domain::ports::Clock;

pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}
