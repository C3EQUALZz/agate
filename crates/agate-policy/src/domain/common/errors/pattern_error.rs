use std::fmt;

/// A failure building a [`Pattern`](crate::domain::decision::Pattern).
#[derive(Debug, Clone)]
pub enum PatternError {
    /// A literal marker was blank.
    Blank,
    /// A regex marker source was blank.
    BlankRegex,
    /// A regex marker did not compile.
    InvalidRegex(regex::Error),
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternError::Blank => write!(f, "pattern must not be blank"),
            PatternError::BlankRegex => write!(f, "pattern regex must not be blank"),
            PatternError::InvalidRegex(error) => write!(f, "invalid pattern regex: {error}"),
        }
    }
}

impl std::error::Error for PatternError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PatternError::InvalidRegex(error) => Some(error),
            PatternError::Blank | PatternError::BlankRegex => None,
        }
    }
}
