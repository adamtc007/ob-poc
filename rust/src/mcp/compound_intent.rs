//! Compound Intent — Shared Feature Extraction for Scenario & Macro Tiers
//!
//! Extracts compound signals from utterances to distinguish multi-verb journeys
//! (e.g., "Onboard a Luxembourg SICAV") from single-verb commands
//! (e.g., "Create an umbrella fund").
//!
//! Used by:
//! - ScenarioIndex (Tier -2A): gate G1 requires compound signals present
//! - ECIR short-circuit: suppressed when compound signals detected
//! - MacroIndex (Tier -2B): jurisdiction/structure hints improve scoring

use std::collections::HashSet;

// ─── Configuration ─────────────────────────────────────────────────────────

/// Compound outcome verbs that signal multi-step intent.
const COMPOUND_ACTIONS: &[&str] = &[
    "onboard",
    "set up",
    "establish",
    "spin up",
    "configure",
    "do the",
    "run the",
    "complete the",
    "prepare",
    "initiate",
    "launch",
    "build out",
    "stand up",
    "screen",
];

/// Structure nouns that indicate fund/entity type (domain-specific).
const STRUCTURE_NOUNS: &[&str] = &[
    "sicav",
    "icav",
    "ucits",
    "aif",
    "raif",
    "eltif",
    "lp",
    "llp",
    "slp",
    "sca",
    "sarl",
    "fund",
    "sub-fund",
    "subfund",
    "compartment",
    "umbrella",
    "feeder",
    "master",
    "spv",
    "vehicle",
    "sub-funds",
    // UK structure types
    "oeic",
    "aut",
    "unit trust",
    "acs",
    "ltaf",
    "long-term asset fund",
    // US structure types
    "etf",
    "40-act",
    "40 act",
    "closed-end",
    "open-end",
    "mutual fund",
    "delaware",
    // PE / hedge
    "pe",
    "private equity",
    "hedge",
    "scsp",
    "qiaif",
    "riaif",
];

/// Phase nouns that indicate workflow stages.
const PHASE_NOUNS: &[&str] = &[
    "kyc",
    "screening",
    "due diligence",
    "onboarding",
    "compliance",
    "aml",
    "cdd",
    "edd",
    "mandate",
    "trading profile",
    "custody",
    "settlement",
    "documentation",
    "approval",
    "case",
    "document",
    "documents",
    "identity",
    "sanctions",
    "pep",
    "adverse media",
];

/// Quantifier patterns that suggest multi-entity scope.
const QUANTIFIER_PATTERNS: &[&str] = &[
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "multiple",
    "several",
    "all",
    "each",
    "every",
    "both",
    "sub-funds",
    "subfunds",
    "compartments",
    "entities",
    "parties",
    "roles",
    "accounts",
];

// ─── Jurisdiction Mapping ──────────────────────────────────────────────────

/// Map of jurisdiction aliases → ISO 2-letter codes.
/// Canonical list; add new entries here to extend jurisdiction detection.
static JURISDICTION_MAP: &[(&str, &str)] = &[
    // Luxembourg
    ("luxembourg", "LU"),
    ("luxemburg", "LU"),
    ("lux", "LU"),
    ("lu", "LU"),
    // Ireland
    ("ireland", "IE"),
    ("irish", "IE"),
    ("ie", "IE"),
    // United Kingdom
    ("united kingdom", "UK"),
    ("uk", "UK"),
    ("british", "UK"),
    ("england", "UK"),
    // United States
    ("united states", "US"),
    ("us", "US"),
    ("usa", "US"),
    ("american", "US"),
    // Germany
    ("germany", "DE"),
    ("german", "DE"),
    ("de", "DE"),
    // France
    ("france", "FR"),
    ("french", "FR"),
    ("fr", "FR"),
    // Cayman Islands
    ("cayman", "KY"),
    ("cayman islands", "KY"),
    ("ky", "KY"),
    // Singapore
    ("singapore", "SG"),
    ("sg", "SG"),
    // Hong Kong
    ("hong kong", "HK"),
    ("hk", "HK"),
    // Switzerland
    ("switzerland", "CH"),
    ("swiss", "CH"),
    ("ch", "CH"),
    // Jersey
    ("jersey", "JE"),
    ("je", "JE"),
    // Guernsey
    ("guernsey", "GG"),
    ("gg", "GG"),
];

