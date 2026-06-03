use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use super::request::Request;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The remainder of the pipeline: the next behavior, or finally the handler.
pub struct Next<R: Request> {
    inner: Box<dyn FnOnce(R) -> BoxFuture<R::Response> + Send>,
}

impl<R: Request> Next<R> {
    pub(crate) fn new(inner: Box<dyn FnOnce(R) -> BoxFuture<R::Response> + Send>) -> Self {
        Self { inner }
    }

    /// Continue the pipeline with (possibly modified) `request`.
    pub fn call(self, request: R) -> BoxFuture<R::Response> {
        (self.inner)(request)
    }
}

/// A pipeline behavior wrapping handler execution (chain of responsibility):
/// tracing, metrics, validation, unit-of-work, event publishing, ...
///
/// Call `next.call(request)` to continue; skip it to short-circuit.
#[async_trait]
pub trait Behavior<R: Request>: Send + Sync {
    async fn handle(&self, request: R, next: Next<R>) -> R::Response;
}
