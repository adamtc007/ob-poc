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

// ─── Vehicle Type Mapping ─────────────────────────────────────────────────
//
// Maps structure nouns to canonical vehicle types for two-axis macro selection.
// Ordered most-specific first — first match wins.

static VEHICLE_TYPE_MAP: &[(&str, &str)] = &[
    // Luxembourg
    ("sicav", "sicav"),
    ("ucits", "sicav"),
    ("raif", "raif"),
    ("scsp", "scsp"),
    ("slp", "scsp"),
    // Ireland
    ("qiaif", "aif-icav"),
    ("riaif", "aif-icav"),
    ("icav", "icav"),
    // UK
    ("oeic", "oeic"),
    ("unit trust", "aut"),
    ("aut", "aut"),
    ("acs", "acs"),
    ("ltaf", "ltaf"),
    ("long-term asset fund", "ltaf"),
    ("llp", "manager-llp"),
    // US
    ("etf", "etf"),
    ("closed-end", "40act-closed-end"),
    ("closed end", "40act-closed-end"),
    ("open-end", "40act-open-end"),
    ("mutual fund", "40act-open-end"),
    ("40-act", "40act-open-end"),
    ("40 act", "40act-open-end"),
    ("delaware", "delaware-lp"),
    // Cross-jurisdiction (resolved with jurisdiction context)
    ("hedge", "hedge"),
    ("pe", "pe"),
    ("private equity", "pe"),
    ("aif", "aif"),
    ("lp", "lp"),
];

/// Maps vehicle types that uniquely identify a jurisdiction.
/// Used as fallback when jurisdiction is not explicitly stated.
static VEHICLE_JURISDICTION_MAP: &[(&str, &str)] = &[
    ("sicav", "LU"),
    ("raif", "LU"),
    ("scsp", "LU"),
    ("icav", "IE"),
    ("aif-icav", "IE"),
    ("oeic", "UK"),
    ("aut", "UK"),
    ("acs", "UK"),
    ("ltaf", "UK"),
    ("manager-llp", "UK"),
    ("etf", "US"),
    ("40act-open-end", "US"),
    ("40act-closed-end", "US"),
    ("delaware-lp", "US"),
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
    /// Canonical vehicle type derived from structure nouns (e.g., "sicav", "oeic", "etf").
    /// Used for two-axis macro selection, not for scoring.
    pub vehicle_type: Option<String>,
    /// Action stem extracted from the utterance (e.g., "create", "list", "update").
    /// Used as a pre-filter to narrow candidate verbs within the active workspace.
    pub action_stem: Option<String>,
    /// Query direction for ownership/control queries (upward/downward/full).
    pub query_direction: Option<String>,
    /// Relationship type for ownership/control queries (ownership/control/all).
    pub relationship_type: Option<String>,
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

    // 6. Vehicle type extraction (from structure nouns, most-specific-first)
    signals.vehicle_type = extract_vehicle_type(&lower);

    // 6b. Vehicle-implies-jurisdiction fallback: if no explicit jurisdiction
    // was detected but the vehicle type uniquely identifies one, infer it.
    if signals.jurisdiction.is_none() {
        if let Some(ref vt) = signals.vehicle_type {
            if let Some((_, jur)) = VEHICLE_JURISDICTION_MAP.iter().find(|(v, _)| *v == vt.as_str())
            {
                signals.jurisdiction = Some(jur.to_string());
            }
        }
    }

    // 7. Action stem extraction
    signals.action_stem = extract_action_stem(&lower);

    // 8. Query direction extraction (ownership/control queries)
    signals.query_direction = extract_query_direction(&lower);

    // 8. Relationship type extraction (ownership vs control)
    signals.relationship_type = extract_relationship_type(&lower);

    // 9. Derived compound signals
    signals.has_jurisdiction_structure_pair =
        signals.jurisdiction.is_some() && !signals.structure_nouns.is_empty();

    signals.has_multi_noun_workflow = {
        let total_nouns = signals.structure_nouns.len() + signals.phase_nouns.len();
        total_nouns >= 2
    };

    signals
}

