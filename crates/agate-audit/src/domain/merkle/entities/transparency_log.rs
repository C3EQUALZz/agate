use agate_crypto::{Digest, HashAlgo};

use super::super::events::AuditEvent;
use super::super::services::{MerkleHasher, MerkleProofs, MerkleTree};
use super::super::values::{
    ConsistencyProof, InclusionProof, LeafIndex, LogId, TreeHead, TreeSize,
};
use crate::domain::common::entities::{AggregateRoot, Entity};
use crate::domain::common::events::EventCollection;
use crate::domain::common::values::{Timestamp, Timestamps};

/// Append-only transparency log (aggregate root).
///
/// Invariants: append-only, monotonic size, single hash-algorithm epoch.
/// Constructed through `TransparencyLogFactory`, not directly.
pub struct TransparencyLog {
    id: LogId,
    timestamps: Timestamps,
    hasher: MerkleHasher,
    leaves: Vec<Digest>,
    events: EventCollection<AuditEvent>,
}

impl TransparencyLog {
    pub(crate) fn new(
        id: LogId,
        timestamps: Timestamps,
        hasher: MerkleHasher,
        events: EventCollection<AuditEvent>,
    ) -> Self {
        Self {
            id,
            timestamps,
            hasher,
            leaves: Vec::new(),
            events,
        }
    }

    pub(crate) fn reconstitute(
        id: LogId,
        timestamps: Timestamps,
        hasher: MerkleHasher,
        leaves: Vec<Digest>,
        events: EventCollection<AuditEvent>,
    ) -> Self {
        Self {
            id,
            timestamps,
            hasher,
            leaves,
            events,
        }
    }

    pub fn created_at(&self) -> Timestamp {
        self.timestamps.created_at()
    }

    pub fn updated_at(&self) -> Timestamp {
        self.timestamps.updated_at()
    }

    pub fn algo(&self) -> HashAlgo {
        self.hasher.algo()
    }

    pub fn size(&self) -> TreeSize {
        TreeSize(self.leaves.len() as u64)
    }

    pub fn append(&mut self, record: &[u8]) -> LeafIndex {
        let index = LeafIndex(self.leaves.len() as u64);
        let leaf = self.hasher.leaf(record);
        self.leaves.push(leaf.clone());
        self.events
            .record(AuditEvent::RecordAppended { index, leaf });
        index
    }

    pub fn root(&self) -> Digest {
        MerkleTree::root(&self.hasher, &self.leaves)
    }

    pub fn head(&self, at: Timestamp) -> TreeHead {
        TreeHead {
            size: self.size(),
            root: self.root(),
            at,
        }
    }

    /// Issue a checkpoint: snapshot the head and record a domain event. The
    /// application layer signs the returned head into a `SignedTreeHead`.
    pub fn issue_checkpoint(&mut self, at: Timestamp) -> TreeHead {
        let head = self.head(at);
        self.events
            .record(AuditEvent::CheckpointIssued { head: head.clone() });
        head
    }

    pub fn leaf_hashes(&self) -> &[Digest] {
        &self.leaves
    }

    pub fn prove_inclusion(&self, index: LeafIndex) -> Option<InclusionProof> {
        MerkleProofs::inclusion(&self.hasher, &self.leaves, index.value() as usize)
    }

    pub fn prove_consistency(&self, first_size: TreeSize) -> Option<ConsistencyProof> {
        MerkleProofs::consistency(&self.hasher, &self.leaves, first_size.value() as usize)
    }
}

impl Entity for TransparencyLog {
    type Id = LogId;

    fn id(&self) -> &LogId {
        &self.id
    }
}

impl AggregateRoot for TransparencyLog {
    type Event = AuditEvent;

    fn events_mut(&mut self) -> &mut EventCollection<AuditEvent> {
        &mut self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::merkle::factories::TransparencyLogFactory;
    use agate_crypto::CryptoRegistry;
    use uuid::Uuid;

    fn factory() -> TransparencyLogFactory {
        TransparencyLogFactory::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
    }

    fn ts(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis).unwrap()
    }

    fn log() -> TransparencyLog {
        factory().create(LogId(Uuid::nil()), ts(0))
    }

    #[test]
    fn append_assigns_monotonic_indices_and_grows_size() {
        let mut l = log();
        assert_eq!(l.size(), TreeSize(0));
        assert_eq!(l.append(b"first"), LeafIndex(0));
        assert_eq!(l.append(b"second"), LeafIndex(1));
        assert_eq!(l.size(), TreeSize(2));
    }

    #[test]
    fn append_records_domain_events() {
        let mut l = log();
        l.append(b"x");
        l.append(b"y");
        assert_eq!(l.pull_events().len(), 2);
        assert!(l.pull_events().is_empty());
    }

    #[test]
    fn root_changes_after_append() {
        let mut l = log();
        let r0 = l.root();
        l.append(b"x");
        assert_ne!(r0, l.root());
    }

    #[test]
    fn reconstitute_reproduces_the_same_root() {
        let f = factory();
        let mut a = f.create(LogId(Uuid::nil()), ts(0));
        a.append(b"alpha");
        a.append(b"beta");

        let b = f.reconstitute(
            LogId(Uuid::nil()),
            Timestamps::new(ts(0)),
            a.leaf_hashes().to_vec(),
        );
        assert_eq!(a.root(), b.root());
        assert_eq!(a.size(), b.size());
    }

    #[test]
    fn head_carries_size_root_and_timestamp() {
        let mut l = log();
        l.append(b"only");
        let head = l.head(ts(1_700_000_000_000));
        assert_eq!(head.size, TreeSize(1));
        assert_eq!(head.root, l.root());
        assert_eq!(head.at, ts(1_700_000_000_000));
    }
}
