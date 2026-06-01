use jiff::Timestamp as Instant;

use super::base::ValueObject;
use crate::domain::common::errors::DomainError;

/// A UTC instant (wraps `jiff::Timestamp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(Instant);

impl Timestamp {
    pub fn from_instant(instant: Instant) -> Self {
        Self(instant)
    }

    pub fn from_millis(millis: i64) -> Result<Self, DomainError> {
        Instant::from_millisecond(millis)
            .map(Self)
            .map_err(|err| DomainError::Field(format!("invalid timestamp: {err}")))
    }

    pub fn as_instant(self) -> Instant {
        self.0
    }

    pub fn as_millis(self) -> i64 {
        self.0.as_millisecond()
    }
}

impl ValueObject for Timestamp {}
