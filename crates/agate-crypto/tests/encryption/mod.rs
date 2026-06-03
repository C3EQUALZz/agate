#![allow(dead_code)]

use std::sync::Arc;

use agate_crypto::application::usecases::decrypt::{Decrypt, DecryptHandler};
use agate_crypto::application::usecases::encrypt::{Encrypt, EncryptHandler};
use agate_crypto::{
    AeadAlgo, AeadFactory, AssociatedData, Ciphertext, CryptoError, Nonce, RustCryptoAeadFactory,
};

use crate::common::secret_key;

fn handlers() -> (EncryptHandler, DecryptHandler) {
    let factory = Arc::new(RustCryptoAeadFactory);
    (
        EncryptHandler::new(factory.clone()),
        DecryptHandler::new(factory),
    )
}

fn encrypt(algo: AeadAlgo, nonce: &Nonce, aad: &AssociatedData, plaintext: &[u8]) -> Ciphertext {
    handlers()
        .0
        .handle(Encrypt {
            algo,
            key: secret_key(algo.key_len()),
            nonce: nonce.clone(),
            aad: aad.clone(),
            plaintext: plaintext.to_vec(),
        })
        .unwrap()
}

/// Drive every general AEAD invariant for one algorithm: round-trip,
/// self-description, ciphertext length (= plaintext + tag), nonce sensitivity,
/// and rejection of a wrong key or nonce at decryption.
fn exercise_aead(algo: AeadAlgo) {
    let (enc, dec) = handlers();
    let nonce = Nonce::new(vec![0u8; algo.nonce_len()]);
    let other_nonce = Nonce::new({
        let mut n = vec![0u8; algo.nonce_len()];
        n[algo.nonce_len() - 1] = 0x01;
        n
    });
    let aad = AssociatedData::new(b"agate".to_vec());
    let plaintext = b"attack at dawn".to_vec();

    let ciphertext = enc
        .handle(Encrypt {
            algo,
            key: secret_key(algo.key_len()),
            nonce: nonce.clone(),
            aad: aad.clone(),
            plaintext: plaintext.clone(),
        })
        .unwrap();

    assert_eq!(ciphertext.algo, algo, "ciphertext is self-describing");
    assert_ne!(ciphertext.bytes, plaintext);
    assert_eq!(
        ciphertext.bytes.len(),
        plaintext.len() + algo.tag_len(),
        "AEAD output is plaintext length plus the authentication tag"
    );

    let recovered = dec
        .handle(Decrypt {
            key: secret_key(algo.key_len()),
            nonce: nonce.clone(),
            aad: aad.clone(),
            ciphertext: ciphertext.clone(),
        })
        .unwrap();
    assert_eq!(recovered, plaintext, "round-trips back to the plaintext");

    let other = encrypt(algo, &other_nonce, &aad, &plaintext);
    assert_ne!(
        other.bytes, ciphertext.bytes,
        "a different nonce yields a different ciphertext"
    );

    let empty = encrypt(algo, &nonce, &aad, &[]);
    let recovered_empty = dec
        .handle(Decrypt {
            key: secret_key(algo.key_len()),
            nonce: nonce.clone(),
            aad: aad.clone(),
            ciphertext: empty,
        })
        .unwrap();
    assert!(recovered_empty.is_empty(), "empty plaintext round-trips");

    let wrong_key = dec.handle(Decrypt {
        key: agate_crypto::SecretKey::new(vec![0xaa; algo.key_len()]),
        nonce: nonce.clone(),
        aad: aad.clone(),
        ciphertext: ciphertext.clone(),
    });
    assert_eq!(wrong_key.unwrap_err(), CryptoError::Decryption);

    let wrong_nonce = dec.handle(Decrypt {
        key: secret_key(algo.key_len()),
        nonce: other_nonce,
        aad,
        ciphertext,
    });
    assert_eq!(wrong_nonce.unwrap_err(), CryptoError::Decryption);
}

#[cfg(feature = "aes-gcm")]
#[test]
fn aes_256_gcm_satisfies_aead_invariants() {
    exercise_aead(AeadAlgo::Aes256Gcm);
}

#[cfg(feature = "chacha20poly1305")]
#[test]
fn chacha20poly1305_satisfies_aead_invariants() {
    exercise_aead(AeadAlgo::ChaCha20Poly1305);
}

#[cfg(feature = "gost-cipher")]
#[test]
fn kuznyechik_mgm_satisfies_aead_invariants() {
    exercise_aead(AeadAlgo::KuznyechikMgm);
}

#[cfg(feature = "gost-cipher")]
#[test]
fn magma_mgm_satisfies_aead_invariants() {
    exercise_aead(AeadAlgo::MagmaMgm);
}

#[cfg(feature = "aes-gcm")]
#[test]
fn tampered_ciphertext_is_rejected() {
    let (enc, dec) = handlers();
    let key = secret_key(32);
    let nonce = Nonce::new(vec![1u8; 12]);

    let mut ciphertext = enc
        .handle(Encrypt {
            algo: AeadAlgo::Aes256Gcm,
            key: key.clone(),
            nonce: nonce.clone(),
            aad: AssociatedData::empty(),
            plaintext: b"secret".to_vec(),
        })
        .unwrap();
    ciphertext.bytes[0] ^= 0x01;

    let err = dec
        .handle(Decrypt {
            key,
            nonce,
            aad: AssociatedData::empty(),
            ciphertext,
        })
        .unwrap_err();

    assert_eq!(err, CryptoError::Decryption);
}

