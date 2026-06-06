# agate-crypto

> Crypto agility for Agate: pluggable, self-describing **hash**, **signature**,
> and **AEAD** algorithms.

`agate-crypto` is a **generic subdomain** published as a *library* — **not** a
DDD shared kernel. Depending on it is like depending on `ring` or `sha2`: a
stable technical capability, not a shared domain model. Internally it still
follows Clean Architecture so the design stays uniform with the
bounded-context crates.

## Responsibility

Provide algorithms that are **pluggable and self-describing**: the algorithm tag
travels with every `Digest` and `Signature`, so consumers (notably
[`agate-audit`](audit.md)) can verify material without out-of-band knowledge of
which algorithm produced it.

- **Hashes:** SHA-2, SHA-3, and **Streebog (GOST R 34.11-2012)** — feature-gated.
- **Signatures:** **Ed25519** (GOST R 34.10-2012 planned).
- **AEAD:** AES-GCM, ChaCha20-Poly1305, and GOST **Kuznyechik** / **Magma** in
  MGM mode — feature-gated.

Hashing and signing are **pure strategies** (traits). Key loading is I/O and
lives behind a port, outside the pure core.

## Domain language

- `Digest`, `HashAlgo`, `Hasher` — hashing strategy and its self-describing output.
- `Signature`, `SignAlgo`, `Signer`, `Verifier`, `KeyId` — signing strategy.
- `Aead`, `AeadAlgo`, `Nonce`, `Ciphertext`, `AssociatedData` — authenticated encryption.
- `SecretKey`, `CryptoError` — shared value/error types.

## Layering

| Layer | Contents |
| --- | --- |
| `domain` | Pure, dependency-free: self-describing algorithm values and the strategy traits (`Hasher`, `Signer`, `Verifier`, `Aead`). |
| `application` | Abstract-factory ports (`HasherFactory`, `SignatureFactory`, `AeadFactory`) and thin use cases over them. |
| `infrastructure` | Concrete RustCrypto backends and the factories (`RustCryptoHasherFactory`, `RustCryptoSignatureFactory`, `RustCryptoAeadFactory`, `CryptoRegistry`). |

The two core patterns are **strategy** (the algorithm traits) and **abstract
factory** (resolve a self-describing algorithm to a strategy).

## Crypto agility & Merkle epochs

A single Merkle tree uses **one** hash algorithm — an *epoch*. Switching
algorithms starts a **new epoch**, recorded in the signed tree head. Because the
algorithm tag travels with each digest, the audit log remains verifiable across
epoch boundaries. See [agate-audit](audit.md).
