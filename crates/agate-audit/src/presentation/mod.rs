//! HTTP presentation: route handlers, request/response schemas, and error
//! mapping, organized by API version and aggregate. Assembly (the container,
//! route wiring, and serving) lives in [`crate::setup`].

pub mod http;