// ─── Core Types ────────────────────────────────────────────────────────────

/// Extracted compound signals from an utterance.
///
/// These signals determine whether an utterance is a single-verb command
/// or a multi-step journey request. The ScenarioIndex (Tier -2A) requires
/// at least one compound signal present (gate G1).
#[derive(Debug, Clone, Default)]
pub struct CompoundSignals {
    /// Whether a compound action verb was detected ("onboard", "set up", etc.)
    pub has_compound_action: bool,
    /// The specific compound action matched, if any.
    pub compound_action: Option<String>,
    /// Detected jurisdiction ISO code (LU, IE, UK, etc.)
    pub jurisdiction: Option<String>,
    /// Domain-specific structure nouns found ("sicav", "icav", "lp", etc.)
    pub structure_nouns: Vec<String>,
    /// Workflow phase nouns found ("kyc", "screening", "mandate", etc.)
    pub phase_nouns: Vec<String>,
    /// Whether a quantifier suggesting multi-entity scope was detected.
    pub has_quantifier: bool,
    /// Compound: both jurisdiction AND structure noun present.
    pub has_jurisdiction_structure_pair: bool,
    /// Compound: multiple phase/structure nouns indicating a workflow.
    pub has_multi_noun_workflow: bool,
}

impl CompoundSignals {
    /// Returns true if ANY compound signal is present.
    ///
    /// This is the gate G1 check for the ScenarioIndex — if no compound
    /// signals exist, the utterance should be resolved at ECIR (single verb).
    pub fn has_any(&self) -> bool {
        self.has_compound_action
            || self.has_jurisdiction_structure_pair
            || self.has_multi_noun_workflow
            || self.has_quantifier
    }

    /// Strength score for compound signals (higher = more likely composite).
    /// Used to decide whether to suppress ECIR short-circuit.
    pub fn strength(&self) -> u32 {
        let mut s = 0u32;
        if self.has_compound_action {
            s += 2;
        }
        if self.jurisdiction.is_some() {
            s += 1;
        }
        if !self.structure_nouns.is_empty() {
            s += 1;
        }
        if !self.phase_nouns.is_empty() {
            s += 1;
        }
        if self.has_quantifier {
            s += 1;
        }
        if self.has_jurisdiction_structure_pair {
            s += 2;
        }
        if self.has_multi_noun_workflow {
            s += 1;
        }
        s
    }
}

// ─── Extraction Functions ──────────────────────────────────────────────────

/// Extract compound signals from an utterance.
///
/// This is the primary entry point. Performs all signal detection in a single
/// pass over the normalized utterance.
pub fn extract_compound_signals(utterance: &str) -> CompoundSignals {
    let lower = utterance.to_lowercase();
    let mut signals = CompoundSignals::default();

    // 1. Compound action detection
    for action in COMPOUND_ACTIONS {
        if contains_phrase(&lower, action) {
            signals.has_compound_action = true;
            signals.compound_action = Some(action.to_string());
            break;
        }
    }

    // 2. Jurisdiction extraction
    signals.jurisdiction = extract_jurisdiction(&lower);

    // 3. Structure nouns
    let mut found_structure: Vec<String> = Vec::new();
    for noun in STRUCTURE_NOUNS {
        if contains_word(&lower, noun) {
            found_structure.push(noun.to_string());
        }
    }
    signals.structure_nouns = found_structure;

    // 4. Phase nouns
    let mut found_phase: Vec<String> = Vec::new();
    for noun in PHASE_NOUNS {
        if contains_phrase(&lower, noun) {
            found_phase.push(noun.to_string());
        }
    }
    signals.phase_nouns = found_phase;

    // 5. Quantifier detection
    for pattern in QUANTIFIER_PATTERNS {
        if contains_word(&lower, pattern) {
            signals.has_quantifier = true;
            break;
        }
    }

    // 6. Derived compound signals
    signals.has_jurisdiction_structure_pair =
        signals.jurisdiction.is_some() && !signals.structure_nouns.is_empty();

    signals.has_multi_noun_workflow = {
        let total_nouns = signals.structure_nouns.len() + signals.phase_nouns.len();
        total_nouns >= 2
    };

    signals
}

