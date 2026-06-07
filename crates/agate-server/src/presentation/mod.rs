//! HTTP presentation owned by the composition root: the operational probes the
//! proxy itself does not provide. Route handlers live here; assembly (wiring the
//! adapters and serving) lives in [`crate::setup`].

pub mod http;
