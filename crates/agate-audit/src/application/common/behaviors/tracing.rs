//! Pipeline behavior that wraps every dispatched request in a tracing span.

use async_trait::async_trait;
use tracing::Instrument;

use crate::application::common::messaging::{Behavior, Next, Request};

/// Pipeline behavior that opens an `audit.request` span around the rest of the
/// chain, so each dispatched command/query — and every span its handler and
/// gateways open inside it — nests under one span per request.
///
/// Registered **outermost** on every request type at the composition root, so
/// the span encloses the metrics and transaction behaviors too. Unlike those,
/// it is request-agnostic: a single blanket `impl` covers any [`Request`], so
/// new use cases get tracing for free.
pub struct TracingBehavior;

#[async_trait]
impl<R: Request> Behavior<R> for TracingBehavior {
    async fn handle(&self, request: R, next: Next<R>) -> R::Response {
        let span = tracing::info_span!("audit.request", request = short_type_name::<R>());
        next.call(request).instrument(span).await
    }
}

/// The last `::`-separated segment of `R`'s type name (e.g. `AppendRecord`),
/// so spans read as the use case rather than its full module path.
fn short_type_name<R>() -> &'static str {
    let full = std::any::type_name::<R>();
    match full.rsplit_once("::") {
        Some((_, last)) => last,
        None => full,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::application::common::messaging::{Mediator, RequestHandler};

    struct Ping;
    impl Request for Ping {
        type Response = u32;
    }

    struct PingHandler;
    #[async_trait]
    impl RequestHandler<Ping> for PingHandler {
        async fn handle(&self, _request: Ping) -> u32 {
            7
        }
    }

    #[tokio::test]
    async fn passes_the_request_through_unchanged() {
        let behaviors: Vec<Arc<dyn Behavior<Ping>>> = vec![Arc::new(TracingBehavior)];
        let mediator = Mediator::new(Arc::new(PingHandler), behaviors);

        assert_eq!(mediator.send(Ping).await, 7);
    }

    #[test]
    fn short_type_name_keeps_the_last_segment() {
        assert_eq!(short_type_name::<Ping>(), "Ping");
    }
}
