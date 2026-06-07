//! # agate-server
//!
//! The composition root for the whole system: it wires the **proxy** data plane
//! ([`agate_proxy`]) to the **audit** transparency log ([`agate_audit`]) so that
//! every event the proxy inspects — and the verdict it reaches — is recorded.
//!
//! The proxy depends only on its `AuditSink` *port*; this crate supplies the
//! adapter ([`infrastructure::audit`]) that turns each inspected event into an
//! append on the audit log, off the forwarding hot path via a background outbox.
//!
//! This crate owns no domain of its own — it is the outermost layer that
//! composes two bounded contexts behind their public ports.

pub mod infrastructure;
pub mod presentation;
pub mod setup;
