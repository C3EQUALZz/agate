use std::sync::Arc;

use froodi::{
    DefaultScope::App, Inject, async_impl::Container, async_registry, instance, registry,
};

use crate::application::common::ports::{AuditSink, PolicyPort};
use crate::application::inspection::Inspector;
use crate::infrastructure::{
    AllowAllPolicy, NoopAuditSink, ProxyMetricsRecorder, ReqwestAgentClient,
};
use crate::setup::configs::ProxyConfig;
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
                provide(move || Ok(Inspector::new(policy.clone(), audit.clone()))),
            ]
        })
    };
    Container::new_with_start_scope(ioc, App)
}
