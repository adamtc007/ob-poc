//! ContextStack — Unified context replacing scattered session state
//!
//! The `ContextStack` is a pure data structure built from a runbook fold.
//! It replaces `ClientContext` and `JourneyContext` with a single, derivable
//! view of session state. The governing principle:
//!
//! > Session state is a left fold over executed runbook entries.
//! > No session table. No mutable scope object.
//!
//! # Architecture
//!
//! ```text
//! ContextStack
//! ├── DerivedScope        — client group, CBU, book (from runbook fold)
//! ├── PackContext          — active pack verbs, domain, constraints
//! ├── TemplateStepHint    — next expected step from active template
//! ├── FocusContext         — pronoun/shorthand resolution
//! ├── RecentContext        — last N entity mentions for carry-forward
//! ├── ExclusionSet         — rejected candidates with 3-turn decay
//! ├── OutcomeRegistry      — execution results for @N references
//! └── accumulated_answers  — Q&A answers from pack questions
//! ```
//!
//! # Invariant
//!
//! `ContextStack::from_runbook()` is the ONLY constructor. There is no
//! mutable builder. If you need different context, produce a different
//! runbook and fold again.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use uuid::Uuid;

use super::runbook::{EntryStatus, Runbook, RunbookEntry};
use crate::journey::pack::PackManifest;
use crate::journey::router::PackRouter;

// ---------------------------------------------------------------------------
// ContextStack (top-level)
// ---------------------------------------------------------------------------

/// Unified context for a REPL turn — built from runbook fold.
#[derive(Debug, Clone)]
pub struct ContextStack {
    /// Session scope derived from executed runbook entries.
    pub derived_scope: DerivedScope,

    /// Active pack context (staged preferred over executed).
    pub pack_staged: Option<PackContext>,
    pub pack_executed: Option<PackContext>,

    /// Template step hint for the next expected verb.
    pub template_hint: Option<TemplateStepHint>,

    /// Focus context for pronoun resolution.
    pub focus: FocusContext,

    /// Recent entity mentions for carry-forward.
    pub recent: RecentContext,

    /// Excluded candidates from user rejections.
    pub exclusions: ExclusionSet,

    /// Execution results keyed by entry ID for @N references.
    pub outcomes: OutcomeRegistry,

    /// Accumulated Q&A answers from pack questions.
    pub accumulated_answers: HashMap<String, serde_json::Value>,

    /// Verbs that have been executed (Completed) in the runbook.
    pub executed_verbs: HashSet<String>,

    /// Verbs that are staged (Proposed/Confirmed/Resolved) but not yet executed.
    pub staged_verbs: HashSet<String>,

    /// Current turn number (for exclusion decay).
    pub turn: u32,
}

impl ContextStack {
    /// Build a ContextStack by folding over a runbook.
    ///
    /// This is the ONLY constructor. If `staged_pack` is provided, it
    /// becomes `pack_staged` (preferred over any pack derived from
    /// executed entries).
    ///
    /// If `pack_router` is provided, the fold will look up the full
    /// `PackManifest` from `pack.select` entries' `manifest-hash` arg,
    /// giving a rich `PackContext` with allowed_verbs, forbidden_verbs, etc.
    /// Without it, a minimal context is derived from used verbs.
    pub fn from_runbook(
        runbook: &Runbook,
        staged_pack: Option<Arc<PackManifest>>,
        turn: u32,
    ) -> Self {
        Self::from_runbook_with_router(runbook, staged_pack, turn, None)
    }

    /// Build a ContextStack with access to the PackRouter for manifest lookup.
    pub fn from_runbook_with_router(
        runbook: &Runbook,
        staged_pack: Option<Arc<PackManifest>>,
        turn: u32,
        pack_router: Option<&PackRouter>,
    ) -> Self {
        let derived_scope = derive_session_state(runbook);
        let pack_executed = derive_pack_context(runbook, pack_router);
        let pack_staged = staged_pack.map(|m| PackContext::from_manifest(&m));
        let template_hint = derive_template_hint(runbook);
        let focus = derive_focus(runbook);
        let recent = derive_recent(runbook);
        let outcomes = derive_outcomes(runbook);
        let accumulated_answers = derive_answers(runbook);

        let mut exclusions = derive_exclusions(runbook, turn);
        exclusions.prune(turn);

        let executed_verbs = derive_executed_verbs(runbook);
        let staged_verbs = derive_staged_verbs(runbook);

        Self {
            derived_scope,
            pack_staged,
            pack_executed,
            template_hint,
            focus,
            recent,
            exclusions,
            outcomes,
            accumulated_answers,
            executed_verbs,
            staged_verbs,
            turn,
        }
    }

    /// The active pack context — staged preferred over executed.
    pub fn active_pack(&self) -> Option<&PackContext> {
        self.pack_staged.as_ref().or(self.pack_executed.as_ref())
    }

    /// Whether a verb is allowed by the active pack.
    /// If no pack is active, all verbs are allowed.
    pub fn is_verb_allowed(&self, verb: &str) -> bool {
        match self.active_pack() {
            Some(pack) => !pack.forbidden_verbs.contains(verb),
            None => true,
        }
    }

    /// Whether a verb is in the active pack's allowed set.
    /// If no pack is active, returns false (no boost).
    pub fn is_verb_in_pack(&self, verb: &str) -> bool {
        match self.active_pack() {
            Some(pack) => pack.allowed_verbs.contains(verb),
            None => false,
        }
    }

