//! SagePreClassification — deterministic signals computed before any LLM call.
//!
//! Three signals, computed in order:
//! 1. ObservationPlane — from session context (stage_focus, goals)
//! 2. IntentPolarity — from clue word prefix scan of the raw utterance
//! 3. Domain hints — from NounIndex noun extraction on the raw utterance
//!
//! All three are deterministic and O(n) in utterance length. No embedding search,
//! no database access, no LLM calls.

use serde::{Deserialize, Serialize};

use super::context::SageContext;
use super::plane::ObservationPlane;
use super::polarity::IntentPolarity;

/// The deterministic pre-classification output.
///
/// This is used by both DeterministicSage (returns it directly as low-confidence OutcomeIntent)
/// and LlmSage (uses it to constrain the LLM prompt, reducing token count and hallucination).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagePreClassification {
    /// Observation plane determined from session context.
    /// Always deterministic — derived from stage_focus + goals + utterance signals.
    pub plane: ObservationPlane,

    /// Read/Write/Ambiguous determined from utterance clue words.
    pub polarity: IntentPolarity,

    /// The clue word that triggered the polarity classification.
    pub polarity_clue: Option<String>,

    /// Domain noun hints extracted from the utterance (e.g., ["fund", "deal", "schema"]).
    /// Empty if no known domain nouns found.
    pub domain_hints: Vec<String>,

    /// True if this utterance is a noun-only Structure exploration with no instance targeting.
    /// When true + polarity == Read → CoderEngine can be skipped (fast path eligible).
    pub sage_only: bool,
}

/// Classify the utterance against the session context without any LLM or DB calls.
///
/// ## Plane Classification Rules (deterministic)
///
/// | Condition | Plane |
/// |-----------|-------|
/// | stage_focus ∈ {semos-data-management, semos-data} AND no explicit instance targeting | Structure |
/// | stage_focus = semos-stewardship | Registry |
/// | Everything else | Instance |
///
/// Explicit instance targeting is detected when:
/// - A UUID appears in the utterance, OR
/// - An `@`-binding reference appears in the utterance
pub fn pre_classify(utterance: &str, ctx: &SageContext) -> SagePreClassification {
    let plane = classify_plane(utterance, ctx);
    let domain_hints = extract_domain_hints(utterance, plane);
    let (polarity, polarity_clue) = classify_polarity(utterance, &domain_hints);

    // sage_only: Read + Structure + no pending clarifications → safe for fast-path
    let sage_only = plane == ObservationPlane::Structure
        && polarity == IntentPolarity::Read
        && !has_explicit_instance_targeting(utterance);

    SagePreClassification {
        plane,
        polarity,
        polarity_clue,
        domain_hints,
        sage_only,
    }
}

/// Determine the ObservationPlane from stage_focus and instance targeting signals.
fn classify_plane(utterance: &str, ctx: &SageContext) -> ObservationPlane {
    let stage_focus = ctx.stage_focus.as_deref().unwrap_or("");

    // Stewardship focus → Registry plane
    if stage_focus.contains("stewardship") {
        return ObservationPlane::Registry;
    }

    // Data management focus → Structure plane (unless instance targeted)
    if stage_focus.contains("data-management")
        || stage_focus.contains("semos-data")
        || stage_focus == "data"
    {
        if !has_explicit_instance_targeting(utterance) {
            return ObservationPlane::Structure;
        }
    }

    // Check for explicit schema/structure vocabulary in utterance (even outside data-management focus)
    if has_structure_vocabulary(utterance) && !has_explicit_instance_targeting(utterance) {
        return ObservationPlane::Structure;
    }

    ObservationPlane::Instance
}

/// Returns true if the utterance contains explicit instance targeting signals.
///
/// Explicit instance targeting signals:
/// - UUID pattern in utterance (e.g., 550e8400-e29b-41d4-a716-446655440000)
/// - @-binding reference (e.g., @deal, @fund)
fn has_explicit_instance_targeting(utterance: &str) -> bool {
    // UUID pattern (simplified — 8-4-4-4-12 hex)
    if contains_uuid_pattern(utterance) {
        return true;
    }

    // @-binding reference
    if utterance.contains('@') {
        return true;
    }

    false
}

/// Returns true if the utterance contains structure/schema vocabulary.
fn has_structure_vocabulary(utterance: &str) -> bool {
    let lower = utterance.to_lowercase();
    let structure_words = [
        "schema",
        "structure",
        "entity type",
        "field",
        "attribute",
        "definition",
        "taxonomy",
        "type definition",
        "data model",
        "data structure",
    ];
    structure_words.iter().any(|w| lower.contains(w))
}

/// Detect UUID pattern (simplified: 8-4-4-4-12 hex characters with hyphens).
fn contains_uuid_pattern(s: &str) -> bool {
    // Look for the pattern xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    let bytes = s.as_bytes();
    let len = bytes.len();

    for i in 0..len {
        if i + 36 <= len {
            let slice = &s[i..i + 36];
            if is_uuid_pattern(slice) {
                return true;
            }
        }
    }
    false
}

