/// A ceiling on how much one run's response stream may carry, so a hostile or
/// runaway upstream agent cannot stream unbounded output to the client. Counted
/// over the events the proxy reads from upstream (before forwarding), which is
/// the figure a DoS bound cares about.
///
/// Each limit is independent; `0` disables that limit. Crossing either ends the
/// run with a `RUN_ERROR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseBudget {
    /// Maximum number of response events in a run (`0` = unlimited).
    pub max_events: usize,
    /// Maximum total response bytes in a run (`0` = unlimited).
    pub max_bytes: usize,
}

impl ResponseBudget {
    /// An unbounded budget — no event or byte ceiling.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            max_events: 0,
            max_bytes: 0,
        }
    }

    /// Whether `events` / `bytes` seen so far have crossed a configured limit,
    /// with a short reason for the `RUN_ERROR` when they have.
    #[must_use]
    pub fn exceeded(self, events: usize, bytes: usize) -> Option<&'static str> {
        if self.max_events != 0 && events > self.max_events {
            return Some("response event budget exceeded");
        }
        if self.max_bytes != 0 && bytes > self.max_bytes {
            return Some("response byte budget exceeded");
        }
        None
    }
}

impl Default for ResponseBudget {
    fn default() -> Self {
        Self::unlimited()
    }
}

#[cfg(test)]
mod tests {
    use super::ResponseBudget;

    #[test]
    fn unlimited_never_trips() {
        let budget = ResponseBudget::unlimited();
        assert_eq!(budget.exceeded(1_000_000, 1_000_000), None);
    }

    #[test]
    fn each_limit_trips_independently() {
        let budget = ResponseBudget {
            max_events: 10,
            max_bytes: 100,
        };
        assert_eq!(budget.exceeded(10, 100), None, "at the limit is allowed");
        assert_eq!(
            budget.exceeded(11, 100),
            Some("response event budget exceeded")
        );
        assert_eq!(
            budget.exceeded(10, 101),
            Some("response byte budget exceeded")
        );
    }

    #[test]
    fn a_zeroed_limit_is_disabled() {
        let only_bytes = ResponseBudget {
            max_events: 0,
            max_bytes: 50,
        };
        assert_eq!(only_bytes.exceeded(9_999, 10), None, "events unlimited");
        assert_eq!(
            only_bytes.exceeded(9_999, 51),
            Some("response byte budget exceeded")
        );
    }
}
