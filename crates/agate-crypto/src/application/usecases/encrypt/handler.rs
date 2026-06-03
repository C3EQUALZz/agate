use std::sync::Arc;

use super::command::Encrypt;
use crate::application::common::ports::AeadFactory;
use crate::domain::common::errors::CryptoError;
use crate::domain::encryption::Ciphertext;

pub struct EncryptHandler {
    factory: Arc<dyn AeadFactory>,
}

impl EncryptHandler {
    pub fn new(factory: Arc<dyn AeadFactory>) -> Self {
        Self { factory }
    }

    pub fn handle(&self, command: Encrypt) -> Result<Ciphertext, CryptoError> {
        let Encrypt {
            algo,
            key,
            nonce,
            aad,
            plaintext,
        } = command;
        let aead = self.factory.aead(algo, &key)?;
        aead.encrypt(&nonce, &aad, &plaintext)
    }
}
