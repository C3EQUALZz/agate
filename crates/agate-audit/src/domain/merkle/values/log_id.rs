use uuid::Uuid;

use crate::domain::common::values::ValueObject;

/// Identity of a transparency log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogId(pub Uuid);

impl ValueObject for LogId {}
