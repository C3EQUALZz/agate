use crate::domain::common::values::ValueObject;

/// Self-describing AEAD (authenticated encryption with associated data)
/// algorithm identifier. Key/nonce/tag sizes are algorithm-intrinsic and
/// exposed here so a factory can validate inputs before constructing a cipher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AeadAlgo {
    Aes256Gcm,
    ChaCha20Poly1305,
    /// Kuznyechik (GOST R 34.12-2015, 128-bit block) in MGM mode.
    KuznyechikMgm,
    /// Magma (GOST R 34.12-2015, 64-bit block) in MGM mode.
    MagmaMgm,
}

impl AeadAlgo {
    /// Stable byte code for self-describing serialization.
    pub fn code(self) -> u8 {
        match self {
            AeadAlgo::Aes256Gcm => 1,
            AeadAlgo::ChaCha20Poly1305 => 2,
            AeadAlgo::KuznyechikMgm => 3,
            AeadAlgo::MagmaMgm => 4,
        }
    }

    /// Inverse of [`code`](Self::code): resolve a stored byte code.
    pub fn from_code(code: u8) -> Option<AeadAlgo> {
        match code {
            1 => Some(AeadAlgo::Aes256Gcm),
            2 => Some(AeadAlgo::ChaCha20Poly1305),
            3 => Some(AeadAlgo::KuznyechikMgm),
            4 => Some(AeadAlgo::MagmaMgm),
            _ => None,
        }
    }

    /// Required secret-key length in bytes.
    pub fn key_len(self) -> usize {
        match self {
            // All four are 256-bit-keyed constructions.
            AeadAlgo::Aes256Gcm
            | AeadAlgo::ChaCha20Poly1305
            | AeadAlgo::KuznyechikMgm
            | AeadAlgo::MagmaMgm => 32,
        }
    }

    /// Required nonce length in bytes (one block for the MGM constructions).
    pub fn nonce_len(self) -> usize {
        match self {
            AeadAlgo::Aes256Gcm | AeadAlgo::ChaCha20Poly1305 => 12,
            AeadAlgo::KuznyechikMgm => 16,
            AeadAlgo::MagmaMgm => 8,
        }
    }

    /// Authentication tag length in bytes appended to the ciphertext.
    pub fn tag_len(self) -> usize {
        match self {
            AeadAlgo::Aes256Gcm | AeadAlgo::ChaCha20Poly1305 | AeadAlgo::KuznyechikMgm => 16,
            AeadAlgo::MagmaMgm => 8,
        }
    }
}

impl ValueObject for AeadAlgo {}

#[cfg(test)]
mod tests {
    use super::AeadAlgo;

    const ALL: [AeadAlgo; 4] = [
        AeadAlgo::Aes256Gcm,
        AeadAlgo::ChaCha20Poly1305,
        AeadAlgo::KuznyechikMgm,
        AeadAlgo::MagmaMgm,
    ];

    #[test]
    fn code_round_trips_for_every_variant() {
        for algo in ALL {
            assert_eq!(AeadAlgo::from_code(algo.code()), Some(algo));
        }
    }

    #[test]
    fn unknown_code_resolves_to_none() {
        assert_eq!(AeadAlgo::from_code(0), None);
        assert_eq!(AeadAlgo::from_code(99), None);
    }

    #[test]
    fn sizes_match_the_constructions() {
        for algo in ALL {
            assert_eq!(algo.key_len(), 32, "{algo:?} is a 256-bit construction");
        }
        assert_eq!(AeadAlgo::Aes256Gcm.nonce_len(), 12);
        assert_eq!(AeadAlgo::Aes256Gcm.tag_len(), 16);
        assert_eq!(AeadAlgo::ChaCha20Poly1305.nonce_len(), 12);
        // MGM nonce and tag equal the block size: Kuznyechik 128-bit, Magma 64-bit.
        assert_eq!(AeadAlgo::KuznyechikMgm.nonce_len(), 16);
        assert_eq!(AeadAlgo::KuznyechikMgm.tag_len(), 16);
        assert_eq!(AeadAlgo::MagmaMgm.nonce_len(), 8);
        assert_eq!(AeadAlgo::MagmaMgm.tag_len(), 8);
    }
}
