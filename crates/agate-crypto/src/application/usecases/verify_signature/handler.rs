use std::sync::Arc;

use super::query::VerifySignature;
use crate::application::common::ports::SignatureFactory;
use crate::domain::common::errors::CryptoError;

pub struct VerifySignatureHandler {
    factory: Arc<dyn SignatureFactory>,
}

impl VerifySignatureHandler {
    pub fn new(factory: Arc<dyn SignatureFactory>) -> Self {
        Self { factory }
    }

    pub fn handle(&self, query: VerifySignature) -> Result<bool, CryptoError> {
        let VerifySignature {
            public_key,
            data,
            signature,
        } = query;
        let verifier = self.factory.verifier(signature.algo, &public_key)?;
        Ok(verifier.verify(&data, &signature))
    }
}
