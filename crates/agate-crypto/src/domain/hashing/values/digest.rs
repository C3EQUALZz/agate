use std::fmt::Write as _;

use super::HashAlgo;
use crate::domain::common::values::ValueObject;

/// A hash value tagged with the algorithm that produced it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Digest {
    pub algo: HashAlgo,
    pub bytes: Vec<u8>,
}

impl Digest {
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(self.bytes.len() * 2);
        for b in &self.bytes {
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

impl ValueObject for Digest {}

#[cfg(test)]
mod tests {
    use super::{Digest, HashAlgo};

    #[test]
    fn to_hex_is_lowercase_and_zero_padded() {
        let digest = Digest {
            algo: HashAlgo::Sha256,
            bytes: vec![0x00, 0x0f, 0xa0, 0xff],
        };
        assert_eq!(digest.to_hex(), "000fa0ff");
    }

    #[test]
    fn empty_digest_renders_empty_hex() {
        let digest = Digest {
            algo: HashAlgo::Sha256,
            bytes: Vec::new(),
        };
        assert_eq!(digest.to_hex(), "");
    }
}
