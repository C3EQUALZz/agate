use jiff::Timestamp as Instant;

use crate::domain::common::values::Timestamp;
use crate::domain::ports::Clock;

/// `Clock` backed by the system wall clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_instant(Instant::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_after_2020() {
        // 2020-01-01T00:00:00Z in milliseconds.
        assert!(SystemClock.now().as_millis() > 1_577_836_800_000);
    }
}