/// Extract the action stem from an utterance.
///
/// Maps natural language verbs to canonical DSL action stems. The stem
/// matches the first segment of verb FQNs: `domain.{stem}-topic`.
/// Used as a pre-filter to narrow candidate verbs within the active workspace.
///
/// Returns `None` if no recognizable action stem is found (falls through
/// to embedding search).
pub fn extract_action_stem(utterance: &str) -> Option<String> {
    // Ordered: multi-word phrases first, then single words.
    // Each entry is (user_phrase, canonical_stem).
    const STEM_MAP: &[(&str, &str)] = &[
        // Multi-word (check first)
        ("set up", "create"),
        ("sign off", "approve"),
        ("close out", "close"),
        ("kick off", "create"),
        ("spin up", "create"),
        ("open a", "create"),
        ("open the", "read"),
        ("look up", "find"),
        ("pull up", "read"),
        ("mark as", "mark"),
        ("run a", "run"),
        ("run the", "run"),
        // Single-word stems (exact DSL stems)
        ("create", "create"),
        ("list", "list"),
        ("show", "list"),
        ("display", "list"),
        ("read", "read"),
        ("get", "get"),
        ("view", "read"),
        ("update", "update"),
        ("modify", "update"),
        ("change", "update"),
        ("edit", "update"),
        ("delete", "delete"),
        ("remove", "remove"),
        ("add", "add"),
        ("assign", "assign"),
        ("set", "set"),
        ("trace", "trace"),
        ("find", "find"),
        ("search", "search"),
        ("check", "check"),
        ("verify", "verify"),
        ("validate", "validate"),
        ("run", "run"),
        ("compute", "compute"),
        ("calculate", "compute"),
        ("import", "import"),
        ("export", "export"),
        ("load", "load"),
        ("record", "record"),
        ("mark", "mark"),
        ("link", "link"),
        ("unlink", "remove"),
        ("approve", "approve"),
        ("reject", "reject"),
        ("close", "close"),
        ("open", "create"),
        ("start", "create"),
        ("begin", "create"),
        ("activate", "activate"),
        ("deactivate", "deactivate"),
        ("suspend", "suspend"),
        ("resume", "activate"),
        ("define", "define"),
        ("configure", "set"),
        ("ensure", "ensure"),
        ("resolve", "resolve"),
        ("escalate", "escalate"),
        ("solicit", "solicit"),
        ("request", "solicit"),
        ("submit", "submit"),
        ("fetch", "fetch"),
        ("sync", "sync"),
        ("reconcile", "reconcile"),
        ("distribute", "distribute"),
        ("transfer", "transfer"),
        ("waive", "waive"),
    ];

    for (phrase, stem) in STEM_MAP {
        if contains_phrase(utterance, phrase) {
            return Some(stem.to_string());
        }
    }
    None
}

/// Extract query direction for ownership/control queries.
///
/// - "upward" = who owns/controls this entity (tracing to parents/UBOs)
/// - "downward" = what does this entity own/control (tracing to subsidiaries)
/// - "full" = complete structure / graph / all relationships
pub fn extract_query_direction(utterance: &str) -> Option<String> {
    // Upward patterns: "who owns X", "shareholders", "parent", "UBOs"
    const UPWARD: &[&str] = &[
        "who owns",
        "who controls",
        "owners of",
        "shareholders",
        "parent",
        "beneficial owner",
        "beneficial owners",
        "ubo",
        "ubos",
        "list owners",
        "show owners",
        "identify ubos",
        "controllers of",
        "who has control",
    ];
    // Downward patterns: "what does X own", "subsidiaries", "controlled by"
    const DOWNWARD: &[&str] = &[
        "what does",
        "subsidiaries",
        "children",
        "owned by",
        "controlled entities",
        "list owned",
        "list controlled",
        "holdings",
    ];
    // Full patterns: "complete structure", "entire graph", "all relationships"
    const FULL: &[&str] = &[
        "structure",
        "graph",
        "full chain",
        "complete chain",
        "entire",
        "all relationships",
        "ownership structure",
        "control structure",
        "map",
        "trace chains",
        "trace all",
        "gaps",
        "positions",
    ];

    // Check longest matches first (full > upward > downward)
    for p in FULL {
        if contains_phrase(utterance, p) {
            return Some("full".to_string());
        }
    }
    for p in UPWARD {
        if contains_phrase(utterance, p) {
            return Some("upward".to_string());
        }
    }
    for p in DOWNWARD {
        if contains_phrase(utterance, p) {
            return Some("downward".to_string());
        }
    }
    None
}

