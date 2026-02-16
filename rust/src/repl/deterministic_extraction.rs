//! Deterministic Arg Extraction — Phase F
//!
//! Attempts to fill verb arguments without LLM calls by leveraging:
//!
//! 1. **Template carry-forward** — Previous step results and args
//! 2. **Focus / pronoun resolution** — "it", "the manco" → concrete entity ref
//! 3. **Accumulated context** — Q&A answers from pack questions
//! 4. **Canonicalization** — "Luxembourg" → "LU", "ta" → "transfer_agent"
//!
//! If all required args are filled, returns `Some(ExtractionResult)`.
//! Otherwise returns `None` and caller falls back to LLM.
//!
//! # Closed-World Candidate List
//!
//! When LLM fallback is needed, `build_closed_world_prompt()` constrains
//! the LLM to pick from entities in scope — it never invents UUIDs.
//!
//! # Multi-Intent Splitting
//!
//! `detect_multi_intent()` detects conjunctive patterns like
//! "Yes, and add State Street as TA" → 2 separate intent halves.

use std::collections::HashMap;

#[cfg(test)]
use serde::{Deserialize, Serialize};

use super::context_stack::ContextStack;
use super::runbook::SlotSource;
use super::verb_config_index::{ArgSummary, VerbConfigIndex};

// ============================================================================
// Extraction Result
// ============================================================================

/// Outcome of deterministic extraction.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// Filled args (key → value).
    pub args: HashMap<String, String>,
    /// Provenance per slot.
    pub provenance: HashMap<String, SlotSource>,
    /// Model ID to record in audit (always "deterministic").
    pub model_id: &'static str,
}

impl Default for ExtractionResult {
    fn default() -> Self {
        Self {
            args: HashMap::new(),
            provenance: HashMap::new(),
            model_id: "deterministic",
        }
    }
}

impl ExtractionResult {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(&mut self, key: String, value: String, source: SlotSource) {
        self.args.insert(key.clone(), value);
        self.provenance.insert(key, source);
    }

    fn has(&self, key: &str) -> bool {
        self.args.contains_key(key)
    }
}

// ============================================================================
// Closed-World Prompt
// ============================================================================

#[cfg(test)]
/// Structured context for constraining LLM fallback prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedWorldPrompt {
    /// Client group context (if any).
    pub client_group: Option<String>,
    /// Active pack name (if any).
    pub active_pack: Option<String>,
    /// Current CBU(s) in scope.
    pub cbus_in_scope: Vec<ClosedWorldEntity>,
    /// Recent entity mentions.
    pub recent_entities: Vec<ClosedWorldEntity>,
    /// Available jurisdictions (from canon).
    pub jurisdictions: Vec<String>,
    /// Available roles (from role synonyms).
    pub roles: Vec<String>,
    /// Template step hint (if in template).
    pub expected_step: Option<String>,
    /// Missing args that need filling.
    pub missing_args: Vec<String>,
}

#[cfg(test)]
/// An entity in the closed-world candidate list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedWorldEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
}

// ============================================================================
// Multi-Intent Detection
// ============================================================================

#[cfg(test)]
/// A detected split in user input.
#[derive(Debug, Clone)]
pub struct IntentSplit {
    /// The leading portion (may be a confirmation like "yes").
    pub first: String,
    /// The trailing portion (a new intent).
    pub second: String,
    /// The conjunction that triggered the split.
    pub conjunction: String,
}

// ============================================================================
// Deterministic Extraction
// ============================================================================

