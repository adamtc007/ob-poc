//! IntentPolarity — read vs write intent, determined from clue words.
//!
//! Polarity is computed deterministically from the first few words of the utterance.
//! It narrows verb candidates before any embedding search: Read → list/get/show verbs,
//! Write → create/update/delete verbs.

use serde::{Deserialize, Serialize};

/// Read-intent clue words — utterance starts with one of these → Read polarity.
pub const READ_CLUE_WORDS: &[&str] = &[
    "show",
    "list",
    "what",
    "who",
    "find",
    "trace",
    "describe",
    "get",
    "fetch",
    "display",
    "view",
    "search",
    "check",
    "tell",
    "how many",
    "how does",
    "which",
    "where",
    "when",
    "count",
    "summarize",
    "summary",
    "inspect",
    "explain",
    "look",
    "lookup",
    "query",
    "read",
    "report",
    "explore",
];

/// Write-intent clue words — utterance starts with one of these → Write polarity.
pub const WRITE_CLUE_WORDS: &[&str] = &[
    "create",
    "add",
    "make",
    "new",
    "set up",
    "setup",
    "build",
    "generate",
    "update",
    "edit",
    "change",
    "modify",
    "rename",
    "move",
    "set",
    "delete",
    "remove",
    "drop",
    "archive",
    "retire",
    "deprecate",
    "import",
    "sync",
    "load",
    "push",
    "publish",
    "deploy",
    "assign",
    "attach",
    "link",
    "register",
    "enroll",
    "onboard",
    "approve",
    "reject",
    "submit",
    "propose",
    "flag",
];

/// Ambiguous clue words — require more context to determine polarity.
pub const AMBIGUOUS_CLUE_WORDS: &[&str] = &[
    "run", "execute", "process", "handle", "manage", "review", "validate", "verify", "confirm",
    "analyze", "analyse",
];

/// Intent polarity extracted from the utterance.
///
/// This is the second most important signal (after ObservationPlane) for
/// narrowing the verb candidate set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntentPolarity {
    /// Read-only operations: list, get, show, find, describe, trace, etc.
    Read,
    /// Mutations: create, update, delete, import, assign, publish, etc.
    Write,
    /// Could be either — requires further analysis.
    Ambiguous,
}

impl IntentPolarity {
    /// Classify polarity from the utterance using prefix scan over clue words.
    ///
    /// Scans the first 5 words only (prefix scan is sufficient and fast).
    pub fn from_utterance(utterance: &str) -> (Self, Option<String>) {
        let lower = utterance.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();
        let prefix: String = words.iter().take(5).cloned().collect::<Vec<_>>().join(" ");

        // Check 2-word bigrams first (e.g., "how many", "set up")
        for i in 0..words.len().saturating_sub(1) {
            let bigram = format!("{} {}", words[i], words[i + 1]);
            if READ_CLUE_WORDS.contains(&bigram.as_str()) {
                return (IntentPolarity::Read, Some(bigram));
            }
            if WRITE_CLUE_WORDS.contains(&bigram.as_str()) {
                return (IntentPolarity::Write, Some(bigram));
            }
        }

        // Then check single words in prefix
        for word in prefix.split_whitespace() {
            if READ_CLUE_WORDS.contains(&word) {
                return (IntentPolarity::Read, Some(word.to_string()));
            }
            if WRITE_CLUE_WORDS.contains(&word) {
                return (IntentPolarity::Write, Some(word.to_string()));
            }
            if AMBIGUOUS_CLUE_WORDS.contains(&word) {
                return (IntentPolarity::Ambiguous, Some(word.to_string()));
            }
        }

        (IntentPolarity::Ambiguous, None)
    }

    /// Returns the canonical string key for logging and telemetry.
    pub fn as_str(&self) -> &'static str {
        match self {
            IntentPolarity::Read => "read",
            IntentPolarity::Write => "write",
            IntentPolarity::Ambiguous => "ambiguous",
        }
    }
}

impl std::fmt::Display for IntentPolarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_polarity() {
        let (pol, clue) = IntentPolarity::from_utterance("show me all the deals");
        assert_eq!(pol, IntentPolarity::Read);
        assert_eq!(clue.as_deref(), Some("show"));
    }

    #[test]
    fn test_write_polarity() {
        let (pol, clue) = IntentPolarity::from_utterance("create a new fund");
        assert_eq!(pol, IntentPolarity::Write);
        assert_eq!(clue.as_deref(), Some("create"));
    }

    #[test]
    fn test_bigram_clue() {
        let (pol, clue) = IntentPolarity::from_utterance("set up a luxembourg sicav");
        assert_eq!(pol, IntentPolarity::Write);
        assert_eq!(clue.as_deref(), Some("set up"));
    }

    #[test]
    fn test_ambiguous_polarity() {
        let (pol, _clue) = IntentPolarity::from_utterance("process the kyc case");
        assert_eq!(pol, IntentPolarity::Ambiguous);
    }

    #[test]
    fn test_unknown_defaults_to_ambiguous() {
        let (pol, clue) = IntentPolarity::from_utterance("the deal schema structure");
        assert_eq!(pol, IntentPolarity::Ambiguous);
        assert!(clue.is_none());
    }

    #[test]
    fn test_list_is_read() {
        let (pol, _) = IntentPolarity::from_utterance("list all entities in this fund");
        assert_eq!(pol, IntentPolarity::Read);
    }

    #[test]
    fn test_describe_is_read() {
        let (pol, _) = IntentPolarity::from_utterance("describe the deal schema");
        assert_eq!(pol, IntentPolarity::Read);
    }

    #[test]
    fn test_import_is_write() {
        let (pol, _) = IntentPolarity::from_utterance("import gleif corporate hierarchy");
        assert_eq!(pol, IntentPolarity::Write);
    }
}
