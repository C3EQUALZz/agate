use super::super::values::{Digest, HashAlgo};

/// Hashing strategy (the *strategy* pattern). Pure computation with no I/O, so
/// it sits on the domain side of the dependency rule even though concrete
/// backends live in `infrastructure`.
pub trait Hasher: Send + Sync {
    fn algo(&self) -> HashAlgo;
    fn hash(&self, data: &[u8]) -> Digest;
}
