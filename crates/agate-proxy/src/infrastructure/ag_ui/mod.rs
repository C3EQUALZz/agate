//! The AG-UI protocol adapter: translates AG-UI wire events to and from the
//! protocol-agnostic domain [`Fragment`](crate::domain::inspection::Fragment) /
//! [`AgentEvent`](crate::domain::inspection::AgentEvent). The first transport
//! adapter; an agent↔LLM adapter can be added beside it without touching the
//! inspection core.

pub mod error;
pub mod event_type;
pub mod mapper;
pub mod request;

pub use error::AgUiError;
pub use mapper::{to_event, to_fragment};
pub use request::parse_request;
