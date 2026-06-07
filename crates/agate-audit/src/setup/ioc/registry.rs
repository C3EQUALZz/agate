use froodi::async_impl::Container;

use crate::application::common::behaviors::{MetricsBehavior, TransactionBehavior};
use crate::application::common::messaging::Registry;
use crate::application::usecases::append_record::{AppendRecord, AppendRecordHandler};
use crate::application::usecases::create_log::{CreateLog, CreateLogHandler};
use crate::application::usecases::get_consistency_proof::{
    GetConsistencyProof, GetConsistencyProofHandler,
};
use crate::application::usecases::get_inclusion_proof::{
    GetInclusionProof, GetInclusionProofHandler,
};
use crate::application::usecases::issue_checkpoint::{IssueCheckpoint, IssueCheckpointHandler};

/// The routing table: which handler and behaviors each request type uses.
/// Commands run inside the transaction behavior; read-only queries do not.
#[must_use]
pub fn build_registry() -> Registry<Container> {
    let mut registry = Registry::new();
    registry.handler::<CreateLog, CreateLogHandler>();
    registry.handler::<AppendRecord, AppendRecordHandler>();
    registry.handler::<GetInclusionProof, GetInclusionProofHandler>();
    registry.handler::<GetConsistencyProof, GetConsistencyProofHandler>();
    registry.handler::<IssueCheckpoint, IssueCheckpointHandler>();
    registry.behavior::<CreateLog, TransactionBehavior>();
    registry.behavior::<IssueCheckpoint, TransactionBehavior>();
    // MetricsBehavior is registered first so it is the outermost link on
    // AppendRecord: it records the outcome after TransactionBehavior has
    // committed or rolled back.
    registry.behavior::<AppendRecord, MetricsBehavior>();
    registry.behavior::<AppendRecord, TransactionBehavior>();
    registry
}
