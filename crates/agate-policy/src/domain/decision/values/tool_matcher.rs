use std::hash::{Hash, Hasher};

use regex::Regex;

use crate::domain::common::errors::DomainError;
use crate::domain::common::values::ValueObject;

/// Matches a tool name in an allow/deny list. Unlike [`Pattern`] (a
/// substring/regex matcher for secret text), a tool matcher is **anchored** —
/// it matches the whole name — and **case-sensitive**, because tool names are
/// identifiers (`fs.read`, `search`), not prose. So `search` never matches
/// `research`, and `fs.*` matches every `fs.` tool but nothing else.
///
/// Three kinds:
/// - **Exact** — the default; equals the name after trimming. Cheapest and
///   impossible to widen by accident.
/// - **Glob** — shell-style `*` (any run) and `?` (one char); every other
///   character is literal.
/// - **Regex** — full `regex` syntax, anchored to the whole name.
///
/// The kind is sealed behind validating constructors: a blank name, an invalid
/// glob, and an invalid regex are all rejected at construction.
///
/// [`Pattern`]: super::Pattern
#[derive(Clone, Debug)]
pub struct ToolMatcher(Kind);

#[derive(Clone, Debug)]
enum Kind {
    Exact(String),
    /// A glob or a regex — both compiled to one anchored regex; `source` keeps
    /// the author's original text for identity and display. Glob and regex are
    /// distinct variants (not one flag) so an identical source under each kind
    /// stays distinct: `glob("a.b")` (literal dot) ≠ `regex("a.b")` (any char).
    Glob {
        source: String,
        regex: Box<Regex>,
    },
    Regex {
        source: String,
        regex: Box<Regex>,
    },
}

impl ToolMatcher {
    /// An exact name, rejected when blank. Trimmed so a padded config entry
    /// (`" search "`) still names the tool it means.
    pub fn exact(name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into();
        let normalized = name.trim();
        if normalized.is_empty() {
            return Err(DomainError::Field("tool name must not be blank".into()));
        }
        Ok(Self(Kind::Exact(normalized.to_owned())))
    }

    /// A glob over the whole name (`*` = any run, `?` = one char), rejected when
    /// blank.
    pub fn glob(source: impl Into<String>) -> Result<Self, DomainError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(DomainError::Field("tool glob must not be blank".into()));
        }
        // glob → anchored regex can only produce valid syntax, so this compile
        // never fails; surface it as a field error rather than panic if it ever
        // does.
        let regex = Regex::new(&glob_to_regex(&source))
            .map_err(|error| DomainError::Field(format!("invalid tool glob: {error}")))?;
        Ok(Self(Kind::Glob {
            source,
            regex: Box::new(regex),
        }))
    }

    /// A regex anchored to the whole name, rejected when blank or invalid.
    pub fn regex(source: impl Into<String>) -> Result<Self, DomainError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(DomainError::Field("tool regex must not be blank".into()));
        }
        let anchored = format!("^(?:{source})$");
        let regex = Regex::new(&anchored)
            .map_err(|error| DomainError::Field(format!("invalid tool regex: {error}")))?;
        Ok(Self(Kind::Regex {
            source,
            regex: Box::new(regex),
        }))
    }

    /// Whether this matcher names `tool` (whole-name, case-sensitive).
    #[must_use]
    pub fn matches(&self, tool: &str) -> bool {
        match &self.0 {
            Kind::Exact(name) => name == tool,
            Kind::Glob { regex, .. } | Kind::Regex { regex, .. } => regex.is_match(tool),
        }
    }

    /// The matcher source — its identity for equality/hashing and display.
    fn source(&self) -> &str {
        match &self.0 {
            Kind::Exact(name) => name,
            Kind::Glob { source, .. } | Kind::Regex { source, .. } => source,
        }
    }

    fn tag(&self) -> u8 {
        match &self.0 {
            Kind::Exact(_) => 0,
            Kind::Glob { .. } => 1,
            Kind::Regex { .. } => 2,
        }
    }
}

