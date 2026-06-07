use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tracing::warn;

use agate_audit::application::common::ports::HealthCheck;

/// The `/readyz` route, reporting readiness from the backing store's health.
///
/// Liveness (`/healthz`, served by the proxy) only says the process is up;
/// readiness adds "can it reach its dependencies", so an orchestrator holds
/// traffic until the transparency-log store is reachable. The check is behind
/// the [`HealthCheck`] port, so the store backend is swappable without touching
/// this route.
pub fn router(health: Arc<dyn HealthCheck>) -> Router {
    Router::new()
        .route("/readyz", get(readyz))
        .with_state(health)
}

/// `200` when the backing store is reachable, `503` otherwise.
async fn readyz(State(health): State<Arc<dyn HealthCheck>>) -> Response {
    match health.check().await {
        Ok(()) => (StatusCode::OK, "ready").into_response(),
        Err(error) => {
            warn!(%error, "readiness probe failed: backing store unavailable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "not ready: backing store unavailable",
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use agate_audit::application::errors::AuditError;

    struct ReachableStore;
    #[async_trait::async_trait]
    impl HealthCheck for ReachableStore {
        async fn check(&self) -> Result<(), AuditError> {
            Ok(())
        }
    }

    struct UnreachableStore;
    #[async_trait::async_trait]
    impl HealthCheck for UnreachableStore {
        async fn check(&self) -> Result<(), AuditError> {
            Err(AuditError::Storage("connection refused".to_owned()))
        }
    }

    #[tokio::test]
    async fn ready_when_the_store_is_reachable() {
        let response = readyz(State(Arc::new(ReachableStore))).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn not_ready_when_the_store_is_unreachable() {
        let response = readyz(State(Arc::new(UnreachableStore))).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
