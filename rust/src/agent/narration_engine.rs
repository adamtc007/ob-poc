//! Proactive Narration Engine — goal-directed workflow guidance.
//!
//! Computes [`NarrationPayload`] from constellation slot deltas after every
//! state-changing action. The operator sees progress, gaps, and suggested
//! next steps without asking.
//!
//! Design: ADR 043 (ai-thoughts/043-sage-proactive-narration.md)

use ob_poc_types::narration::{
    ActionPriority, NarrationBlocker, NarrationGap, NarrationPayload, NarrationVerbosity,
    SlotDelta, SuggestedAction,
};

use crate::sem_os_runtime::constellation_runtime::{HydratedCardinality, HydratedSlot};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute proactive narration from pre/post constellation slot states.
///
/// Call this after verb execution when `writes_since_push > 0` and
/// the constellation has been re-hydrated.
pub fn compute_narration(
    pre_slots: &[SlotSnapshot],
    post_slots: &[HydratedSlot],
    last_verb: &str,
    writes_since_push: u32,
    is_first_action: bool,
    constellation_label: &str,
) -> NarrationPayload {
    let verbosity = compute_verbosity(last_verb, writes_since_push, is_first_action, post_slots);

    if verbosity == NarrationVerbosity::Silent {
        return NarrationPayload::silent();
    }

    let delta = compute_delta(pre_slots, post_slots);
    let (required_gaps, optional_gaps) = compute_gaps(post_slots);
    let suggested_next = compute_suggested_next(&required_gaps, &optional_gaps);
    let blockers = compute_blockers(post_slots);

    let total = count_fillable_slots(post_slots);
    let filled = total - required_gaps.len() - optional_gaps.len();
    let progress = if total > 0 {
        Some(format!(
            "{} of {} slots filled for {}",
            filled, total, constellation_label
        ))
    } else {
        None
    };

    NarrationPayload {
        progress,
        delta,
        required_gaps,
        optional_gaps,
        suggested_next,
        blockers,
        verbosity,
    }
}

/// Check if an utterance is a contextual query that should bypass verb search
/// and route directly to the NarrationEngine.
///
/// These are "where are we" / "what's next" questions that the constellation
/// can answer deterministically without embedding search.
pub fn is_contextual_query(utterance: &str) -> bool {
    let normalized = utterance.trim().to_lowercase();
    CONTEXTUAL_PATTERNS.iter().any(|p| normalized.contains(p))
}

/// Contextual query patterns — short fixed phrases that signal the operator
/// wants a progress/gap report, not a verb execution.
const CONTEXTUAL_PATTERNS: &[&str] = &[
    "what's left",
    "whats left",
    "what's missing",
    "whats missing",
    "what's remaining",
    "what's next",
    "whats next",
    "what do i need to do",
    "what needs to be done",
    "where are we",
    "show progress",
    "show me progress",
    "how far along",
    "what's outstanding",
    "whats outstanding",
    "what's still needed",
    "are we done",
    "is everything complete",
    "what's blocking",
    "any blockers",
    "show gaps",
    "what gaps",
    "status update",
    "progress report",
    "progress summary",
    "what haven't we done",
    "remaining steps",
    "next steps",
];

/// Compute a Full narration for an on-demand contextual query.
///
/// Unlike `compute_narration()` (which is post-execution with delta),
/// this produces a snapshot view: current gaps, blockers, progress,
/// and suggested next steps — no delta since nothing changed.
pub fn query_narration(post_slots: &[HydratedSlot], constellation_label: &str) -> NarrationPayload {
    let (required_gaps, optional_gaps) = compute_gaps(post_slots);
    let suggested_next = compute_suggested_next(&required_gaps, &optional_gaps);
    let blockers = compute_blockers(post_slots);

    let total = count_fillable_slots(post_slots);
    let filled = total - required_gaps.len() - optional_gaps.len();
    let progress = if total > 0 {
        Some(format!(
            "{} of {} slots filled for {}",
            filled, total, constellation_label
        ))
    } else {
        None
    };

    NarrationPayload {
        progress,
        delta: Vec::new(), // No delta — this is a snapshot query
        required_gaps,
        optional_gaps,
        suggested_next,
        blockers,
        verbosity: NarrationVerbosity::Full,
    }
}

