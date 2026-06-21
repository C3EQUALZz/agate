//! Domain error hierarchy.

pub mod base;
pub mod json_path_error;
pub mod pattern_error;
pub mod tool_matcher_error;
pub mod tool_name_error;

pub use base::DomainError;
pub use json_path_error::JsonPathError;
pub use pattern_error::PatternError;
pub use tool_matcher_error::ToolMatcherError;
pub use tool_name_error::ToolNameError;