#[cfg(feature = "aes-gcm")]
#[test]
fn mismatched_associated_data_is_rejected() {
    let (enc, dec) = handlers();
    let key = secret_key(32);
    let nonce = Nonce::new(vec![1u8; 12]);

    let ciphertext = enc
        .handle(Encrypt {
            algo: AeadAlgo::Aes256Gcm,
            key: key.clone(),
            nonce: nonce.clone(),
            aad: AssociatedData::new(b"context-a".to_vec()),
            plaintext: b"secret".to_vec(),
        })
        .unwrap();

    let err = dec
        .handle(Decrypt {
            key,
            nonce,
            aad: AssociatedData::new(b"context-b".to_vec()),
            ciphertext,
        })
        .unwrap_err();

    assert_eq!(err, CryptoError::Decryption);
}

#[cfg(feature = "aes-gcm")]
#[test]
fn wrong_nonce_length_is_rejected() {
    let (enc, _) = handlers();

    let err = enc
        .handle(Encrypt {
            algo: AeadAlgo::Aes256Gcm,
            key: secret_key(32),
            nonce: Nonce::new(vec![0u8; 5]),
            aad: AssociatedData::empty(),
            plaintext: b"secret".to_vec(),
        })
        .unwrap_err();

    assert!(matches!(err, CryptoError::InvalidNonce(_)));
}

#[cfg(feature = "aes-gcm")]
#[test]
fn wrong_key_length_is_rejected() {
    let (enc, _) = handlers();

    let err = enc
        .handle(Encrypt {
            algo: AeadAlgo::Aes256Gcm,
            key: agate_crypto::SecretKey::new(vec![0u8; 16]),
            nonce: Nonce::new(vec![0u8; 12]),
            aad: AssociatedData::empty(),
            plaintext: b"secret".to_vec(),
        })
        .unwrap_err();

    assert!(matches!(err, CryptoError::InvalidKey(_)));
}

#[cfg(feature = "gost-cipher")]
#[test]
fn mgm_nonce_with_set_high_bit_is_rejected() {
    let aead = RustCryptoAeadFactory
        .aead(AeadAlgo::KuznyechikMgm, &secret_key(32))
        .unwrap();

    let mut nonce = vec![0u8; 16];
    nonce[0] = 0x80;

    let err = aead
        .encrypt(&Nonce::new(nonce), &AssociatedData::empty(), b"x")
        .unwrap_err();

    assert!(matches!(err, CryptoError::InvalidNonce(_)));
}

#[cfg(not(feature = "gost-cipher"))]
#[test]
fn disabled_gost_cipher_reports_unsupported() {
    let err = RustCryptoAeadFactory
        .aead(AeadAlgo::KuznyechikMgm, &secret_key(32))
        .err();

    assert_eq!(
        err,
        Some(CryptoError::UnsupportedAead(AeadAlgo::KuznyechikMgm))
    );
}

#[cfg(any(
    feature = "aes-gcm",
    feature = "chacha20poly1305",
    feature = "gost-cipher"
))]
mod properties {
    use super::{Decrypt, DecryptHandler, Encrypt, EncryptHandler, secret_key};
    use agate_crypto::{AeadAlgo, AssociatedData, Nonce, RustCryptoAeadFactory};
    use proptest::prelude::*;
    use std::sync::Arc;

    fn available() -> Vec<AeadAlgo> {
        let mut algos = Vec::new();
        #[cfg(feature = "aes-gcm")]
        algos.push(AeadAlgo::Aes256Gcm);
        #[cfg(feature = "chacha20poly1305")]
        algos.push(AeadAlgo::ChaCha20Poly1305);
        #[cfg(feature = "gost-cipher")]
        {
            algos.push(AeadAlgo::KuznyechikMgm);
            algos.push(AeadAlgo::MagmaMgm);
        }
        algos
    }

    proptest! {
        #[test]
        fn round_trips_for_arbitrary_input(
            plaintext in proptest::collection::vec(any::<u8>(), 0..512),
            aad in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let factory = Arc::new(RustCryptoAeadFactory);
            let enc = EncryptHandler::new(factory.clone());
            let dec = DecryptHandler::new(factory);

            for algo in available() {
                let nonce = Nonce::new(vec![0u8; algo.nonce_len()]);
                let ciphertext = enc.handle(Encrypt {
                    algo,
                    key: secret_key(algo.key_len()),
                    nonce: nonce.clone(),
                    aad: AssociatedData::new(aad.clone()),
                    plaintext: plaintext.clone(),
                }).unwrap();

                let recovered = dec.handle(Decrypt {
                    key: secret_key(algo.key_len()),
                    nonce,
                    aad: AssociatedData::new(aad.clone()),
                    ciphertext,
                }).unwrap();

                prop_assert_eq!(recovered, plaintext.clone());
            }
        }
    }
}
