use std::sync::Arc;

use froodi::{
    DefaultScope::App, Inject, async_impl::Container, async_registry, instance, registry,
};

use crate::application::common::ports::{AuditSink, PolicyPort};
use crate::application::inspection::Inspector;
use crate::infrastructure::{AllowAllPolicy, NoopAuditSink, ReqwestAgentClient};
use crate::setup::configs::ProxyConfig;

/// Build the IoC container (everything App-scoped: the proxy holds no
/// per-request state). The `froodi` axum layer opens a request scope per
/// request and resolves these singletons from the App parent.
#[must_use]
pub fn build_container(config: ProxyConfig) -> Container {
    let ioc = async_registry! {
        extend(registry! {
            scope(App) [
                provide(instance(config)),
                provide(|| Ok(AllowAllPolicy)),
                provide(|| Ok(NoopAuditSink)),
                provide(|Inject(config): Inject<ProxyConfig>| {
                    Ok(ReqwestAgentClient::new(config.agent_endpoint.clone()))
                }),
                provide(
                    |Inject(policy): Inject<AllowAllPolicy>, Inject(audit): Inject<NoopAuditSink>| {
                        let policy: Arc<dyn PolicyPort> = policy;
                        let audit: Arc<dyn AuditSink> = audit;
                        Ok(Inspector::new(policy, audit))
                    },
                ),
            ]
        })
    };
    Container::new_with_start_scope(ioc, App)
}