/// Try to fill all required args without LLM.
///
/// Strategy priority:
/// 1. Template carry-forward args (from `TemplateStepHint`)
/// 2. Focus / pronoun resolution (from `FocusContext`)
/// 3. Accumulated Q&A answers (from pack questions)
/// 4. Canonicalization of user input tokens
///
/// Returns `Some(ExtractionResult)` if ALL required args are filled.
/// Returns `None` if any required arg is missing (caller should use LLM).
pub fn try_deterministic_extraction(
    verb: &str,
    user_input: &str,
    context: &ContextStack,
    verb_config: &VerbConfigIndex,
) -> Option<ExtractionResult> {
    let entry = verb_config.get(verb)?;
    let required_args: Vec<&ArgSummary> = entry.args.iter().filter(|a| a.required).collect();

    // No required args → trivially satisfied.
    if required_args.is_empty() {
        return Some(ExtractionResult::new());
    }

    let mut result = ExtractionResult::new();

    // ── Strategy 1: Template carry-forward ──────────────────────────────
    if let Some(ref hint) = context.template_hint {
        for (key, value) in &hint.carry_forward_args {
            if !result.has(key) && has_arg(&required_args, key) {
                result.insert(key.clone(), value.clone(), SlotSource::CopiedFromPrevious);
            }
        }
    }

    // ── Strategy 2: Focus / pronoun resolution ──────────────────────────
    let input_lower = user_input.to_lowercase();
    for arg in &required_args {
        if result.has(&arg.name) {
            continue;
        }

        // Check if arg expects an entity type that matches focus.
        if is_entity_arg(&arg.arg_type) {
            if let Some(focus_ref) = resolve_focus_for_arg(arg, &input_lower, context) {
                result.insert(
                    arg.name.clone(),
                    focus_ref.id.to_string(),
                    SlotSource::InferredFromContext,
                );
            }
        }
    }

    // ── Strategy 3: Accumulated answers ─────────────────────────────────
    for arg in &required_args {
        if result.has(&arg.name) {
            continue;
        }
        if let Some(answer) = context.accumulated_answers.get(&arg.name) {
            if let Some(val) = answer.as_str() {
                result.insert(
                    arg.name.clone(),
                    val.to_string(),
                    SlotSource::InferredFromContext,
                );
            }
        }
    }

    // ── Strategy 4: Canonicalization of input tokens ────────────────────
    let canonical = super::context_stack::canonicalize_mention(user_input);
    for arg in &required_args {
        if result.has(&arg.name) {
            continue;
        }
        // Try to match canonical form against arg type expectations.
        if (arg.arg_type == "string" || arg.name == "jurisdiction")
            && is_jurisdiction_value(&canonical)
            && arg.name.contains("jurisdiction")
        {
            result.insert(
                arg.name.clone(),
                canonical.clone(),
                SlotSource::InferredFromContext,
            );
        }
    }

    // ── Check completeness ──────────────────────────────────────────────
    let all_required_filled = required_args.iter().all(|a| result.has(&a.name));
    if all_required_filled {
        Some(result)
    } else {
        None
    }
}

// ============================================================================
// Closed-World Prompt Builder
// ============================================================================

#[cfg(test)]
/// Build a closed-world candidate list for constraining LLM fallback.
///
/// The LLM MUST pick from this list — never invent entity IDs.
pub fn build_closed_world_prompt(
    _verb: &str,
    context: &ContextStack,
    _verb_config: &VerbConfigIndex,
    missing_args: &[String],
) -> ClosedWorldPrompt {
    let client_group = context.derived_scope.client_group_name.clone();
    let active_pack = context.active_pack().map(|p| p.pack_id.clone());

    // CBUs in scope.
    let cbus_in_scope: Vec<ClosedWorldEntity> = context
        .derived_scope
        .loaded_cbu_ids
        .iter()
        .map(|id| ClosedWorldEntity {
            id: id.to_string(),
            name: format!("CBU {}", &id.to_string()[..8]),
            entity_type: "cbu".to_string(),
        })
        .collect();

    // Recent entity mentions.
    let recent_entities: Vec<ClosedWorldEntity> = context
        .recent
        .mentions
        .iter()
        .map(|m| ClosedWorldEntity {
            id: m.entity_id.to_string(),
            name: m.display_name.clone(),
            entity_type: m.entity_type.clone(),
        })
        .collect();

    // Jurisdictions from canon.
    let jurisdictions = super::context_stack::JURISDICTION_CANON
        .iter()
        .map(|(_, code)| (*code).to_string())
        .collect::<Vec<_>>();

    // Roles from synonyms.
    let roles = super::context_stack::ROLE_SYNONYMS
        .iter()
        .map(|(_, role)| (*role).to_string())
        .collect::<Vec<_>>();

    // Template step hint.
    let expected_step = context
        .template_hint
        .as_ref()
        .map(|h| format!("{} ({})", h.expected_verb, h.progress_label()));

    ClosedWorldPrompt {
        client_group,
        active_pack,
        cbus_in_scope,
        recent_entities,
        jurisdictions,
        roles,
        expected_step,
        missing_args: missing_args.to_vec(),
    }
}

