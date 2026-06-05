//! Boot the proxy + audit and assert a proxied run lands in the transparency log.

use std::time::Duration;

use agate_audit::application::usecases::get_inclusion_proof::GetInclusionProof;
use agate_audit::domain::merkle::{LeafIndex, LogId};
use froodi::async_impl::Container;

use crate::fixture::{self, spawn};

/// Outbox writes are asynchronous; poll the log up to ~5s (50 × 100ms).
const POLL_ATTEMPTS: usize = 50;
const POLL_INTERVAL_MS: u64 = 100;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxied_run_is_recorded_to_the_transparency_log() {
    let app = spawn().await;

    // Drive a run through the proxy: it forwards to the stub agent and streams
    // the inspected SSE back. The body proves the proxy forwarded the run.
    let body = reqwest::Client::new()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");
    assert!(body.contains("RUN_STARTED"), "run forwarded: {body}");
    assert!(body.contains("RUN_FINISHED"), "run finished: {body}");

    // The audit write happens off the hot path via the outbox, so poll until the
    // three events (leaves 0, 1, 2) have been appended.
    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();

    let present = poll_inclusion(&container, &registry, app.log, LeafIndex(2)).await;
    assert!(
        present,
        "expected the three inspected events recorded to the log"
    );
}

/// Poll the log for an inclusion proof of `index`, tolerating the outbox's
/// asynchronous write. Returns whether the leaf appeared within the timeout.
async fn poll_inclusion(
    container: &Container,
    registry: &std::sync::Arc<agate_audit::application::common::messaging::Registry<Container>>,
    log: LogId,
    index: LeafIndex,
) -> bool {
    for _ in 0..POLL_ATTEMPTS {
        let proof = fixture::dispatch(container, registry, GetInclusionProof { log, index }).await;
        if proof.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
    false
}
