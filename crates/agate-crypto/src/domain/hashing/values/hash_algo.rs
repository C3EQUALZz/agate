use crate::domain::common::values::ValueObject;

/// Self-describing hash algorithm identifier (think JWS `alg` / multicodec).
///
/// The identifier is persisted alongside every [`Digest`](super::Digest) so
/// records stay verifiable even after the configured default algorithm changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HashAlgo {
    Sha256,
    Sha512,
    Sha3_256,
    /// GOST R 34.11-2012, 256-bit (Streebog).
    Streebog256,
    /// GOST R 34.11-2012, 512-bit (Streebog).
    Streebog512,
}

impl HashAlgo {
    /// Stable byte code for self-describing serialization / canonical forms.
    pub fn code(self) -> u8 {
        match self {
            HashAlgo::Sha256 => 1,
            HashAlgo::Sha512 => 2,
            HashAlgo::Sha3_256 => 3,
            HashAlgo::Streebog256 => 4,
            HashAlgo::Streebog512 => 5,
        }
    }

    /// Inverse of [`code`](Self::code): resolve a stored byte code.
    pub fn from_code(code: u8) -> Option<HashAlgo> {
        match code {
            1 => Some(HashAlgo::Sha256),
            2 => Some(HashAlgo::Sha512),
            3 => Some(HashAlgo::Sha3_256),
            4 => Some(HashAlgo::Streebog256),
            5 => Some(HashAlgo::Streebog512),
            _ => None,
        }
    }
}

impl ValueObject for HashAlgo {}

#[cfg(test)]
mod tests {
    use super::HashAlgo;

    const ALL: [HashAlgo; 5] = [
        HashAlgo::Sha256,
        HashAlgo::Sha512,
        HashAlgo::Sha3_256,
        HashAlgo::Streebog256,
        HashAlgo::Streebog512,
    ];

    #[test]
    fn code_round_trips_for_every_variant() {
        for algo in ALL {
            assert_eq!(HashAlgo::from_code(algo.code()), Some(algo));
        }
    }

    #[test]
    fn codes_are_unique() {
        let mut codes: Vec<u8> = ALL.iter().map(|a| a.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), ALL.len());
    }

    #[test]
    fn unknown_code_resolves_to_none() {
        assert_eq!(HashAlgo::from_code(0), None);
        assert_eq!(HashAlgo::from_code(250), None);
    }
}
