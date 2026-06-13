//! Per-client-IP request-rate middleware: cap how fast one source IP may open
//! runs, rejecting the excess with `429 Too Many Requests`.

use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::{Arc, Weak};
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::StatusCode;
use axum::http::header::RETRY_AFTER;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use governor::clock::{Clock, DefaultClock};
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use tracing::debug;

/// How often the background task prunes rate-limiter entries for IPs that have
/// gone quiet, bounding the keyed map so a spray of distinct source IPs cannot
/// grow it without limit.
const PRUNE_INTERVAL: Duration = Duration::from_mins(1);

/// Cap the per-client-IP request rate on `router`. `per_second` is the
/// sustained rate and `burst` the bucket depth (the largest instantaneous
/// burst). `per_second == 0` disables the limit and leaves the router
/// unchanged; a `burst` of 0 falls back to `per_second`.
///
/// The client IP is the connection's peer address, so the served app must carry
/// [`ConnectInfo<SocketAddr>`] (see the composition root). Behind a load
/// balancer that is the balancer's address — front Agate with a limiter that
/// sees the real client, or terminate closer to it.
pub fn apply(router: Router, per_second: u32, burst: u32) -> Router {
    let Some(rate) = NonZeroU32::new(per_second) else {
        return router;
    };
    let burst = NonZeroU32::new(burst).unwrap_or(rate);
    let limiter: Arc<DefaultKeyedRateLimiter<IpAddr>> = Arc::new(RateLimiter::keyed(
        Quota::per_second(rate).allow_burst(burst),
    ));
    spawn_pruner(&limiter);
    router.layer(middleware::from_fn_with_state(limiter, limit))
}

/// Reject the request with `429` if its source IP is over budget, attaching a
/// `Retry-After` hint; otherwise pass it down the stack.
async fn limit(
    State(limiter): State<Arc<DefaultKeyedRateLimiter<IpAddr>>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let ip = addr.ip();
    match limiter.check_key(&ip) {
        Ok(()) => next.run(request).await,
        Err(not_until) => {
            let retry_after = not_until
                .wait_time_from(DefaultClock::default().now())
                .as_secs()
                .max(1);
            // debug, not warn: a flood would otherwise turn throttling into a
            // secondary DoS through log volume. Rate is a metrics concern.
            debug!(%ip, retry_after, "rate limit exceeded; rejecting with 429");
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(RETRY_AFTER, retry_after.to_string())],
                "rate limit exceeded",
            )
                .into_response()
        }
    }
}

/// Periodically drop limiter state for IPs that have replenished, so the keyed
/// map tracks only currently-active sources. Holds a [`Weak`] so the task ends
/// once the router (and its limiter) is dropped.
fn spawn_pruner(limiter: &Arc<DefaultKeyedRateLimiter<IpAddr>>) {
    let weak = Arc::downgrade(limiter);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(PRUNE_INTERVAL);
        loop {
            tick.tick().await;
            let Some(limiter) = Weak::upgrade(&weak) else {
                return;
            };
            limiter.retain_recent();
            limiter.shrink_to_fit();
        }
    });
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use axum::Router;
    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    use super::apply;

    fn app(per_second: u32, burst: u32) -> Router {
        apply(
            Router::new().route("/", get(|| async { "ok" })),
            per_second,
            burst,
        )
    }

    /// Build a request carrying a peer address, as the connect-info service
    /// would; without it the `ConnectInfo` extractor has nothing to read.
    fn request_from(ip: Ipv4Addr) -> Request<Body> {
        let mut request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .expect("a valid request");
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from((ip, 4000))));
        request
    }

    #[tokio::test]
    async fn allows_requests_within_the_burst_then_sheds_with_429() {
        // burst 2 → two requests pass immediately, the third is over budget.
        let app = app(1, 2);
        let ip = Ipv4Addr::new(10, 0, 0, 1);

        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(request_from(ip))
                .await
                .expect("a response");
            assert_eq!(response.status(), StatusCode::OK);
        }

        let limited = app
            .clone()
            .oneshot(request_from(ip))
            .await
            .expect("a response");
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(
            limited
                .headers()
                .contains_key(axum::http::header::RETRY_AFTER),
            "a Retry-After hint is attached"
        );
    }

    #[tokio::test]
    async fn the_budget_is_per_ip() {
        // One IP exhausts its single-request burst; a different IP is unaffected.
        let app = app(1, 1);
        let busy = Ipv4Addr::new(10, 0, 0, 1);
        let other = Ipv4Addr::new(10, 0, 0, 2);

        let first = app
            .clone()
            .oneshot(request_from(busy))
            .await
            .expect("a response");
        assert_eq!(first.status(), StatusCode::OK);

        let shed = app
            .clone()
            .oneshot(request_from(busy))
            .await
            .expect("a response");
        assert_eq!(shed.status(), StatusCode::TOO_MANY_REQUESTS);

        let fresh = app
            .clone()
            .oneshot(request_from(other))
            .await
            .expect("a response");
        assert_eq!(fresh.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn zero_rate_disables_the_limit() {
        let app = app(0, 0);
        let ip = Ipv4Addr::new(10, 0, 0, 1);

        // Far more requests than any burst would allow; none are shed.
        for _ in 0..10 {
            let response = app
                .clone()
                .oneshot(request_from(ip))
                .await
                .expect("a response");
            assert_eq!(response.status(), StatusCode::OK);
        }
    }
}
