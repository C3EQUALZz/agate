use std::sync::Arc;

use agate_crypto::application::usecases::hash_data::{HashData, HashDataHandler};
use agate_crypto::{HashAlgo, RustCryptoHasherFactory};

fn handler() -> HashDataHandler {
    HashDataHandler::new(Arc::new(RustCryptoHasherFactory))
}

#[test]
fn hash_data_produces_known_sha256_vector() {
    let digest = handler()
        .handle(HashData {
            algo: HashAlgo::Sha256,
            data: b"abc".to_vec(),
        })
        .unwrap();

    assert_eq!(digest.algo, HashAlgo::Sha256);
    assert_eq!(
        digest.to_hex(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn digest_is_self_describing_and_deterministic() {
    let a = handler()
        .handle(HashData {
            algo: HashAlgo::Sha256,
            data: b"abc".to_vec(),
        })
        .unwrap();
    let b = handler()
        .handle(HashData {
            algo: HashAlgo::Sha256,
            data: b"abc".to_vec(),
        })
        .unwrap();

    assert_eq!(a, b);
    assert_eq!(a.bytes.len(), 32);
}

#[test]
fn hash_data_produces_known_sha512_vector() {
    let digest = handler()
        .handle(HashData {
            algo: HashAlgo::Sha512,
            data: b"abc".to_vec(),
        })
        .unwrap();

    assert_eq!(digest.algo, HashAlgo::Sha512);
    assert_eq!(
        digest.to_hex(),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
         2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );
}

#[cfg(feature = "sha3")]
#[test]
fn hash_data_produces_known_sha3_256_vector() {
    let digest = handler()
        .handle(HashData {
            algo: HashAlgo::Sha3_256,
            data: b"abc".to_vec(),
        })
        .unwrap();

    assert_eq!(digest.algo, HashAlgo::Sha3_256);
    assert_eq!(
        digest.to_hex(),
        "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
    );
}

#[cfg(not(feature = "sha3"))]
#[test]
fn disabled_algorithm_reports_unsupported() {
    let err = handler()
        .handle(HashData {
            algo: HashAlgo::Sha3_256,
            data: Vec::new(),
        })
        .unwrap_err();

    assert_eq!(
        err,
        agate_crypto::CryptoError::UnsupportedHash(HashAlgo::Sha3_256)
    );
}

#[cfg(feature = "streebog")]
#[test]
fn streebog256_is_a_distinct_256_bit_digest() {
    let streebog = handler()
        .handle(HashData {
            algo: HashAlgo::Streebog256,
            data: b"abc".to_vec(),
        })
        .unwrap();
    let sha = handler()
        .handle(HashData {
            algo: HashAlgo::Sha256,
            data: b"abc".to_vec(),
        })
        .unwrap();

    assert_eq!(streebog.algo, HashAlgo::Streebog256);
    assert_eq!(streebog.bytes.len(), 32);
    assert_ne!(streebog.bytes, sha.bytes);
}
