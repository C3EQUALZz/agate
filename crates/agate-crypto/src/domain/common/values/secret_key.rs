use std::fmt;

use zeroize::Zeroize;

/// Secret key material shared by the signing and encryption subdomains.
///
/// Deliberately *not* a [`ValueObject`](super::base::ValueObject): it neither
/// implements `PartialEq` (equality checks on secrets invite timing leaks) nor
/// a revealing `Debug`, and it wipes its bytes on drop.
#[derive(Clone)]
pub struct SecretKey(Vec<u8>);

impl SecretKey {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the raw key bytes. The caller must not retain or log them.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey([redacted; {} bytes])", self.0.len())
    }
}

#[cfg(test)]
mod tests {
    use super::SecretKey;

    #[test]
    fn exposes_its_bytes_and_length() {
        let key = SecretKey::new(vec![1, 2, 3]);
        assert_eq!(key.expose(), &[1, 2, 3]);
        assert_eq!(key.len(), 3);
        assert!(!key.is_empty());
        assert!(SecretKey::new(Vec::new()).is_empty());
    }

    #[test]
    fn debug_never_reveals_the_secret() {
        let key = SecretKey::new(vec![0xde, 0xad, 0xbe, 0xef]);
        let rendered = format!("{key:?}");
        assert!(rendered.contains("redacted"));
        assert!(!rendered.contains("de"));
        assert!(!rendered.contains("222")); // no decimal byte values either
    }
}
