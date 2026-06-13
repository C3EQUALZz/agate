use serde_json::Value;

use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// A dotted path into a tool call's parsed arguments, e.g. `url` or
/// `config.endpoint`. Used by an [`ArgumentRule`] to match a marker against one
/// field rather than the whole argument blob, so `args.url` can be screened
/// without a rule also firing on an unrelated field that happens to contain the
/// same text.
///
/// Object keys only (no array indexing yet); a blank path or a blank segment is
/// rejected at construction.
///
/// [`ArgumentRule`]: super::ArgumentRule
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JsonPath {
    segments: Vec<String>,
    /// The original dotted text, kept for display and round-tripping.
    source: String,
}

impl JsonPath {
    /// Parse a dotted path, rejecting a blank path or any blank segment (`a..b`).
    pub fn parse(source: impl Into<String>) -> Result<Self, DomainError> {
        let source = source.into();
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return Err(DomainError::Field("argument path must not be blank".into()));
        }
        let segments: Vec<String> = trimmed.split('.').map(|s| s.trim().to_owned()).collect();
        if segments.iter().any(String::is_empty) {
            return Err(DomainError::Field(format!(
                "argument path '{trimmed}' has an empty segment"
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

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
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
