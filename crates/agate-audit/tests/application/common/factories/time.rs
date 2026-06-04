use agate_audit::domain::common::values::Timestamp;

pub fn epoch() -> Timestamp {
    Timestamp::from_millis(0).unwrap()
}
