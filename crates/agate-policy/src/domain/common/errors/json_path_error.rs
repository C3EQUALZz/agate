use std::fmt;

/// A failure parsing a [`JsonPath`](crate::domain::decision::JsonPath).
#[derive(Debug, Clone)]
pub enum JsonPathError {
    /// The path was blank.
    Blank,
    /// A dotted path had an empty segment (e.g. `a..b`).
    EmptySegment { path: String },
    /// The path used array indexing, which is unsupported (object keys only).
    ArrayIndexing { path: String },
}

impl fmt::Display for JsonPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonPathError::Blank => write!(f, "rule path must not be blank"),
            JsonPathError::EmptySegment { path } => {
                write!(f, "rule path '{path}' has an empty segment")
            }
            JsonPathError::ArrayIndexing { path } => write!(
                f,
                "rule path '{path}' uses array indexing, which is not supported (object keys only)"
            ),
        }
    }
}

impl std::error::Error for JsonPathError {}
