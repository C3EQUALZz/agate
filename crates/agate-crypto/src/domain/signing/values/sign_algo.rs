use crate::domain::common::values::ValueObject;

/// Self-describing signature algorithm identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignAlgo {
    Ed25519,
    /// GOST R 34.10-2012, 256-bit (not yet implemented).
    GostR3410_2012_256,
    /// GOST R 34.10-2012, 512-bit (not yet implemented).
    GostR3410_2012_512,
}

impl SignAlgo {
    /// Stable byte code for self-describing serialization.
    pub fn code(self) -> u8 {
        match self {
            SignAlgo::Ed25519 => 1,
            SignAlgo::GostR3410_2012_256 => 2,
            SignAlgo::GostR3410_2012_512 => 3,
        }
    }

    /// Inverse of [`code`](Self::code): resolve a stored byte code.
    pub fn from_code(code: u8) -> Option<SignAlgo> {
        match code {
            1 => Some(SignAlgo::Ed25519),
            2 => Some(SignAlgo::GostR3410_2012_256),
            3 => Some(SignAlgo::GostR3410_2012_512),
            _ => None,
        }
    }
}

impl ValueObject for SignAlgo {}

#[cfg(test)]
mod tests {
    use super::SignAlgo;

    const ALL: [SignAlgo; 3] = [
        SignAlgo::Ed25519,
        SignAlgo::GostR3410_2012_256,
        SignAlgo::GostR3410_2012_512,
    ];

    #[test]
    fn code_round_trips_for_every_variant() {
        for algo in ALL {
            assert_eq!(SignAlgo::from_code(algo.code()), Some(algo));
        }
    }

    #[test]
    fn unknown_code_resolves_to_none() {
        assert_eq!(SignAlgo::from_code(0), None);
        assert_eq!(SignAlgo::from_code(99), None);
    }
}
