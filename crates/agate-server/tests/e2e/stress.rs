//! Stress the full data plane: drive many proxied runs concurrently and assert
//! every inspected event still lands in the transparency log (no record lost
//! under concurrent outbox producers) and that no run deadlocks or is dropped.

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::PolicyRuleset;

use crate::fixture::{self, spawn};

/// Concurrent proxied runs to fire at once. Each emits three Ready events
/// (RUN_STARTED, one message chunk, RUN_FINISHED) = three log leaves. Kept under
/// the default concurrency cap (256) so none is shed with a `503`.
const RUNS: u64 = 64;
const EVENTS_PER_RUN: u64 = 3;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_concurrent_runs_all_land_in_the_log() {
    let app = spawn(PolicyRuleset::allow_all(), fixture::SSE_BODY).await;
    let client = fixture::client();

    // Fire every run at once; each drives a full proxy → upstream → audit cycle,
    // and the audit outbox sees all of them as concurrent producers.
    let mut tasks = Vec::with_capacity(RUNS as usize);
    for run in 0..RUNS {
        let client = client.clone();
        let url = app.base_url.clone();
        tasks.push(tokio::spawn(async move {
            client
                .post(&url)
                // A distinct session/run per task, so the runs are independent.
                .body(format!(r#"{{"threadId":"t{run}","runId":"r{run}"}}"#))
                .send()
                .await
                .expect("proxy responds")
                .text()
                .await
                .expect("read streamed body")
        }));
    }

    // Every run completes (no deadlock, no leaked concurrency permit) and is
    // forwarded whole (no truncation under load).
    for task in tasks {
        let body = task.await.expect("run task joins");
        assert!(
            body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
            "a concurrent run was truncated or dropped: {body}"
        );
    }

    // The outbox has a single consumer draining a FIFO channel, so if the last
    // leaf is present then every one of the RUNS * EVENTS_PER_RUN records was
    // drained in order with none lost under concurrent producers.
    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    let total = RUNS * EVENTS_PER_RUN;
    let recorded =
        fixture::poll_inclusion(&container, &registry, app.log, LeafIndex(total - 1)).await;
    assert!(
        recorded,
        "all {total} records landed in the log under concurrent load"
    );
}
