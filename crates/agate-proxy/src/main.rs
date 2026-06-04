//! Runs the proxy. Configure with `AGENT_ENDPOINT` (required) and `BIND_ADDR`
//! (default `0.0.0.0:8080`).

use agate_proxy::setup::bootstrap::build_app;
use agate_proxy::setup::configs::ProxyConfig;

#[tokio::main]
async fn main() {
    let config = ProxyConfig::from_env();
    let bind_addr = config.bind_addr.clone();

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind listener");
    axum::serve(listener, build_app(config))
        .await
        .expect("serve");
}
