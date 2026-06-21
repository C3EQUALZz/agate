//! Runs the full Agate server: an inspecting reverse proxy that records every
//! decision to the audit transparency log.
//!
//! Configuration is loaded from `agate.toml` (path from `AGATE_CONFIG`, default
//! `/etc/agate/agate.toml`) layered with `AGATE__SECTION__KEY` environment
//! overrides — see `agate.example.toml`. `AUDIT_LOG_ID` (a UUID) optionally
//! pins the transparency log; when unset a fresh log is created on startup.

use std::net::SocketAddr;
use std::sync::Arc;

use axum_server::Handle;
use uuid::Uuid;

use agate_audit::application::common::messaging::Dispatcher;
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::domain::merkle::LogId;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_audit::setup::storage::Storage;
use agate_policy::application::PolicyService;
use agate_proxy::application::common::ports::PolicyPort;
use agate_server::infrastructure::policy::PolicyAdapter;
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
use agate_server::infrastructure::policy::ReloadablePolicy;
use agate_server::setup::bootstrap::{ServerConfig, Supervisor, build_server};
use agate_server::setup::configs::{AppConfig, PolicyBackendKind, load};
use agate_server::setup::observability::{init_logging, init_metrics};
use agate_server::setup::tls::load_tls;
use tracing::info;

#[tokio::main]
async fn main() {
    let config = load().expect("load configuration");
    config
        .validate()
        .unwrap_or_else(|error| panic!("invalid configuration: {error}"));
    let tracer_provider =
        init_logging(&config.observability.logging, &config.observability.tracing);
    if init_metrics(&config.observability.metrics) {
        info!(bind = %config.observability.metrics.bind, "Prometheus metrics endpoint serving /metrics");
    }

    // Build everything that can fail from config before any I/O, so a bad config
    // aborts startup before connecting to Postgres or creating a log.
    let proxy = config.proxy_config();
    let bind_addr = proxy.bind_addr.clone();
    // Supervises every background task (audit outbox, checkpoint scheduler, policy
    // hot-reload) under one shutdown token + tracker.
    let supervisor = Supervisor::new();
    // The decision engine — the static ruleset or a plugin engine (CEL / Rego) —
    // built here at the composition root; a bad ruleset / unparsable policy aborts.
    let policy = build_policy(&config, &supervisor);
    let storage_config = config.storage_config();
    let pinned_log_id = std::env::var("AUDIT_LOG_ID")
        .ok()
        .map(|raw| LogId(raw.parse::<Uuid>().expect("AUDIT_LOG_ID must be a UUID")));
    let addr: SocketAddr = bind_addr
        .parse()
        .unwrap_or_else(|_| panic!("proxy.bind must be a host:port address, got {bind_addr}"));
    // Load the TLS cert/key now (local file I/O) so a bad listener config aborts
    // startup before we connect to Postgres or create a log.
    let tls_acceptor = match config.tls_config() {
        Some(tls) => Some(load_tls(tls).await),
        None => None,
    };

    info!("configuration loaded; starting agate-server");

    let storage = Storage::connect(&storage_config)
        .await
        .expect("connect to the transparency-log store");
    info!("connected to the store and applied migrations");

    let log = resolve_log(&storage, pinned_log_id).await;
    info!(log = %log.0, "recording to transparency log");
    let server = build_server(
        &storage,
        ServerConfig {
            proxy,
            log,
            policy,
            fail_mode: config.policy_fail_mode(),
            decision_timeout: config.policy_decision_timeout(),
            checkpoint: config.checkpoint_settings(),
            outbox: config.outbox_settings(),
        },
        &supervisor,
    );

    // Drive graceful shutdown through an axum-server Handle: on SIGINT/SIGTERM
    // trip the supervisor's token (background tasks return at their next
    // boundary), then stop accepting and wait (no deadline) for in-flight runs.
    let handle = Handle::new();
    tokio::spawn({
        let handle = handle.clone();
        let supervisor = supervisor.clone();
        async move {
            shutdown_signal().await;
            supervisor.trigger();
            handle.graceful_shutdown(None);
        }
    });

    // Carry the connection's peer address into each request so the per-IP rate
    // limiter can key on it (axum-server populates ConnectInfo on both paths).
    let make_service = server
        .app
        .into_make_service_with_connect_info::<SocketAddr>();
    if let Some(rustls) = tls_acceptor {
        info!(%bind_addr, "agate-server listening (TLS)");
        axum_server::bind_rustls(addr, rustls)
            .handle(handle)
            .serve(make_service)
            .await
            .expect("serve TLS");
    } else {
        info!(%bind_addr, "agate-server listening");
        axum_server::bind(addr)
            .handle(handle)
            .serve(make_service)
            .await
            .expect("serve");
    }

    // `serve` has returned, so the served app — and the audit sink inside it — is
    // dropped, closing the outbox channel. Wait for every supervised task: the
    // checkpoint scheduler (already returning on the token) and the outbox (now
    // draining its remaining queued records to the log) before the process exits.
    info!("draining background tasks");
    supervisor.wait().await;

    // Flush any spans still buffered in the OTLP batch exporter before exit.
    if let Some(provider) = tracer_provider
        && let Err(error) = provider.shutdown()
    {
        tracing::warn!(%error, "failed to flush the OTLP tracer on shutdown");
    }
    info!("shutdown complete");
}

