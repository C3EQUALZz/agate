use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;

use agate_crypto::Digest;

use crate::application::common::ports::LogQueryGateway;
use crate::application::common::query_models::{ConsistencyProofView, InclusionProofView};
use crate::application::errors::AuditError;
use crate::domain::merkle::{LeafIndex, LogId, MerkleHasher, MerkleProofs, MerkleTree, TreeSize};
use crate::infrastructure::persistence::postgres::storage_error;

/// Read-side gateway backed by PostgreSQL, building proof read models.
pub struct PostgresLogQueryGateway {
    pool: PgPool,
    hasher: MerkleHasher,
}

impl PostgresLogQueryGateway {
    pub fn new(pool: PgPool, hasher: MerkleHasher) -> Self {
        Self { pool, hasher }
    }

    async fn log_exists(&self, id: LogId) -> Result<bool, AuditError> {
        let (exists,): (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM audit_log WHERE id = $1)")
                .bind(id.0)
                .fetch_one(&self.pool)
                .await
                .map_err(storage_error)?;
        Ok(exists)
    }

    async fn load_leaves(&self, id: LogId) -> Result<Vec<Digest>, AuditError> {
        let rows = sqlx::query_as::<_, (Vec<u8>,)>(
            "SELECT leaf_hash FROM audit_leaf WHERE log_id = $1 ORDER BY leaf_index",
        )
        .bind(id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(storage_error)?;

        Ok(rows
            .into_iter()
            .map(|(bytes,)| Digest {
                algo: self.hasher.algo(),
                bytes,
            })
            .collect())
    }
}

#[async_trait]
impl LogQueryGateway for PostgresLogQueryGateway {
    #[instrument(name = "db.proof.inclusion", skip_all, fields(log = %id.0, index = index.value()))]
    async fn inclusion_proof(
        &self,
        id: LogId,
        index: LeafIndex,
    ) -> Result<InclusionProofView, AuditError> {
        if !self.log_exists(id).await? {
            return Err(AuditError::LogNotFound(id));
        }
        let leaves = self.load_leaves(id).await?;
        let i = index.value() as usize;
        let proof = MerkleProofs::inclusion(&self.hasher, &leaves, i).ok_or(
            AuditError::LeafOutOfRange {
                index,
                size: leaves.len() as u64,
            },
        )?;
        Ok(InclusionProofView {
            proof,
            leaf_hash: leaves[i].clone(),
            root: MerkleTree::root(&self.hasher, &leaves),
        })
    }

    #[instrument(name = "db.proof.consistency", skip_all, fields(log = %id.0, first = first.value()))]
    async fn consistency_proof(
        &self,
        id: LogId,
        first: TreeSize,
    ) -> Result<ConsistencyProofView, AuditError> {
        if !self.log_exists(id).await? {
            return Err(AuditError::LogNotFound(id));
        }
        let leaves = self.load_leaves(id).await?;
        let f = first.value() as usize;
        let proof = MerkleProofs::consistency(&self.hasher, &leaves, f).ok_or(
            AuditError::SizeOutOfRange {
                requested: first.value(),
                current: leaves.len() as u64,
            },
        )?;
        Ok(ConsistencyProofView {
            proof,
            old_root: MerkleTree::root(&self.hasher, &leaves[..f]),
            new_root: MerkleTree::root(&self.hasher, &leaves),
        })
    }
}
