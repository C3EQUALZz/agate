use crate::domain::hashing::{Digest, HashAlgo, Hasher};

/// Factory for a fresh boxed digest state. A non-capturing closure, so it
/// coerces to a plain function pointer.
pub(crate) type MakeDigest = fn() -> Box<dyn digest::DynDigest>;

/// Adapts any RustCrypto `DynDigest` into our [`Hasher`] strategy.
pub(crate) struct DigestHasher {
    algo: HashAlgo,
    make: MakeDigest,
}

impl DigestHasher {
    pub(crate) fn new(algo: HashAlgo, make: MakeDigest) -> Self {
        Self { algo, make }
    }
}

impl Hasher for DigestHasher {
    fn algo(&self) -> HashAlgo {
        self.algo
    }

    fn hash(&self, data: &[u8]) -> Digest {
        let mut state = (self.make)();
        state.update(data);
        Digest {
            algo: self.algo,
            bytes: state.finalize().to_vec(),
        }
    }
}
