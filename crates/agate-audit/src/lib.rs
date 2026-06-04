//! # agate-audit
//!
//! The audit bounded context: an append-only **transparency log** modeled as
//! an RFC 6962-style Merkle tree.
//!
//! Layers are modules (Clean Architecture inside one bounded-context crate):
//! - [`domain`] — pure entities, value objects and domain services
//!   (Merkle hashing, the `TransparencyLog` aggregate, proofs). No I/O.
//! - [`application`] — CQRS use cases (command/query handlers) over a mediator
//!   pipeline, plus outbound ports implemented by infrastructure.
//! - [`infrastructure`] — concrete adapters implementing the ports.
//!
//! Dependencies point inward only; the crate depends on `agate-crypto` for the
//! hashing/signing strategies.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