fn is_uuid_pattern(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    // Check hyphens at positions 8, 13, 18, 23
    if b[8] != b'-' || b[13] != b'-' || b[18] != b'-' || b[23] != b'-' {
        return false;
    }
    // Check all other chars are hex digits
    for (i, &ch) in b.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            continue;
        }
        if !ch.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// Extract domain noun hints from the utterance using a lightweight keyword scan.
///
/// This is a fast heuristic — not the full NounIndex scan (that's too heavy for
/// the Sage's pre-classification phase). We extract the most common domain nouns.
fn classify_polarity(utterance: &str, domain_hints: &[String]) -> (IntentPolarity, Option<String>) {
    let (base_polarity, base_clue) = IntentPolarity::from_utterance(utterance);
    if base_polarity == IntentPolarity::Write {
        return (base_polarity, base_clue);
    }

    let lower = utterance.to_lowercase();
    let write_overrides = [
        "confirm",
        "reject",
        "switch to",
        "teach",
        "pull in",
        "open a",
        "request",
        "send out",
        "upload",
        "verify",
        "resolve",
        "enrich",
        "collect",
        "run ",
        "screen ",
        "undo",
        "zoom",
        "filter to",
        "waive",
        "kick off",
        "counter ",
        "cancel",
        "launch ",
        "establish ",
        "go back",
        "set up",
        "onboard ",
    ];
    if let Some(clue) = write_overrides.iter().find(|clue| lower.contains(**clue)) {
        return (IntentPolarity::Write, Some((*clue).to_string()));
    }

    let screening_domains = [
        "screening",
        "document",
        "session",
        "view",
        "agent",
        "kyc",
        "case",
    ];
    if screening_domains
        .iter()
        .any(|domain| domain_hints.iter().any(|hint| hint == domain))
        && (lower.contains("check")
            || lower.contains("review")
            || lower.contains("process")
            || lower.contains("handle")
            || lower.contains("validate")
            || lower.contains("verify"))
    {
        return (IntentPolarity::Write, Some("domain-override".to_string()));
    }

    if lower.starts_with("what's ")
        || lower.starts_with("where's ")
        || lower.starts_with("is ")
        || lower.starts_with("are ")
        || lower.starts_with("any ")
    {
        return (IntentPolarity::Read, Some("question".to_string()));
    }

    (base_polarity, base_clue)
}