    /// Whether a verb is the next expected template step.
    pub fn is_template_step(&self, verb: &str) -> bool {
        match &self.template_hint {
            Some(hint) => hint.expected_verb == verb,
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// DerivedScope — replaces ClientContext
// ---------------------------------------------------------------------------

/// Session scope derived from executed runbook entries.
/// Replaces the mutable `ClientContext` struct.
#[derive(Debug, Clone, Default)]
pub struct DerivedScope {
    pub client_group_id: Option<Uuid>,
    pub client_group_name: Option<String>,
    pub default_cbu: Option<Uuid>,
    pub default_book: Option<String>,
    pub loaded_cbu_ids: Vec<Uuid>,
}

/// Pure fold over executed runbook entries to derive scope.
fn derive_session_state(runbook: &Runbook) -> DerivedScope {
    let mut scope = DerivedScope {
        client_group_id: runbook.client_group_id,
        ..Default::default()
    };

    for entry in runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
    {
        match entry.verb.as_str() {
            "session.load-cluster" | "session.load-galaxy" => {
                // Extract client group name from args if present.
                // Prefer explicit client-name over the client arg (which may be a UUID).
                if let Some(name) = entry
                    .args
                    .get("client-name")
                    .or(entry.args.get("apex-name"))
                    .or(entry.args.get("client"))
                {
                    scope.client_group_name = Some(name.clone());
                }
                // Extract book/cluster info from result.
                if let Some(result) = &entry.result {
                    if let Some(cbu_ids) = result.get("cbu_ids").and_then(|v| v.as_array()) {
                        scope.loaded_cbu_ids = cbu_ids
                            .iter()
                            .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                            .collect();
                        scope.default_cbu = scope.loaded_cbu_ids.first().copied();
                    }
                }
            }
            "session.load-cbu" => {
                if let Some(result) = &entry.result {
                    if let Some(cbu_id) = result
                        .get("cbu_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok())
                    {
                        if !scope.loaded_cbu_ids.contains(&cbu_id) {
                            scope.loaded_cbu_ids.push(cbu_id);
                        }
                        scope.default_cbu = Some(cbu_id);
                    }
                }
            }
            "session.set-cbu" | "session.focus-cbu" => {
                if let Some(cbu_id) = entry
                    .args
                    .get("cbu-id")
                    .and_then(|s| Uuid::parse_str(s).ok())
                {
                    scope.default_cbu = Some(cbu_id);
                }
            }
            "pack.select" => {
                // Pack selection doesn't change scope, but may carry context.
            }
            _ => {}
        }
    }

    scope
}

// ---------------------------------------------------------------------------
// PackContext — replaces JourneyContext.pack reads
// ---------------------------------------------------------------------------

/// Pack context derived from the active pack manifest.
#[derive(Debug, Clone)]
pub struct PackContext {
    pub pack_id: String,
    pub pack_version: String,
    pub allowed_verbs: HashSet<String>,
    pub forbidden_verbs: HashSet<String>,
    pub dominant_domain: Option<String>,
    pub template_ids: Vec<String>,
    pub invocation_phrases: Vec<String>,
}

impl PackContext {
    /// Build pack context from a manifest.
    pub fn from_manifest(manifest: &PackManifest) -> Self {
        let allowed: HashSet<String> = manifest.allowed_verbs.iter().cloned().collect();
        let forbidden: HashSet<String> = manifest.forbidden_verbs.iter().cloned().collect();

        // Derive dominant domain from allowed verbs.
        let dominant_domain = derive_dominant_domain(&allowed);

        let template_ids = manifest
            .templates
            .iter()
            .map(|t| t.template_id.clone())
            .collect();

        Self {
            pack_id: manifest.id.clone(),
            pack_version: manifest.version.clone(),
            allowed_verbs: allowed,
            forbidden_verbs: forbidden,
            dominant_domain,
            template_ids,
            invocation_phrases: manifest.invocation_phrases.clone(),
        }
    }
}

/// Derive the dominant domain from a set of allowed verbs.
/// The domain is the prefix before the first dot. The most frequent
/// domain wins.
fn derive_dominant_domain(allowed_verbs: &HashSet<String>) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for verb in allowed_verbs {
        if let Some(domain) = verb.split('.').next() {
            *counts.entry(domain).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(domain, _)| domain.to_string())
}

/// Derive pack context from executed runbook entries.
///
/// Strategy:
/// 1. Find the last completed `pack.select` entry.
/// 2. If a `PackRouter` is available, look up the full manifest by hash
///    and build a rich `PackContext` with allowed_verbs, forbidden_verbs, etc.
/// 3. Otherwise, fall back to a minimal context from used verbs.
fn derive_pack_context(runbook: &Runbook, pack_router: Option<&PackRouter>) -> Option<PackContext> {
    // Find the last completed pack.select entry.
    let pack_select_entry = runbook
        .entries
        .iter()
        .rev()
        .find(|e| e.verb == "pack.select" && e.status == EntryStatus::Completed);

    if let Some(entry) = pack_select_entry {
        let pack_id = entry.args.get("pack-id").cloned().unwrap_or_default();
        let manifest_hash = entry.args.get("manifest-hash").cloned();

        // Try to get full manifest from router.
        if let Some(router) = pack_router {
            // First try by hash (precise), then by ID (fallback).
            let manifest = manifest_hash
                .as_deref()
                .and_then(|h| router.get_pack_by_hash(h))
                .or_else(|| router.get_pack(&pack_id));

            if let Some((manifest, _hash)) = manifest {
                return Some(PackContext::from_manifest(manifest));
            }
        }

        // No router or manifest not found — build minimal context.
        let pack_version = entry.args.get("pack-version").cloned().unwrap_or_default();

        let used_verbs: HashSet<String> = runbook
            .entries
            .iter()
            .filter(|e| e.status == EntryStatus::Completed)
            .map(|e| e.verb.clone())
            .collect();

        return Some(PackContext {
            pack_id,
            pack_version,
            allowed_verbs: used_verbs,
            forbidden_verbs: HashSet::new(),
            dominant_domain: None,
            template_ids: Vec::new(),
            invocation_phrases: Vec::new(),
        });
    }

    // Legacy fallback: use runbook metadata if no pack.select entry found.
    let pack_id = runbook.pack_id.as_ref()?;
    let pack_version = runbook.pack_version.clone().unwrap_or_default();

    // Try router by pack_id from runbook metadata.
    if let Some(router) = pack_router {
        if let Some((manifest, _hash)) = router.get_pack(pack_id) {
            return Some(PackContext::from_manifest(manifest));
        }
    }

    let used_verbs: HashSet<String> = runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
        .map(|e| e.verb.clone())
        .collect();

    Some(PackContext {
        pack_id: pack_id.clone(),
        pack_version,
        allowed_verbs: used_verbs,
        forbidden_verbs: HashSet::new(),
        dominant_domain: None,
        template_ids: Vec::new(),
        invocation_phrases: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// TemplateStepHint
// ---------------------------------------------------------------------------

/// Hint about the next expected template step.
///
/// Derived from the runbook by counting completed template entries and
/// finding the next pending one. Includes section tracking and carry-forward
/// args from completed entries so the scoring layer can boost the expected
/// verb and the arg extractor can pre-fill known values.
#[derive(Debug, Clone)]
pub struct TemplateStepHint {
    /// Template identifier (matches `runbook.template_id`).
    pub template_id: String,
    /// 0-based index of the next step to execute.
    pub step_index: usize,
    /// Total template steps (completed + remaining).
    pub total_steps: usize,
    /// FQN of the expected next verb.
    pub expected_verb: String,
    /// Entry ID of the next pending entry (for direct reference).
    pub next_entry_id: Uuid,
    /// Section label from entry labels, if present (e.g. "entities", "products").
    pub section: Option<String>,
    /// (completed_in_section, total_in_section) if section is known.
    pub section_progress: Option<(usize, usize)>,
    /// Arg values carried forward from completed entries (last-write-wins).
    pub carry_forward_args: HashMap<String, String>,
}

impl TemplateStepHint {
    /// Human-readable progress string, e.g. "Step 3 of 8" or "Step 3 of 8 (entities: 2/4)".
    pub fn progress_label(&self) -> String {
        let base = format!("Step {} of {}", self.step_index + 1, self.total_steps);
        match (&self.section, self.section_progress) {
            (Some(sec), Some((done, total))) => format!("{} ({}: {}/{})", base, sec, done, total),
            _ => base,
        }
    }
}

/// Derive template step hint from runbook state.
fn derive_template_hint(runbook: &Runbook) -> Option<TemplateStepHint> {
    let template_id = runbook.template_id.as_ref()?;

    // Collect all template entries (any status except Disabled).
    let template_entries: Vec<&RunbookEntry> = runbook
        .entries
        .iter()
        .filter(|e| {
            e.labels.get("template_id") == Some(template_id) && e.status != EntryStatus::Disabled
        })
        .collect();

    if template_entries.is_empty() {
        return None;
    }

    let total_steps = template_entries.len();

    let completed_count = template_entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
        .count();

    // Find the next proposed/confirmed/resolved entry from the template.
    let next_entry = template_entries.iter().find(|e| {
        matches!(
            e.status,
            EntryStatus::Proposed | EntryStatus::Confirmed | EntryStatus::Resolved
        )
    })?;

    // Section tracking from labels.
    let section = next_entry.labels.get("section").cloned();
    let section_progress = section.as_ref().map(|sec| {
        let in_section: Vec<&&RunbookEntry> = template_entries
            .iter()
            .filter(|e| e.labels.get("section").map(|s| s == sec).unwrap_or(false))
            .collect();
        let done_in_section = in_section
            .iter()
            .filter(|e| e.status == EntryStatus::Completed)
            .count();
        (done_in_section, in_section.len())
    });

    // Build carry-forward args from completed entries.
    let carry_forward = build_carry_forward(runbook);

    Some(TemplateStepHint {
        template_id: template_id.clone(),
        step_index: completed_count,
        total_steps,
        expected_verb: next_entry.verb.clone(),
        next_entry_id: next_entry.id,
        section,
        section_progress,
        carry_forward_args: carry_forward,
    })
}

/// Build carry-forward context from completed entries.
///
/// Maps arg names to their last seen values. This enables the deterministic
/// arg extractor (Phase F) to pre-fill args that are already known from
/// previous steps — e.g. if step 1 resolved `:cbu-id`, step 2 can reuse it.
fn build_carry_forward(runbook: &Runbook) -> HashMap<String, String> {
    let mut carry = HashMap::new();
    for entry in runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
    {
        for (key, value) in &entry.args {
            carry.insert(key.clone(), value.clone());
        }
        // Also carry forward result-derived values (e.g. created UUIDs).
        if let Some(result) = &entry.result {
            if let Some(obj) = result.as_object() {
                for (key, value) in obj {
                    if let Some(s) = value.as_str() {
                        carry.insert(key.clone(), s.to_string());
                    }
                }
            }
        }
    }
    carry
}

// ---------------------------------------------------------------------------
// FocusContext — pronoun and shorthand resolution
// ---------------------------------------------------------------------------

/// Tracks the current focus for pronoun resolution.
///
/// "it", "that", "the manco", "the case" all resolve via this context.
#[derive(Debug, Clone, Default)]
pub struct FocusContext {
    /// The most recently mentioned entity.
    pub entity: Option<FocusRef>,
    /// The most recently mentioned CBU.
    pub cbu: Option<FocusRef>,
    /// The most recently mentioned case.
    pub case: Option<FocusRef>,
}

/// A concrete reference that a pronoun resolves to.
#[derive(Debug, Clone)]
pub struct FocusRef {
    pub id: Uuid,
    pub display_name: String,
    pub entity_type: String,
    pub set_at_turn: u32,
}

/// Role synonyms for shorthand resolution.
/// "the ta" → "transfer_agent", "the im" → "investment_manager", etc.
pub static ROLE_SYNONYMS: &[(&str, &str)] = &[
    ("ta", "transfer_agent"),
    ("im", "investment_manager"),
    ("gp", "general_partner"),
    ("manco", "management_company"),
    ("dp", "depositary"),
    ("custodian", "depositary"),
    ("pm", "portfolio_manager"),
    ("rm", "relationship_manager"),
    ("auditor", "auditor"),
    ("admin", "fund_administrator"),
];

/// Pronoun patterns that resolve to focus context.
pub static PRONOUN_PATTERNS: &[(&str, FocusTarget)] = &[
    ("it", FocusTarget::Entity),
    ("that", FocusTarget::Entity),
    ("this", FocusTarget::Entity),
    ("the entity", FocusTarget::Entity),
    ("the fund", FocusTarget::Cbu),
    ("the cbu", FocusTarget::Cbu),
    ("the structure", FocusTarget::Cbu),
    ("the case", FocusTarget::Case),
    ("the kyc case", FocusTarget::Case),
    ("the manco", FocusTarget::Role("management_company")),
    ("the ta", FocusTarget::Role("transfer_agent")),
    ("the im", FocusTarget::Role("investment_manager")),
    ("the gp", FocusTarget::Role("general_partner")),
    ("the depositary", FocusTarget::Role("depositary")),
];

/// What a pronoun resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Entity,
    Cbu,
    Case,
    Role(&'static str),
}

impl FocusContext {
    /// Try to resolve a pronoun or shorthand to a concrete reference.
    pub fn resolve_pronoun(&self, input: &str) -> Option<&FocusRef> {
        let lower = input.to_lowercase();
        let trimmed = lower.trim();

        for (pattern, target) in PRONOUN_PATTERNS {
            if trimmed == *pattern || trimmed.contains(pattern) {
                match target {
                    FocusTarget::Entity => return self.entity.as_ref(),
                    FocusTarget::Cbu => return self.cbu.as_ref(),
                    FocusTarget::Case => return self.case.as_ref(),
                    FocusTarget::Role(_) => return self.entity.as_ref(),
                }
            }
        }
        None
    }

    /// Resolve a role synonym to its canonical form.
    pub fn resolve_role(input: &str) -> Option<&'static str> {
        let lower = input.to_lowercase();
        let trimmed = lower.trim();
        for (short, canonical) in ROLE_SYNONYMS {
            if trimmed == *short {
                return Some(canonical);
            }
        }
        None
    }

    /// Update entity focus.
    pub fn set_entity(&mut self, id: Uuid, name: String, entity_type: String, turn: u32) {
        self.entity = Some(FocusRef {
            id,
            display_name: name,
            entity_type,
            set_at_turn: turn,
        });
    }

    /// Update CBU focus.
    pub fn set_cbu(&mut self, id: Uuid, name: String, turn: u32) {
        self.cbu = Some(FocusRef {
            id,
            display_name: name,
            entity_type: "cbu".to_string(),
            set_at_turn: turn,
        });
    }

    /// Update case focus.
    pub fn set_case(&mut self, id: Uuid, name: String, turn: u32) {
        self.case = Some(FocusRef {
            id,
            display_name: name,
            entity_type: "kyc_case".to_string(),
            set_at_turn: turn,
        });
    }
}

/// Derive focus context from the runbook (last completed entry mentioning entities).
fn derive_focus(runbook: &Runbook) -> FocusContext {
    let mut focus = FocusContext::default();

    for entry in runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
    {
        // Derive CBU focus from CBU-operating verbs.
        if entry.verb.starts_with("cbu.") || entry.verb.starts_with("session.load-cbu") {
            if let Some(cbu_id) = entry
                .args
                .get("cbu-id")
                .or(entry.args.get("cbu_id"))
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                let name = entry
                    .args
                    .get("cbu-name")
                    .or(entry.args.get("name"))
                    .cloned()
                    .unwrap_or_default();
                focus.cbu = Some(FocusRef {
                    id: cbu_id,
                    display_name: name,
                    entity_type: "cbu".to_string(),
                    set_at_turn: 0,
                });
            }
        }

        // Derive entity focus from entity-operating verbs.
        if entry.verb.starts_with("entity.") || entry.verb.starts_with("kyc.add-entity") {
            if let Some(entity_id) = entry
                .args
                .get("entity-id")
                .or(entry.args.get("entity_id"))
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                let name = entry
                    .args
                    .get("entity-name")
                    .or(entry.args.get("name"))
                    .cloned()
                    .unwrap_or_default();
                focus.entity = Some(FocusRef {
                    id: entity_id,
                    display_name: name,
                    entity_type: "entity".to_string(),
                    set_at_turn: 0,
                });
            }
        }

        // Derive case focus from KYC verbs.
        if entry.verb.starts_with("kyc.") {
            if let Some(case_id) = entry
                .args
                .get("case-id")
                .or(entry.args.get("case_id"))
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                let name = entry.args.get("case-name").cloned().unwrap_or_default();
                focus.case = Some(FocusRef {
                    id: case_id,
                    display_name: name,
                    entity_type: "kyc_case".to_string(),
                    set_at_turn: 0,
                });
            }
        }
    }

    focus
}

// ---------------------------------------------------------------------------
// FocusMode — domain gating for soft scoring boost
// ---------------------------------------------------------------------------

/// Focus mode derived from the active pack and recent verbs.
///
/// Used for a soft domain-affinity boost: verbs matching the focus mode's
/// domain get a small positive adjustment in the scoring pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusMode {
    /// KYC / case management focus.
    KycCase,
    /// Document / proof collection.
    Proofs,
    /// Trading profile / mandate management.
    Trading,
    /// CBU / structure management.
    CbuManagement,
    /// General — no specific domain focus.
    General,
}

impl FocusMode {
    /// The primary domain associated with this focus mode.
    pub fn domain(&self) -> Option<&'static str> {
        match self {
            FocusMode::KycCase => Some("kyc"),
            FocusMode::Proofs => Some("document"),
            FocusMode::Trading => Some("trading-profile"),
            FocusMode::CbuManagement => Some("cbu"),
            FocusMode::General => None,
        }
    }
}

/// Derive focus mode from the context stack.
///
/// Priority:
/// 1. Active pack's dominant domain (strongest signal)
/// 2. Domain of the most recent executed verb (weaker signal)
/// 3. General (no focus)
pub fn derive_focus_mode(context: &ContextStack) -> FocusMode {
    // 1. Active pack dominant domain
    if let Some(pack) = context.active_pack() {
        if let Some(ref domain) = pack.dominant_domain {
            return match domain.as_str() {
                "kyc" | "kyc-case" => FocusMode::KycCase,
                "document" | "requirement" => FocusMode::Proofs,
                "trading-profile" | "custody" => FocusMode::Trading,
                "cbu" | "entity" => FocusMode::CbuManagement,
                _ => FocusMode::General,
            };
        }
    }

    // 2. Most recent executed verb's domain
    for verb in &context.executed_verbs {
        if let Some(domain) = verb.split('.').next() {
            return match domain {
                "kyc" => FocusMode::KycCase,
                "document" | "requirement" => FocusMode::Proofs,
                "trading-profile" | "custody" => FocusMode::Trading,
                "cbu" | "entity" => FocusMode::CbuManagement,
                _ => continue,
            };
        }
    }

    FocusMode::General
}

// ---------------------------------------------------------------------------
// RecentContext — last N mentions
// ---------------------------------------------------------------------------

/// Recent entity mentions for carry-forward and context.
#[derive(Debug, Clone, Default)]
pub struct RecentContext {
    /// Last N entity mentions (most recent first).
    pub mentions: Vec<RecentMention>,
}

/// A recent entity mention.
#[derive(Debug, Clone)]
pub struct RecentMention {
    pub entity_id: Uuid,
    pub display_name: String,
    pub entity_type: String,
    pub mentioned_at_turn: u32,
}

const MAX_RECENT_MENTIONS: usize = 10;

impl RecentContext {
    /// Add a mention, maintaining the max size.
    pub fn add(&mut self, mention: RecentMention) {
        // Remove duplicate if present.
        self.mentions.retain(|m| m.entity_id != mention.entity_id);
        self.mentions.insert(0, mention);
        self.mentions.truncate(MAX_RECENT_MENTIONS);
    }
}

/// Derive recent context from runbook.
fn derive_recent(runbook: &Runbook) -> RecentContext {
    let mut recent = RecentContext::default();

    // Walk completed entries in reverse to get most recent first.
    for entry in runbook
        .entries
        .iter()
        .rev()
        .filter(|e| e.status == EntryStatus::Completed)
    {
        // Look for entity-id args.
        if let Some(id_str) = entry.args.get("entity-id").or(entry.args.get("entity_id")) {
            if let Ok(id) = Uuid::parse_str(id_str) {
                let name = entry
                    .args
                    .get("entity-name")
                    .or(entry.args.get("name"))
                    .cloned()
                    .unwrap_or_default();
                recent.add(RecentMention {
                    entity_id: id,
                    display_name: name,
                    entity_type: "entity".to_string(),
                    mentioned_at_turn: 0,
                });
            }
        }
    }

    recent
}

// ---------------------------------------------------------------------------
// ExclusionSet — rejected candidates with 3-turn decay
// ---------------------------------------------------------------------------

/// Tracks rejected candidates so they are not re-proposed.
/// Entries decay after 3 turns.
#[derive(Debug, Clone, Default)]
pub struct ExclusionSet {
    pub exclusions: Vec<Exclusion>,
}

/// A single exclusion.
#[derive(Debug, Clone)]
pub struct Exclusion {
    /// The rejected entity or value.
    pub value: String,
    /// Optional entity ID.
    pub entity_id: Option<Uuid>,
    /// Turn when the rejection happened.
    pub rejected_at_turn: u32,
    /// Reason for rejection.
    pub reason: String,
}

/// Number of turns before an exclusion expires.
const EXCLUSION_DECAY_TURNS: u32 = 3;

/// Derive exclusions from `session.exclude` runbook entries.
///
/// Each completed `session.exclude` entry contributes an exclusion.
/// The `rejected_at_turn` is approximated from the entry's position
/// in the runbook (we use the sequence number as a proxy for turn).
fn derive_exclusions(runbook: &Runbook, _current_turn: u32) -> ExclusionSet {
    let mut set = ExclusionSet::default();

    for entry in runbook
        .entries
        .iter()
        .filter(|e| e.verb == "session.exclude" && e.status == EntryStatus::Completed)
    {
        let value = entry.args.get("value").cloned().unwrap_or_default();
        let entity_id = entry
            .args
            .get("entity-id")
            .and_then(|s| Uuid::parse_str(s).ok());
        let reason = entry.args.get("reason").cloned().unwrap_or_default();

        // Use sequence as a proxy for turn number.
        let turn = entry.sequence as u32;

        set.add_from_rejection(value, entity_id, turn, reason);
    }

    set
}

impl ExclusionSet {
    /// Add an exclusion from a user rejection.
    pub fn add_from_rejection(
        &mut self,
        value: String,
        entity_id: Option<Uuid>,
        turn: u32,
        reason: String,
    ) {
        // Don't duplicate.
        if self
            .exclusions
            .iter()
            .any(|e| e.value == value && e.entity_id == entity_id)
        {
            return;
        }
        self.exclusions.push(Exclusion {
            value,
            entity_id,
            rejected_at_turn: turn,
            reason,
        });
    }

