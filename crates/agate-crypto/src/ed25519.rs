//! Ed25519 signing/verification backend (cargo feature `ed25519`).

use ed25519_dalek::{
    Signature as DalekSignature, Signer as DalekSigner, SigningKey, Verifier as DalekVerifier,
    VerifyingKey,
};

use crate::{CryptoError, KeyId, SignAlgo, Signature, Signer, Verifier};

pub struct Ed25519Signer {
    key: SigningKey,
    key_id: KeyId,
}

impl Ed25519Signer {
    /// Build from a 32-byte secret seed.
    pub fn from_seed(seed: &[u8; 32], key_id: KeyId) -> Self {
        Self {
            key: SigningKey::from_bytes(seed),
            key_id,
        }
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
}

impl Signer for Ed25519Signer {
    fn algo(&self) -> SignAlgo {
        SignAlgo::Ed25519
    }

    fn key_id(&self) -> KeyId {
        self.key_id.clone()
    }

    fn sign(&self, data: &[u8]) -> Signature {
        let sig = DalekSigner::sign(&self.key, data);
        Signature {
            algo: SignAlgo::Ed25519,
            key_id: self.key_id.clone(),
            bytes: sig.to_bytes().to_vec(),
        }
    }
}

pub struct Ed25519Verifier {
    key: VerifyingKey,
}

impl Ed25519Verifier {
    pub fn from_public_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        VerifyingKey::from_bytes(bytes)
            .map(|key| Self { key })
            .map_err(|err| CryptoError::InvalidKey(err.to_string()))
    }
}

impl Verifier for Ed25519Verifier {
    fn algo(&self) -> SignAlgo {
        SignAlgo::Ed25519
    }

    fn verify(&self, data: &[u8], sig: &Signature) -> bool {
        if sig.algo != SignAlgo::Ed25519 {
            return false;
        }
        let bytes: [u8; 64] = match sig.bytes.as_slice().try_into() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let dalek_sig = DalekSignature::from_bytes(&bytes);
        DalekVerifier::verify(&self.key, data, &dalek_sig).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trip() {
        let signer = Ed25519Signer::from_seed(&[7u8; 32], KeyId("k1".to_string()));
        let sig = signer.sign(b"hello");
        let verifier = Ed25519Verifier::from_public_bytes(&signer.verifying_key_bytes()).unwrap();
        assert!(verifier.verify(b"hello", &sig));
        assert!(!verifier.verify(b"hell0", &sig));
    }
}
