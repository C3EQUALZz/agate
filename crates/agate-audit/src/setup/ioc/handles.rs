//! DI handles for the storage ports.
//!
//! `froodi` resolves *concrete* types, so it cannot hand back an `Arc<dyn Port>`
//! directly. Each backend's provider module builds its concrete adapter, wraps
//! it in the matching handle below, and registers the handle; handlers and
//! behaviors then `Inject` the handle (backend-agnostic) and read `.0`. The net
//! effect: no handler names a concrete backend, so swapping the store touches
//! only the backend provider module.

use std::sync::Arc;

use crate::application::common::ports::{
    CheckpointAnchor, LogCommandGateway, LogQueryGateway, TransactionManager,
};

/// The write-side gateway for the configured backend.
pub struct LogCommandGatewayHandle(pub Arc<dyn LogCommandGateway>);

/// The read-side gateway for the configured backend.
pub struct LogQueryGatewayHandle(pub Arc<dyn LogQueryGateway>);

/// The transaction manager for the configured backend.
pub struct TransactionManagerHandle(pub Arc<dyn TransactionManager>);

/// The checkpoint anchor for the configured backend.
pub struct CheckpointAnchorHandle(pub Arc<dyn CheckpointAnchor>);
