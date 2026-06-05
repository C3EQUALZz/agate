use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// A marker whose occurrences in emitted text must be redacted (a literal,
/// matched case-insensitively over ASCII). Kept deliberately simple for now — a
/// richer matcher (regex, entropy) can replace the internals behind this same
/// value object without touching callers.
///
/// Validated non-empty: an empty needle would "match" everywhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretPattern(String);

impl SecretPattern {
    pub fn new(needle: impl Into<String>) -> Result<Self, DomainError> {
        let needle = needle.into();
        if needle.is_empty() {
            return Err(DomainError::Field(
                "secret pattern must not be empty".into(),
            ));
        }
        Ok(Self(needle))
    }

    #[must_use]
    pub fn needle(&self) -> &str {
        &self.0
    }
}

impl ValueObject for SecretPattern {}
