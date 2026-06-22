use std::fmt;

use super::json_path_error::JsonPathError;
use super::pattern_error::PatternError;
use super::tool_matcher_error::ToolMatcherError;
use super::tool_name_error::ToolNameError;

/// Root of the policy domain error hierarchy: one variant per value object whose
/// constructor can fail. Each carries that object's typed error, which in turn
/// chains to the underlying cause (e.g. a `regex::Error`) through `source`.
#[derive(Debug, Clone)]
pub enum DomainError {
    Pattern(PatternError),
    ToolName(ToolNameError),
    JsonPath(JsonPathError),
    ToolMatcher(ToolMatcherError),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::Pattern(error) => write!(f, "{error}"),
            DomainError::ToolName(error) => write!(f, "{error}"),
            DomainError::JsonPath(error) => write!(f, "{error}"),
            DomainError::ToolMatcher(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DomainError::Pattern(error) => Some(error),
            DomainError::ToolName(error) => Some(error),
            DomainError::JsonPath(error) => Some(error),
            DomainError::ToolMatcher(error) => Some(error),
        }
    }
}

impl From<PatternError> for DomainError {
    fn from(error: PatternError) -> Self {
        DomainError::Pattern(error)
    }
}

impl From<ToolNameError> for DomainError {
    fn from(error: ToolNameError) -> Self {
        DomainError::ToolName(error)
    }
}

impl From<JsonPathError> for DomainError {
    fn from(error: JsonPathError) -> Self {
        DomainError::JsonPath(error)
    }
}

impl From<ToolMatcherError> for DomainError {
    fn from(error: ToolMatcherError) -> Self {
        DomainError::ToolMatcher(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::domain::common::errors::{
        DomainError, JsonPathError, PatternError, ToolMatcherError, ToolNameError,
    };

    #[test]
    fn from_wraps_and_display_delegates_for_every_variant() {
        assert_eq!(
            DomainError::from(PatternError::Blank).to_string(),
            "pattern must not be blank"
        );
        assert_eq!(
            DomainError::from(ToolNameError::Blank).to_string(),
            "tool name must not be blank"
        );
        assert_eq!(
            DomainError::from(JsonPathError::Blank).to_string(),
            "rule path must not be blank"
        );
        assert_eq!(
            DomainError::from(ToolMatcherError::BlankExact).to_string(),
            "tool name must not be blank"
        );
    }

    #[test]
    fn every_variant_exposes_its_inner_error_as_source() {
        let errors = [
            DomainError::from(PatternError::Blank),
            DomainError::from(ToolNameError::Blank),
            DomainError::from(JsonPathError::Blank),
            DomainError::from(ToolMatcherError::BlankExact),
        ];
        for error in errors {
            assert!(error.source().is_some(), "{error}");
        }
    }
}
