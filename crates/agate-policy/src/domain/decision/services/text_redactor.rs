use crate::domain::common::services::DomainService;
use crate::domain::decision::values::{PolicyDecision, SecretPattern};

// Re-exported from the value object, which owns the mask now that each pattern
// redacts itself (literal or regex).
pub use crate::domain::decision::values::secret_pattern::REDACTION_MASK;

/// Redacts emitted text by masking every occurrence of a configured secret
/// marker. Each [`SecretPattern`] applies itself (literal, ASCII
/// case-insensitive; or regex), so this only sequences them and reports the
/// overall verdict.
pub struct TextRedactor;

impl TextRedactor {
    /// Whether any pattern occurs in `text` — a detection without masking, for
    /// content that cannot be rewritten in place (e.g. a structured state
    /// payload).
    #[must_use]
    pub fn detects(patterns: &[SecretPattern], text: &str) -> bool {
        patterns.iter().any(|pattern| pattern.detects(text))
    }

    /// `RedactText` with the masked text if any pattern matched, else `Allow`.
    #[must_use]
    pub fn redact(patterns: &[SecretPattern], text: &str) -> PolicyDecision {
        let mut current = text.to_owned();
        let mut redacted = false;

        for pattern in patterns {
            let (next, hit) = pattern.mask(&current);
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
