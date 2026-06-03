use std::sync::Arc;

use crate::domain::common::errors::CryptoError;
use crate::domain::common::values::SecretKey;
use crate::domain::encryption::{Aead, AeadAlgo};

/// Abstract factory binding a [`SecretKey`] to an [`AeadAlgo`], yielding a
/// ready-to-use [`Aead`] strategy. Returns [`CryptoError::InvalidKey`] when the
/// key length does not match the algorithm.
pub trait AeadFactory: Send + Sync {
    fn aead(&self, algo: AeadAlgo, key: &SecretKey) -> Result<Arc<dyn Aead>, CryptoError>;
}
