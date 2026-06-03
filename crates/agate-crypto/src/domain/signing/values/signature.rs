use super::{KeyId, SignAlgo};
use crate::domain::common::values::ValueObject;

/// A signature tagged with its algorithm and the signing key id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub algo: SignAlgo,
    pub key_id: KeyId,
    pub bytes: Vec<u8>,
}

impl ValueObject for Signature {}
