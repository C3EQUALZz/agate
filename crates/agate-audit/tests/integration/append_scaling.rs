//! Load measurement (ignored by default): how does one record's append latency
//! scale with the log's current size? Run manually:
//!   cargo test -p agate-audit --test integration append_latency -- --ignored --nocapture

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
#[ignore = "load measurement; run manually with --ignored --nocapture"]
async fn append_latency_grows_with_log_size() {
    let db = start().await;
    let container = build_container(&Storage::postgres(db.pool.clone()));
    let registry = Arc::new(build_registry());
    let log = dispatch(&container, &registry, CreateLog).await.unwrap();

    let n = 500usize;
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
    println!(
        "RATIO last/first: {:.1}x",
        last_avg.as_secs_f64() / first_avg.as_secs_f64().max(f64::MIN_POSITIVE)
    );
}
