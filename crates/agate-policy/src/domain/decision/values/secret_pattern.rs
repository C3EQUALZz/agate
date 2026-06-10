use std::hash::{Hash, Hasher};

use regex::Regex;

use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// The placeholder substituted for each matched secret.
pub const REDACTION_MASK: &str = "[REDACTED]";

/// A marker whose occurrences in emitted text must be redacted. Either a
/// **literal** (matched case-insensitively over ASCII) or a **regex** (full
/// `regex` syntax; add `(?i)` for case-insensitivity). The literal form stays
/// the default — cheaper and impossible to miswrite into a catch-all.
///
/// Validated at construction: an empty literal would "match" everywhere, and an
/// invalid regex is rejected before it can reach the data plane.
#[derive(Clone, Debug)]
pub enum SecretPattern {
    Literal(String),
    Regex(Box<Regex>),
}

impl SecretPattern {
    /// A literal marker, rejected when empty.
    pub fn literal(needle: impl Into<String>) -> Result<Self, DomainError> {
        let needle = needle.into();
        if needle.is_empty() {
            return Err(DomainError::Field(
                "secret pattern must not be empty".into(),
            ));
        }
        Ok(Self::Literal(needle))
    }

    /// A regex marker, rejected when blank or not a valid expression.
    pub fn regex(source: impl Into<String>) -> Result<Self, DomainError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(DomainError::Field("secret regex must not be blank".into()));
        }
        let compiled = Regex::new(&source)
            .map_err(|error| DomainError::Field(format!("invalid secret regex: {error}")))?;
        Ok(Self::Regex(Box::new(compiled)))
    }

    /// Backwards-compatible constructor: a literal marker.
    pub fn new(needle: impl Into<String>) -> Result<Self, DomainError> {
        Self::literal(needle)
    }

    /// Mask every occurrence in `text`, returning the result and whether
    /// anything matched.
    #[must_use]
    pub fn mask(&self, text: &str) -> (String, bool) {
        match self {
            Self::Literal(needle) => mask_literal(text, needle),
            Self::Regex(re) => {
                if re.is_match(text) {
                    (re.replace_all(text, REDACTION_MASK).into_owned(), true)
                } else {
                    (text.to_owned(), false)
                }
            }
        }
    }

    /// Whether the pattern occurs in `text` — a detection without masking.
    #[must_use]
    pub fn detects(&self, text: &str) -> bool {
        match self {
            Self::Literal(needle) => text
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase()),
            Self::Regex(re) => re.is_match(text),
        }
    }

    /// The pattern source — its identity for equality/hashing.
    fn source(&self) -> &str {
        match self {
            Self::Literal(needle) => needle,
            Self::Regex(re) => re.as_str(),
        }
    }

    fn tag(&self) -> u8 {
        match self {
            Self::Literal(_) => 0,
            Self::Regex(_) => 1,
        }
    }
}

// `regex::Regex` is not `Eq`/`Hash`, so derive can't apply; identity is the kind
// plus the source string.
impl PartialEq for SecretPattern {
    fn eq(&self, other: &Self) -> bool {
        self.tag() == other.tag() && self.source() == other.source()
    }
}

impl Eq for SecretPattern {}

impl Hash for SecretPattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        self.source().hash(state);
    }
}

impl ValueObject for SecretPattern {}

/// Replace every ASCII-case-insensitive occurrence of `needle` with the mask.
/// ASCII lowercasing preserves byte length, so positions in the lowered copy
/// index correctly back into the original.
fn mask_literal(haystack: &str, needle: &str) -> (String, bool) {
    let lower_haystack = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();

    let mut result = String::with_capacity(haystack.len());
    let mut cursor = 0;
    let mut matched = false;

    while let Some(offset) = lower_haystack[cursor..].find(&lower_needle) {
        let start = cursor + offset;
        let end = start + lower_needle.len();
        result.push_str(&haystack[cursor..start]);
        result.push_str(REDACTION_MASK);
        cursor = end;
        matched = true;
    }
    result.push_str(&haystack[cursor..]);

    (result, matched)
}

#[cfg(test)]
mod tests {
    use super::{REDACTION_MASK, SecretPattern};

    #[test]
    fn an_empty_literal_is_rejected() {
        assert!(SecretPattern::literal("").is_err());
    }

    #[test]
    fn a_blank_or_invalid_regex_is_rejected() {
        assert!(SecretPattern::regex("   ").is_err());
        assert!(SecretPattern::regex("(unclosed").is_err());
    }

    #[test]
    fn a_literal_masks_case_insensitively() {
        let pattern = SecretPattern::literal("sk-").expect("valid");
        let (out, hit) = pattern.mask("my SK- key");
        assert!(hit);
        assert_eq!(out, format!("my {REDACTION_MASK} key"));
    }

    #[test]
    fn a_regex_masks_each_match() {
        let pattern = SecretPattern::regex(r"sk-[a-z0-9]{4}").expect("valid");
        let (out, hit) = pattern.mask("keys sk-ab12 and sk-cd34 done");
        assert!(hit);
        assert_eq!(
            out,
            format!("keys {REDACTION_MASK} and {REDACTION_MASK} done")
        );
    }

    #[test]
    fn detects_reports_a_match_without_masking() {
        let pattern = SecretPattern::regex(r"AKIA[0-9A-Z]{4}").expect("valid");
        assert!(pattern.detects(r#"{"key":"AKIA1234"}"#));
        assert!(!pattern.detects(r#"{"key":"nope"}"#));
    }

    #[test]
    fn equality_is_by_kind_and_source() {
        assert_eq!(
            SecretPattern::literal("sk-").unwrap(),
            SecretPattern::literal("sk-").unwrap()
        );
        // Same source, different kind → not equal.
        assert_ne!(
            SecretPattern::literal("sk-").unwrap(),
            SecretPattern::regex("sk-").unwrap()
        );
    }
}
