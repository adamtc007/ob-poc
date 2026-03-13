//! SagePreClassification — deterministic signals computed before any LLM call.
//!
//! Three signals, computed in order:
//! 1. ObservationPlane — from session context (stage_focus, goals)
//! 2. IntentPolarity — from clue word prefix scan of the raw utterance
//! 3. Domain hints — from NounIndex noun extraction on the raw utterance
//!
//! All three are deterministic and O(n) in utterance length. No embedding search,
//! no database access, no LLM calls.

use std::collections::HashMap;

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

    /// Strength of the winning domain signal.
    #[serde(default)]
    pub domain_score: i32,

    /// Strength of the runner-up domain signal.
    #[serde(default)]
    pub runner_up_domain_score: i32,

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
    let (domain_hints, domain_score, runner_up_domain_score) =
        extract_domain_hints(utterance, plane, ctx);
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
        domain_score,
        runner_up_domain_score,
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
    if (stage_focus.contains("data-management")
        || stage_focus.contains("semos-data")
        || stage_focus == "data")
        && !has_explicit_instance_targeting(utterance)
    {
        return ObservationPlane::Structure;
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

fn extract_domain_hints(
    utterance: &str,
    plane: ObservationPlane,
    ctx: &SageContext,
) -> (Vec<String>, i32, i32) {
    let lower = utterance.to_lowercase();
    let mut scores: HashMap<&'static str, i32> = HashMap::new();

    if plane == ObservationPlane::Structure {
        add_score(&mut scores, "struct", 8);
    }

    let strong_phrases: &[(&str, &str, i32)] = &[
        ("teach the system", "agent", 12),
        ("research mode", "agent", 10),
        ("what tools", "agent", 8),
        ("booking principal", "booking-principal", 12),
        ("booking entity", "booking-principal", 10),
        ("client group", "client-group", 12),
        ("this group", "client-group", 6),
        ("relationships in this group", "client-group", 10),
        ("kyc case", "case", 12),
        ("open a case", "case", 11),
        ("open a new kyc case", "case", 12),
        ("open a new case", "case", 11),
        ("new case", "case", 8),
        ("full kyc onboarding", "case", 11),
        ("full kyc review", "kyc", 10),
        ("collect documents", "kyc", 9),
        ("request identity documents", "kyc", 10),
        ("due diligence", "kyc", 8),
        ("adverse media", "screening", 12),
        ("politically exposed", "screening", 12),
        ("pep", "screening", 10),
        ("sanctions", "screening", 10),
        ("ofac", "screening", 10),
        ("screening", "screening", 9),
        ("beneficial owner", "ubo", 12),
        ("ownership structure", "ubo", 11),
        ("trace the ownership chain", "ubo", 10),
        ("ultimate beneficial owner", "ubo", 12),
        ("trustee", "ubo", 9),
        ("settlor", "ubo", 9),
        ("nominee", "ubo", 9),
        ("deceased", "ubo", 8),
        ("client business unit", "cbu", 12),
        ("transfer agent", "cbu", 9),
        ("custody services", "cbu", 9),
        ("fund accounting", "cbu", 8),
        ("nav calc", "cbu", 8),
        ("rate card", "deal", 10),
        ("deal timeline", "deal", 10),
        ("pricing", "deal", 8),
        ("fee line", "deal", 8),
        ("doc pack", "document", 10),
        ("passport", "document", 9),
        ("certificate of incorporation", "document", 11),
        ("legal entity", "entity", 10),
        ("placeholder", "entity", 8),
        ("limited partnership", "entity", 10),
        ("lei", "gleif", 10),
        ("gleif", "gleif", 10),
        ("ownership waterfall", "ownership", 10),
        ("ownership percentage", "ownership", 9),
        ("show me everything", "view", 10),
        ("go back", "view", 10),
        ("zoom", "view", 9),
        ("switch to", "session", 8),
        ("load the", "session", 8),
        ("galaxy", "session", 9),
        ("persona", "session", 9),
        ("irish icav", "struct", 12),
        ("lux sicav", "struct", 12),
        ("luxembourg sicav", "struct", 12),
        ("uk oeic", "struct", 12),
        ("40-act", "struct", 12),
        ("cross-border hedge fund", "struct", 11),
        ("pe fund structure", "struct", 10),
        ("fund structure", "struct", 8),
        ("share class", "fund", 9),
        ("sub-fund", "fund", 10),
        ("subfund", "fund", 10),
        ("umbrella", "fund", 9),
        ("changeset", "changeset", 10),
        ("governance", "governance", 10),
        ("registry", "registry", 10),
        ("semantic", "registry", 8),
    ];

    let weak_tokens: &[(&str, &str, i32)] = &[
        ("billing", "billing", 5),
        ("arrears", "billing", 5),
        ("bps", "billing", 5),
        ("bods", "bods", 6),
        ("controller", "control", 6),
        ("psc register", "control", 8),
        ("agreement", "contract", 4),
        ("document", "document", 5),
        ("uploaded", "document", 4),
        ("entity", "entity", 4),
        ("company", "entity", 4),
        ("person", "entity", 4),
        ("organization", "entity", 4),
        ("organisation", "entity", 4),
        ("trust", "entity", 4),
        ("fund", "fund", 5),
        ("cbu", "cbu", 7),
        ("deal", "deal", 6),
        ("timeline", "deal", 6),
        ("mandate", "trading-profile", 5),
        ("trading profile", "trading-profile", 8),
        ("kyc", "kyc", 5),
        ("case", "case", 4),
        ("ownership", "ownership", 6),
        ("party", "party", 5),
        ("schema", "schema", 6),
        ("attribute", "schema", 5),
        ("taxonomy", "schema", 5),
        ("data model", "schema", 6),
        ("book", "session", 4),
        ("scope", "session", 4),
    ];

    for (phrase, domain, weight) in strong_phrases {
        if lower.contains(phrase) {
            add_score(&mut scores, domain, *weight);
        }
    }
    for (token, domain, weight) in weak_tokens {
        if lower.contains(token) {
            add_score(&mut scores, domain, *weight);
        }
    }

    apply_action_domain_bias(&lower, &mut scores);
    apply_context_domain_bias(ctx, plane, &lower, &mut scores);
    apply_domain_precedence(&lower, &mut scores);

    let mut ranked: Vec<(&str, i32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let domain_score = ranked.first().map(|(_, score)| *score).unwrap_or_default();
    let runner_up_domain_score = ranked.get(1).map(|(_, score)| *score).unwrap_or_default();
    let hints = ranked
        .into_iter()
        .filter(|(_, score)| *score >= 4)
        .map(|(domain, _)| domain.to_string())
        .collect();

    (hints, domain_score, runner_up_domain_score)
}

fn add_score(scores: &mut HashMap<&'static str, i32>, domain: &'static str, delta: i32) {
    *scores.entry(domain).or_insert(0) += delta;
}

fn apply_action_domain_bias(lower: &str, scores: &mut HashMap<&'static str, i32>) {
    let first = lower.split_whitespace().next().unwrap_or("");
    match first {
        "open" | "start" | "request" => {
            if lower.contains("case") || lower.contains("kyc") {
                add_score(scores, "case", 4);
            }
            if lower.contains("document") || lower.contains("documents") {
                add_score(scores, "kyc", 3);
            }
        }
        "screen" | "check" => {
            if lower.contains("pep")
                || lower.contains("sanctions")
                || lower.contains("adverse media")
                || lower.contains("screening")
            {
                add_score(scores, "screening", 5);
            }
        }
        "assign" | "add" | "make" => {
            if lower.contains("role")
                || lower.contains("custodian")
                || lower.contains("transfer agent")
            {
                add_score(scores, "cbu", 4);
            }
        }
        "show" | "list" | "what" | "who" => {
            if lower.contains("ownership") || lower.contains("beneficial owner") {
                add_score(scores, "ubo", 3);
            }
            if lower.contains("share class") {
                add_score(scores, "fund", 3);
            }
        }
        "set" | "onboard" | "launch" | "create" => {
            if lower.contains("icav")
                || lower.contains("sicav")
                || lower.contains("oeic")
                || lower.contains("40-act")
                || lower.contains("fund structure")
            {
                add_score(scores, "struct", 5);
            }
        }
        _ => {}
    }
}

fn apply_context_domain_bias(
    ctx: &SageContext,
    plane: ObservationPlane,
    lower: &str,
    scores: &mut HashMap<&'static str, i32>,
) {
    if plane == ObservationPlane::Structure {
        add_score(scores, "struct", 2);
    }

    if let Some(stage_focus) = ctx.stage_focus.as_deref() {
        if stage_focus.contains("data-management") {
            add_score(scores, "struct", 2);
        }
        if stage_focus.contains("stewardship") {
            add_score(scores, "registry", 4);
            add_score(scores, "changeset", 3);
            add_score(scores, "governance", 3);
        }
        if stage_focus.contains("kyc") {
            add_score(scores, "kyc", 3);
            add_score(scores, "case", 2);
            add_score(scores, "screening", 2);
            add_score(scores, "ubo", 2);
        }
    }

    if let Some(kind) = ctx.entity_kind.as_deref() {
        match kind {
            "cbu" => add_score(scores, "cbu", 2),
            "deal" => add_score(scores, "deal", 2),
            "entity" => add_score(scores, "entity", 2),
            "client-group" => add_score(scores, "client-group", 2),
            _ => {}
        }
    }

    if let Some(last) = ctx.last_intents.last() {
        if contains_elliptical_reference(lower) {
            add_score(scores, leak_static(last.domain_concept.as_str()), 3);
        }
    }
}

fn contains_elliptical_reference(lower: &str) -> bool {
    ["this ", "that ", "it", "them", "one", "ones"]
        .iter()
        .any(|term| lower.contains(term))
}

fn leak_static(value: &str) -> &'static str {
    match value {
        "agent" => "agent",
        "billing" => "billing",
        "bods" => "bods",
        "booking-principal" => "booking-principal",
        "case" => "case",
        "cbu" => "cbu",
        "changeset" => "changeset",
        "client-group" => "client-group",
        "control" => "control",
        "contract" => "contract",
        "deal" => "deal",
        "document" => "document",
        "entity" => "entity",
        "fund" => "fund",
        "gleif" => "gleif",
        "governance" => "governance",
        "kyc" => "kyc",
        "ownership" => "ownership",
        "party" => "party",
        "registry" => "registry",
        "schema" => "schema",
        "screening" => "screening",
        "session" => "session",
        "struct" => "struct",
        "trading-profile" => "trading-profile",
        "ubo" => "ubo",
        "view" => "view",
        _ => "entity",
    }
}

fn apply_domain_precedence(lower: &str, scores: &mut HashMap<&'static str, i32>) {
    if lower.contains("full kyc")
        || lower.contains("onboarding process")
        || lower.contains("open a case")
    {
        add_score(scores, "case", 4);
        add_score(scores, "kyc", 2);
        add_score(scores, "document", -2);
        add_score(scores, "screening", -1);
    }

    if lower.contains("collect documents")
        || lower.contains("request identity documents")
        || lower.contains("due diligence")
    {
        add_score(scores, "kyc", 4);
        add_score(scores, "document", -1);
    }

    if lower.contains("beneficial owner")
        || lower.contains("who controls")
        || lower.contains("ownership structure")
    {
        add_score(scores, "ubo", 4);
        add_score(scores, "ownership", 1);
        add_score(scores, "entity", -1);
    }

    if lower.contains("sicav")
        || lower.contains("icav")
        || lower.contains("oeic")
        || lower.contains("40-act")
        || lower.contains("fund structure")
    {
        add_score(scores, "struct", 5);
        add_score(scores, "fund", -1);
    }

    if lower.contains("show me all entities") || lower.contains("all entities") {
        add_score(scores, "entity", 5);
    }

    if lower.contains("allianz")
        || lower.contains("this client")
        || lower.contains("this cbu")
        || lower.contains("cbus")
    {
        add_score(scores, "cbu", 2);
    }
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
    fn test_case_beats_document_for_open_case_flow() {
        let ctx = empty_ctx();
        let result = pre_classify("Open a case and collect the KYC documents", &ctx);
        assert_eq!(
            result.domain_hints.first().map(String::as_str),
            Some("case")
        );
    }

    #[test]
    fn test_ubo_beats_entity_for_control_question() {
        let ctx = empty_ctx();
        let result = pre_classify("who controls this company?", &ctx);
        assert_eq!(result.domain_hints.first().map(String::as_str), Some("ubo"));
    }

    #[test]
    fn test_struct_beats_fund_for_icav_setup() {
        let ctx = empty_ctx();
        let result = pre_classify("Set up an Irish ICAV fund", &ctx);
        assert_eq!(
            result.domain_hints.first().map(String::as_str),
            Some("struct")
        );
    }

    #[test]
    fn test_entity_kept_for_show_all_entities() {
        let ctx = empty_ctx();
        let result = pre_classify("show me all entities", &ctx);
        assert_eq!(
            result.domain_hints.first().map(String::as_str),
            Some("entity")
        );
    }

    #[test]
    fn test_cbu_beats_entity_for_transfer_agent_assignment() {
        let ctx = empty_ctx();
        let result = pre_classify("make State Street the transfer agent", &ctx);
        assert_eq!(result.domain_hints.first().map(String::as_str), Some("cbu"));
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