    /// Prune expired exclusions.
    pub fn prune(&mut self, current_turn: u32) {
        self.exclusions
            .retain(|e| current_turn.saturating_sub(e.rejected_at_turn) < EXCLUSION_DECAY_TURNS);
    }

    /// Check if a value or entity is excluded.
    pub fn is_excluded(&self, value: &str, entity_id: Option<Uuid>) -> bool {
        self.exclusions
            .iter()
            .any(|e| e.value == value || (entity_id.is_some() && e.entity_id == entity_id))
    }

    /// Check if an entity ID is excluded.
    pub fn is_entity_excluded(&self, entity_id: Uuid) -> bool {
        self.exclusions
            .iter()
            .any(|e| e.entity_id == Some(entity_id))
    }

    /// Whether the set has no exclusions.
    pub fn is_empty(&self) -> bool {
        self.exclusions.is_empty()
    }

    /// Return all active (non-pruned) exclusions.
    pub fn active(&self) -> &[Exclusion] {
        &self.exclusions
    }
}

// ---------------------------------------------------------------------------
// OutcomeRegistry — execution results for @N references
// ---------------------------------------------------------------------------

/// Registry of execution results for @N back-references.
#[derive(Debug, Clone, Default)]
pub struct OutcomeRegistry {
    pub outcomes: HashMap<Uuid, serde_json::Value>,
}

impl OutcomeRegistry {
    /// Get an outcome by entry ID.
    pub fn get(&self, entry_id: Uuid) -> Option<&serde_json::Value> {
        self.outcomes.get(&entry_id)
    }
}

/// Derive outcomes from completed runbook entries.
fn derive_outcomes(runbook: &Runbook) -> OutcomeRegistry {
    let mut registry = OutcomeRegistry::default();
    for entry in runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
    {
        if let Some(ref result) = entry.result {
            registry.outcomes.insert(entry.id, result.clone());
        }
    }
    registry
}

// ---------------------------------------------------------------------------
// Accumulated Answers
// ---------------------------------------------------------------------------

/// Derive accumulated answers from runbook entries.
///
/// Answers are stored as `pack.answer` entries or extracted from
/// entry args with `slot_provenance == UserProvided`.
fn derive_answers(runbook: &Runbook) -> HashMap<String, serde_json::Value> {
    let mut answers = HashMap::new();

    for entry in runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
    {
        // pack.answer entries carry Q&A answers.
        if entry.verb == "pack.answer" {
            if let (Some(field), Some(value)) = (entry.args.get("field"), entry.args.get("value")) {
                answers.insert(field.clone(), serde_json::Value::String(value.clone()));
            }
        }
    }

    answers
}

// ---------------------------------------------------------------------------
// Executed / Staged Verb Sets — for precondition evaluation
// ---------------------------------------------------------------------------

/// Collect FQNs of verbs that have been executed (Completed) in the runbook.
fn derive_executed_verbs(runbook: &Runbook) -> HashSet<String> {
    runbook
        .entries
        .iter()
        .filter(|e| e.status == EntryStatus::Completed)
        .map(|e| e.verb.clone())
        .collect()
}

/// Collect FQNs of verbs that are staged but not yet executed.
///
/// Staged means Proposed, Confirmed, or Resolved — visible in Plan mode
/// but not yet facts in Executable mode.
fn derive_staged_verbs(runbook: &Runbook) -> HashSet<String> {
    runbook
        .entries
        .iter()
        .filter(|e| {
            matches!(
                e.status,
                EntryStatus::Proposed | EntryStatus::Confirmed | EntryStatus::Resolved
            )
        })
        .map(|e| e.verb.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Pack Handoff — suggest next pack when current is complete
// ---------------------------------------------------------------------------

#[cfg(test)]
/// A suggestion to hand off to a different pack after the current one completes.
#[derive(Debug, Clone)]
pub struct PackHandoffSuggestion {
    /// The pack that completed.
    pub completed_pack_id: String,
    /// The suggested next pack (from `handoff_target`).
    pub suggested_pack_id: String,
    /// Outcome references (`@N`) carried forward from the completed pack.
    pub outcome_refs: Vec<OutcomeRef>,
}

/// An outcome reference that carries forward across packs.
#[derive(Debug, Clone)]
pub struct OutcomeRef {
    /// The entry ID that produced this outcome.
    pub entry_id: Uuid,
    /// The verb that produced the outcome.
    pub verb: String,
    /// A human-readable label for the outcome.
    pub label: String,
    /// Key result values (e.g., `{"case_id": "uuid-..."}`)
    pub values: HashMap<String, String>,
}

impl ContextStack {
    /// Check if the active pack is complete and has a handoff target.
    ///
    /// A pack is "complete" when:
    /// 1. A template is active, AND
    /// 2. All template entries are Completed (no Proposed/Confirmed/Resolved remain)
    ///
    /// Returns a handoff suggestion with outcome refs from the completed pack.
    #[cfg(test)]
    pub fn check_pack_handoff(&self, runbook: &Runbook) -> Option<PackHandoffSuggestion> {
        let pack = self.active_pack()?;

        // Must have a template and it must be fully completed.
        let template_id = runbook.template_id.as_ref()?;

        let template_entries: Vec<&RunbookEntry> = runbook
            .entries
            .iter()
            .filter(|e| {
                e.labels.get("template_id") == Some(template_id)
                    && e.status != EntryStatus::Disabled
            })
            .collect();

        if template_entries.is_empty() {
            return None;
        }

        // Check if all non-disabled template entries are completed.
        let all_completed = template_entries
            .iter()
            .all(|e| e.status == EntryStatus::Completed);

        if !all_completed {
            return None;
        }

        // We need a handoff target from the manifest.
        // Try to find the manifest's handoff_target via PackContext.
        // The PackContext itself doesn't store handoff_target, but we can
        // derive it from the router or from a stored field. For now, check
        // if the pack_id has a known handoff target in the runbook metadata.
        let handoff_target = derive_handoff_target(runbook, &pack.pack_id)?;

        // Build outcome refs from completed template entries.
        let outcome_refs = template_entries
            .iter()
            .filter(|e| e.result.is_some())
            .map(|e| {
                let mut values = HashMap::new();
                if let Some(result) = &e.result {
                    if let Some(obj) = result.as_object() {
                        for (k, v) in obj {
                            if let Some(s) = v.as_str() {
                                values.insert(k.clone(), s.to_string());
                            }
                        }
                    }
                }
                OutcomeRef {
                    entry_id: e.id,
                    verb: e.verb.clone(),
                    label: e.sentence.clone(),
                    values,
                }
            })
            .collect();

        Some(PackHandoffSuggestion {
            completed_pack_id: pack.pack_id.clone(),
            suggested_pack_id: handoff_target,
            outcome_refs,
        })
    }
}

#[cfg(test)]
/// Derive handoff target from runbook metadata.
///
/// Looks for a `pack.select` entry that stored a `handoff-target` arg,
/// or checks the manifest if available through the router.
fn derive_handoff_target(runbook: &Runbook, pack_id: &str) -> Option<String> {
    // Check for explicit handoff-target in the pack.select entry.
    runbook
        .entries
        .iter()
        .rev()
        .find(|e| {
            e.verb == "pack.select"
                && e.status == EntryStatus::Completed
                && e.args.get("pack-id").map(|s| s.as_str()) == Some(pack_id)
        })
        .and_then(|e| e.args.get("handoff-target").cloned())
}

// ---------------------------------------------------------------------------
// Canonicalization — pre-ML normalization
// ---------------------------------------------------------------------------

/// Jurisdiction canonical forms.
pub static JURISDICTION_CANON: &[(&str, &str)] = &[
    ("luxembourg", "LU"),
    ("lux", "LU"),
    ("ireland", "IE"),
    ("ire", "IE"),
    ("germany", "DE"),
    ("ger", "DE"),
    ("united kingdom", "UK"),
    ("uk", "UK"),
    ("great britain", "UK"),
    ("gb", "UK"),
    ("united states", "US"),
    ("usa", "US"),
    ("us", "US"),
    ("france", "FR"),
    ("fra", "FR"),
    ("netherlands", "NL"),
    ("holland", "NL"),
    ("switzerland", "CH"),
    ("swiss", "CH"),
    ("belgium", "BE"),
    ("italy", "IT"),
    ("spain", "ES"),
    ("austria", "AT"),
    ("sweden", "SE"),
    ("denmark", "DK"),
    ("norway", "NO"),
    ("finland", "FI"),
    ("portugal", "PT"),
    ("singapore", "SG"),
    ("hong kong", "HK"),
    ("japan", "JP"),
    ("cayman islands", "KY"),
    ("cayman", "KY"),
    ("jersey", "JE"),
    ("guernsey", "GG"),
    ("bermuda", "BM"),
    ("mauritius", "MU"),
];

/// Legal form canonical forms.
pub static LEGAL_FORM_CANON: &[(&str, &str)] = &[
    ("s.a.", "SA"),
    ("sa", "SA"),
    ("s.à r.l.", "SARL"),
    ("sarl", "SARL"),
    ("s.a.r.l.", "SARL"),
    ("gmbh", "GMBH"),
    ("g.m.b.h.", "GMBH"),
    ("ag", "AG"),
    ("a.g.", "AG"),
    ("plc", "PLC"),
    ("p.l.c.", "PLC"),
    ("ltd", "LTD"),
    ("ltd.", "LTD"),
    ("limited", "LTD"),
    ("inc", "INC"),
    ("inc.", "INC"),
    ("incorporated", "INC"),
    ("corp", "CORP"),
    ("corp.", "CORP"),
    ("corporation", "CORP"),
    ("llc", "LLC"),
    ("l.l.c.", "LLC"),
    ("lp", "LP"),
    ("l.p.", "LP"),
    ("limited partnership", "LP"),
    ("se", "SE"),
    ("sicav", "SICAV"),
    ("s.i.c.a.v.", "SICAV"),
    ("sif", "SIF"),
    ("raif", "RAIF"),
    ("fcp", "FCP"),
    ("f.c.p.", "FCP"),
    ("bv", "BV"),
    ("b.v.", "BV"),
    ("nv", "NV"),
    ("n.v.", "NV"),
    ("pty", "PTY"),
    ("pty.", "PTY"),
    ("co", "CO"),
    ("co.", "CO"),
    ("company", "CO"),
];

/// Canonicalize a mention — normalize jurisdiction, legal form, and role names.
///
/// This is pure string processing, zero latency, no ML involved.
/// Returns the canonicalized form if a match is found, otherwise
/// returns the input unchanged.
pub fn canonicalize_mention(input: &str) -> String {
    let lower = input.to_lowercase();
    let trimmed = lower.trim();

    // Try jurisdiction.
    for (variant, canon) in JURISDICTION_CANON {
        if trimmed == *variant {
            return canon.to_string();
        }
    }

    // Try legal form.
    for (variant, canon) in LEGAL_FORM_CANON {
        if trimmed == *variant {
            return canon.to_string();
        }
    }

    // Try role synonym.
    for (short, canonical) in ROLE_SYNONYMS {
        if trimmed == *short {
            return canonical.to_string();
        }
    }

    // No match — return as-is (not lowered, preserve original).
    input.to_string()
}

// ---------------------------------------------------------------------------
// ContextEntry — weighted decay for context sources
// ---------------------------------------------------------------------------

/// Source of a context value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSource {
    /// User explicitly said this.
    UserExplicit,
    /// System resolved this (e.g., entity resolution).
    SystemResolved,
    /// Carried forward from a previous step.
    CarriedForward,
    /// Inferred from template context.
    TemplateInferred,
}

/// A context entry with weighted decay.
#[derive(Debug, Clone)]
pub struct ContextEntry {
    pub value: String,
    pub source: ContextSource,
    pub set_at_turn: u32,
}

impl ContextEntry {
    /// Compute the current weight of this entry.
    ///
    /// Weight decays over turns:
    /// - UserExplicit: 1.0 (no decay)
    /// - SystemResolved: 0.9 × decay
    /// - CarriedForward: 0.7 × decay
    /// - TemplateInferred: 0.5 × decay
    ///
    /// Decay factor: 0.9^(current_turn - set_at_turn)
    pub fn weight(&self, current_turn: u32) -> f32 {
        let base = match self.source {
            ContextSource::UserExplicit => 1.0,
            ContextSource::SystemResolved => 0.9,
            ContextSource::CarriedForward => 0.7,
            ContextSource::TemplateInferred => 0.5,
        };

        let age = current_turn.saturating_sub(self.set_at_turn) as f32;
        let decay = 0.9_f32.powf(age);

        base * decay
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::runbook::RunbookEntry;

    fn empty_runbook() -> Runbook {
        Runbook::new(Uuid::new_v4())
    }

    fn runbook_with_scope() -> Runbook {
        let mut rb = Runbook::new(Uuid::new_v4());
        let group_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        rb.client_group_id = Some(group_id);

        let mut entry = RunbookEntry::new(
            "session.load-cluster".to_string(),
            "Load Allianz book".to_string(),
            "(session.load-cluster :client <Allianz>)".to_string(),
        );
        entry
            .args
            .insert("client".to_string(), "Allianz".to_string());
        entry.status = EntryStatus::Completed;

        let cbu1 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let cbu2 = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        entry.result = Some(serde_json::json!({
            "cbu_ids": [cbu1.to_string(), cbu2.to_string()]
        }));

        rb.add_entry(entry);
        rb
    }

    fn runbook_with_pack() -> Runbook {
        let mut rb = runbook_with_scope();
        rb.pack_id = Some("kyc-case".to_string());
        rb.pack_version = Some("1.0".to_string());
        rb.template_id = Some("standard-kyc".to_string());
        rb
    }

    #[test]
    fn test_from_runbook_empty() {
        let rb = empty_runbook();
        let ctx = ContextStack::from_runbook(&rb, None, 0);

        assert!(ctx.derived_scope.client_group_id.is_none());
        assert!(ctx.derived_scope.client_group_name.is_none());
        assert!(ctx.derived_scope.default_cbu.is_none());
        assert!(ctx.derived_scope.loaded_cbu_ids.is_empty());
        assert!(ctx.active_pack().is_none());
        assert!(ctx.template_hint.is_none());
        assert!(ctx.focus.entity.is_none());
        assert!(ctx.exclusions.exclusions.is_empty());
        assert!(ctx.accumulated_answers.is_empty());
    }

    #[test]
    fn test_from_runbook_with_scope() {
        let rb = runbook_with_scope();
        let ctx = ContextStack::from_runbook(&rb, None, 1);

        assert!(ctx.derived_scope.client_group_id.is_some());
        assert_eq!(
            ctx.derived_scope.client_group_name.as_deref(),
            Some("Allianz")
        );
        assert_eq!(ctx.derived_scope.loaded_cbu_ids.len(), 2);
        assert!(ctx.derived_scope.default_cbu.is_some());
    }

    #[test]
    fn test_from_runbook_with_pack() {
        let rb = runbook_with_pack();
        let ctx = ContextStack::from_runbook(&rb, None, 1);

        // Pack executed should be derived from runbook metadata.
        assert!(ctx.pack_executed.is_some());
        assert_eq!(ctx.pack_executed.as_ref().unwrap().pack_id, "kyc-case");
    }

    #[test]
    fn test_active_pack_staged_over_executed() {
        let rb = runbook_with_pack();

        let manifest = PackManifest {
            id: "book-setup".to_string(),
            name: "Book Setup".to_string(),
            version: "2.0".to_string(),
            description: "Setup book".to_string(),
            allowed_verbs: vec!["cbu.create".to_string()],
            forbidden_verbs: vec!["kyc.delete".to_string()],
            ..default_pack_manifest()
        };

        let ctx = ContextStack::from_runbook(&rb, Some(Arc::new(manifest)), 1);

        // Staged should win over executed.
        let active = ctx.active_pack().unwrap();
        assert_eq!(active.pack_id, "book-setup");
        assert_eq!(active.pack_version, "2.0");
    }

    #[test]
    fn test_is_verb_allowed() {
        let rb = empty_runbook();
        let manifest = PackManifest {
            id: "test".to_string(),
            forbidden_verbs: vec!["dangerous.delete".to_string()],
            ..default_pack_manifest()
        };

        let ctx = ContextStack::from_runbook(&rb, Some(Arc::new(manifest)), 0);

        assert!(ctx.is_verb_allowed("cbu.create"));
        assert!(!ctx.is_verb_allowed("dangerous.delete"));
    }

    #[test]
    fn test_is_verb_in_pack() {
        let rb = empty_runbook();
        let manifest = PackManifest {
            id: "test".to_string(),
            allowed_verbs: vec!["kyc.add-entity".to_string(), "kyc.create-case".to_string()],
            ..default_pack_manifest()
        };

        let ctx = ContextStack::from_runbook(&rb, Some(Arc::new(manifest)), 0);

        assert!(ctx.is_verb_in_pack("kyc.add-entity"));
        assert!(!ctx.is_verb_in_pack("cbu.create"));
    }

    // -- Pronoun resolution tests --

    #[test]
    fn test_pronoun_resolution_entity() {
        let mut focus = FocusContext::default();
        let id = Uuid::new_v4();
        focus.set_entity(id, "Allianz SE".to_string(), "company".to_string(), 1);

        let resolved = focus.resolve_pronoun("it");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, id);

        let resolved = focus.resolve_pronoun("that");
        assert!(resolved.is_some());

        let resolved = focus.resolve_pronoun("the entity");
        assert!(resolved.is_some());
    }

    #[test]
    fn test_pronoun_resolution_cbu() {
        let mut focus = FocusContext::default();
        let id = Uuid::new_v4();
        focus.set_cbu(id, "Allianz Lux SICAV".to_string(), 1);

        let resolved = focus.resolve_pronoun("the fund");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, id);

        let resolved = focus.resolve_pronoun("the structure");
        assert!(resolved.is_some());
    }

    #[test]
    fn test_pronoun_resolution_case() {
        let mut focus = FocusContext::default();
        let id = Uuid::new_v4();
        focus.set_case(id, "KYC-2024-001".to_string(), 1);

        let resolved = focus.resolve_pronoun("the case");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, id);

        let resolved = focus.resolve_pronoun("the kyc case");
        assert!(resolved.is_some());
    }

