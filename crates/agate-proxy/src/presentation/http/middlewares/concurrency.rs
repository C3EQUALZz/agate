//! Concurrency-limit middleware: cap in-flight requests, shed the excess.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tokio::sync::Semaphore;
use tracing::warn;

/// Cap the number of concurrently in-flight requests on `router`. A shared
/// semaphore admits up to `max_concurrent` at once; further requests are shed
/// immediately with `503` (non-blocking `try_acquire`), so a flood cannot queue
/// without bound or exhaust memory/upstream connections.
pub fn apply(router: Router, max_concurrent: usize) -> Router {
    let permits = Arc::new(Semaphore::new(max_concurrent));
    router.layer(middleware::from_fn_with_state(permits, limit))
}

/// Hold a permit for the whole request (released when the response — including
/// the streamed body — completes); shed with `503` when none is available.
async fn limit(State(permits): State<Arc<Semaphore>>, request: Request, next: Next) -> Response {
    // The permit is held for the whole request (released on drop when the
    // response — including the streamed body — completes).
    if let Ok(_permit) = Arc::clone(&permits).try_acquire_owned() {
        next.run(request).await
    } else {
        warn!("shedding a request: concurrency limit reached");
        (StatusCode::SERVICE_UNAVAILABLE, "service at capacity").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::apply;

    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tokio::sync::Notify;
    use tower::ServiceExt;

    fn get_root() -> Request<Body> {
        Request::builder()
            .uri("/")
            .body(Body::empty())
            .expect("a valid request")
    }

    #[tokio::test]
    async fn over_capacity_requests_are_shed_with_503() {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let (handler_entered, handler_release) = (entered.clone(), release.clone());

        // A handler that parks once it holds the one permit, so the slot stays
        // occupied until the test releases it.
        let app = apply(
            Router::new().route(
                "/",
                get(move || {
                    let (entered, release) = (handler_entered.clone(), handler_release.clone());
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        "ok"
                    }
                }),
            ),
            1,
        );

        // Occupy the slot and wait until the handler is actually running.
        let busy = app.clone();
        let first = tokio::spawn(async move { busy.oneshot(get_root()).await });
        entered.notified().await;

        // With the slot taken, the next request is shed (try_acquire fails) — it
        // returns immediately rather than queuing.
        let response = app.clone().oneshot(get_root()).await.expect("a response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Release the first and confirm it succeeded (permit was held, not lost).
        release.notify_one();
        let first = first
            .await
            .expect("the first task joins")
            .expect("a response");
        assert_eq!(first.status(), StatusCode::OK);
    }
}
