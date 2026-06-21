//! Graceful-shutdown drain through the booted server: a record enqueued just
//! before shutdown must still reach the transparency log. The Supervisor / outbox
//! drain is unit-tested and concurrency-audited; this proves the wired shutdown
//! path (`main`'s SIGTERM → stop serving → drop app → close channel → drain →
//! `Supervisor::wait`) does not lose audit records.

use agate_audit::application::usecases::get_inclusion_proof::GetInclusionProof;
use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::{PolicyRuleset, ToolPolicy};

use crate::fixture::{self, spawn};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn graceful_shutdown_drains_queued_records_to_the_log() {
    // Allow-all: the run's three events (start, message, finish) are all recorded.
    let ruleset = PolicyRuleset::new(ToolPolicy::AllowAll, vec![], vec![]);
    let mut app = spawn(ruleset, fixture::SSE_BODY).await;

    // Drive a run: its events are inspected and enqueued on the audit outbox.
    let body = fixture::client()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");
    assert!(body.contains("RUN_FINISHED"), "the run streamed: {body}");

    // Shut down and await the drain — then assert the last event's leaf is in the
    // log with a SINGLE lookup (not a poll): `shutdown_and_drain` only returns
    // once the outbox has finished appending, so a still-queued record here would
    // mean the shutdown path lost it.
    app.shutdown_and_drain().await;

    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    let present = fixture::dispatch(
        &container,
        &registry,
        GetInclusionProof {
            log: app.log,
            index: LeafIndex(2),
        },
    )
    .await
    .is_ok();
    assert!(
        present,
        "every queued record was drained to the log before shutdown returned"
    );
}