    #[test]
    fn test_pronoun_resolution_role_shorthand() {
        let mut focus = FocusContext::default();
        let id = Uuid::new_v4();
        focus.set_entity(
            id,
            "BNY Mellon".to_string(),
            "management_company".to_string(),
            1,
        );

        let resolved = focus.resolve_pronoun("the manco");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, id);
    }

    #[test]
    fn test_pronoun_no_focus_returns_none() {
        let focus = FocusContext::default();
        assert!(focus.resolve_pronoun("it").is_none());
        assert!(focus.resolve_pronoun("the fund").is_none());
    }

    #[test]
    fn test_role_synonym_resolution() {
        assert_eq!(FocusContext::resolve_role("ta"), Some("transfer_agent"));
        assert_eq!(FocusContext::resolve_role("im"), Some("investment_manager"));
        assert_eq!(FocusContext::resolve_role("gp"), Some("general_partner"));
        assert_eq!(
            FocusContext::resolve_role("manco"),
            Some("management_company")
        );
        assert_eq!(FocusContext::resolve_role("dp"), Some("depositary"));
        assert_eq!(FocusContext::resolve_role("custodian"), Some("depositary"));
        assert_eq!(FocusContext::resolve_role("unknown"), None);
    }

    // -- Exclusion tests --

    #[test]
    fn test_exclusion_add_and_check() {
        let mut exclusions = ExclusionSet::default();
        let id = Uuid::new_v4();
        exclusions.add_from_rejection(
            "Goldman Sachs".to_string(),
            Some(id),
            5,
            "wrong entity".to_string(),
        );

        assert!(exclusions.is_excluded("Goldman Sachs", Some(id)));
        assert!(exclusions.is_entity_excluded(id));
        assert!(!exclusions.is_excluded("Morgan Stanley", None));
    }

    #[test]
    fn test_exclusion_decay() {
        let mut exclusions = ExclusionSet::default();
        exclusions.add_from_rejection("old reject".to_string(), None, 1, "rejected".to_string());
        exclusions.add_from_rejection("recent reject".to_string(), None, 4, "rejected".to_string());

        // At turn 4, "old reject" (turn 1) has age 3 → expired.
        exclusions.prune(4);
        assert!(!exclusions.is_excluded("old reject", None));
        assert!(exclusions.is_excluded("recent reject", None));
    }

    #[test]
    fn test_exclusion_no_duplicates() {
        let mut exclusions = ExclusionSet::default();
        let id = Uuid::new_v4();
        exclusions.add_from_rejection("X".to_string(), Some(id), 1, "r".to_string());
        exclusions.add_from_rejection("X".to_string(), Some(id), 2, "r".to_string());

        assert_eq!(exclusions.exclusions.len(), 1);
    }

    // -- Canonicalization tests --

    #[test]
    fn test_canonicalize_jurisdiction() {
        assert_eq!(canonicalize_mention("Luxembourg"), "LU");
        assert_eq!(canonicalize_mention("luxembourg"), "LU");
        assert_eq!(canonicalize_mention("lux"), "LU");
        assert_eq!(canonicalize_mention("Ireland"), "IE");
        assert_eq!(canonicalize_mention("united kingdom"), "UK");
        assert_eq!(canonicalize_mention("usa"), "US");
        assert_eq!(canonicalize_mention("Cayman Islands"), "KY");
        assert_eq!(canonicalize_mention("cayman"), "KY");
        assert_eq!(canonicalize_mention("switzerland"), "CH");
        assert_eq!(canonicalize_mention("hong kong"), "HK");
    }

    #[test]
    fn test_canonicalize_legal_form() {
        assert_eq!(canonicalize_mention("s.a."), "SA");
        assert_eq!(canonicalize_mention("S.A."), "SA");
        assert_eq!(canonicalize_mention("gmbh"), "GMBH");
        assert_eq!(canonicalize_mention("plc"), "PLC");
        assert_eq!(canonicalize_mention("ltd."), "LTD");
        assert_eq!(canonicalize_mention("limited"), "LTD");
        assert_eq!(canonicalize_mention("incorporated"), "INC");
        assert_eq!(canonicalize_mention("sicav"), "SICAV");
        assert_eq!(canonicalize_mention("s.à r.l."), "SARL");
    }

    #[test]
    fn test_canonicalize_role() {
        assert_eq!(canonicalize_mention("ta"), "transfer_agent");
        assert_eq!(canonicalize_mention("im"), "investment_manager");
        assert_eq!(canonicalize_mention("gp"), "general_partner");
    }

    #[test]
    fn test_canonicalize_unknown() {
        assert_eq!(canonicalize_mention("Allianz SE"), "Allianz SE");
        assert_eq!(canonicalize_mention("random text"), "random text");
    }

    // -- Context weight decay tests --

    #[test]
    fn test_context_entry_weight_user_explicit_no_decay() {
        let entry = ContextEntry {
            value: "test".to_string(),
            source: ContextSource::UserExplicit,
            set_at_turn: 0,
        };
        // UserExplicit has base 1.0 and decay is 0.9^0 = 1.0.
        assert!((entry.weight(0) - 1.0).abs() < f32::EPSILON);
        // Even after 5 turns, UserExplicit doesn't fully decay.
        assert!((entry.weight(5) - 0.9_f32.powi(5)).abs() < 0.001);
    }

    #[test]
    fn test_context_entry_weight_system_resolved() {
        let entry = ContextEntry {
            value: "test".to_string(),
            source: ContextSource::SystemResolved,
            set_at_turn: 0,
        };
        assert!((entry.weight(0) - 0.9).abs() < f32::EPSILON);
        // After 1 turn: 0.9 × 0.9 = 0.81
        assert!((entry.weight(1) - 0.81).abs() < 0.001);
    }

    #[test]
    fn test_context_entry_weight_carried_forward() {
        let entry = ContextEntry {
            value: "test".to_string(),
            source: ContextSource::CarriedForward,
            set_at_turn: 0,
        };
        assert!((entry.weight(0) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_context_entry_weight_template_inferred() {
        let entry = ContextEntry {
            value: "test".to_string(),
            source: ContextSource::TemplateInferred,
            set_at_turn: 0,
        };
        assert!((entry.weight(0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_context_entry_weight_decay_over_turns() {
        let entry = ContextEntry {
            value: "test".to_string(),
            source: ContextSource::SystemResolved,
            set_at_turn: 2,
        };
        // At turn 2: age=0, weight = 0.9 × 1.0 = 0.9
        assert!((entry.weight(2) - 0.9).abs() < 0.001);
        // At turn 5: age=3, weight = 0.9 × 0.9^3 = 0.9 × 0.729 = 0.6561
        assert!((entry.weight(5) - 0.6561).abs() < 0.001);
    }

    // -- Recent context tests --

    #[test]
    fn test_recent_context_dedup() {
        let mut recent = RecentContext::default();
        let id = Uuid::new_v4();
        recent.add(RecentMention {
            entity_id: id,
            display_name: "First".to_string(),
            entity_type: "entity".to_string(),
            mentioned_at_turn: 1,
        });
        recent.add(RecentMention {
            entity_id: id,
            display_name: "First (updated)".to_string(),
            entity_type: "entity".to_string(),
            mentioned_at_turn: 2,
        });

        // Should have 1 entry (deduped by entity_id), with updated name.
        assert_eq!(recent.mentions.len(), 1);
        assert_eq!(recent.mentions[0].display_name, "First (updated)");
    }

    #[test]
    fn test_recent_context_max_size() {
        let mut recent = RecentContext::default();
        for i in 0..15 {
            recent.add(RecentMention {
                entity_id: Uuid::new_v4(),
                display_name: format!("Entity {}", i),
                entity_type: "entity".to_string(),
                mentioned_at_turn: i,
            });
        }
        assert_eq!(recent.mentions.len(), MAX_RECENT_MENTIONS);
        // Most recent should be first.
        assert_eq!(recent.mentions[0].display_name, "Entity 14");
    }

    // -- Outcome registry tests --

    #[test]
    fn test_outcome_registry() {
        let mut rb = empty_runbook();
        let mut entry = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create CBU".to_string(),
            "(cbu.create)".to_string(),
        );
        entry.status = EntryStatus::Completed;
        entry.result = Some(serde_json::json!({"cbu_id": "abc-123"}));
        let id = rb.add_entry(entry);

        let ctx = ContextStack::from_runbook(&rb, None, 0);
        let outcome = ctx.outcomes.get(id);
        assert!(outcome.is_some());
        assert_eq!(outcome.unwrap()["cbu_id"], "abc-123");
    }

    // -- Accumulated answers tests --

    #[test]
    fn test_accumulated_answers() {
        let mut rb = empty_runbook();

        let mut entry = RunbookEntry::new(
            "pack.answer".to_string(),
            "Answer: jurisdiction = LU".to_string(),
            "(pack.answer :field jurisdiction :value LU)".to_string(),
        );
        entry.status = EntryStatus::Completed;
        entry
            .args
            .insert("field".to_string(), "jurisdiction".to_string());
        entry.args.insert("value".to_string(), "LU".to_string());
        rb.add_entry(entry);

        let ctx = ContextStack::from_runbook(&rb, None, 0);
        assert_eq!(
            ctx.accumulated_answers.get("jurisdiction"),
            Some(&serde_json::Value::String("LU".to_string()))
        );
    }

    // -- Pack context tests --

    #[test]
    fn test_pack_context_from_manifest() {
        let manifest = PackManifest {
            id: "kyc-case".to_string(),
            version: "1.0".to_string(),
            allowed_verbs: vec![
                "kyc.create-case".to_string(),
                "kyc.add-entity".to_string(),
                "entity.ensure-person".to_string(),
            ],
            forbidden_verbs: vec!["cbu.delete".to_string()],
            ..default_pack_manifest()
        };

        let ctx = PackContext::from_manifest(&manifest);
        assert_eq!(ctx.pack_id, "kyc-case");
        assert!(ctx.allowed_verbs.contains("kyc.create-case"));
        assert!(ctx.forbidden_verbs.contains("cbu.delete"));
        assert_eq!(ctx.dominant_domain.as_deref(), Some("kyc"));
    }

    #[test]
    fn test_dominant_domain_most_frequent() {
        let mut verbs = HashSet::new();
        verbs.insert("kyc.a".to_string());
        verbs.insert("kyc.b".to_string());
        verbs.insert("kyc.c".to_string());
        verbs.insert("entity.x".to_string());
        verbs.insert("cbu.y".to_string());

        let domain = derive_dominant_domain(&verbs);
        assert_eq!(domain.as_deref(), Some("kyc"));
    }

    // -- Phase B: pack.select runbook entry → pack_executed tests --

    #[test]
    fn test_pack_select_entry_derives_pack_executed() {
        let mut rb = empty_runbook();

        let mut entry = RunbookEntry::new(
            "pack.select".to_string(),
            "Select journey: KYC Case Management".to_string(),
            "(pack.select :pack-id \"kyc-case\" :pack-version \"1.0\" :manifest-hash \"abc123\")"
                .to_string(),
        );
        entry.status = EntryStatus::Completed;
        entry
            .args
            .insert("pack-id".to_string(), "kyc-case".to_string());
        entry
            .args
            .insert("pack-version".to_string(), "1.0".to_string());
        entry
            .args
            .insert("manifest-hash".to_string(), "abc123".to_string());
        entry.result = Some(serde_json::json!({
            "pack_id": "kyc-case",
            "pack_name": "KYC Case Management",
            "pack_version": "1.0",
        }));
        rb.add_entry(entry);

        let ctx = ContextStack::from_runbook(&rb, None, 1);

        // pack_executed should be derived from the pack.select entry.
        assert!(ctx.pack_executed.is_some());
        let pack = ctx.pack_executed.as_ref().unwrap();
        assert_eq!(pack.pack_id, "kyc-case");
        assert_eq!(pack.pack_version, "1.0");
    }

    #[test]
    fn test_pack_select_last_wins() {
        let mut rb = empty_runbook();

        // First pack selection.
        let mut entry1 = RunbookEntry::new(
            "pack.select".to_string(),
            "Select KYC".to_string(),
            "(pack.select :pack-id \"kyc-case\")".to_string(),
        );
        entry1.status = EntryStatus::Completed;
        entry1
            .args
            .insert("pack-id".to_string(), "kyc-case".to_string());
        rb.add_entry(entry1);

        // Handoff to book-setup.
        let mut entry2 = RunbookEntry::new(
            "pack.select".to_string(),
            "Select Book Setup".to_string(),
            "(pack.select :pack-id \"book-setup\")".to_string(),
        );
        entry2.status = EntryStatus::Completed;
        entry2
            .args
            .insert("pack-id".to_string(), "book-setup".to_string());
        rb.add_entry(entry2);

        let ctx = ContextStack::from_runbook(&rb, None, 2);

        // The LAST pack.select should win.
        assert!(ctx.pack_executed.is_some());
        assert_eq!(ctx.pack_executed.as_ref().unwrap().pack_id, "book-setup");
    }

    #[test]
    fn test_multiple_pack_answers_accumulate() {
        let mut rb = empty_runbook();

        // First answer.
        let mut a1 = RunbookEntry::new(
            "pack.answer".to_string(),
            "Answer: client_name = Allianz".to_string(),
            "(pack.answer :field \"client_name\" :value \"Allianz\")".to_string(),
        );
        a1.status = EntryStatus::Completed;
        a1.args
            .insert("field".to_string(), "client_name".to_string());
        a1.args.insert("value".to_string(), "Allianz".to_string());
        rb.add_entry(a1);

        // Second answer.
        let mut a2 = RunbookEntry::new(
            "pack.answer".to_string(),
            "Answer: jurisdiction = LU".to_string(),
            "(pack.answer :field \"jurisdiction\" :value \"LU\")".to_string(),
        );
        a2.status = EntryStatus::Completed;
        a2.args
            .insert("field".to_string(), "jurisdiction".to_string());
        a2.args.insert("value".to_string(), "LU".to_string());
        rb.add_entry(a2);

        // Third answer — updates first field.
        let mut a3 = RunbookEntry::new(
            "pack.answer".to_string(),
            "Answer: client_name = Aviva".to_string(),
            "(pack.answer :field \"client_name\" :value \"Aviva\")".to_string(),
        );
        a3.status = EntryStatus::Completed;
        a3.args
            .insert("field".to_string(), "client_name".to_string());
        a3.args.insert("value".to_string(), "Aviva".to_string());
        rb.add_entry(a3);

        let ctx = ContextStack::from_runbook(&rb, None, 3);

        // Should have 2 distinct fields (last write wins for client_name).
        assert_eq!(ctx.accumulated_answers.len(), 2);
        assert_eq!(
            ctx.accumulated_answers.get("client_name"),
            Some(&serde_json::Value::String("Aviva".to_string()))
        );
        assert_eq!(
            ctx.accumulated_answers.get("jurisdiction"),
            Some(&serde_json::Value::String("LU".to_string()))
        );
    }

    #[test]
    fn test_full_session_fold_scope_pack_answers() {
        let mut rb = Runbook::new(Uuid::new_v4());
        let group_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        rb.client_group_id = Some(group_id);

        // 1. Scope entry.
        let mut scope = RunbookEntry::new(
            "session.load-cluster".to_string(),
            "Load Allianz".to_string(),
            "(session.load-cluster :client <Allianz>)".to_string(),
        );
        scope
            .args
            .insert("client".to_string(), "Allianz".to_string());
        scope.status = EntryStatus::Completed;
        rb.add_entry(scope);

        // 2. Pack selection.
        let mut pack = RunbookEntry::new(
            "pack.select".to_string(),
            "Select KYC".to_string(),
            "(pack.select :pack-id \"kyc-case\")".to_string(),
        );
        pack.args
            .insert("pack-id".to_string(), "kyc-case".to_string());
        pack.args
            .insert("pack-version".to_string(), "1.0".to_string());
        pack.status = EntryStatus::Completed;
        rb.add_entry(pack);

        // 3. Answer.
        let mut answer = RunbookEntry::new(
            "pack.answer".to_string(),
            "Answer: entity_name = Acme".to_string(),
            "(pack.answer :field \"entity_name\" :value \"Acme\")".to_string(),
        );
        answer
            .args
            .insert("field".to_string(), "entity_name".to_string());
        answer.args.insert("value".to_string(), "Acme".to_string());
        answer.status = EntryStatus::Completed;
        rb.add_entry(answer);

        // 4. Domain verb.
        let mut verb = RunbookEntry::new(
            "kyc.add-entity".to_string(),
            "Add Acme".to_string(),
            "(kyc.add-entity :name \"Acme\")".to_string(),
        );
        verb.status = EntryStatus::Completed;
        rb.add_entry(verb);

        let ctx = ContextStack::from_runbook(&rb, None, 4);

        // Scope derived.
        assert_eq!(
            ctx.derived_scope.client_group_name.as_deref(),
            Some("Allianz")
        );

        // Pack derived.
        assert!(ctx.pack_executed.is_some());
        assert_eq!(ctx.pack_executed.as_ref().unwrap().pack_id, "kyc-case");

        // Answer accumulated.
        assert_eq!(
            ctx.accumulated_answers.get("entity_name"),
            Some(&serde_json::Value::String("Acme".to_string()))
        );

        // No staged pack, so active_pack should be the executed one.
        assert_eq!(ctx.active_pack().unwrap().pack_id, "kyc-case");
    }

    // -- Exclusion fold tests --

    #[test]
    fn test_exclusions_derived_from_session_exclude_entries() {
        let mut rb = empty_runbook();

        let excluded_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

        let mut entry = RunbookEntry::new(
            "session.exclude".to_string(),
            "Excluded: Goldman Sachs (wrong entity)".to_string(),
            "(session.exclude :value \"Goldman Sachs\" :entity-id \"aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\" :reason \"wrong entity\")".to_string(),
        );
        entry.status = EntryStatus::Completed;
        entry
            .args
            .insert("value".to_string(), "Goldman Sachs".to_string());
        entry
            .args
            .insert("entity-id".to_string(), excluded_id.to_string());
        entry
            .args
            .insert("reason".to_string(), "wrong entity".to_string());
        rb.add_entry(entry);

        let ctx = ContextStack::from_runbook(&rb, None, 1);

        assert!(ctx
            .exclusions
            .is_excluded("Goldman Sachs", Some(excluded_id)));
        assert!(ctx.exclusions.is_entity_excluded(excluded_id));
        assert!(!ctx.exclusions.is_excluded("Morgan Stanley", None));
    }

    #[test]
    fn test_exclusions_pruned_by_turn() {
        let mut rb = empty_runbook();

        // First exclusion — gets sequence 0 after renumber.
        let mut entry1 = RunbookEntry::new(
            "session.exclude".to_string(),
            "Excluded: Old Corp".to_string(),
            "(session.exclude :value \"Old Corp\")".to_string(),
        );
        entry1.status = EntryStatus::Completed;
        entry1
            .args
            .insert("value".to_string(), "Old Corp".to_string());
        entry1
            .args
            .insert("reason".to_string(), "rejected".to_string());
        rb.add_entry(entry1);

        // Second exclusion — gets sequence 1 after renumber.
        let mut entry2 = RunbookEntry::new(
            "session.exclude".to_string(),
            "Excluded: Recent Corp".to_string(),
            "(session.exclude :value \"Recent Corp\")".to_string(),
        );
        entry2.status = EntryStatus::Completed;
        entry2
            .args
            .insert("value".to_string(), "Recent Corp".to_string());
        entry2
            .args
            .insert("reason".to_string(), "rejected".to_string());
        rb.add_entry(entry2);

        // After renumber: entry1=seq 1, entry2=seq 2 (1-based).
        // At turn 4: exclusion from seq 1 has age=4-1=3 (>=3, pruned),
        //            exclusion from seq 2 has age=4-2=2 (<3, survives).
        let ctx = ContextStack::from_runbook(&rb, None, 4);

        assert!(!ctx.exclusions.is_excluded("Old Corp", None));
        assert!(ctx.exclusions.is_excluded("Recent Corp", None));
    }

    // -- Helper --

    // -- Phase E: Enhanced template step hint tests --

    #[test]
    fn test_template_hint_total_steps_and_section() {
        let mut rb = empty_runbook();
        rb.template_id = Some("kyc-flow".to_string());

        // Step 1: completed, section "setup"
        let mut e1 = RunbookEntry::new(
            "kyc.create-case".to_string(),
            "Create case".to_string(),
            "(kyc.create-case)".to_string(),
        );
        e1.status = EntryStatus::Completed;
        e1.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        e1.labels.insert("section".to_string(), "setup".to_string());
        rb.add_entry(e1);

        // Step 2: proposed, section "entities"
        let mut e2 = RunbookEntry::new(
            "kyc.add-entity".to_string(),
            "Add entity".to_string(),
            "(kyc.add-entity)".to_string(),
        );
        e2.status = EntryStatus::Proposed;
        e2.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        e2.labels
            .insert("section".to_string(), "entities".to_string());
        rb.add_entry(e2);

        // Step 3: proposed, section "entities"
        let mut e3 = RunbookEntry::new(
            "kyc.add-entity".to_string(),
            "Add another entity".to_string(),
            "(kyc.add-entity)".to_string(),
        );
        e3.status = EntryStatus::Proposed;
        e3.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        e3.labels
            .insert("section".to_string(), "entities".to_string());
        rb.add_entry(e3);

        let ctx = ContextStack::from_runbook(&rb, None, 3);
        let hint = ctx.template_hint.unwrap();

        assert_eq!(hint.template_id, "kyc-flow");
        assert_eq!(hint.step_index, 1); // 1 completed
        assert_eq!(hint.total_steps, 3); // 3 total
        assert_eq!(hint.expected_verb, "kyc.add-entity");
        assert_eq!(hint.section.as_deref(), Some("entities"));
        assert_eq!(hint.section_progress, Some((0, 2))); // 0 of 2 done in "entities"
    }

    #[test]
    fn test_template_hint_progress_label() {
        let hint = TemplateStepHint {
            template_id: "test".to_string(),
            step_index: 2,
            total_steps: 8,
            expected_verb: "cbu.create".to_string(),
            next_entry_id: Uuid::new_v4(),
            section: Some("products".to_string()),
            section_progress: Some((1, 3)),
            carry_forward_args: HashMap::new(),
        };

        assert_eq!(hint.progress_label(), "Step 3 of 8 (products: 1/3)");
    }

    #[test]
    fn test_template_hint_progress_label_no_section() {
        let hint = TemplateStepHint {
            template_id: "test".to_string(),
            step_index: 0,
            total_steps: 5,
            expected_verb: "cbu.create".to_string(),
            next_entry_id: Uuid::new_v4(),
            section: None,
            section_progress: None,
            carry_forward_args: HashMap::new(),
        };

        assert_eq!(hint.progress_label(), "Step 1 of 5");
    }

    #[test]
    fn test_carry_forward_includes_result_values() {
        let mut rb = empty_runbook();

        let mut e = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create CBU".to_string(),
            "(cbu.create)".to_string(),
        );
        e.status = EntryStatus::Completed;
        e.args.insert("name".to_string(), "Allianz Lux".to_string());
        e.result = Some(serde_json::json!({
            "cbu_id": "uuid-123",
            "created": true
        }));
        rb.add_entry(e);

        let carry = build_carry_forward(&rb);
        assert_eq!(carry.get("name").map(|s| s.as_str()), Some("Allianz Lux"));
        assert_eq!(carry.get("cbu_id").map(|s| s.as_str()), Some("uuid-123"));
    }

    #[test]
    fn test_template_hint_none_when_all_completed() {
        let mut rb = empty_runbook();
        rb.template_id = Some("flow".to_string());

        let mut e = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create".to_string(),
            "(cbu.create)".to_string(),
        );
        e.status = EntryStatus::Completed;
        e.labels
            .insert("template_id".to_string(), "flow".to_string());
        rb.add_entry(e);

        let ctx = ContextStack::from_runbook(&rb, None, 1);
        // All steps completed → no next step → None.
        assert!(ctx.template_hint.is_none());
    }

    #[test]
    fn test_template_hint_skips_disabled_entries() {
        let mut rb = empty_runbook();
        rb.template_id = Some("flow".to_string());

        // Disabled entry should not count.
        let mut e1 = RunbookEntry::new(
            "cbu.create".to_string(),
            "Create".to_string(),
            "(cbu.create)".to_string(),
        );
        e1.status = EntryStatus::Disabled;
        e1.labels
            .insert("template_id".to_string(), "flow".to_string());
        rb.add_entry(e1);

        // Proposed entry should be the next hint.
        let mut e2 = RunbookEntry::new(
            "cbu.assign-role".to_string(),
            "Assign".to_string(),
            "(cbu.assign-role)".to_string(),
        );
        e2.status = EntryStatus::Proposed;
        e2.labels
            .insert("template_id".to_string(), "flow".to_string());
        rb.add_entry(e2);

        let ctx = ContextStack::from_runbook(&rb, None, 1);
        let hint = ctx.template_hint.unwrap();
        assert_eq!(hint.total_steps, 1); // Only non-disabled count.
        assert_eq!(hint.expected_verb, "cbu.assign-role");
    }

    // -- Pack handoff tests --

    #[test]
    fn test_pack_handoff_when_complete_with_target() {
        let mut rb = empty_runbook();
        rb.pack_id = Some("kyc-case".to_string());
        rb.template_id = Some("kyc-flow".to_string());

        // pack.select with handoff-target
        let mut ps = RunbookEntry::new(
            "pack.select".to_string(),
            "Select KYC".to_string(),
            "(pack.select :pack-id \"kyc-case\")".to_string(),
        );
        ps.status = EntryStatus::Completed;
        ps.args
            .insert("pack-id".to_string(), "kyc-case".to_string());
        ps.args
            .insert("handoff-target".to_string(), "book-setup".to_string());
        rb.add_entry(ps);

        // Template entry — completed
        let mut e1 = RunbookEntry::new(
            "kyc.create-case".to_string(),
            "Create case".to_string(),
            "(kyc.create-case)".to_string(),
        );
        e1.status = EntryStatus::Completed;
        e1.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        e1.result = Some(serde_json::json!({"case_id": "uuid-case-1"}));
        rb.add_entry(e1);

        let ctx = ContextStack::from_runbook(&rb, None, 2);
        let handoff = ctx.check_pack_handoff(&rb);

        assert!(handoff.is_some());
        let h = handoff.unwrap();
        assert_eq!(h.completed_pack_id, "kyc-case");
        assert_eq!(h.suggested_pack_id, "book-setup");
        assert_eq!(h.outcome_refs.len(), 1);
        assert_eq!(h.outcome_refs[0].verb, "kyc.create-case");
        assert_eq!(
            h.outcome_refs[0].values.get("case_id"),
            Some(&"uuid-case-1".to_string())
        );
    }

    #[test]
    fn test_pack_handoff_not_complete() {
        let mut rb = empty_runbook();
        rb.pack_id = Some("kyc-case".to_string());
        rb.template_id = Some("kyc-flow".to_string());

        let mut ps = RunbookEntry::new(
            "pack.select".to_string(),
            "Select KYC".to_string(),
            "(pack.select :pack-id \"kyc-case\")".to_string(),
        );
        ps.status = EntryStatus::Completed;
        ps.args
            .insert("pack-id".to_string(), "kyc-case".to_string());
        ps.args
            .insert("handoff-target".to_string(), "book-setup".to_string());
        rb.add_entry(ps);

        // One completed, one proposed — not complete
        let mut e1 = RunbookEntry::new(
            "kyc.create-case".to_string(),
            "Create case".to_string(),
            "(kyc.create-case)".to_string(),
        );
        e1.status = EntryStatus::Completed;
        e1.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        rb.add_entry(e1);

        let mut e2 = RunbookEntry::new(
            "kyc.add-entity".to_string(),
            "Add entity".to_string(),
            "(kyc.add-entity)".to_string(),
        );
        e2.status = EntryStatus::Proposed;
        e2.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        rb.add_entry(e2);

        let ctx = ContextStack::from_runbook(&rb, None, 2);
        assert!(ctx.check_pack_handoff(&rb).is_none());
    }

    #[test]
    fn test_pack_handoff_no_target() {
        let mut rb = empty_runbook();
        rb.pack_id = Some("kyc-case".to_string());
        rb.template_id = Some("kyc-flow".to_string());

        // pack.select WITHOUT handoff-target
        let mut ps = RunbookEntry::new(
            "pack.select".to_string(),
            "Select KYC".to_string(),
            "(pack.select :pack-id \"kyc-case\")".to_string(),
        );
        ps.status = EntryStatus::Completed;
        ps.args
            .insert("pack-id".to_string(), "kyc-case".to_string());
        rb.add_entry(ps);

        let mut e1 = RunbookEntry::new(
            "kyc.create-case".to_string(),
            "Create case".to_string(),
            "(kyc.create-case)".to_string(),
        );
        e1.status = EntryStatus::Completed;
        e1.labels
            .insert("template_id".to_string(), "kyc-flow".to_string());
        rb.add_entry(e1);

        let ctx = ContextStack::from_runbook(&rb, None, 2);
        // No handoff target → None
        assert!(ctx.check_pack_handoff(&rb).is_none());
    }

    // -- Helper --

    fn default_pack_manifest() -> PackManifest {
        PackManifest {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            description: String::new(),
            invocation_phrases: Vec::new(),
            required_context: Vec::new(),
            optional_context: Vec::new(),
            allowed_verbs: Vec::new(),
            forbidden_verbs: Vec::new(),
            risk_policy: Default::default(),
            required_questions: Vec::new(),
            optional_questions: Vec::new(),
            stop_rules: Vec::new(),
            templates: Vec::new(),
            pack_summary_template: None,
            section_layout: Vec::new(),
            definition_of_done: Vec::new(),
            progress_signals: Vec::new(),
            handoff_target: None,
        }
    }
}
