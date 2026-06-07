use froodi::async_impl::Container;

use crate::application::common::behaviors::{
    MetricsBehavior, TracingBehavior, TransactionBehavior,
};
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
    // TracingBehavior is registered first on every request type, so its
    // `audit.request` span is the outermost link — it encloses the metrics and
    // transaction behaviors and any spans the handler/gateways open inside.
    registry.behavior::<CreateLog, TracingBehavior>();
    registry.behavior::<AppendRecord, TracingBehavior>();
    registry.behavior::<GetInclusionProof, TracingBehavior>();
    registry.behavior::<GetConsistencyProof, TracingBehavior>();
    registry.behavior::<IssueCheckpoint, TracingBehavior>();
    registry.behavior::<CreateLog, TransactionBehavior>();
    registry.behavior::<IssueCheckpoint, TransactionBehavior>();
    // MetricsBehavior is registered before TransactionBehavior so it is the
    // outer of the two on AppendRecord: it records the outcome after the
    // transaction behavior has committed or rolled back.
    registry.behavior::<AppendRecord, MetricsBehavior>();
    registry.behavior::<AppendRecord, TransactionBehavior>();
    registry
}
