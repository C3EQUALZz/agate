use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// A dotted path into a tool call's parsed arguments, e.g. `url` or
/// `config.endpoint`. Used by an [`ArgumentRule`] to match a marker against one
/// field rather than the whole argument blob, so `args.url` can be screened
/// without a rule also firing on an unrelated field that happens to contain the
/// same text.
///
/// Object keys only (no array indexing); a blank path, a blank segment, or an
/// index-style segment (`items[0]`) is rejected at construction so a path that
/// could never match array-shaped JSON can't silently weaken a deny rule.
///
/// [`ArgumentRule`]: super::ArgumentRule
#[derive(Clone, Debug)]
pub struct JsonPath {
    segments: Vec<String>,
    /// The original dotted text, kept for display only — *not* part of identity
    /// (`a.b` and `a . b` parse to the same segments and must compare equal).
    source: String,
}

impl JsonPath {
    /// Parse a dotted path, rejecting a blank path, a blank segment (`a..b`,
    /// `a.`), or an index-style segment (`items[0]`).
    pub fn parse(source: impl Into<String>) -> Result<Self, DomainError> {
        let source = source.into();
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return Err(DomainError::Field("rule path must not be blank".into()));
        }
        let segments: Vec<String> = trimmed.split('.').map(|s| s.trim().to_owned()).collect();
        if segments.iter().any(String::is_empty) {
            return Err(DomainError::Field(format!(
                "rule path '{trimmed}' has an empty segment"
            )));
        }
        if segments.iter().any(|s| s.contains('[') || s.contains(']')) {
            return Err(DomainError::Field(format!(
                "rule path '{trimmed}' uses array indexing, which is not supported \
                 (object keys only)"
            )));
        }
        Ok(Self {
            segments,
            source: trimmed.to_owned(),
        })
    }

    /// Resolve the path against `value`, returning the node at the path or
    /// `None` if any segment is missing or traverses a non-object.
    #[must_use]
    pub fn get<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let mut current = value;
        for segment in &self.segments {
            current = current.get(segment)?;
        }
        Some(current)
    }

    /// Resolve the path and render the node as match text: the inner string for
    /// a JSON string (so `"http://x"` matches as `http://x`), or the node's
    /// compact JSON otherwise (so a number/object/array can still be screened).
    /// `None` if the path is absent.
    #[must_use]
    pub fn get_text(&self, value: &Value) -> Option<String> {
        self.get(value).map(|node| match node {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }
}

// Identity is the resolved segments, not the author's source text: `a.b` and
// `a . b` denote the same path and must compare (and hash) equal.
impl PartialEq for JsonPath {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments
    }
}

impl Eq for JsonPath {}

impl Hash for JsonPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.segments.hash(state);
    }
}

impl ValueObject for JsonPath {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::JsonPath;

    #[test]
    fn a_blank_path_or_segment_is_rejected() {
        assert!(JsonPath::parse("   ").is_err());
        assert!(JsonPath::parse("a..b").is_err());
        assert!(JsonPath::parse(".a").is_err());
        assert!(JsonPath::parse("a.").is_err()); // trailing dot
    }

    #[test]
    fn an_index_style_segment_is_rejected() {
        assert!(JsonPath::parse("items[0]").is_err());
        assert!(JsonPath::parse("a.b[1].c").is_err());
    }

    #[test]
    fn identity_ignores_surrounding_whitespace_in_the_source() {
        // Same segments, different source spelling → equal value objects.
        assert_eq!(
            JsonPath::parse("a.b").unwrap(),
            JsonPath::parse("a . b").unwrap()
        );
    }

    #[test]
    fn resolves_a_nested_object_path() {
        let value = json!({ "config": { "endpoint": "http://x" } });
        let path = JsonPath::parse("config.endpoint").expect("valid");
        assert_eq!(path.get(&value).unwrap(), &json!("http://x"));
    }

    #[test]
    fn a_missing_or_non_object_segment_resolves_to_none() {
        let value = json!({ "url": "http://x" });
        assert!(JsonPath::parse("missing").unwrap().get(&value).is_none());
        // `url` is a string, so descending into it finds nothing.
        assert!(JsonPath::parse("url.host").unwrap().get(&value).is_none());
    }
}
