use serde_json::Value;

use super::json_path::JsonPath;
use super::pattern::Pattern;
use super::tool_name::ToolName;

/// The shared mechanics behind an [`ArgumentRule`] and a [`ResultRule`]: an
/// optional tool scope, an optional [`JsonPath`] into the parsed payload, and a
/// forbidden-content [`Pattern`]. Both rules are thin wrappers that give this
/// the right domain meaning (input vs. output) and the right `matches`
/// signature; the matching itself lives here once.
///
/// [`ArgumentRule`]: super::ArgumentRule
/// [`ResultRule`]: super::ResultRule
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentMatch {
    tool: Option<ToolName>,
    path: Option<JsonPath>,
    marker: Pattern,
}

impl ContentMatch {
    pub fn new(tool: Option<ToolName>, marker: Pattern) -> Self {
        Self {
            tool,
            path: None,
            marker,
        }
    }

    #[must_use]
    pub fn with_path(mut self, path: JsonPath) -> Self {
        self.path = Some(path);
        self
    }

    /// Whether the marker fires for a payload from tool `name` (`None` when the
    /// caller cannot attribute it to a tool): the tool scope matches (or is
    /// unscoped) **and** the marker occurs in the targeted text — the value at
    /// the path, or the whole `content` when there is no path. A tool-scoped
    /// rule never fires on an unattributed payload — the scope can't be
    /// confirmed. `parsed` is `content` deserialized once by the caller; `None`
    /// when it was not valid JSON, so a path rule then does not fire.
    #[must_use]
    pub fn matches(&self, name: Option<&str>, content: &str, parsed: Option<&Value>) -> bool {
        if let Some(tool) = &self.tool
            && name != Some(tool.as_str())
        {
            return false;
        }
        match &self.path {
            None => self.marker.matches(content),
            Some(path) => parsed
                .and_then(|value| path.get_text(value))
                .is_some_and(|text| self.marker.matches(&text)),
        }
    }

    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_ref().map(ToolName::as_str)
    }

    #[must_use]
    pub fn path(&self) -> Option<&JsonPath> {
        self.path.as_ref()
    }

    #[must_use]
    pub fn marker(&self) -> &Pattern {
        &self.marker
    }
}
