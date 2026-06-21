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
