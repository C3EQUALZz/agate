use std::sync::Arc;

use super::command::SignData;
use crate::application::common::ports::SignatureFactory;
use crate::domain::common::errors::CryptoError;
use crate::domain::signing::Signature;

pub struct SignDataHandler {
    factory: Arc<dyn SignatureFactory>,
}

impl SignDataHandler {
    pub fn new(factory: Arc<dyn SignatureFactory>) -> Self {
        Self { factory }
    }

    pub fn handle(&self, command: SignData) -> Result<Signature, CryptoError> {
        let SignData {
            algo,
            key,
            key_id,
            data,
        } = command;
        let signer = self.factory.signer(algo, &key, key_id)?;
        Ok(signer.sign(&data))
    }
}
