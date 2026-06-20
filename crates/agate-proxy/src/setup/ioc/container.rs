use std::sync::Arc;
use std::time::Duration;

use froodi::{
    DefaultScope::App, Inject, async_impl::Container, async_registry, instance, registry,
};

use crate::application::common::ports::{AuditSink, HostResolver, PolicyPort, SessionMemory};
use crate::application::inspection::Inspector;
use crate::infrastructure::{
    AllowAllPolicy, InMemorySessionMemory, NoopAuditSink, NoopSessionMemory, ProxyMetricsRecorder,
    ReqwestAgentClient, TokioHostResolver,
};
use crate::setup::configs::{ProxyConfig, SessionMemoryBackend, SessionMemoryConfig};
use crate::setup::ioc::handles::{ProxyMetricsHandle, UpstreamAgentClientHandle};

/// Build the IoC container with the default adapters: an allow-all policy and a
/// no-op audit sink — the proxy run standalone, deciding nothing and recording
/// nowhere. The top-level server replaces these via [`build_container_with`].
#[must_use]
pub fn build_container(config: ProxyConfig) -> Container {
    build_container_with(config, Arc::new(AllowAllPolicy), Arc::new(NoopAuditSink))
}

/// Build the IoC container with an explicit policy and audit sink (everything
/// App-scoped: the proxy holds no per-request state). The `froodi` axum layer
/// opens a request scope per request and resolves these singletons from the App
/// parent.
///
/// The session-replay ledger is assembled here from [`ProxyConfig`] (a no-op
/// unless a TTL is configured), since the in-memory adapter is proxy-internal —
/// only the cross-context policy and audit adapters are injected.
#[must_use]
pub fn build_container_with(
    config: ProxyConfig,
    policy: Arc<dyn PolicyPort>,
    audit: Arc<dyn AuditSink>,
) -> Container {
    let ioc = async_registry! {
        extend(registry! {
            scope(App) [
                provide(instance(config)),
                provide(|Inject(config): Inject<ProxyConfig>| {
                    let client = reqwest::Client::builder()
                        .connect_timeout(config.connect_timeout)
                        .read_timeout(config.read_timeout)
                        .build()
                        .expect("build the upstream reqwest client");
                    Ok(UpstreamAgentClientHandle(Arc::new(
                        ReqwestAgentClient::with_client(client, config.agent_endpoint.clone()),
                    )))
                }),
                provide(|| Ok(ProxyMetricsHandle(Arc::new(ProxyMetricsRecorder)))),
                provide(move |Inject(config): Inject<ProxyConfig>| {
                    let resolver: Arc<dyn HostResolver> = Arc::new(TokioHostResolver);
                    let memory = build_session_memory(config.session_memory.as_ref());
                    Ok(Inspector::new(policy.clone(), audit.clone(), resolver, memory))
                }),
            ]
        })
    };
    Container::new_with_start_scope(ioc, App)
}

/// Assemble the session-replay ledger for the configured backend (or a no-op
/// when disabled). The in-memory backend is built directly; the Redis backend
/// is built behind the `redis` feature.
fn build_session_memory(config: Option<&SessionMemoryConfig>) -> Arc<dyn SessionMemory> {
    let Some(config) = config else {
        return Arc::new(NoopSessionMemory);
    };
    match &config.backend {
        SessionMemoryBackend::InMemory => Arc::new(InMemorySessionMemory::new(config.ttl)),
        SessionMemoryBackend::Redis(url) => build_redis_session_memory(url, config.ttl),
    }
}

#[cfg(feature = "redis")]
fn build_redis_session_memory(url: &str, ttl: Duration) -> Arc<dyn SessionMemory> {
    Arc::new(
        crate::infrastructure::RedisSessionMemory::new(url, ttl)
            .expect("a valid Redis URL for session memory"),
    )
}

#[cfg(not(feature = "redis"))]
fn build_redis_session_memory(_url: &str, _ttl: Duration) -> Arc<dyn SessionMemory> {
    panic!(
        "the `redis` session-memory backend requires building agate-proxy with the `redis` feature"
    )
}

#[cfg(test)]
mod tests {
    use super::{SessionMemoryBackend, SessionMemoryConfig, build_session_memory};
    use std::time::Duration;

    // `InMemorySessionMemory` spawns a pruner task, so a runtime is required.
    #[tokio::test]
    async fn builds_the_configured_session_memory_backend() {
        // Disabled → a (no-op) ledger is still produced.
        let _disabled = build_session_memory(None);

        let _in_memory = build_session_memory(Some(&SessionMemoryConfig {
            backend: SessionMemoryBackend::InMemory,
            ttl: Duration::from_mins(1),
        }));

        // The Redis backend only parses the URL here (no connection), so this
        // builds without a live Redis.
        #[cfg(feature = "redis")]
        let _redis = build_session_memory(Some(&SessionMemoryConfig {
            backend: SessionMemoryBackend::Redis("redis://127.0.0.1:6379".to_owned()),
            ttl: Duration::from_mins(1),
        }));
    }
}