// `regex::Regex` is not `Eq`/`Hash`, so identity is the kind tag plus the
// author's source text (an exact `fs` and a glob `fs` are distinct rules).
impl PartialEq for ToolMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.tag() == other.tag() && self.source() == other.source()
    }
}

impl Eq for ToolMatcher {}

impl Hash for ToolMatcher {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        self.source().hash(state);
    }
}

impl ValueObject for ToolMatcher {}

/// Translate a shell-style glob into an anchored regex: `*` → `.*`, `?` → `.`,
/// every other character escaped to its literal self.
fn glob_to_regex(glob: &str) -> String {
    let mut regex = String::with_capacity(glob.len() * 2 + 2);
    regex.push('^');
    // Accumulate literal runs and escape them in one call, rather than per char.
    let mut literal = String::new();
    let flush = |literal: &mut String, regex: &mut String| {
        if !literal.is_empty() {
            regex.push_str(&regex::escape(literal));
            literal.clear();
        }
    };
    for ch in glob.chars() {
        match ch {
            '*' => {
                flush(&mut literal, &mut regex);
                regex.push_str(".*");
            }
            '?' => {
                flush(&mut literal, &mut regex);
                regex.push('.');
            }
            other => literal.push(other),
        }
    }
    flush(&mut literal, &mut regex);
    regex.push('$');
    regex
}

#[cfg(test)]
mod tests {
    use super::ToolMatcher;

    #[test]
    fn blank_inputs_are_rejected() {
        assert!(ToolMatcher::exact("  ").is_err());
        assert!(ToolMatcher::glob("").is_err());
        assert!(ToolMatcher::regex("   ").is_err());
    }

    #[test]
    fn an_invalid_regex_is_rejected() {
        assert!(ToolMatcher::regex("(unclosed").is_err());
    }

    #[test]
    fn exact_matches_the_whole_name_case_sensitively() {
        let matcher = ToolMatcher::exact(" search ").expect("valid");
        assert!(matcher.matches("search"));
        // anchored: not a substring match
        assert!(!matcher.matches("research"));
        assert!(!matcher.matches("searching"));
        // case-sensitive: tool names are identifiers
        assert!(!matcher.matches("Search"));
    }

    #[test]
    fn a_glob_matches_a_prefix_family_but_not_others() {
        let matcher = ToolMatcher::glob("fs.*").expect("valid");
        assert!(matcher.matches("fs.read"));
        assert!(matcher.matches("fs.write"));
        assert!(matcher.matches("fs."));
        assert!(!matcher.matches("net.read"));
        // anchored at both ends
        assert!(!matcher.matches("xfs.read"));
    }

    #[test]
    fn a_glob_question_mark_matches_one_character() {
        let matcher = ToolMatcher::glob("tool?").expect("valid");
        assert!(matcher.matches("tool1"));
        assert!(!matcher.matches("tool"));
        assert!(!matcher.matches("tool12"));
    }

    #[test]
    fn glob_metacharacters_are_literal() {
        // The `.` is a literal dot, not "any char".
        let matcher = ToolMatcher::glob("fs.read").expect("valid");
        assert!(matcher.matches("fs.read"));
        assert!(!matcher.matches("fsxread"));
    }

    #[test]
    fn a_regex_is_anchored_to_the_whole_name() {
        let matcher = ToolMatcher::regex("db_.*").expect("valid");
        assert!(matcher.matches("db_query"));
        assert!(!matcher.matches("mydb_query"));
        assert!(!matcher.matches("db"));
    }

    #[test]
    fn equality_is_by_kind_and_source() {
        assert_eq!(
            ToolMatcher::exact("fs").unwrap(),
            ToolMatcher::exact("fs").unwrap()
        );
        // same source, different kind → distinct rules
        assert_ne!(
            ToolMatcher::exact("fs").unwrap(),
            ToolMatcher::glob("fs").unwrap()
        );
        // glob and regex with identical source differ in semantics, so they
        // must not collapse to equal (a `.` is literal under glob, any-char
        // under regex).
        assert_ne!(
            ToolMatcher::glob("a.b").unwrap(),
            ToolMatcher::regex("a.b").unwrap()
        );
    }
}
