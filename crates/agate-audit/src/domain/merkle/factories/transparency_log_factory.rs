use std::sync::Arc;

use agate_crypto::{Digest, Hasher};

use super::super::entities::TransparencyLog;
use super::super::services::MerkleHasher;
use super::super::values::LogId;
use crate::domain::common::events::EventCollection;
use crate::domain::common::factories::Factory;
use crate::domain::common::values::{Timestamp, Timestamps};

/// Assembles `TransparencyLog` aggregates, injecting the hashing strategy and a
/// fresh event collection.
pub struct TransparencyLogFactory {
    hasher: Arc<dyn Hasher>,
}

impl TransparencyLogFactory {
    pub fn new(hasher: Arc<dyn Hasher>) -> Self {
        Self { hasher }
    }

    pub fn create(&self, id: LogId, now: Timestamp) -> TransparencyLog {
        TransparencyLog::new(
            id,
            Timestamps::new(now),
            MerkleHasher::new(self.hasher.clone()),
            EventCollection::new(),
        )
    }

    pub fn reconstitute(
        &self,
        id: LogId,
        timestamps: Timestamps,
        leaves: Vec<Digest>,
    ) -> TransparencyLog {
        TransparencyLog::reconstitute(
            id,
            timestamps,
            MerkleHasher::new(self.hasher.clone()),
            leaves,
            EventCollection::new(),
        )
    }
}

impl Factory for TransparencyLogFactory {}
