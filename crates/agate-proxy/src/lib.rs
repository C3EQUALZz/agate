//! # agate-proxy
//!
//! The proxy bounded context: an inline reverse proxy that inspects LLM-agent
//! traffic and decides — per event — whether to allow, deny, transform, buffer,
//! or terminate it.
//!
//! The inspection core is **protocol-agnostic**: the wire protocol (AG-UI
//! first, an agent↔LLM adapter later) enters through an adapter that translates
//! wire events into domain events. See `docs/design/agate-proxy-threat-model.md`.
//!
//! Layers are modules (Clean Architecture inside one bounded-context crate);
//! dependencies point inward only. Only [`domain`] exists so far.

pub mod domain;
