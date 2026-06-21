use std::fmt;

/// A failure building a [`ToolMatcher`](crate::domain::decision::ToolMatcher).
#[derive(Debug, Clone)]
pub enum ToolMatcherError {
    /// An exact-match name was blank.
    BlankExact,
    /// A glob source was blank.
    BlankGlob,
    /// A glob did not compile to a valid anchored regex.
    InvalidGlob(regex::Error),
    /// A regex source was blank.
    BlankRegex,
    /// A regex did not compile.
    InvalidRegex(regex::Error),
}

impl fmt::Display for ToolMatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolMatcherError::BlankExact => write!(f, "tool name must not be blank"),
            ToolMatcherError::BlankGlob => write!(f, "tool glob must not be blank"),
            ToolMatcherError::InvalidGlob(error) => write!(f, "invalid tool glob: {error}"),
            ToolMatcherError::BlankRegex => write!(f, "tool regex must not be blank"),
            ToolMatcherError::InvalidRegex(error) => write!(f, "invalid tool regex: {error}"),
        }
    }
}

impl std::error::Error for ToolMatcherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolMatcherError::InvalidGlob(error) | ToolMatcherError::InvalidRegex(error) => {
                Some(error)
            }
            ToolMatcherError::BlankExact
            | ToolMatcherError::BlankGlob
            | ToolMatcherError::BlankRegex => None,
        }
    }
}
