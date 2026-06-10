//! The inspection use case: feed wire fragments through the run state machine,
//! the policy, and the audit sink, producing a per-frame [`InspectionAction`].

pub mod action;
pub mod context;
pub mod inspector;
pub mod malformed;
pub mod request;

pub use action::InspectionAction;
pub use context::InspectionContext;
pub use inspector::Inspector;
pub use malformed::MalformedEventMode;
pub use request::{RequestContent, RequestDecision};
