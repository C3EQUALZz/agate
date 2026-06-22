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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::ToolMatcherError;

    fn regex_error() -> regex::Error {
        let invalid = String::from("(");
        regex::Regex::new(&invalid).expect_err("invalid regex")
    }

    #[test]
    fn displays_each_variant() {
        assert_eq!(
            ToolMatcherError::BlankExact.to_string(),
            "tool name must not be blank"
        );
        assert_eq!(
            ToolMatcherError::BlankGlob.to_string(),
            "tool glob must not be blank"
        );
        assert_eq!(
            ToolMatcherError::BlankRegex.to_string(),
            "tool regex must not be blank"
        );
        assert!(
            ToolMatcherError::InvalidGlob(regex_error())
                .to_string()
                .starts_with("invalid tool glob:")
        );
        assert!(
            ToolMatcherError::InvalidRegex(regex_error())
                .to_string()
                .starts_with("invalid tool regex:")
        );
    }

    #[test]
    fn only_compile_failures_have_a_source() {
        assert!(ToolMatcherError::BlankExact.source().is_none());
        assert!(ToolMatcherError::BlankGlob.source().is_none());
        assert!(ToolMatcherError::BlankRegex.source().is_none());
        assert!(
            ToolMatcherError::InvalidGlob(regex_error())
                .source()
                .is_some()
        );
        assert!(
            ToolMatcherError::InvalidRegex(regex_error())
                .source()
                .is_some()
        );
    }
}
