use super::event_id::EventId;
use crate::domain::common::values::Timestamp;

pub trait DomainEvent: Clone {
    fn event_type(&self) -> &'static str;
}

/// Metadata assigned to an event when recorded (by the application layer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventMeta {
    pub event_id: EventId,
    pub occurred_at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedEvent<E> {
    pub meta: EventMeta,
    pub payload: E,
}
