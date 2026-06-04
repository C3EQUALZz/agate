//! Application layer of the proxy context: the inspection use case over the
//! outbound ports (policy, audit). No transport here — that is presentation.

pub mod common;
pub mod inspection;
