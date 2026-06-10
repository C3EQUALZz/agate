use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{PolicyDecision, SecretPattern};

/// The placeholder substituted for each matched secret.
pub const REDACTION_MASK: &str = "[REDACTED]";

/// Redacts emitted text by masking every occurrence of a configured secret
/// marker (ASCII case-insensitive).
pub struct TextRedactor;

impl TextRedactor {
    /// Whether any pattern occurs in `text` (ASCII case-insensitive) — a
    /// detection without masking, for content that cannot be rewritten in place
    /// (e.g. a structured state payload).
    #[must_use]
    pub fn detects(patterns: &[SecretPattern], text: &str) -> bool {
        let lowered = text.to_ascii_lowercase();
        patterns
            .iter()
            .any(|pattern| lowered.contains(&pattern.needle().to_ascii_lowercase()))
    }

    /// `RedactText` with the masked text if any pattern matched, else `Allow`.
    #[must_use]
    pub fn redact(patterns: &[SecretPattern], text: &str) -> PolicyDecision {
        let mut current = text.to_owned();
        let mut redacted = false;

        for pattern in patterns {
            let (next, hit) = mask_all(&current, pattern.needle());
            if hit {
                current = next;
                redacted = true;
            }
        }

        if redacted {
            PolicyDecision::RedactText(current)
        } else {
            PolicyDecision::Allow
        }
    }
}

impl DomainService for TextRedactor {}

/// Replace every ASCII-case-insensitive occurrence of `needle` in `haystack`
/// with [`REDACTION_MASK`]. Returns the result and whether anything matched.
///
/// ASCII lowercasing preserves byte length, so positions found in the lowered
/// copy index correctly back into the original.
fn mask_all(haystack: &str, needle: &str) -> (String, bool) {
    let lower_haystack = haystack.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();

    let mut result = String::with_capacity(haystack.len());
    let mut cursor = 0;
    let mut matched = false;

    while let Some(offset) = lower_haystack[cursor..].find(&lower_needle) {
        let start = cursor + offset;
        let end = start + lower_needle.len();
        result.push_str(&haystack[cursor..start]);
        result.push_str(REDACTION_MASK);
        cursor = end;
        matched = true;
    }
    result.push_str(&haystack[cursor..]);

    (result, matched)
}
