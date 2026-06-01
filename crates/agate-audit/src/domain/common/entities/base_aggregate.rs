use super::base_entity::Entity;
use crate::domain::common::events::{DomainEvent, EventCollection};

/// An entity that is a consistency boundary and records domain events.
pub trait AggregateRoot: Entity {
    type Event: DomainEvent;

    fn events_mut(&mut self) -> &mut EventCollection<Self::Event>;

    fn pull_events(&mut self) -> Vec<Self::Event> {
        self.events_mut().pull()
    }
}
