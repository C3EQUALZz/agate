use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use agate_audit::application::common::ports::{LogCommandGateway, LogQueryGateway};
use agate_audit::application::common::query_models::{ConsistencyProofView, InclusionProofView};
use agate_audit::application::errors::AuditError;
use agate_audit::domain::common::entities::Entity;
use agate_audit::domain::common::values::Timestamps;
use agate_audit::domain::merkle::{
    LeafIndex, LogId, MerkleHasher, MerkleProofs, MerkleTree, TransparencyLog,
    TransparencyLogFactory, TreeSize,
};
use agate_crypto::Digest;

use crate::common::factories::{epoch, log_factory, merkle_hasher};

/// In-memory store backing both CQRS gateways over the same leaf hashes.
pub struct InMemoryLogStore {
    factory: TransparencyLogFactory,
    hasher: MerkleHasher,
    leaves: Mutex<HashMap<LogId, Vec<Digest>>>,
}

impl InMemoryLogStore {
    pub fn new() -> Self {
        Self {
            factory: log_factory(),
            hasher: merkle_hasher(),
            leaves: Mutex::new(HashMap::new()),
        }
    }

    pub fn seed_empty(&self, id: LogId) {
        self.leaves.lock().unwrap().insert(id, Vec::new());
    }

    pub fn seed_with(&self, id: LogId, records: &[&[u8]]) {
        let mut log = self.factory.create(id, epoch());
        for record in records {
            log.append(record);
        }
        self.leaves
            .lock()
            .unwrap()
            .insert(id, log.leaf_hashes().to_vec());
    }
}

impl Default for InMemoryLogStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LogCommandGateway for InMemoryLogStore {
    async fn load(&self, id: LogId) -> Result<Option<TransparencyLog>, AuditError> {
        let guard = self.leaves.lock().unwrap();
        Ok(guard.get(&id).map(|leaves| {
            self.factory
                .reconstitute(id, Timestamps::new(epoch()), leaves.clone())
        }))
    }

    async fn save(&self, log: &TransparencyLog) -> Result<(), AuditError> {
        self.leaves
            .lock()
            .unwrap()
            .insert(*log.id(), log.leaf_hashes().to_vec());
        Ok(())
    }

    async fn append_record(
        &self,
        id: LogId,
        record: &[u8],
    ) -> Result<Option<LeafIndex>, AuditError> {
        let mut guard = self.leaves.lock().unwrap();
        let Some(leaves) = guard.get_mut(&id) else {
            return Ok(None);
        };
        let index = LeafIndex(leaves.len() as u64);
        leaves.push(self.hasher.leaf(record));
        Ok(Some(index))
    }
}

#[async_trait]
impl LogQueryGateway for InMemoryLogStore {
    async fn inclusion_proof(
        &self,
        id: LogId,
        index: LeafIndex,
    ) -> Result<InclusionProofView, AuditError> {
        let guard = self.leaves.lock().unwrap();
        let leaves = guard.get(&id).ok_or(AuditError::LogNotFound(id))?;
        let i = index.value() as usize;
        let proof =
            MerkleProofs::inclusion(&self.hasher, leaves, i).ok_or(AuditError::LeafOutOfRange {
                index,
                size: leaves.len() as u64,
            })?;
        Ok(InclusionProofView {
            proof,
            leaf_hash: leaves[i].clone(),
            root: MerkleTree::root(&self.hasher, leaves),
        })
    }

    async fn consistency_proof(
        &self,
        id: LogId,
        first: TreeSize,
    ) -> Result<ConsistencyProofView, AuditError> {
        let guard = self.leaves.lock().unwrap();
        let leaves = guard.get(&id).ok_or(AuditError::LogNotFound(id))?;
        let f = first.value() as usize;
        let proof = MerkleProofs::consistency(&self.hasher, leaves, f).ok_or(
            AuditError::SizeOutOfRange {
                requested: first.value(),
                current: leaves.len() as u64,
            },
        )?;
        Ok(ConsistencyProofView {
            proof,
            old_root: MerkleTree::root(&self.hasher, &leaves[..f]),
            new_root: MerkleTree::root(&self.hasher, leaves),
        })
    }
}