/// Extract relationship type for ownership/control queries.
///
/// - "ownership" = shareholding, equity, economic interest
/// - "control" = board, voting, management, PSC
/// - "all" = both ownership and control vectors
pub fn extract_relationship_type(utterance: &str) -> Option<String> {
    const OWNERSHIP: &[&str] = &[
        "ownership",
        "shareholder",
        "shareholders",
        "shareholding",
        "equity",
        "economic interest",
        "who owns",
        "owners",
        "owned",
        "beneficial owner",
        "ubo",
    ];
    const CONTROL: &[&str] = &[
        "control",
        "board",
        "voting",
        "management",
        "psc",
        "governance",
        "who controls",
        "controllers",
    ];
    // "all" is the default when both or neither are present — detected by absence
    // of specific type cues, or explicit "all" / "both" / "complete"

    let has_ownership = OWNERSHIP.iter().any(|p| contains_phrase(utterance, p));
    let has_control = CONTROL.iter().any(|p| contains_phrase(utterance, p));

    match (has_ownership, has_control) {
        (true, false) => Some("ownership".to_string()),
        (false, true) => Some("control".to_string()),
        (true, true) => Some("all".to_string()),
        (false, false) => None, // no relationship type signal — selector will use default
    }
}

/// Extract canonical vehicle type from an utterance.
///
/// Uses `VEHICLE_TYPE_MAP` ordered most-specific-first. Prefers multi-word
/// matches (longer aliases) over single-word matches.
pub fn extract_vehicle_type(utterance: &str) -> Option<String> {
    // Sort by alias length descending so "unit trust" beats "aut"
    let mut sorted: Vec<_> = VEHICLE_TYPE_MAP.iter().collect();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (alias, canonical) in sorted {
        if contains_phrase(utterance, alias) {
            return Some(canonical.to_string());
        }
    }
    None
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

    // --- Query direction extraction ---

    #[test]
    fn test_query_direction_upward() {
        assert_eq!(extract_query_direction("who owns this company"), Some("upward".into()));
        assert_eq!(extract_query_direction("show me the shareholders"), Some("upward".into()));
        assert_eq!(extract_query_direction("who are the beneficial owners"), Some("upward".into()));
        assert_eq!(extract_query_direction("identify ubos for this entity"), Some("upward".into()));
    }

    #[test]
    fn test_query_direction_downward() {
        assert_eq!(extract_query_direction("what does Allianz own"), Some("downward".into()));
        assert_eq!(extract_query_direction("show subsidiaries"), Some("downward".into()));
        assert_eq!(extract_query_direction("list owned entities"), Some("downward".into()));
    }

    #[test]
    fn test_query_direction_full() {
        assert_eq!(extract_query_direction("show me the ownership structure"), Some("full".into()));
        assert_eq!(extract_query_direction("build the control graph"), Some("full".into()));
        assert_eq!(extract_query_direction("trace all chains"), Some("full".into()));
        assert_eq!(extract_query_direction("analyze ownership gaps"), Some("full".into()));
    }

    #[test]
    fn test_query_direction_none() {
        assert_eq!(extract_query_direction("create a fund"), None);
        assert_eq!(extract_query_direction("assign depositary"), None);
    }

    // --- Relationship type extraction ---

    #[test]
    fn test_relationship_type_ownership() {
        assert_eq!(extract_relationship_type("who owns this"), Some("ownership".into()));
        assert_eq!(extract_relationship_type("show shareholding"), Some("ownership".into()));
    }

    #[test]
    fn test_relationship_type_control() {
        assert_eq!(extract_relationship_type("who controls this"), Some("control".into()));
        assert_eq!(extract_relationship_type("show board members"), Some("control".into()));
        assert_eq!(extract_relationship_type("list governance controllers"), Some("control".into()));
    }

    #[test]
    fn test_relationship_type_both() {
        assert_eq!(
            extract_relationship_type("show ownership and control relationships"),
            Some("all".into())
        );
    }

    #[test]
    fn test_relationship_type_none() {
        assert_eq!(extract_relationship_type("create a fund"), None);
        assert_eq!(extract_relationship_type("assign depositary"), None);
    }

    // --- Combined signals for UBO utterances ---

    #[test]
    fn test_ubo_utterance_signals() {
        let s = extract_compound_signals("who are the beneficial owners of this company");
        assert_eq!(s.query_direction.as_deref(), Some("upward"));
        assert_eq!(s.relationship_type.as_deref(), Some("ownership"));
    }

    #[test]
    fn test_control_query_signals() {
        let s = extract_compound_signals("who controls this entity through the board");
        assert_eq!(s.query_direction.as_deref(), Some("upward"));
        assert_eq!(s.relationship_type.as_deref(), Some("control"));
    }

    #[test]
    fn test_full_structure_signals() {
        let s = extract_compound_signals("show me the complete ownership structure");
        assert_eq!(s.query_direction.as_deref(), Some("full"));
        assert_eq!(s.relationship_type.as_deref(), Some("ownership"));
    }

    // --- Action stem extraction ---

    #[test]
    fn test_action_stem_create() {
        assert_eq!(extract_action_stem("create a fund"), Some("create".into()));
        assert_eq!(extract_action_stem("set up a new CBU"), Some("create".into()));
        assert_eq!(extract_action_stem("open a KYC case"), Some("create".into()));
        assert_eq!(extract_action_stem("spin up a new entity"), Some("create".into()));
    }

    #[test]
    fn test_action_stem_list() {
        assert_eq!(extract_action_stem("list all owners"), Some("list".into()));
        assert_eq!(extract_action_stem("show me the shareholders"), Some("list".into()));
        assert_eq!(extract_action_stem("display the requirements"), Some("list".into()));
    }

    #[test]
    fn test_action_stem_update() {
        assert_eq!(extract_action_stem("update the entity address"), Some("update".into()));
        assert_eq!(extract_action_stem("modify the trading profile"), Some("update".into()));
        assert_eq!(extract_action_stem("change the risk rating"), Some("update".into()));
    }

    #[test]
    fn test_action_stem_trace() {
        assert_eq!(extract_action_stem("trace the ownership chain"), Some("trace".into()));
    }

    #[test]
    fn test_action_stem_none() {
        // Practitioner slang without clear action stem
        assert_eq!(extract_action_stem("chase the passport"), None);
        assert_eq!(extract_action_stem("ISDA terms for counterparty"), None);
    }

    #[test]
    fn test_action_stem_in_compound_signals() {
        let s = extract_compound_signals("create a new fund in Luxembourg");
        assert_eq!(s.action_stem.as_deref(), Some("create"));

        let s = extract_compound_signals("list outstanding KYC requirements");
        assert_eq!(s.action_stem.as_deref(), Some("list"));

        let s = extract_compound_signals("trace the ownership chain");
        assert_eq!(s.action_stem.as_deref(), Some("trace"));
    }
}
