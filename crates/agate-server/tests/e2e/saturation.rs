//! Lifecycle e2e: under [`FullPolicy::Shed`] the data plane stays fully available
//! even when the audit outbox cannot keep up. With a single-slot outbox and a
//! burst of events, most audit records are shed — but the proxy forwards every
//! event to the client and never blocks on the saturated channel. This proves
//! the booted-server wiring (`ServerConfig` → `OutboxSettings` → `AuditLogSink`)
//! and the Shed availability contract end-to-end. The drop accounting itself
//! (counted + logged, never silent) is unit-tested in `sink.rs`.

use std::fmt::Write as _;
use std::sync::Arc;

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::application::PolicyService;
use agate_policy::domain::decision::{PolicyRuleset, ToolPolicy};
use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::setup::configs::ProxyConfig;
use agate_server::infrastructure::audit::FullPolicy;
use agate_server::infrastructure::policy::PolicyAdapter;
use agate_server::setup::bootstrap::OutboxSettings;

use crate::fixture::{
    audit_container, audit_registry, client, poll_inclusion, spawn_core_with_outbox,
    stub_agent_sequence,
};

/// Enough message chunks that, against a single-slot outbox, the burst far
/// outruns the serial Postgres drain — so shedding is exercised, not incidental.
const CHUNKS: usize = 40;

/// One run that streams `CHUNKS` message chunks between the lifecycle events —
/// each chunk is a recordable event, so the audit outbox sees a rapid burst.
fn burst_sse() -> String {
    let mut sse = String::from("data: {\"type\":\"RUN_STARTED\"}\n\n");
    for i in 0..CHUNKS {
        let _ = write!(
            sse,
            "data: {{\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"chunk-{i}\"}}\n\n"
        );
    }
    sse.push_str("data: {\"type\":\"RUN_FINISHED\"}\n\n");
    sse
}

fn allow_all() -> Arc<dyn PolicyPort> {
    Arc::new(PolicyAdapter::new(PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        vec![],
        vec![],
    ))))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shed_keeps_the_data_plane_available_under_audit_saturation() {
    let endpoint = stub_agent_sequence(vec![burst_sse()]).await;
    let app = spawn_core_with_outbox(
        allow_all(),
        ProxyConfig::new(endpoint, "127.0.0.1:0".to_string()),
        OutboxSettings {
            capacity: 1,
            on_full: FullPolicy::Shed,
        },
    )
    .await;

    let body = client()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");

    // Availability: every forwarded chunk reached the client even though the
    // single-slot outbox sheds most audit records — Shed never blocks the proxy.
    assert!(
        body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
        "lifecycle forwarded: {body}"
    );
    for i in 0..CHUNKS {
        assert!(
            body.contains(&format!("chunk-{i}")),
            "chunk {i} forwarded despite audit shedding: {body}"
        );
    }

    // Audit is not disabled by Shed: the first record (enqueued while the channel
    // is empty) is durably appended — shedding drops records under pressure, it
    // does not break the write path.
    let container = audit_container(app.pool.clone());
    let registry = audit_registry();
    assert!(
        poll_inclusion(&container, &registry, app.log, LeafIndex(0)).await,
        "the first record is durably appended even under Shed"
    );
}