fn extract_domain_hints(utterance: &str, plane: ObservationPlane) -> Vec<String> {
    let lower = utterance.to_lowercase();
    let mut hints = Vec::new();

    if plane == ObservationPlane::Structure {
        hints.push("struct".to_string());
    }

    let domain_keywords: &[(&str, &str)] = &[
        ("research mode", "agent"),
        ("teach the system", "agent"),
        ("what tools", "agent"),
        ("billing", "billing"),
        ("arrears", "billing"),
        ("bps", "billing"),
        ("bods", "bods"),
        ("booking principal", "booking-principal"),
        ("booking entity", "booking-principal"),
        ("kyc case", "case"),
        ("open a case", "case"),
        ("open a new kyc case", "case"),
        ("open a new case", "case"),
        ("new case", "case"),
        ("client group", "client-group"),
        ("this group", "client-group"),
        ("relationships in this group", "client-group"),
        ("psc register", "control"),
        ("controller", "control"),
        ("agreement", "contract"),
        ("rate card", "deal"),
        ("pricing", "deal"),
        ("timeline", "deal"),
        ("fee", "deal"),
        ("document", "document"),
        ("doc pack", "document"),
        ("passport", "document"),
        ("certificate of incorporation", "document"),
        ("uploaded", "document"),
        ("legal entity", "entity"),
        ("placeholder", "entity"),
        ("trust", "entity"),
        ("partnership", "entity"),
        ("gleif", "gleif"),
        ("lei", "gleif"),
        ("collect documents", "kyc"),
        ("kyc review", "kyc"),
        ("mandate", "mandate"),
        ("ownership", "ownership"),
        ("waterfall", "ownership"),
        ("party", "party"),
        ("sanctions", "screening"),
        ("pep", "screening"),
        ("adverse media", "screening"),
        ("ofac", "screening"),
        ("aml", "screening"),
        ("rba", "screening"),
        ("screening", "screening"),
        ("session", "session"),
        ("cluster", "session"),
        ("galaxy", "session"),
        ("persona", "session"),
        ("undo", "session"),
        ("switch to", "session"),
        ("load the", "session"),
        ("filter to", "session"),
        ("zoom", "view"),
        ("show me everything", "view"),
        ("go back", "view"),
        ("ubo", "ubo"),
        ("beneficial owner", "ubo"),
        ("trustee", "ubo"),
        ("settlor", "ubo"),
        ("nominee", "ubo"),
        ("deceased", "ubo"),
        ("fund", "fund"),
        ("cbu", "cbu"),
        ("client business unit", "cbu"),
        ("sicav", "fund"),
        ("icav", "fund"),
        ("subfund", "fund"),
        ("sub-fund", "fund"),
        ("umbrella", "fund"),
        // Deal domain
        ("deal", "deal"),
        ("mandate", "trading-profile"),
        ("trading profile", "trading-profile"),
        // Entity domain
        ("entity", "entity"),
        ("company", "entity"),
        ("person", "entity"),
        ("organization", "entity"),
        ("organisation", "entity"),
        // KYC domain
        ("kyc", "kyc"),
        ("case", "kyc"),
        ("screening", "screening"),
        ("ubo", "ubo"),
        ("beneficial owner", "ubo"),
        // Schema/structure domain
        ("schema", "schema"),
        ("attribute", "schema"),
        ("taxonomy", "schema"),
        ("data model", "schema"),
        // Registry domain
        ("changeset", "changeset"),
        ("governance", "governance"),
        ("registry", "registry"),
        ("semantic", "registry"),
        ("session", "session"),
        ("book", "session"),
        ("scope", "session"),
    ];

    for (keyword, domain) in domain_keywords {
        if lower.contains(keyword) {
            let domain_str = domain.to_string();
            if !hints.contains(&domain_str) {
                hints.push(domain_str);
            }
        }
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_focus(stage_focus: &str) -> SageContext {
        SageContext {
            session_id: None,
            stage_focus: Some(stage_focus.to_string()),
            goals: vec![],
            entity_kind: None,
            dominant_entity_name: None,
            last_intents: vec![],
        }
    }

    fn empty_ctx() -> SageContext {
        SageContext {
            session_id: None,
            stage_focus: None,
            goals: vec![],
            entity_kind: None,
            dominant_entity_name: None,
            last_intents: vec![],
        }
    }

    #[test]
    fn test_structure_plane_from_data_management_focus() {
        let ctx = ctx_with_focus("semos-data-management");
        let result = pre_classify("describe the deal schema", &ctx);
        assert_eq!(result.plane, ObservationPlane::Structure);
    }

    #[test]
    fn test_structure_plane_from_semos_data_focus() {
        let ctx = ctx_with_focus("semos-data");
        let result = pre_classify("list entity types", &ctx);
        assert_eq!(result.plane, ObservationPlane::Structure);
    }

    #[test]
    fn test_registry_plane_from_stewardship_focus() {
        let ctx = ctx_with_focus("semos-stewardship");
        let result = pre_classify("show pending changesets", &ctx);
        assert_eq!(result.plane, ObservationPlane::Registry);
    }

    #[test]
    fn test_instance_plane_by_default() {
        let ctx = empty_ctx();
        let result = pre_classify("create a new fund", &ctx);
        assert_eq!(result.plane, ObservationPlane::Instance);
    }

    #[test]
    fn test_instance_targeting_uuid_overrides_structure() {
        let ctx = ctx_with_focus("semos-data-management");
        let result = pre_classify("describe 550e8400-e29b-41d4-a716-446655440000", &ctx);
        // UUID present → instance targeting → Instance plane despite data-management focus
        assert_eq!(result.plane, ObservationPlane::Instance);
    }

    #[test]
    fn test_instance_targeting_at_binding() {
        let ctx = ctx_with_focus("semos-data-management");
        let result = pre_classify("show me @deal attributes", &ctx);
        assert_eq!(result.plane, ObservationPlane::Instance);
    }

    #[test]
    fn test_data_management_focus_ignores_selected_entity_kind_for_structure_reads() {
        let mut ctx = ctx_with_focus("semos-data-management");
        ctx.entity_kind = Some("deal".to_string());
        let result = pre_classify("show me documents", &ctx);
        assert_eq!(result.plane, ObservationPlane::Structure);
        assert!(result.sage_only);
    }

    #[test]
    fn test_structure_vocabulary_without_focus() {
        let ctx = empty_ctx();
        let result = pre_classify("what are the schema attributes for deal", &ctx);
        assert_eq!(result.plane, ObservationPlane::Structure);
    }

    #[test]
    fn test_read_polarity() {
        let ctx = empty_ctx();
        let result = pre_classify("show me all funds", &ctx);
        assert_eq!(result.polarity, IntentPolarity::Read);
    }

    #[test]
    fn test_sage_only_for_structure_read() {
        let ctx = ctx_with_focus("semos-data-management");
        let result = pre_classify("list entity types", &ctx);
        assert!(
            result.sage_only,
            "Structure + Read + no instance targeting → sage_only"
        );
    }

    #[test]
    fn test_not_sage_only_for_write() {
        let ctx = ctx_with_focus("semos-data-management");
        let result = pre_classify("create a new attribute", &ctx);
        assert!(!result.sage_only, "Write intent → not sage_only");
    }

    #[test]
    fn test_domain_hints_extracted() {
        let ctx = empty_ctx();
        let result = pre_classify("describe the deal schema attributes", &ctx);
        assert!(result.domain_hints.contains(&"deal".to_string()));
        assert!(result.domain_hints.contains(&"schema".to_string()));
    }

    #[test]
    fn test_uuid_detection() {
        assert!(contains_uuid_pattern(
            "show 550e8400-e29b-41d4-a716-446655440000 details"
        ));
        assert!(!contains_uuid_pattern("show the deal details"));
        assert!(!contains_uuid_pattern("not-a-uuid-at-all-xxxxxx"));
    }
}
