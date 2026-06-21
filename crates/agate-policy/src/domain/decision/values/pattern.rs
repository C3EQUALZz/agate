use std::hash::{Hash, Hasher};

use regex::Regex;

use crate::domain::common::errors::{DomainError, PatternError};
use crate::domain::common::values::ValueObject;

/// The placeholder substituted for each matched occurrence when masking.
pub const REDACTION_MASK: &str = "[REDACTED]";

/// A text matcher used across the decision rules — secret redaction and
/// argument deny rules both match content the same way. Either a **literal**
/// (matched case-insensitively over ASCII — only the literal's own bytes are
/// folded, not Unicode) or a **regex** (full `regex` syntax; add `(?i)` for
/// case-insensitivity). The literal form is the default — cheaper and
/// impossible to miswrite into a catch-all.
///
/// The kind is sealed behind private constructors: an empty literal (which
/// would "match" everywhere and loop forever while masking) and an invalid
/// regex are both rejected at construction, so no caller can build a pattern
/// that violates those invariants.
#[derive(Clone, Debug)]
pub struct Pattern(Kind);

#[derive(Clone, Debug)]
enum Kind {
    Literal(String),
    Regex(Box<Regex>),
}

impl Pattern {
    /// A literal marker, rejected when blank.
    pub fn literal(needle: impl Into<String>) -> Result<Self, DomainError> {
        let needle = needle.into();
        if needle.trim().is_empty() {
            return Err(PatternError::Blank.into());
        }
        Ok(Self(Kind::Literal(needle)))
    }

    /// A regex marker, rejected when blank or not a valid expression.
    pub fn regex(source: impl Into<String>) -> Result<Self, DomainError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(PatternError::BlankRegex.into());
        }
        let compiled = Regex::new(&source).map_err(PatternError::InvalidRegex)?;
        Ok(Self(Kind::Regex(Box::new(compiled))))
    }

    /// Whether the pattern occurs in `text` (ASCII case-insensitive for a
    /// literal; regex semantics otherwise).
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        match &self.0 {
            Kind::Literal(needle) => text
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase()),
            Kind::Regex(re) => re.is_match(text),
        }
    }

    /// Mask every occurrence in `text`, returning the result and whether
    /// anything matched.
    #[must_use]
    pub fn mask(&self, text: &str) -> (String, bool) {
        match &self.0 {
            Kind::Literal(needle) => mask_literal(text, needle),
            Kind::Regex(re) => {
                if re.is_match(text) {
                    (re.replace_all(text, REDACTION_MASK).into_owned(), true)
                } else {
                    (text.to_owned(), false)
                }
            }
        }
    }

    /// The pattern source — its identity for equality/hashing.
    fn source(&self) -> &str {
        match &self.0 {
            Kind::Literal(needle) => needle,
            Kind::Regex(re) => re.as_str(),
        }
    }

    fn tag(&self) -> u8 {
        match &self.0 {
            Kind::Literal(_) => 0,
            Kind::Regex(_) => 1,
        }
    }
}

// `regex::Regex` is not `Eq`/`Hash`, so derive can't apply; identity is the kind
// plus the source string.
impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.tag() == other.tag() && self.source() == other.source()
    }
}

impl Eq for Pattern {}

impl Hash for Pattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        self.source().hash(state);
    }
}

impl ValueObject for Pattern {}

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
    use super::{Pattern, REDACTION_MASK};

    #[test]
    fn a_blank_literal_is_rejected() {
        assert!(Pattern::literal("").is_err());
        assert!(Pattern::literal("   ").is_err());
    }

    #[test]
    fn a_blank_or_invalid_regex_is_rejected() {
        assert!(Pattern::regex("   ").is_err());
        assert!(Pattern::regex("(unclosed").is_err());
    }

    #[test]
    fn an_invalid_regex_error_chains_to_its_source() {
        use std::error::Error;
        let error = Pattern::regex("(unclosed").expect_err("invalid regex rejected");
        // DomainError -> PatternError -> regex::Error
        let pattern_error = error.source().expect("a typed sub-error");
        assert!(
            pattern_error.source().is_some(),
            "chains to the regex error"
        );
    }

    #[test]
    fn a_literal_matches_and_masks_case_insensitively() {
        let pattern = Pattern::literal("sk-").expect("valid");
        assert!(pattern.matches("my SK- key"));
        let (out, hit) = pattern.mask("my SK- key");
        assert!(hit);
        assert_eq!(out, format!("my {REDACTION_MASK} key"));
    }

    #[test]
    fn a_regex_matches_and_masks_each_match() {
        let pattern = Pattern::regex(r"sk-[a-z0-9]{4}").expect("valid");
        assert!(pattern.matches("here sk-ab12"));
        let (out, hit) = pattern.mask("keys sk-ab12 and sk-cd34 done");
        assert!(hit);
        assert_eq!(
            out,
            format!("keys {REDACTION_MASK} and {REDACTION_MASK} done")
        );
    }

    #[test]
    fn matches_reports_a_hit_without_masking() {
        let pattern = Pattern::regex(r"AKIA[0-9A-Z]{4}").expect("valid");
        assert!(pattern.matches(r#"{"key":"AKIA1234"}"#));
        assert!(!pattern.matches(r#"{"key":"nope"}"#));
    }

    #[test]
    fn equality_is_by_kind_and_source() {
        assert_eq!(
            Pattern::literal("sk-").unwrap(),
            Pattern::literal("sk-").unwrap()
        );
        // Same source, different kind → not equal.
        assert_ne!(
            Pattern::literal("sk-").unwrap(),
            Pattern::regex("sk-").unwrap()
        );
    }
}
