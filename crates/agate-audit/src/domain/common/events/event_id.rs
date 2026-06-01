use uuid::Uuid;

use crate::domain::common::values::ValueObject;

/// Identifier of a domain event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventId(pub Uuid);

impl ValueObject for EventId {}