#[cfg(test)]
impl ClosedWorldPrompt {
    /// Render as a text block for embedding in an LLM system prompt.
    pub fn render(&self) -> String {
        let mut lines = Vec::new();

        lines.push("=== CLOSED-WORLD CONTEXT ===".to_string());

        if let Some(ref cg) = self.client_group {
            lines.push(format!("Client group: {}", cg));
        }
        if let Some(ref pack) = self.active_pack {
            lines.push(format!("Active pack: {}", pack));
        }
        if let Some(ref step) = self.expected_step {
            lines.push(format!("Expected step: {}", step));
        }

        if !self.cbus_in_scope.is_empty() {
            lines.push(format!("CBUs in scope ({}):", self.cbus_in_scope.len()));
            for cbu in &self.cbus_in_scope {
                lines.push(format!("  - {} ({})", cbu.name, cbu.id));
            }
        }

        if !self.recent_entities.is_empty() {
            lines.push("Recent entities:".to_string());
            for e in &self.recent_entities {
                lines.push(format!("  - {} [{}] ({})", e.name, e.entity_type, e.id));
            }
        }

        if !self.missing_args.is_empty() {
            lines.push(format!("Missing args to fill: {:?}", self.missing_args));
        }

        lines.push("RULE: Pick entity IDs from the lists above. Never invent UUIDs.".to_string());
        lines.push("=== END CONTEXT ===".to_string());

        lines.join("\n")
    }
}

// ============================================================================
// Multi-Intent Detection
// ============================================================================

#[cfg(test)]
/// Conjunction patterns that indicate multi-intent input.
const CONJUNCTIONS: &[&str] = &[
    " and also ",
    ", and also ",
    ", and ",
    ", also ",
    ", then ",
    ", plus ",
    " and ",
    " also ",
    " plus ",
    " then ",
];

#[cfg(test)]
/// Detect if user input contains multiple intents joined by conjunctions.
///
/// Returns `Some(IntentSplit)` if a conjunction is found AND the leading
/// portion looks like a confirmation/acknowledgement.
///
/// Examples:
/// - "Yes, and add State Street as TA" → Some(first="Yes", second="add State Street as TA")
/// - "Add IRS product" → None (single intent)
pub fn detect_multi_intent(input: &str) -> Option<IntentSplit> {
    let input_lower = input.to_lowercase();

    // Only split when the leading portion is a short acknowledgement.
    let ack_prefixes = [
        "yes",
        "yeah",
        "yep",
        "ok",
        "okay",
        "sure",
        "right",
        "correct",
        "confirmed",
        "done",
        "good",
        "great",
    ];

    for conj in CONJUNCTIONS {
        if let Some(pos) = input_lower.find(conj) {
            let first = input[..pos].trim();
            let second = input[pos + conj.len()..].trim();

            // Only split if first part is short and looks like an acknowledgement.
            let first_lower = first.to_lowercase();
            let is_ack = ack_prefixes.iter().any(|&p| first_lower == p)
                || (first.len() < 20 && ack_prefixes.iter().any(|&p| first_lower.starts_with(p)));

            if is_ack && !second.is_empty() {
                return Some(IntentSplit {
                    first: first.to_string(),
                    second: second.to_string(),
                    conjunction: conj.trim().to_string(),
                });
            }
        }
    }

    None
}

// ============================================================================
// Pack-Enriched Prompt
// ============================================================================

