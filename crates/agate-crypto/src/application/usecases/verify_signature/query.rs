use crate::domain::signing::Signature;

/// Verify `signature` over `data` using `public_key`. The algorithm is taken
/// from the self-describing signature.
pub struct VerifySignature {
    pub public_key: Vec<u8>,
    pub data: Vec<u8>,
    pub signature: Signature,
}
