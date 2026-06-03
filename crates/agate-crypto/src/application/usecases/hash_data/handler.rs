use std::sync::Arc;

use super::command::HashData;
use crate::application::common::ports::HasherFactory;
use crate::domain::common::errors::CryptoError;
use crate::domain::hashing::Digest;

pub struct HashDataHandler {
    factory: Arc<dyn HasherFactory>,
}

impl HashDataHandler {
    pub fn new(factory: Arc<dyn HasherFactory>) -> Self {
        Self { factory }
    }

    pub fn handle(&self, command: HashData) -> Result<Digest, CryptoError> {
        let HashData { algo, data } = command;
        let hasher = self.factory.hasher(algo)?;
        Ok(hasher.hash(&data))
    }
}
