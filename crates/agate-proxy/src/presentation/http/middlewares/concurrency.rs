//! Concurrency-limit middleware: cap in-flight requests, shed the excess.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::warn;

/// Cap the number of concurrently in-flight requests on `router`. A shared
/// semaphore admits up to `max_concurrent` at once; further requests are shed
/// immediately with `503` (non-blocking `try_acquire`), so a flood cannot queue
/// without bound or exhaust memory/upstream connections.
pub fn apply(router: Router, max_concurrent: usize) -> Router {
    let permits = Arc::new(Semaphore::new(max_concurrent));
    router.layer(middleware::from_fn_with_state(permits, limit))
}

/// Admit the request if a permit is free, else shed it with `503`. The permit is
/// tied to the response **body**, not just the handler, so it is held for the
/// whole (possibly streaming) response and released only when the stream ends or
/// the client disconnects — bounding concurrent active streams, not just request
/// setup (a streaming handler returns its `Response` long before the body ends).
async fn limit(State(permits): State<Arc<Semaphore>>, request: Request, next: Next) -> Response {
    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
        warn!("shedding a request: concurrency limit reached");
        return (StatusCode::SERVICE_UNAVAILABLE, "service at capacity").into_response();
    };

    let (parts, body) = next.run(request).await.into_parts();
    Response::from_parts(parts, hold_permit(body, permit))
}

/// Re-stream `body`, keeping `permit` alive until the stream is exhausted or the
/// body is dropped (the generator owns the permit and drops it with itself).
fn hold_permit(body: Body, permit: OwnedSemaphorePermit) -> Body {
    let mut data = body.into_data_stream();
    Body::from_stream(async_stream::stream! {
        let _permit = permit;
        while let Some(chunk) = data.next().await {
            yield chunk;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::apply;

    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use futures::stream;
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
        // occupied (while `next.run` is pending) until the test releases it.
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

        let busy = app.clone();
        let first = tokio::spawn(async move { busy.oneshot(get_root()).await });
        entered.notified().await;

        let response = app.clone().oneshot(get_root()).await.expect("a response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        release.notify_one();
        let first = first
            .await
            .expect("the first task joins")
            .expect("a response");
        assert_eq!(first.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn permit_is_held_until_the_response_body_is_dropped() {
        // The handler returns immediately with a streaming body. The permit must
        // stay held while that body is alive (the stream is still in flight),
        // not be released the moment the handler returned.
        let app = apply(
            Router::new().route(
                "/",
                get(|| async {
                    Body::from_stream(stream::once(async { Ok::<_, std::io::Error>("data") }))
                }),
            ),
            1,
        );

        // First response holds the permit inside its (undrained) body.
        let first = app.clone().oneshot(get_root()).await.expect("a response");
        assert_eq!(first.status(), StatusCode::OK);

        // While that body is alive, a second request is shed.
        let second = app.clone().oneshot(get_root()).await.expect("a response");
        assert_eq!(second.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Dropping the first response releases the permit, so the next succeeds.
        drop(first);
        let third = app.clone().oneshot(get_root()).await.expect("a response");
        assert_eq!(third.status(), StatusCode::OK);
    }
}
