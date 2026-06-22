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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::PatternError;

    fn regex_error() -> regex::Error {
        let invalid = String::from("(");
        regex::Regex::new(&invalid).expect_err("invalid regex")
    }

    #[test]
    fn displays_each_variant() {
        assert_eq!(PatternError::Blank.to_string(), "pattern must not be blank");
        assert_eq!(
            PatternError::BlankRegex.to_string(),
            "pattern regex must not be blank"
        );
        assert!(
            PatternError::InvalidRegex(regex_error())
                .to_string()
                .starts_with("invalid pattern regex:")
        );
    }

    #[test]
    fn only_an_invalid_regex_has_a_source() {
        assert!(PatternError::Blank.source().is_none());
        assert!(PatternError::BlankRegex.source().is_none());
        assert!(PatternError::InvalidRegex(regex_error()).source().is_some());
    }
}
