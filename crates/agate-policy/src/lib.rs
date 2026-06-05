//! # agate-policy
//!
//! The policy bounded context: it decides **content and authorization** verdicts
//! for the actions an agent attempts — which tools may run, and whether emitted
//! text must be redacted before it reaches the client.
//!
//! It is a **pure, self-contained context**: it speaks its own ubiquitous
//! language ([`InspectedAction`](domain::decision::InspectedAction) in,
//! [`PolicyDecision`](domain::decision::PolicyDecision) out) and depends on no
//! other context. The proxy's structural inspection and this content policy meet
//! only at the composition root, which translates between the two vocabularies —
//! there is no shared kernel.
//!
//! Layers are modules (Clean Architecture inside one crate); dependencies point
//! inward only and the domain is free of async/I/O.

pub mod application;
pub mod domain;
