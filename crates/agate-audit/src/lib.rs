//! # agate-audit
//!
//! The audit bounded context: an append-only **transparency log** modeled as
//! an RFC 6962-style Merkle tree.
//!
//! Layers are modules (Clean Architecture inside one bounded-context crate):
//! - [`domain`] — pure entities, value objects and domain services
//!   (Merkle hashing, the `TransparencyLog` aggregate, proofs). No I/O.
//! - `application` (todo) — use cases + ports (`LeafStore`, `CheckpointAnchor`,
//!   `KeyStore`, `Clock`).
//!
//! Dependencies point inward only; the crate depends on `agate-crypto` for the
//! hashing/signing strategies.

pub mod domain;
