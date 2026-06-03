use std::sync::Arc;

use super::command::Decrypt;
use crate::application::common::ports::AeadFactory;
use crate::domain::common::errors::CryptoError;

pub struct DecryptHandler {
    factory: Arc<dyn AeadFactory>,
}

impl DecryptHandler {
    pub fn new(factory: Arc<dyn AeadFactory>) -> Self {
        Self { factory }
    }

    pub fn handle(&self, command: Decrypt) -> Result<Vec<u8>, CryptoError> {
        let Decrypt {
            key,
            nonce,
            aad,
            ciphertext,
        } = command;
        let aead = self.factory.aead(ciphertext.algo, &key)?;
        aead.decrypt(&nonce, &aad, &ciphertext)
    }
}
