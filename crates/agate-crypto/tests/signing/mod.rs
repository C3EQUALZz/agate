use std::sync::Arc;

use agate_crypto::application::usecases::sign_data::{SignData, SignDataHandler};
use agate_crypto::application::usecases::verify_signature::{
    VerifySignature, VerifySignatureHandler,
};
use agate_crypto::ed25519::Ed25519Signer;
use agate_crypto::{CryptoError, KeyId, RustCryptoSignatureFactory, SecretKey, SignAlgo};

const SEED: [u8; 32] = [7u8; 32];

fn sign_handler() -> SignDataHandler {
    SignDataHandler::new(Arc::new(RustCryptoSignatureFactory))
}

fn verify_handler() -> VerifySignatureHandler {
    VerifySignatureHandler::new(Arc::new(RustCryptoSignatureFactory))
}

fn public_key() -> Vec<u8> {
    Ed25519Signer::from_seed(&SEED, KeyId("k1".to_string()))
        .verifying_key_bytes()
        .to_vec()
}

#[test]
fn ed25519_sign_then_verify_round_trips() {
    let signature = sign_handler()
        .handle(SignData {
            algo: SignAlgo::Ed25519,
            key: SecretKey::new(SEED.to_vec()),
            key_id: KeyId("k1".to_string()),
            data: b"transfer 100".to_vec(),
        })
        .unwrap();

    assert_eq!(signature.algo, SignAlgo::Ed25519);
    assert_eq!(signature.key_id, KeyId("k1".to_string()));

    let ok = verify_handler()
        .handle(VerifySignature {
            public_key: public_key(),
            data: b"transfer 100".to_vec(),
            signature,
        })
        .unwrap();

    assert!(ok);
}

#[test]
fn tampered_message_fails_verification() {
    let signature = sign_handler()
        .handle(SignData {
            algo: SignAlgo::Ed25519,
            key: SecretKey::new(SEED.to_vec()),
            key_id: KeyId("k1".to_string()),
            data: b"transfer 100".to_vec(),
        })
        .unwrap();

    let ok = verify_handler()
        .handle(VerifySignature {
            public_key: public_key(),
            data: b"transfer 900".to_vec(),
            signature,
        })
        .unwrap();

    assert!(!ok);
}

#[test]
fn signature_from_another_key_is_rejected() {
    let signature = sign_handler()
        .handle(SignData {
            algo: SignAlgo::Ed25519,
            key: SecretKey::new(SEED.to_vec()),
            key_id: KeyId("k1".to_string()),
            data: b"transfer 100".to_vec(),
        })
        .unwrap();

    let other_public = Ed25519Signer::from_seed(&[9u8; 32], KeyId("k2".to_string()))
        .verifying_key_bytes()
        .to_vec();

    let ok = verify_handler()
        .handle(VerifySignature {
            public_key: other_public,
            data: b"transfer 100".to_vec(),
            signature,
        })
        .unwrap();

    assert!(!ok);
}

#[test]
fn signer_rejects_wrong_seed_length() {
    let err = sign_handler()
        .handle(SignData {
            algo: SignAlgo::Ed25519,
            key: SecretKey::new(vec![0u8; 16]),
            key_id: KeyId("k1".to_string()),
            data: Vec::new(),
        })
        .unwrap_err();

    assert!(matches!(err, CryptoError::InvalidKey(_)));
}

#[test]
fn verifier_rejects_wrong_public_key_length() {
    let signature = sign_handler()
        .handle(SignData {
            algo: SignAlgo::Ed25519,
            key: SecretKey::new(SEED.to_vec()),
            key_id: KeyId("k1".to_string()),
            data: b"msg".to_vec(),
        })
        .unwrap();

    let err = verify_handler()
        .handle(VerifySignature {
            public_key: vec![0u8; 8],
            data: b"msg".to_vec(),
            signature,
        })
        .unwrap_err();

    assert!(matches!(err, CryptoError::InvalidKey(_)));
}

#[test]
fn unimplemented_gost_signature_reports_unsupported() {
    let err = sign_handler()
        .handle(SignData {
            algo: SignAlgo::GostR3410_2012_256,
            key: SecretKey::new(vec![0u8; 32]),
            key_id: KeyId("k1".to_string()),
            data: Vec::new(),
        })
        .unwrap_err();

    assert_eq!(
        err,
        CryptoError::UnsupportedSignature(SignAlgo::GostR3410_2012_256)
    );
}