/// Build the configured decision engine. The static `ruleset` backend bridges
/// the `agate-policy` ruleset; the `cel` / `rego` backends compile the operator's
/// policy (each only when built with its feature). All are wrapped in the
/// fail-mode guard later by `build_server`. A bad ruleset or policy aborts.
fn build_policy(config: &AppConfig, supervisor: &Supervisor) -> Arc<dyn PolicyPort> {
    match config.policy.backend {
        PolicyBackendKind::Ruleset => {
            let ruleset = config
                .policy_ruleset()
                .expect("invalid policy configuration");
            Arc::new(PolicyAdapter::new(PolicyService::new(ruleset)))
        }
        PolicyBackendKind::Cel => build_cel_policy(config, supervisor),
        PolicyBackendKind::Rego => build_rego_policy(config, supervisor),
    }
}

#[cfg(feature = "policy-cel")]
fn build_cel_policy(config: &AppConfig, supervisor: &Supervisor) -> Arc<dyn PolicyPort> {
    use agate_server::infrastructure::policy::CelPolicyAdapter;

    let path = config
        .policy
        .cel
        .policy_path
        .as_deref()
        .expect("validate() requires policy.cel.policy_path when backend = cel");
    let adapter = Arc::new(
        CelPolicyAdapter::load(path, config.policy.cel.max_rules)
            .unwrap_or_else(|error| panic!("invalid CEL policy: {error}")),
    );
    let reloadable: Arc<dyn ReloadablePolicy> = adapter.clone();
    wire_policy_reload(reloadable, path, config.policy.cel.watch, supervisor);
    adapter
}

#[cfg(not(feature = "policy-cel"))]
fn build_cel_policy(_config: &AppConfig, _supervisor: &Supervisor) -> Arc<dyn PolicyPort> {
    panic!("policy.backend = \"cel\" requires building agate-server with the `policy-cel` feature")
}

#[cfg(feature = "policy-rego")]
fn build_rego_policy(config: &AppConfig, supervisor: &Supervisor) -> Arc<dyn PolicyPort> {
    use agate_server::infrastructure::policy::RegoPolicyAdapter;

    let path = config
        .policy
        .rego
        .policy_path
        .as_deref()
        .expect("validate() requires policy.rego.policy_path when backend = rego");
    let adapter = Arc::new(
        RegoPolicyAdapter::load(path)
            .unwrap_or_else(|error| panic!("invalid Rego policy: {error}")),
    );
    let reloadable: Arc<dyn ReloadablePolicy> = adapter.clone();
    wire_policy_reload(reloadable, path, config.policy.rego.watch, supervisor);
    adapter
}

#[cfg(not(feature = "policy-rego"))]
fn build_rego_policy(_config: &AppConfig, _supervisor: &Supervisor) -> Arc<dyn PolicyPort> {
    panic!(
        "policy.backend = \"rego\" requires building agate-server with the `policy-rego` feature"
    )
}

/// Wire a file-backed policy engine's reloads: always SIGHUP (Unix), plus the
/// on-disk file-watch when the operator opted in. Shared by the CEL and Rego
/// backends through the [`ReloadablePolicy`] trait.
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
fn wire_policy_reload(
    adapter: Arc<dyn ReloadablePolicy>,
    path: &str,
    watch: bool,
    supervisor: &Supervisor,
) {
    // Hot-reload on SIGHUP (Unix only). The reload is fail-safe — a bad file keeps
    // the running policy — so the listener never loses its rules.
    #[cfg(unix)]
    spawn_policy_reload(Arc::clone(&adapter), supervisor);
    // Optionally also auto-reload when the file changes on disk (cross-platform,
    // opt-in via `[policy.*].watch`). Same fail-safe reload as SIGHUP.
    if watch {
        spawn_policy_watch(adapter, path.to_owned(), supervisor);
    }
}

