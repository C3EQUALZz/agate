use std::fmt;

/// A failure building a [`ToolName`](crate::domain::decision::ToolName).
#[derive(Debug, Clone)]
pub enum ToolNameError {
    /// The name was blank after trimming.
    Blank,
}

impl fmt::Display for ToolNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolNameError::Blank => write!(f, "tool name must not be blank"),
        }
    }
}

impl std::error::Error for ToolNameError {}

#[cfg(test)]
mod tests {
    use super::ToolNameError;

    #[test]
    fn displays_the_blank_variant() {
        assert_eq!(
            ToolNameError::Blank.to_string(),
            "tool name must not be blank"
        );
    }
}
