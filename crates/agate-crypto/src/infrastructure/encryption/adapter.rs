//! Generic bridge from any RustCrypto `aead::Aead` cipher to our [`Aead`]
//! strategy. The concrete cipher type is erased behind `Arc<dyn Aead>`, so all
//! backends share one nonce-validation and error-mapping path.

use aead::{Aead as RcAead, Nonce as RcNonce, Payload};

use crate::domain::common::errors::CryptoError;
use crate::domain::encryption::{Aead, AeadAlgo, AssociatedData, Ciphertext, Nonce};

pub(super) struct RustCryptoAead<C> {
    cipher: C,
    algo: AeadAlgo,
}

impl<C> RustCryptoAead<C> {
    pub(super) fn new(cipher: C, algo: AeadAlgo) -> Self {
        Self { cipher, algo }
    }
}

impl<C> Aead for RustCryptoAead<C>
where
    C: RcAead + Send + Sync,
{
    fn algo(&self) -> AeadAlgo {
        self.algo
    }

    fn encrypt(
        &self,
        nonce: &Nonce,
        aad: &AssociatedData,
        plaintext: &[u8],
    ) -> Result<Ciphertext, CryptoError> {
        let nonce = checked_nonce::<C>(nonce, self.algo)?;
        let payload = Payload {
            msg: plaintext,
            aad: aad.as_bytes(),
        };
        let bytes = self
            .cipher
            .encrypt(nonce, payload)
            .map_err(|_| CryptoError::Encryption)?;
        Ok(Ciphertext::new(self.algo, bytes))
    }

    fn decrypt(
        &self,
        nonce: &Nonce,
        aad: &AssociatedData,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, CryptoError> {
        let nonce = checked_nonce::<C>(nonce, self.algo)?;
        let payload = Payload {
            msg: &ciphertext.bytes,
            aad: aad.as_bytes(),
        };
        self.cipher
            .decrypt(nonce, payload)
            .map_err(|_| CryptoError::Decryption)
    }
}

/// Validate the nonce against the algorithm before handing it to the cipher
/// (whose `from_slice` would otherwise panic on a length mismatch).
fn checked_nonce<C: RcAead>(nonce: &Nonce, algo: AeadAlgo) -> Result<&RcNonce<C>, CryptoError> {
    if nonce.len() != algo.nonce_len() {
        return Err(CryptoError::InvalidNonce(format!(
            "{algo:?} expects a {}-byte nonce, got {}",
            algo.nonce_len(),
            nonce.len()
        )));
    }
    // MGM reserves the most significant nonce bit; a set bit is rejected by the
    // standard, so guard here rather than risk a backend panic.
    if matches!(algo, AeadAlgo::KuznyechikMgm | AeadAlgo::MagmaMgm)
        && nonce.as_bytes().first().is_some_and(|b| b & 0x80 != 0)
    {
        return Err(CryptoError::InvalidNonce(
            "MGM nonce must have its most significant bit cleared".to_string(),
        ));
    }
    Ok(RcNonce::<C>::from_slice(nonce.as_bytes()))
}