#[cfg(test)]
/// Build a pack-enriched system prompt section for LLM arg extraction.
///
/// Provides the LLM with structured context so it can extract args
/// from constrained candidates rather than hallucinating.
pub fn build_pack_enriched_prompt(context: &ContextStack) -> String {
    let mut lines = Vec::new();

    lines.push("=== SESSION CONTEXT ===".to_string());

    // Client group.
    if let Some(ref name) = context.derived_scope.client_group_name {
        lines.push(format!("Client: {}", name));
    }

    // Default CBU.
    if let Some(ref cbu) = context.derived_scope.default_cbu {
        lines.push(format!("Default CBU: {}", cbu));
    }

    // Active pack.
    if let Some(pack) = context.active_pack() {
        lines.push(format!(
            "Pack: {} (domain: {})",
            pack.pack_id,
            pack.dominant_domain.as_deref().unwrap_or("unknown")
        ));
        if !pack.allowed_verbs.is_empty() {
            let verbs: Vec<&String> = pack.allowed_verbs.iter().take(10).collect();
            lines.push(format!("  Allowed verbs (sample): {:?}", verbs));
        }
    }

    // Focus entities.
    if let Some(ref f) = context.focus.entity {
        lines.push(format!("Focus entity: {} ({})", f.display_name, f.id));
    }
    if let Some(ref f) = context.focus.cbu {
        lines.push(format!("Focus CBU: {} ({})", f.display_name, f.id));
    }
    if let Some(ref f) = context.focus.case {
        lines.push(format!("Focus case: {} ({})", f.display_name, f.id));
    }

    // Template step.
    if let Some(ref hint) = context.template_hint {
        lines.push(format!(
            "Template step: {} — {}",
            hint.progress_label(),
            hint.expected_verb
        ));
        if !hint.carry_forward_args.is_empty() {
            lines.push(format!("  Carry-forward: {:?}", hint.carry_forward_args));
        }
    }

    // Accumulated answers.
    if !context.accumulated_answers.is_empty() {
        lines.push("Known answers:".to_string());
        for (key, val) in &context.accumulated_answers {
            lines.push(format!("  {}: {}", key, val));
        }
    }

    // Exclusions.
    if !context.exclusions.is_empty() {
        lines.push("Excluded (do not suggest):".to_string());
        for excl in context.exclusions.active() {
            lines.push(format!("  - {} (reason: {})", excl.value, excl.reason));
        }
    }

    lines.push("=== END SESSION CONTEXT ===".to_string());

    lines.join("\n")
}

// ============================================================================
// Helpers
// ============================================================================

/// Check if an arg type refers to an entity (uuid, entity_ref, etc.).
fn is_entity_arg(arg_type: &str) -> bool {
    matches!(
        arg_type,
        "uuid" | "entity_ref" | "entity-ref" | "structure_ref" | "party_ref"
    )
}

/// Check if a value is a known jurisdiction code.
fn is_jurisdiction_value(value: &str) -> bool {
    super::context_stack::JURISDICTION_CANON
        .iter()
        .any(|(_, code)| *code == value)
}

/// Check if a required arg list contains an arg with the given name.
fn has_arg(required_args: &[&ArgSummary], name: &str) -> bool {
    required_args.iter().any(|a| a.name == name)
}

/// Resolve focus context for an entity-typed arg.
///
/// Uses focus + pronoun patterns from the user input to find a matching
/// entity reference.
fn resolve_focus_for_arg(
    arg: &ArgSummary,
    input_lower: &str,
    context: &ContextStack,
) -> Option<FocusRef> {
    // 1. Type-specific focus (highest priority — arg name implies type).
    if arg.name.contains("cbu") {
        return context.focus.cbu.clone();
    }
    if arg.name.contains("case") {
        return context.focus.case.clone();
    }

    // 2. Pronoun resolution via input text ("it", "the manco", etc.).
    if let Some(focus_ref) = context.focus.resolve_pronoun(input_lower) {
        if arg.name.contains("entity") || arg.name.contains("id") {
            return Some(focus_ref.clone());
        }
    }

    // 3. Generic entity focus as last resort for entity-typed args.
    if arg.name.contains("entity") {
        return context.focus.entity.clone();
    }

    None
}

