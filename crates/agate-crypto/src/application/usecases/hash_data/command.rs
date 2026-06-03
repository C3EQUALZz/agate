use crate::domain::hashing::HashAlgo;

/// Hash `data` with the named algorithm.
pub struct HashData {
    pub algo: HashAlgo,
    pub data: Vec<u8>,
}
