use super::base::ValueObject;
use super::timestamp::Timestamp;
use crate::domain::common::errors::{DomainError, TimeError};

/// Lifecycle timestamps of an entity, holding the invariant
/// `created_at <= updated_at`. Immutable: `touched` returns a new value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timestamps {
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl Timestamps {
    pub fn new(now: Timestamp) -> Self {
        Self {
            created_at: now,
            updated_at: now,
        }
    }

    pub fn reconstitute(created_at: Timestamp, updated_at: Timestamp) -> Result<Self, DomainError> {
        if updated_at < created_at {
            return Err(TimeError::InconsistentTime {
                created_at: created_at.as_millis(),
                updated_at: updated_at.as_millis(),
            }
            .into());
        }
        Ok(Self {
            created_at,
            updated_at,
        })
    }

    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }

    pub fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    pub fn touched(&self, now: Timestamp) -> Result<Self, DomainError> {
        if now < self.created_at {
            return Err(TimeError::InconsistentTime {
                created_at: self.created_at.as_millis(),
                updated_at: now.as_millis(),
            }
            .into());
        }
        Ok(Self {
            created_at: self.created_at,
            updated_at: now,
        })
    }
}

impl ValueObject for Timestamps {}