/// Debounce window for file-watch reloads: a single editor save fires several
/// filesystem events (truncate, write, rename), so wait briefly and coalesce
/// them into one reload.
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
const WATCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(200);

/// Auto-reload the policy when its file changes on disk. Supervised, so it
/// returns when the shutdown token is tripped. A failure to install the watcher
/// is logged and leaves the SIGHUP reload (Unix) still in place.
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
fn spawn_policy_watch(adapter: Arc<dyn ReloadablePolicy>, path: String, supervisor: &Supervisor) {
    use agate_server::infrastructure::policy::policy_watch;

    let mut watch = match policy_watch::watch(std::path::Path::new(&path)) {
        Ok(watch) => watch,
        Err(error) => {
            tracing::error!(%error, "cannot watch the policy file; auto-reload disabled");
            return;
        }
    };
    let shutdown = supervisor.token();
    supervisor.spawn(async move {
        info!(
            path,
            "policy file-watch armed: edits auto-reload the policy"
        );
        loop {
            tokio::select! {
                // Stop promptly on shutdown rather than waiting for another change.
                biased;
                () = shutdown.cancelled() => break,
                change = watch.changes.recv() => {
                    if change.is_none() {
                        break;
                    }
                    // Coalesce the burst of events a single save emits into one
                    // reload: wait a beat, then drain whatever else has queued.
                    tokio::time::sleep(WATCH_DEBOUNCE).await;
                    while watch.changes.try_recv().is_ok() {}
                    reload_in_place(&adapter, "file change").await;
                }
            }
        }
    });
}

/// Reload the policy on every `SIGHUP`. Supervised, so it returns when the
/// shutdown token is tripped; it only swaps an in-memory rule set, holding no
/// resource that shutdown must drain.
#[cfg(all(unix, any(feature = "policy-cel", feature = "policy-rego")))]
fn spawn_policy_reload(adapter: Arc<dyn ReloadablePolicy>, supervisor: &Supervisor) {
    use tokio::signal::unix::{SignalKind, signal};

    let shutdown = supervisor.token();
    supervisor.spawn(async move {
        let mut sighup = match signal(SignalKind::hangup()) {
            Ok(stream) => stream,
            Err(error) => {
                tracing::error!(%error, "cannot install the SIGHUP handler; hot-reload disabled");
                return;
            }
        };
        info!("policy hot-reload armed: send SIGHUP to reload the policy file");
        loop {
            tokio::select! {
                // Stop promptly on shutdown rather than waiting for another signal.
                biased;
                () = shutdown.cancelled() => break,
                signal = sighup.recv() => {
                    if signal.is_none() {
                        break;
                    }
                    reload_in_place(&adapter, "SIGHUP").await;
                }
            }
        }
    });
}

/// Run one fail-safe reload on a blocking thread. `spawn_blocking` keeps the
/// blocking file read off the async workers and isolates a panic in compilation
/// as a `JoinError`, so a single bad reload can never kill the handler.
#[cfg(any(feature = "policy-cel", feature = "policy-rego"))]
async fn reload_in_place(adapter: &Arc<dyn ReloadablePolicy>, trigger: &str) {
    let adapter = Arc::clone(adapter);
    match tokio::task::spawn_blocking(move || adapter.reload_policy()).await {
        Ok(Ok(())) => info!(trigger, "reloaded policy"),
        Ok(Err(error)) => {
            tracing::error!(%error, trigger, "policy reload failed; keeping the current policy");
        }
        Err(join) => {
            tracing::error!(%join, trigger, "policy reload panicked; keeping the current policy");
        }
    }
}

/// Resolves once the process receives SIGINT (Ctrl+C) or SIGTERM (the signal a
/// container runtime sends to stop), triggering an axum graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install the Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install the SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("shutdown signal received; stopping new work");
}

/// The transparency log to record into: `AUDIT_LOG_ID` if set, else a freshly
/// created log (so a first run is self-contained; set the env var to keep
/// appending to the same log across restarts).
async fn resolve_log(storage: &Storage, pinned: Option<LogId>) -> LogId {
    if let Some(id) = pinned {
        return id;
    }

    let container = build_container(storage);
    let registry = Arc::new(build_registry());
    let scope = Arc::new(container.enter_build().expect("open request scope"));
    let dispatcher = Dispatcher::new(scope.clone(), registry);
    let log = dispatcher
        .send(CreateLog)
        .await
        .expect("create transparency log");
    scope.close().await;
    // Printed (not just traced) so it shows without a subscriber configured:
    // operators need the id to set AUDIT_LOG_ID and keep the same log on restart.
    println!(
        "created transparency log {0}; set AUDIT_LOG_ID={0} to reuse it",
        log.0
    );
    log
}
