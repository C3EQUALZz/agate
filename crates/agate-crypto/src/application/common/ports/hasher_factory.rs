use std::sync::Arc;

use crate::domain::common::errors::CryptoError;
use crate::domain::hashing::{HashAlgo, Hasher};

/// Abstract factory resolving a self-describing [`HashAlgo`] to a concrete
/// [`Hasher`] strategy. Implemented in `infrastructure`; the available
/// algorithms depend on the enabled cargo features.
pub trait HasherFactory: Send + Sync {
    fn hasher(&self, algo: HashAlgo) -> Result<Arc<dyn Hasher>, CryptoError>;
}
