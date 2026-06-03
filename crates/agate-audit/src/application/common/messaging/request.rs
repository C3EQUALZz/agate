/// A message processed by exactly one handler, producing `Response`.
///
/// `Response` may itself be a `Result<_, _>`; the messaging layer is
/// error-agnostic and treats the response as opaque.
pub trait Request: Send + 'static {
    type Response: Send + 'static;
}

/// Marker for a request that changes state.
pub trait Command: Request {}

/// Marker for a read-only request.
pub trait Query: Request {}
