//! Perf regression guard: one record's append latency must NOT grow with the
//! log's current size. Before the O(1) append fix this was ~10x at 500 leaves
//! (load-all + re-insert-all per append, O(n²)); now appending one leaf is a
//! single `INSERT`, so the latency of the last appends matches the first.
//! Run with `--nocapture` to see the numbers.

use std::sync::Arc;
use std::time::{Duration, Instant};

use froodi::async_impl::Container;

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::usecases::append_record::AppendRecord;
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_audit::setup::storage::Storage;

use crate::fixture::start;

async fn dispatch<R: Request>(
    container: &Container,
    registry: &Arc<Registry<Container>>,
    request: R,
) -> R::Response {
    let scope = Arc::new(container.clone().enter_build().expect("open request scope"));
    let dispatcher = Dispatcher::new(scope.clone(), registry.clone());
    let response = dispatcher.send(request).await;
    scope.close().await;
    response
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn append_latency_is_flat_in_log_size() {
    let db = start().await;
    let container = build_container(&Storage::postgres(db.pool.clone()));
    let registry = Arc::new(build_registry());
    let log = dispatch(&container, &registry, CreateLog).await.unwrap();

    let n = 300usize;
    let sample = 50usize;
    let mut first = Duration::ZERO;
    let mut last = Duration::ZERO;
    for i in 0..n {
        let started = Instant::now();
        dispatch(
            &container,
            &registry,
            AppendRecord {
                log,
                record: vec![(i & 0xff) as u8; 32],
            },
        )
        .await
        .unwrap();
        let elapsed = started.elapsed();
        if i < sample {
            first += elapsed;
        }
        if i >= n - sample {
            last += elapsed;
        }
    }
    let first_avg = first / sample as u32;
    let last_avg = last / sample as u32;
    println!("APPEND first {sample} avg: {first_avg:?}");
    println!("APPEND last  {sample} avg: {last_avg:?}  (log grown to ~{n} leaves)");
    let ratio = last_avg.as_secs_f64() / first_avg.as_secs_f64().max(f64::MIN_POSITIVE);
    println!("RATIO last/first: {ratio:.1}x");

    // O(1) append ⇒ flat. The pre-fix O(n²) was ~10x here; a generous 4x ceiling
    // guards the regression without flaking on CI scheduling noise.
    assert!(
        last_avg < first_avg * 4,
        "append latency grew with log size — the O(n²) append regressed: \
         first {sample} avg {first_avg:?}, last {sample} avg {last_avg:?} ({ratio:.1}x)"
    );
}