use super::context_stack::FocusRef;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::context_stack::{
        ContextStack, DerivedScope, ExclusionSet, FocusContext, FocusRef, OutcomeRegistry,
        PackContext, RecentContext, RecentMention, TemplateStepHint,
    };
    use super::*;
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    fn empty_context() -> ContextStack {
        ContextStack {
            derived_scope: DerivedScope::default(),
            pack_staged: None,
            pack_executed: None,
            template_hint: None,
            focus: FocusContext::default(),
            recent: RecentContext::default(),
            exclusions: ExclusionSet::default(),
            outcomes: OutcomeRegistry::default(),
            accumulated_answers: HashMap::new(),
            executed_verbs: HashSet::new(),
            staged_verbs: HashSet::new(),
            turn: 0,
        }
    }

    fn simple_verb_config() -> VerbConfigIndex {
        use super::super::verb_config_index::{ArgSummary, VerbIndexEntry};

        let mut index = VerbConfigIndex::empty();
        // Manually insert a test verb entry.
        // We'll use the from_entries helper.
        index.insert_test_entry(VerbIndexEntry {
            fqn: "cbu.create".to_string(),
            description: "Create a CBU".to_string(),
            invocation_phrases: vec!["create cbu".to_string()],
            sentence_templates: vec![],
            sentences: None,
            args: vec![
                ArgSummary {
                    name: "name".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: Some("Name of the CBU".to_string()),
                    maps_to: Some("name".to_string()),
                    lookup_entity_type: None,
                },
                ArgSummary {
                    name: "jurisdiction".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: Some("Jurisdiction code".to_string()),
                    maps_to: Some("jurisdiction_code".to_string()),
                    lookup_entity_type: None,
                },
                ArgSummary {
                    name: "kind".to_string(),
                    arg_type: "string".to_string(),
                    required: false,
                    description: Some("CBU kind".to_string()),
                    maps_to: Some("kind".to_string()),
                    lookup_entity_type: None,
                },
            ],
            crud_key: None,
            confirm_policy: super::super::runbook::ConfirmPolicy::Always,
            precondition_checks: vec![],
        });
        index.insert_test_entry(VerbIndexEntry {
            fqn: "kyc.add-entity".to_string(),
            description: "Add an entity to a KYC case".to_string(),
            invocation_phrases: vec!["add entity to kyc case".to_string()],
            sentence_templates: vec![],
            sentences: None,
            args: vec![
                ArgSummary {
                    name: "case-id".to_string(),
                    arg_type: "uuid".to_string(),
                    required: true,
                    description: Some("KYC case ID".to_string()),
                    maps_to: Some("case_id".to_string()),
                    lookup_entity_type: None,
                },
                ArgSummary {
                    name: "entity-id".to_string(),
                    arg_type: "uuid".to_string(),
                    required: true,
                    description: Some("Entity to add".to_string()),
                    maps_to: Some("entity_id".to_string()),
                    lookup_entity_type: Some("entity".to_string()),
                },
            ],
            crud_key: Some("case_id".to_string()),
            confirm_policy: super::super::runbook::ConfirmPolicy::Always,
            precondition_checks: vec![],
        });
        index.insert_test_entry(VerbIndexEntry {
            fqn: "session.info".to_string(),
            description: "Show session info".to_string(),
            invocation_phrases: vec!["show session".to_string()],
            sentence_templates: vec![],
            sentences: None,
            args: vec![],
            crud_key: None,
            confirm_policy: super::super::runbook::ConfirmPolicy::QuickConfirm,
            precondition_checks: vec![],
        });
        index
    }

    // ── try_deterministic_extraction ─────────────────────────────────

    #[test]
    fn test_extraction_no_required_args() {
        let ctx = empty_context();
        let vc = simple_verb_config();
        let result = try_deterministic_extraction("session.info", "show session", &ctx, &vc);
        assert!(result.is_some());
        assert!(result.unwrap().args.is_empty());
    }

    #[test]
    fn test_extraction_missing_required_returns_none() {
        let ctx = empty_context();
        let vc = simple_verb_config();
        // cbu.create requires "name" and "jurisdiction" — neither available.
        let result = try_deterministic_extraction("cbu.create", "create cbu", &ctx, &vc);
        assert!(result.is_none());
    }

    #[test]
    fn test_extraction_carry_forward_fills_args() {
        let mut ctx = empty_context();
        ctx.template_hint = Some(TemplateStepHint {
            template_id: "standard-kyc".to_string(),
            step_index: 1,
            total_steps: 5,
            expected_verb: "cbu.create".to_string(),
            next_entry_id: Uuid::new_v4(),
            section: None,
            section_progress: None,
            carry_forward_args: HashMap::from([
                ("name".to_string(), "Allianz Lux".to_string()),
                ("jurisdiction".to_string(), "LU".to_string()),
            ]),
        });
        let vc = simple_verb_config();
        let result = try_deterministic_extraction("cbu.create", "create the cbu", &ctx, &vc);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.args.get("name").unwrap(), "Allianz Lux");
        assert_eq!(r.args.get("jurisdiction").unwrap(), "LU");
        assert_eq!(
            r.provenance.get("name"),
            Some(&SlotSource::CopiedFromPrevious)
        );
    }

    #[test]
    fn test_extraction_accumulated_answers_fill_args() {
        let mut ctx = empty_context();
        ctx.accumulated_answers.insert(
            "name".to_string(),
            serde_json::Value::String("Aviva Fund".to_string()),
        );
        ctx.accumulated_answers.insert(
            "jurisdiction".to_string(),
            serde_json::Value::String("IE".to_string()),
        );
        let vc = simple_verb_config();
        let result = try_deterministic_extraction("cbu.create", "create cbu", &ctx, &vc);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.args.get("name").unwrap(), "Aviva Fund");
        assert_eq!(r.args.get("jurisdiction").unwrap(), "IE");
        assert_eq!(
            r.provenance.get("jurisdiction"),
            Some(&SlotSource::InferredFromContext)
        );
    }

    #[test]
    fn test_extraction_focus_fills_entity_arg() {
        let mut ctx = empty_context();
        let case_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        ctx.focus.case = Some(FocusRef {
            id: case_id,
            display_name: "KYC-001".to_string(),
            entity_type: "kyc_case".to_string(),
            set_at_turn: 1,
        });
        ctx.focus.entity = Some(FocusRef {
            id: entity_id,
            display_name: "Allianz SE".to_string(),
            entity_type: "company".to_string(),
            set_at_turn: 1,
        });
        let vc = simple_verb_config();
        // Input contains "it" which should trigger pronoun resolution.
        let result =
            try_deterministic_extraction("kyc.add-entity", "add it to the case", &ctx, &vc);
        // Focus resolves entity-id via pronoun, case-id via type match.
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.args.get("case-id").unwrap(), &case_id.to_string());
        assert_eq!(r.args.get("entity-id").unwrap(), &entity_id.to_string());
    }

    #[test]
    fn test_extraction_partial_fill_returns_none() {
        let mut ctx = empty_context();
        ctx.accumulated_answers.insert(
            "name".to_string(),
            serde_json::Value::String("Acme".to_string()),
        );
        // "jurisdiction" still missing.
        let vc = simple_verb_config();
        let result = try_deterministic_extraction("cbu.create", "create cbu", &ctx, &vc);
        assert!(result.is_none());
    }

    #[test]
    fn test_extraction_unknown_verb_returns_none() {
        let ctx = empty_context();
        let vc = simple_verb_config();
        let result = try_deterministic_extraction("nonexistent.verb", "do something", &ctx, &vc);
        assert!(result.is_none());
    }

    #[test]
    fn test_extraction_carry_forward_priority_over_accumulated() {
        // Carry-forward should be checked first, so it wins.
        let mut ctx = empty_context();
        ctx.template_hint = Some(TemplateStepHint {
            template_id: "t".to_string(),
            step_index: 0,
            total_steps: 1,
            expected_verb: "cbu.create".to_string(),
            next_entry_id: Uuid::new_v4(),
            section: None,
            section_progress: None,
            carry_forward_args: HashMap::from([
                ("name".to_string(), "From Template".to_string()),
                ("jurisdiction".to_string(), "LU".to_string()),
            ]),
        });
        ctx.accumulated_answers.insert(
            "name".to_string(),
            serde_json::Value::String("From Answers".to_string()),
        );
        let vc = simple_verb_config();
        let result = try_deterministic_extraction("cbu.create", "create", &ctx, &vc);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.args.get("name").unwrap(), "From Template");
        assert_eq!(
            r.provenance.get("name"),
            Some(&SlotSource::CopiedFromPrevious)
        );
    }

    // ── detect_multi_intent ─────────────────────────────────────────

    #[test]
    fn test_multi_intent_yes_and() {
        let split = detect_multi_intent("Yes, and add State Street as TA");
        assert!(split.is_some());
        let s = split.unwrap();
        assert_eq!(s.first, "Yes");
        assert_eq!(s.second, "add State Street as TA");
    }

    #[test]
    fn test_multi_intent_ok_also() {
        let split = detect_multi_intent("ok, also create a fund");
        assert!(split.is_some());
        let s = split.unwrap();
        assert_eq!(s.first, "ok");
        assert_eq!(s.second, "create a fund");
    }

    #[test]
    fn test_multi_intent_no_conjunction() {
        let split = detect_multi_intent("add State Street as TA");
        assert!(split.is_none());
    }

    #[test]
    fn test_multi_intent_long_first_part_not_ack() {
        // "Add the fund and create the profile" — not an ack+intent split.
        let split = detect_multi_intent("Add the fund and create the profile");
        assert!(split.is_none());
    }

    #[test]
    fn test_multi_intent_sure_then() {
        let split = detect_multi_intent("Sure, then load the book");
        assert!(split.is_some());
        let s = split.unwrap();
        assert_eq!(s.first, "Sure");
        assert_eq!(s.second, "load the book");
    }

    // ── closed-world prompt ─────────────────────────────────────────

    #[test]
    fn test_closed_world_prompt_render() {
        let mut ctx = empty_context();
        ctx.derived_scope.client_group_name = Some("Allianz".to_string());
        ctx.recent.mentions.push(RecentMention {
            entity_id: Uuid::nil(),
            display_name: "Allianz SE".to_string(),
            entity_type: "company".to_string(),
            mentioned_at_turn: 1,
        });
        let vc = simple_verb_config();
        let prompt =
            build_closed_world_prompt("cbu.create", &ctx, &vc, &["jurisdiction".to_string()]);
        let rendered = prompt.render();
        assert!(rendered.contains("Client group: Allianz"));
        assert!(rendered.contains("Allianz SE"));
        assert!(rendered.contains("jurisdiction"));
        assert!(rendered.contains("Never invent UUIDs"));
    }

    // ── pack-enriched prompt ────────────────────────────────────────

    #[test]
    fn test_pack_enriched_prompt_includes_context() {
        let mut ctx = empty_context();
        ctx.derived_scope.client_group_name = Some("Aviva".to_string());
        ctx.pack_executed = Some(PackContext {
            pack_id: "kyc-case".to_string(),
            pack_version: "1.0".to_string(),
            dominant_domain: Some("kyc".to_string()),
            allowed_verbs: HashSet::from(["kyc.add-entity".to_string()]),
            forbidden_verbs: HashSet::new(),
            template_ids: vec![],
            invocation_phrases: vec![],
        });
        let prompt = build_pack_enriched_prompt(&ctx);
        assert!(prompt.contains("Client: Aviva"));
        assert!(prompt.contains("Pack: kyc-case"));
    }
}
