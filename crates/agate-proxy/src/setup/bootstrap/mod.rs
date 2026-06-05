//! Bootstrap: assemble the running application from its parts.

pub mod http;

pub use http::{build_app, build_app_with};
