use super::tool_matcher::ToolMatcher;
use crate::domain::common::values::ValueObject;

/// How tool invocations are authorized.
///
/// - `AllowAll` — no tool restriction (the permissive default).
/// - `Allowlist` — only tools matching an entry may run; everything else is denied.
/// - `Denylist` — every tool may run except those matching an entry.
///
/// Each entry is a [`ToolMatcher`] (exact, glob, or regex), so a list can name a
/// single tool or a whole family (`fs.*`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ToolPolicy {
    AllowAll,
    Allowlist(Vec<ToolMatcher>),
    Denylist(Vec<ToolMatcher>),
}

impl ToolPolicy {
    /// Whether `name` is permitted under this policy.
    ///
    /// In `Denylist` mode a non-ASCII tool name is **always denied** even when no
    /// entry explicitly matches it. Legitimate tool names are ASCII identifiers
    /// (`search`, `fs.read`); a non-ASCII name is a potential Unicode homoglyph
    /// confusable (e.g. Cyrillic `е` standing in for Latin `e`) that would
    /// otherwise slip past every ASCII-only exact matcher.
    #[must_use]
    pub fn permits(&self, name: &str) -> bool {
        match self {
            ToolPolicy::AllowAll => true,
            ToolPolicy::Allowlist(matchers) => matches_any(matchers, name),
            ToolPolicy::Denylist(matchers) => name.is_ascii() && !matches_any(matchers, name),
        }
    }
}

fn matches_any(matchers: &[ToolMatcher], name: &str) -> bool {
    matchers.iter().any(|matcher| matcher.matches(name))
}

impl ValueObject for ToolPolicy {}

#[cfg(test)]
mod tests {
    use super::{ToolMatcher, ToolPolicy};

    #[test]
    fn allow_all_permits_everything() {
        assert!(ToolPolicy::AllowAll.permits("anything"));
    }

    #[test]
    fn an_allowlist_permits_only_matching_tools() {
        let policy = ToolPolicy::Allowlist(vec![
            ToolMatcher::exact("search").unwrap(),
            ToolMatcher::glob("fs.*").unwrap(),
        ]);
        assert!(policy.permits("search"));
        assert!(policy.permits("fs.read"));
        assert!(!policy.permits("rm"));
        assert!(!policy.permits("research"));
    }

    #[test]
    fn a_denylist_blocks_only_matching_tools() {
        let policy = ToolPolicy::Denylist(vec![ToolMatcher::glob("fs.*").unwrap()]);
        assert!(!policy.permits("fs.delete"));
        assert!(policy.permits("search"));
    }

    #[test]
    fn a_denylist_blocks_non_ascii_names_as_potential_homoglyphs() {
        // "dеlеtе_filе" uses Cyrillic е (U+0435) in place of Latin e.
        // Without the ASCII guard it would bypass an exact "delete_file" entry
        // because the byte sequences differ. The denylist must fail-closed on
        // any non-ASCII tool name rather than silently permitting it.
        let policy = ToolPolicy::Denylist(vec![ToolMatcher::exact("delete_file").unwrap()]);
        assert!(
            !policy.permits("dеlеtе_filе"),
            "Cyrillic homoglyph must be denied"
        );
        // Allowlist mode is unaffected: the homoglyph is simply not listed.
        let allowlist = ToolPolicy::Allowlist(vec![ToolMatcher::exact("search").unwrap()]);
        assert!(!allowlist.permits("dеlеtе_filе"), "not in allowlist");
    }
}
