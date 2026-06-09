//! Backend-agnostic App-scope singletons: clock, id generation, the Merkle
//! hasher/factory, and the signing key store. Shared by every storage backend,
//! so they live apart from the backend-specific provider modules.

use std::sync::Arc;

use froodi::{DefaultScope::App, async_impl::RegistryWithSync, async_registry, registry};

use agate_crypto::{CryptoRegistry, HashAlgo, Hasher};

use crate::domain::merkle::{MerkleHasher, TransparencyLogFactory};
use crate::infrastructure::{Ed25519KeyStore, SystemClock, UuidLogIdGenerator};

/// The store-independent singletons every backend's pipeline relies on.
pub(crate) fn agnostic_providers() -> RegistryWithSync {
    async_registry! {
        extend(registry! {
            scope(App) [
                provide(|| Ok(UuidLogIdGenerator)),
                provide(|| Ok(SystemClock)),
                provide(|| Ok(TransparencyLogFactory::new(default_hasher()))),
                provide(|| Ok(MerkleHasher::new(default_hasher()))),
                provide(|| Ok(Ed25519KeyStore::from_env())),
            ]
        }),
    }
}

fn default_hasher() -> Arc<dyn Hasher> {
    CryptoRegistry::hasher(HashAlgo::Sha256).expect("SHA-256 is always available")
}