// ---------------------------------------------------------------------------
// Slot snapshot (pre-execution state capture)
// ---------------------------------------------------------------------------

/// Lightweight snapshot of a slot's state before execution.
/// Captured before verb execution so we can compute the delta.
#[derive(Debug, Clone)]
pub struct SlotSnapshot {
    pub name: String,
    pub effective_state: String,
    pub entity_name: Option<String>,
}

impl SlotSnapshot {
    /// Capture snapshots from hydrated slots (call before execution).
    pub fn capture(slots: &[HydratedSlot]) -> Vec<Self> {
        fn collect(slots: &[HydratedSlot], out: &mut Vec<SlotSnapshot>) {
            for slot in slots {
                out.push(SlotSnapshot {
                    name: slot.name.clone(),
                    effective_state: slot.effective_state.clone(),
                    entity_name: None, // Entity name not available in slot directly
                });
                collect(&slot.children, out);
            }
        }
        let mut result = Vec::new();
        collect(slots, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Verbosity decision
// ---------------------------------------------------------------------------

/// Determine narration verbosity based on context.
///
/// | Context                          | Verbosity |
/// |----------------------------------|-----------|
/// | First action in workspace        | Full      |
/// | After filling last required slot | Full      |
/// | After filling a required slot    | Medium    |
/// | After filling an optional slot   | Light     |
/// | Read-only verb (no state change) | Silent    |
fn compute_verbosity(
    last_verb: &str,
    writes_since_push: u32,
    is_first_action: bool,
    post_slots: &[HydratedSlot],
) -> NarrationVerbosity {
    // First action in workspace: full context
    if is_first_action {
        return NarrationVerbosity::Full;
    }

    // Read-only verbs: silent
    if is_read_verb(last_verb) {
        return NarrationVerbosity::Silent;
    }

    // No writes yet: silent (shouldn't happen but be safe)
    if writes_since_push == 0 {
        return NarrationVerbosity::Silent;
    }

    // Check if all required slots are now filled
    let has_required_gaps = has_mandatory_gaps(post_slots);
    if !has_required_gaps {
        return NarrationVerbosity::Full; // Celebrate completion
    }

    // Default for write actions: medium
    NarrationVerbosity::Medium
}

/// Heuristic: is this verb read-only (no state mutation)?
fn is_read_verb(verb: &str) -> bool {
    let read_patterns = [
        ".read",
        ".list",
        ".get",
        ".show",
        ".info",
        ".query",
        ".search",
        ".state",
        ".describe",
        ".trace",
        ".diff",
        ".compare",
        ".export",
        ".details",
        ".parties",
        ".roles",
        ".preview",
        ".check",
        "view.",
        "session.info",
        "session.list",
    ];
    read_patterns.iter().any(|p| verb.contains(p))
}

// ---------------------------------------------------------------------------
// Delta computation
// ---------------------------------------------------------------------------

fn compute_delta(pre_slots: &[SlotSnapshot], post_slots: &[HydratedSlot]) -> Vec<SlotDelta> {
    let mut deltas = Vec::new();

    fn collect_post(slots: &[HydratedSlot], out: &mut Vec<(String, String)>) {
        for slot in slots {
            out.push((slot.name.clone(), slot.effective_state.clone()));
            collect_post(&slot.children, out);
        }
    }

    let mut post_states: Vec<(String, String)> = Vec::new();
    collect_post(post_slots, &mut post_states);

    for pre in pre_slots {
        if let Some((_, post_state)) = post_states.iter().find(|(name, _)| *name == pre.name) {
            if *post_state != pre.effective_state {
                deltas.push(SlotDelta {
                    slot_name: pre.name.clone(),
                    slot_label: humanize_slot_name(&pre.name),
                    from_state: pre.effective_state.clone(),
                    to_state: post_state.clone(),
                    entity_name: pre.entity_name.clone(),
                });
            }
        }
    }

    deltas
}

// ---------------------------------------------------------------------------
// Gap analysis
// ---------------------------------------------------------------------------

fn compute_gaps(post_slots: &[HydratedSlot]) -> (Vec<NarrationGap>, Vec<NarrationGap>) {
    let mut required = Vec::new();
    let mut optional = Vec::new();

    fn collect(
        slots: &[HydratedSlot],
        required: &mut Vec<NarrationGap>,
        optional: &mut Vec<NarrationGap>,
    ) {
        for slot in slots {
            let is_empty = slot.effective_state == "empty" || slot.effective_state == "placeholder";
            if is_empty {
                let gap = NarrationGap {
                    slot_name: slot.name.clone(),
                    slot_label: humanize_slot_name(&slot.name),
                    why_required: if matches!(slot.cardinality, HydratedCardinality::Mandatory) {
                        Some(format!("{} is required", humanize_slot_name(&slot.name)))
                    } else {
                        None
                    },
                    suggested_verb: slot
                        .available_verbs
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "cbu.assign-role".into()),
                    suggested_macro: None,
                    suggested_utterance: format!("assign a {}", humanize_slot_name(&slot.name)),
                };

                match slot.cardinality {
                    HydratedCardinality::Mandatory | HydratedCardinality::Root => {
                        required.push(gap);
                    }
                    HydratedCardinality::Optional | HydratedCardinality::Recursive => {
                        optional.push(gap);
                    }
                }
            }
            collect(&slot.children, required, optional);
        }
    }

    collect(post_slots, &mut required, &mut optional);
    (required, optional)
}

fn has_mandatory_gaps(slots: &[HydratedSlot]) -> bool {
    for slot in slots {
        if matches!(
            slot.cardinality,
            HydratedCardinality::Mandatory | HydratedCardinality::Root
        ) && (slot.effective_state == "empty" || slot.effective_state == "placeholder")
        {
            return true;
        }
        if has_mandatory_gaps(&slot.children) {
            return true;
        }
    }
    false
}

fn count_fillable_slots(slots: &[HydratedSlot]) -> usize {
    let mut count = 0;
    fn walk(slots: &[HydratedSlot], count: &mut usize) {
        for slot in slots {
            if !matches!(slot.cardinality, HydratedCardinality::Root) {
                *count += 1;
            }
            walk(&slot.children, count);
        }
    }
    walk(slots, &mut count);
    count
}

// ---------------------------------------------------------------------------
// Suggested next actions
// ---------------------------------------------------------------------------

fn compute_suggested_next(
    required_gaps: &[NarrationGap],
    optional_gaps: &[NarrationGap],
) -> Vec<SuggestedAction> {
    let mut actions: Vec<SuggestedAction> = Vec::new();

    for gap in required_gaps {
        actions.push(SuggestedAction {
            verb_fqn: gap.suggested_verb.clone(),
            macro_fqn: gap.suggested_macro.clone(),
            utterance: gap.suggested_utterance.clone(),
            priority: ActionPriority::Critical,
            reason: gap
                .why_required
                .clone()
                .unwrap_or_else(|| "required slot".into()),
        });
    }

    // Include up to 3 optional suggestions
    for gap in optional_gaps.iter().take(3) {
        actions.push(SuggestedAction {
            verb_fqn: gap.suggested_verb.clone(),
            macro_fqn: gap.suggested_macro.clone(),
            utterance: gap.suggested_utterance.clone(),
            priority: ActionPriority::Optional,
            reason: format!("{} (optional)", gap.slot_label),
        });
    }

    actions
}

// ---------------------------------------------------------------------------
// Blockers
// ---------------------------------------------------------------------------

fn compute_blockers(post_slots: &[HydratedSlot]) -> Vec<NarrationBlocker> {
    let mut blockers = Vec::new();

    fn collect(slots: &[HydratedSlot], blockers: &mut Vec<NarrationBlocker>) {
        for slot in slots {
            for blocked in &slot.blocked_verbs {
                blockers.push(NarrationBlocker {
                    blocked_verb: blocked.verb.clone(),
                    reason: blocked
                        .reasons
                        .iter()
                        .map(|r| format!("{:?}", r))
                        .collect::<Vec<_>>()
                        .join("; "),
                    unblock_hint: format!("fill {} first", humanize_slot_name(&slot.name)),
                });
            }
            collect(&slot.children, blockers);
        }
    }

    collect(post_slots, &mut blockers);
    // Deduplicate by verb
    blockers.sort_by(|a, b| a.blocked_verb.cmp(&b.blocked_verb));
    blockers.dedup_by(|a, b| a.blocked_verb == b.blocked_verb);
    blockers
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a slot name like "management_company" → "Management Company"
fn humanize_slot_name(name: &str) -> String {
    name.replace(['_', '-'], " ")
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sem_os_runtime::constellation_runtime::{
        HydratedCardinality, HydratedSlot, HydratedSlotType, RuntimeBlockReason, RuntimeBlockedVerb,
    };

    fn make_slot(
        name: &str,
        state: &str,
        cardinality: HydratedCardinality,
        available_verbs: Vec<&str>,
        blocked_verbs: Vec<RuntimeBlockedVerb>,
    ) -> HydratedSlot {
        HydratedSlot {
            name: name.into(),
            path: format!("/{}", name),
            slot_type: HydratedSlotType::Entity,
            cardinality,
            entity_id: None,
            record_id: None,
            computed_state: state.into(),
            effective_state: state.into(),
            progress: if state == "filled" { 100 } else { 0 },
            blocking: false,
            warnings: Vec::new(),
            overlays: Vec::new(),
            graph_node_count: None,
            graph_edge_count: None,
            graph_nodes: Vec::new(),
            graph_edges: Vec::new(),
            available_verbs: available_verbs.into_iter().map(|s| s.into()).collect(),
            blocked_verbs,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_silent_for_read_verb() {
        let slots = vec![make_slot(
            "depositary",
            "filled",
            HydratedCardinality::Mandatory,
            vec![],
            vec![],
        )];
        let result = compute_narration(&[], &slots, "cbu.read", 1, false, "Test Fund");
        assert_eq!(result.verbosity, NarrationVerbosity::Silent);
    }

    #[test]
    fn test_full_on_first_action() {
        let slots = vec![
            make_slot(
                "depositary",
                "empty",
                HydratedCardinality::Mandatory,
                vec!["cbu.assign-role"],
                vec![],
            ),
            make_slot(
                "auditor",
                "empty",
                HydratedCardinality::Optional,
                vec!["cbu.assign-role"],
                vec![],
            ),
        ];
        let result = compute_narration(&[], &slots, "cbu.create", 1, true, "Lux UCITS SICAV Alpha");
        assert_eq!(result.verbosity, NarrationVerbosity::Full);
        assert_eq!(result.required_gaps.len(), 1);
        assert_eq!(result.optional_gaps.len(), 1);
        assert!(result.progress.is_some());
        assert!(result.progress.unwrap().contains("0 of 2"));
    }

    #[test]
    fn test_medium_after_write() {
        let slots = vec![
            make_slot(
                "depositary",
                "filled",
                HydratedCardinality::Mandatory,
                vec![],
                vec![],
            ),
            make_slot(
                "management_company",
                "empty",
                HydratedCardinality::Mandatory,
                vec!["cbu.assign-role"],
                vec![],
            ),
        ];
        let result = compute_narration(
            &[],
            &slots,
            "cbu.assign-role",
            2,
            false,
            "Lux UCITS SICAV Alpha",
        );
        assert_eq!(result.verbosity, NarrationVerbosity::Medium);
        assert_eq!(result.required_gaps.len(), 1);
        assert_eq!(result.required_gaps[0].slot_label, "Management Company");
    }

    #[test]
    fn test_full_when_all_required_filled() {
        let slots = vec![
            make_slot(
                "depositary",
                "filled",
                HydratedCardinality::Mandatory,
                vec![],
                vec![],
            ),
            make_slot(
                "auditor",
                "empty",
                HydratedCardinality::Optional,
                vec!["cbu.assign-role"],
                vec![],
            ),
        ];
        let result = compute_narration(
            &[],
            &slots,
            "cbu.assign-role",
            3,
            false,
            "Lux UCITS SICAV Alpha",
        );
        // All mandatory filled → Full (celebration)
        assert_eq!(result.verbosity, NarrationVerbosity::Full);
        assert!(result.required_gaps.is_empty());
        assert_eq!(result.optional_gaps.len(), 1);
    }

    #[test]
    fn test_delta_computed() {
        let pre = vec![SlotSnapshot {
            name: "depositary".into(),
            effective_state: "empty".into(),
            entity_name: None,
        }];
        let post = vec![make_slot(
            "depositary",
            "filled",
            HydratedCardinality::Mandatory,
            vec![],
            vec![],
        )];
        let result = compute_narration(&pre, &post, "cbu.assign-role", 1, false, "Test Fund");
        assert_eq!(result.delta.len(), 1);
        assert_eq!(result.delta[0].from_state, "empty");
        assert_eq!(result.delta[0].to_state, "filled");
    }

    #[test]
    fn test_suggested_next_critical_first() {
        let slots = vec![
            make_slot(
                "depositary",
                "empty",
                HydratedCardinality::Mandatory,
                vec!["cbu.assign-role"],
                vec![],
            ),
            make_slot(
                "auditor",
                "empty",
                HydratedCardinality::Optional,
                vec!["cbu.assign-role"],
                vec![],
            ),
        ];
        let result = compute_narration(&[], &slots, "cbu.create", 1, true, "Test");
        assert!(!result.suggested_next.is_empty());
        assert_eq!(result.suggested_next[0].priority, ActionPriority::Critical);
    }

    #[test]
    fn test_humanize_slot_name() {
        assert_eq!(
            humanize_slot_name("management_company"),
            "Management Company"
        );
        assert_eq!(humanize_slot_name("depositary"), "Depositary");
        assert_eq!(humanize_slot_name("prime-broker"), "Prime Broker");
    }

    #[test]
    fn test_blockers_collected() {
        let blocked = RuntimeBlockedVerb {
            verb: "case.open".into(),
            reasons: vec![RuntimeBlockReason {
                message: "structure not yet created".into(),
            }],
        };
        let slots = vec![make_slot(
            "case",
            "empty",
            HydratedCardinality::Optional,
            vec![],
            vec![blocked],
        )];
        let result = compute_narration(&[], &slots, "cbu.create", 1, true, "Test");
        assert_eq!(result.blockers.len(), 1);
        assert_eq!(result.blockers[0].blocked_verb, "case.open");
    }

    // ── Contextual query tests ──────────────────────────────────────────

    #[test]
    fn test_contextual_query_detection() {
        assert!(is_contextual_query("what's next"));
        assert!(is_contextual_query("What's next?"));
        assert!(is_contextual_query("whats left"));
        assert!(is_contextual_query("show me progress"));
        assert!(is_contextual_query("where are we"));
        assert!(is_contextual_query("any blockers?"));
        assert!(is_contextual_query("next steps please"));
        assert!(is_contextual_query("  What's Missing  "));
    }

    #[test]
    fn test_non_contextual_queries() {
        assert!(!is_contextual_query("assign the depositary"));
        assert!(!is_contextual_query("create a fund"));
        assert!(!is_contextual_query("open a KYC case"));
        assert!(!is_contextual_query("hello"));
        assert!(!is_contextual_query(""));
    }

    #[test]
    fn test_query_narration_all_filled() {
        let slots = vec![make_slot(
            "depositary",
            "filled",
            HydratedCardinality::Mandatory,
            vec![],
            vec![],
        )];
        let result = query_narration(&slots, "Lux UCITS SICAV Alpha");
        assert_eq!(result.verbosity, NarrationVerbosity::Full);
        assert!(result.required_gaps.is_empty());
        assert!(result.delta.is_empty()); // No delta for queries
        assert!(result.progress.unwrap().contains("1 of 1"));
    }

    #[test]
    fn test_query_narration_with_gaps() {
        let slots = vec![
            make_slot(
                "depositary",
                "filled",
                HydratedCardinality::Mandatory,
                vec![],
                vec![],
            ),
            make_slot(
                "management_company",
                "empty",
                HydratedCardinality::Mandatory,
                vec!["cbu.assign-role"],
                vec![],
            ),
            make_slot(
                "auditor",
                "empty",
                HydratedCardinality::Optional,
                vec!["cbu.assign-role"],
                vec![],
            ),
        ];
        let result = query_narration(&slots, "Test Fund");
        assert_eq!(result.required_gaps.len(), 1);
        assert_eq!(result.optional_gaps.len(), 1);
        assert_eq!(result.required_gaps[0].slot_label, "Management Company");
        assert!(result.progress.unwrap().contains("1 of 3"));
    }
}