/// Extract jurisdiction ISO code from an utterance.
///
/// Returns the FIRST matching jurisdiction. Uses word-boundary checks to
/// avoid false positives (e.g., "us" in "focus").
pub fn extract_jurisdiction(utterance: &str) -> Option<String> {
    let lower = utterance.to_lowercase();

    // Check multi-word aliases first (longer matches take priority)
    let mut sorted_aliases: Vec<_> = JURISDICTION_MAP.iter().collect();
    sorted_aliases.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (alias, code) in sorted_aliases {
        // Short aliases (2 chars) require strict word-boundary matching
        // to avoid false positives like "us" in "focus" or "discuss"
        if alias.len() <= 2 {
            if contains_strict_word(&lower, alias) {
                return Some(code.to_string());
            }
        } else if contains_phrase(&lower, alias) {
            return Some(code.to_string());
        }
    }

    None
}

// ─── Helper Functions ──────────────────────────────────────────────────────

/// Check if `haystack` contains `phrase` with word boundaries.
fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    let mut search_from = 0;
    while let Some(pos) = haystack[search_from..].find(phrase) {
        let start = search_from + pos;
        let end = start + phrase.len();

        let left_ok = start == 0 || !haystack.as_bytes()[start - 1].is_ascii_alphanumeric();
        let right_ok = end == haystack.len() || !haystack.as_bytes()[end].is_ascii_alphanumeric();

        if left_ok && right_ok {
            return true;
        }
        search_from = start + 1;
    }
    false
}

/// Check if `haystack` contains `word` as a standalone word.
/// Same as `contains_phrase` for single words.
fn contains_word(haystack: &str, word: &str) -> bool {
    contains_phrase(haystack, word)
}

/// Strict word boundary check for very short words (2 chars).
/// Requires whitespace or string boundaries on both sides to avoid
/// false positives like "us" in "focus", "discuss", "use".
fn contains_strict_word(haystack: &str, word: &str) -> bool {
    for w in haystack.split_whitespace() {
        if w == word {
            return true;
        }
    }
    false
}

