use crate::domain::common::values::SecretKey;
use crate::domain::signing::{KeyId, SignAlgo};

/// Sign `data` with `key` under the named algorithm, tagging the result with
/// `key_id`.
pub struct SignData {
    pub algo: SignAlgo,
    pub key: SecretKey,
    pub key_id: KeyId,
    pub data: Vec<u8>,
}