/// Return the set of known jurisdiction codes.
pub fn known_jurisdiction_codes() -> HashSet<String> {
    JURISDICTION_MAP
        .iter()
        .map(|(_, code)| code.to_string())
        .collect()
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- Compound action detection ---

    #[test]
    fn test_compound_action_onboard() {
        let s = extract_compound_signals("Onboard a Luxembourg SICAV");
        assert!(s.has_compound_action);
        assert_eq!(s.compound_action.as_deref(), Some("onboard"));
    }

    #[test]
    fn test_compound_action_set_up() {
        let s = extract_compound_signals("Set up structure for Lux SICAV");
        assert!(s.has_compound_action);
        assert_eq!(s.compound_action.as_deref(), Some("set up"));
    }

    #[test]
    fn test_compound_action_establish() {
        let s = extract_compound_signals("Establish an Irish ICAV");
        assert!(s.has_compound_action);
        assert_eq!(s.compound_action.as_deref(), Some("establish"));
    }

    #[test]
    fn test_no_compound_action() {
        let s = extract_compound_signals("create umbrella fund");
        assert!(!s.has_compound_action);
        assert!(s.compound_action.is_none());
    }

    // --- Jurisdiction extraction ---

    #[test]
    fn test_jurisdiction_luxembourg() {
        assert_eq!(extract_jurisdiction("Luxembourg SICAV"), Some("LU".into()));
        assert_eq!(extract_jurisdiction("Lux fund"), Some("LU".into()));
    }

    #[test]
    fn test_jurisdiction_ireland() {
        assert_eq!(extract_jurisdiction("Irish ICAV"), Some("IE".into()));
        assert_eq!(
            extract_jurisdiction("Ireland fund setup"),
            Some("IE".into())
        );
    }

    #[test]
    fn test_jurisdiction_uk() {
        assert_eq!(extract_jurisdiction("UK fund structure"), Some("UK".into()));
    }

    #[test]
    fn test_jurisdiction_none() {
        assert_eq!(extract_jurisdiction("create a fund"), None);
    }

    #[test]
    fn test_jurisdiction_no_false_positive_us() {
        // "us" should not match inside "focus" or "discuss"
        assert_eq!(extract_jurisdiction("focus on this fund"), None);
        assert_eq!(extract_jurisdiction("discuss the fund structure"), None);
    }

    #[test]
    fn test_jurisdiction_us_standalone() {
        assert_eq!(extract_jurisdiction("US fund structure"), Some("US".into()));
    }

    // --- Structure nouns ---

    #[test]
    fn test_structure_nouns_sicav() {
        let s = extract_compound_signals("Onboard a Luxembourg SICAV with three sub-funds");
        assert!(s.structure_nouns.contains(&"sicav".to_string()));
        assert!(s.structure_nouns.contains(&"sub-funds".to_string()));
    }

    #[test]
    fn test_structure_nouns_icav() {
        let s = extract_compound_signals("Set up an Irish ICAV");
        assert!(s.structure_nouns.contains(&"icav".to_string()));
    }

    #[test]
    fn test_structure_nouns_lp() {
        let s = extract_compound_signals("Establish a Cayman LP");
        assert!(s.structure_nouns.contains(&"lp".to_string()));
    }

    // --- Phase nouns ---

    #[test]
    fn test_phase_nouns_kyc() {
        let s = extract_compound_signals("Complete KYC screening for the fund");
        assert!(s.phase_nouns.contains(&"kyc".to_string()));
        assert!(s.phase_nouns.contains(&"screening".to_string()));
    }

    // --- Quantifiers ---

    #[test]
    fn test_quantifier_three_subfunds() {
        let s = extract_compound_signals("Onboard a SICAV with three sub-funds");
        assert!(s.has_quantifier);
    }

    #[test]
    fn test_quantifier_all_roles() {
        let s = extract_compound_signals("assign all roles to the fund");
        assert!(s.has_quantifier);
    }

    #[test]
    fn test_no_quantifier() {
        let s = extract_compound_signals("create a fund");
        assert!(!s.has_quantifier);
    }

    // --- Compound derived signals ---

    #[test]
    fn test_jurisdiction_structure_pair() {
        let s = extract_compound_signals("Onboard a Luxembourg SICAV");
        assert!(s.has_jurisdiction_structure_pair);
        assert_eq!(s.jurisdiction.as_deref(), Some("LU"));
        assert!(s.structure_nouns.contains(&"sicav".to_string()));
    }

    #[test]
    fn test_no_jurisdiction_structure_pair() {
        let s = extract_compound_signals("create a fund");
        assert!(!s.has_jurisdiction_structure_pair);
    }

    #[test]
    fn test_multi_noun_workflow() {
        let s = extract_compound_signals("Run KYC screening and due diligence");
        assert!(s.has_multi_noun_workflow);
    }

    // --- has_any / strength ---

    #[test]
    fn test_has_any_compound() {
        let s = extract_compound_signals("Onboard a Luxembourg SICAV with three sub-funds");
        assert!(s.has_any());
        assert!(s.strength() >= 4);
    }

    #[test]
    fn test_has_any_false_for_simple() {
        let s = extract_compound_signals("create a fund");
        // "fund" IS a structure noun, so structure_nouns is non-empty
        // but has_any checks for compound_action OR pair OR multi-noun OR quantifier
        assert!(!s.has_compound_action);
        // "create a fund" has only "fund" as structure noun (1 noun total)
        // so has_jurisdiction_structure_pair = false (no jurisdiction)
        // has_multi_noun_workflow = false (only 1 noun)
        // has_quantifier = false
        assert!(!s.has_any());
    }

    #[test]
    fn test_full_compound_utterance() {
        let s = extract_compound_signals(
            "Onboard this Luxembourg SICAV with three sub-funds and complete KYC screening",
        );
        assert!(s.has_compound_action);
        assert_eq!(s.jurisdiction.as_deref(), Some("LU"));
        assert!(s.structure_nouns.contains(&"sicav".to_string()));
        assert!(s.structure_nouns.contains(&"sub-funds".to_string()));
        assert!(s.phase_nouns.contains(&"kyc".to_string()));
        assert!(s.phase_nouns.contains(&"screening".to_string()));
        assert!(s.has_quantifier);
        assert!(s.has_jurisdiction_structure_pair);
        assert!(s.has_multi_noun_workflow);
        assert!(s.strength() >= 6);
    }
}
