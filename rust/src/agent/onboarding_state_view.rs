//! Projects `GroupCompositeState` into `OnboardingStateView` for UI rendering.
//!
//! Pure conversion — no DB, no I/O. Takes the engine-internal composite state
//! and builds the UI-facing DAG layers with verb suggestions.
//!
//! ## Key Invariants
//!
//! 1. **Undo is composite-level** — revert verbs move case/screening/entity state
//!    backward (e.g., REVIEW → ASSESSMENT). Factual attributes (name, LEI) are
//!    corrected, not undone.
//!
//! 2. **Utterance alignment** — every `suggested_utterance` MUST resolve through
//!    `HybridVerbSearcher` to the same `verb_fqn`. We use the verb's canonical
//!    invocation phrases, not invented text.
//!
//! 3. **Pruned by composite** — only verbs relevant to the current group state
//!    appear. No noise from the full 1,400-verb registry.

use ob_poc_types::onboarding_state::*;

use super::composite_state::{
    BlockedVerbHint, CbuStateSummary, GroupCompositeState, ScoredVerbHint,
};

/// Build an `OnboardingStateView` from the engine's `GroupCompositeState`.
///
/// This is the bridge between the scoring engine and the UI.
/// Called once per chat response when a group is in scope.
pub fn project_onboarding_state(
    composite: &GroupCompositeState,
    group_name: Option<&str>,
) -> OnboardingStateView {
    let layers = build_layers(composite);
    let cbu_cards = build_cbu_cards(&composite.cbu_states);

    // Active layer = lowest incomplete layer
    let active_layer_index = layers
        .iter()
        .find(|l| l.state != LayerState::Complete)
        .map(|l| l.index)
        .unwrap_or(0);

    // Overall progress: average of layer progress
    let overall = if layers.is_empty() {
        0
    } else {
        let total: u32 = layers.iter().map(|l| l.progress_pct as u32).sum();
        (total / layers.len() as u32) as u8
    };

    OnboardingStateView {
        group_name: group_name.map(|s| s.to_string()),
        overall_progress_pct: overall,
        active_layer_index,
        layers,
        cbu_cards,
        context_reset_hint: None, // Set by orchestrator when utterance is off-context
    }
}

fn build_layers(composite: &GroupCompositeState) -> Vec<OnboardingLayer> {
    vec![
        build_group_identity_layer(composite),
        build_cbu_identification_layer(composite),
        build_kyc_case_layer(composite),
        build_screening_layer(composite),
        build_document_layer(composite),
        build_approval_layer(composite),
    ]
}

// ── Layer 0: Group Identity (UBO / ownership / control) ────────────

fn build_group_identity_layer(c: &GroupCompositeState) -> OnboardingLayer {
    let ubo_done = c.has_ubo_determination;
    let control_done = c.has_control_chain;

    let (state, progress) = match (ubo_done, control_done) {
        (true, true) => (LayerState::Complete, 100),
        (true, false) => (LayerState::InProgress, 50),
        (false, _) => (LayerState::NotStarted, 0),
    };

    let forward = filter_hints(
        &c.next_likely_verbs,
        &[
            "ubo.discover",
            "ownership.trace-chain",
            "gleif.import-tree",
            "control.build-graph",
        ],
        VerbDirection::Forward,
    );

    let summary = match (ubo_done, control_done) {
        (true, true) => Some("Ownership and control mapped".into()),
        (true, false) => Some("UBO determined, control chain pending".into()),
        _ => Some("Group ownership not yet determined".into()),
    };

    OnboardingLayer {
        index: 0,
        name: "Group Ownership".into(),
        description: "Determine UBO, ownership chain, and control structure for the group".into(),
        state,
        progress_pct: progress,
        summary,
        forward_verbs: forward,
        revert_verbs: vec![], // UBO discovery can't be "undone" — it's factual
        blocked_verbs: vec![],
        unreachable_verbs: vec![],
    }
}

// ── Layer 1: Controlled CBU Linkage ─────────────────────────────────

fn build_cbu_identification_layer(c: &GroupCompositeState) -> OnboardingLayer {
    let (state, progress) = if c.cbu_count == 0 {
        if !c.has_ubo_determination {
            (LayerState::Blocked, 0)
        } else {
            (LayerState::NotStarted, 0)
        }
    } else {
        (LayerState::Complete, 100)
    };

    let forward = filter_hints(
        &c.next_likely_verbs,
        &["cbu.ensure", "cbu.create"],
        VerbDirection::Forward,
    );
    let blocked = filter_blocked(
        &c.blocked_verbs,
        &["kyc-case", "screening", "document", "custody"],
    );

    OnboardingLayer {
        index: 1,
        name: "Controlled CBU Linkage".into(),
        description:
            "Confirm or add the existing controlled CBUs that inherit the group clearance baseline"
                .into(),
        state,
        progress_pct: progress,
        summary: Some(format!("{} linked CBU(s) in scope", c.cbu_count)),
        forward_verbs: forward,
        revert_verbs: vec![], // CBUs are entities — delete/archive, not "undo"
        blocked_verbs: blocked,
        unreachable_verbs: vec![],
    }
}

// ── Layer 2: CBU Delta KYC ──────────────────────────────────────────

fn build_kyc_case_layer(c: &GroupCompositeState) -> OnboardingLayer {
    if c.cbu_count == 0 {
        return blocked_layer(
            2,
            "CBU Delta KYC",
            "Open delta KYC cases for linked CBUs after group clearance",
            "No controlled CBUs linked",
        );
    }

    let with_case = c.cbu_states.iter().filter(|s| s.has_kyc_case).count();
    let total = c.cbu_states.len();
    let progress = if total == 0 {
        0
    } else {
        ((with_case * 100) / total) as u8
    };

    let state = match (with_case, total) {
        (w, t) if w == t && t > 0 => LayerState::Complete,
        (0, _) => LayerState::NotStarted,
        _ => LayerState::InProgress,
    };

    let forward = filter_hints(
        &c.next_likely_verbs,
        &["kyc-case.create", "kyc.open-case"],
        VerbDirection::Forward,
    );

    // Revert: withdraw cases that are open (composite-level undo)
    let revert = if with_case > 0 {
        vec![SuggestedVerb {
            verb_fqn: "kyc-case.update-status".into(),
            label: "Withdraw Case".into(),
            suggested_utterance: "Withdraw the KYC case".into(),
            reason: "Revert: close case without completing review".into(),
            boost: 0.0,
            direction: VerbDirection::Revert,
            governance_tier: Some("governed".into()),
        }]
    } else {
        vec![]
    };

    OnboardingLayer {
        index: 2,
        name: "CBU Delta KYC".into(),
        description:
            "Open delta KYC cases only for the linked CBUs that need local review beyond the group baseline"
                .into(),
        state,
        progress_pct: progress,
        summary: Some(format!(
            "{with_case} of {total} linked CBU(s) have delta KYC cases"
        )),
        forward_verbs: forward,
        revert_verbs: revert,
        blocked_verbs: vec![],
        unreachable_verbs: vec![],
    }
}

// ── Layer 3: Screening ──────────────────────────────────────────────

fn build_screening_layer(c: &GroupCompositeState) -> OnboardingLayer {
    if c.cbu_count == 0 {
        return blocked_layer(
            3,
            "Screening",
            "Run delta screening checks",
            "No controlled CBUs linked",
        );
    }

    let cbus_with_case: Vec<&CbuStateSummary> =
        c.cbu_states.iter().filter(|s| s.has_kyc_case).collect();
    if cbus_with_case.is_empty() {
        return blocked_layer(
            3,
            "Screening",
            "Run delta screening checks",
            "No delta KYC cases opened",
        );
    }

    let screened = cbus_with_case.iter().filter(|s| s.has_screening).count();
    let total = cbus_with_case.len();
    let progress = ((screened * 100) / total) as u8;

    let state = match (screened, total) {
        (s, t) if s == t => LayerState::Complete,
        (0, _) => LayerState::NotStarted,
        _ => LayerState::InProgress,
    };

    let forward = filter_hints(
        &c.next_likely_verbs,
        &["screening.run", "screening.sanctions", "screening.pep"],
        VerbDirection::Forward,
    );

    // Revert: reopen screening (back to case review)
    let revert = if screened > 0 {
        vec![SuggestedVerb {
            verb_fqn: "kyc-case.update-status".into(),
            label: "Reopen for Discovery".into(),
            suggested_utterance: "Reopen the case for discovery".into(),
            reason: "Revert: move case back to discovery phase".into(),
            boost: 0.0,
            direction: VerbDirection::Revert,
            governance_tier: Some("governed".into()),
        }]
    } else {
        vec![]
    };

    OnboardingLayer {
        index: 3,
        name: "Screening".into(),
        description: "Run sanctions, PEP, and adverse media checks for the delta KYC perimeter"
            .into(),
        state,
        progress_pct: progress,
        summary: Some(format!(
            "{screened} of {total} linked CBU(s) screened for delta KYC"
        )),
        forward_verbs: forward,
        revert_verbs: revert,
        blocked_verbs: vec![],
        unreachable_verbs: vec![],
    }
}

// ── Layer 4: Document Collection ────────────────────────────────────

fn build_document_layer(c: &GroupCompositeState) -> OnboardingLayer {
    if c.cbu_count == 0 {
        return blocked_layer(
            4,
            "Documents",
            "Collect delta KYC documents",
            "No controlled CBUs linked",
        );
    }

    let cbus_with_screening: Vec<&CbuStateSummary> = c
        .cbu_states
        .iter()
        .filter(|s| s.has_kyc_case && s.has_screening)
        .collect();

    if cbus_with_screening.is_empty() {
        return blocked_layer(
            4,
            "Documents",
            "Collect delta KYC documents",
            "Screening not complete",
        );
    }

    let complete = cbus_with_screening
        .iter()
        .filter(|s| s.document_coverage_pct.unwrap_or(0.0) >= 1.0)
        .count();
    let total = cbus_with_screening.len();
    let progress = ((complete * 100) / total) as u8;

    let state = match (complete, total) {
        (c, t) if c == t => LayerState::Complete,
        (0, _) => LayerState::NotStarted,
        _ => LayerState::InProgress,
    };

    let forward = filter_hints(
        &c.next_likely_verbs,
        &["document.solicit", "document.solicit-set"],
        VerbDirection::Forward,
    );

    OnboardingLayer {
        index: 4,
        name: "Documents".into(),
        description:
            "Collect only the local identity and corporate documents required for delta KYC".into(),
        state,
        progress_pct: progress,
        summary: Some(format!(
            "{complete} of {total} linked CBU(s) have complete delta-KYC documentation"
        )),
        forward_verbs: forward,
        revert_verbs: vec![], // Document solicitation isn't "undoable" — it's a request
        blocked_verbs: vec![],
        unreachable_verbs: vec![],
    }
}

// ── Layer 5: Clearance & Handoff ────────────────────────────────────

fn build_approval_layer(c: &GroupCompositeState) -> OnboardingLayer {
    let approved = c
        .cbu_states
        .iter()
        .filter(|s| s.kyc_case_status.as_deref() == Some("APPROVED"))
        .count();
    let total = c.cbu_count;

    if total == 0 {
        return blocked_layer(
            5,
            "Approval",
            "Final KYC approval tollgate",
            "No CBUs identified",
        );
    }

    let progress = ((approved * 100) / total) as u8;

    let state = match (approved, total) {
        (a, t) if a == t => LayerState::Complete,
        (0, _) => LayerState::NotStarted,
        _ => LayerState::InProgress,
    };

    let mut forward = filter_hints(
        &c.next_likely_verbs,
        &[
            "deal.request-onboarding",
            "kyc-case.read",
            "deal.read-record",
        ],
        VerbDirection::Query,
    );

    if approved == total
        && total > 0
        && !forward
            .iter()
            .any(|v| v.verb_fqn == "deal.request-onboarding")
    {
        forward.insert(
            0,
            SuggestedVerb {
                verb_fqn: "deal.request-onboarding".into(),
                label: "Request Onboarding Handoff".into(),
                suggested_utterance: "request onboarding for this deal".into(),
                reason: "Delta KYC is cleared — hand off the contracted deal into onboarding"
                    .into(),
                boost: 0.08,
                direction: VerbDirection::Forward,
                governance_tier: Some("governed".into()),
            },
        );
    }

    // Revert: reopen approved case for review
    let revert = if approved > 0 {
        vec![SuggestedVerb {
            verb_fqn: "kyc-case.reopen".into(),
            label: "Reopen for Review".into(),
            suggested_utterance: "Reopen the approved case for review".into(),
            reason: "Revert: move approved case back to review".into(),
            boost: 0.0,
            direction: VerbDirection::Revert,
            governance_tier: Some("governed".into()),
        }]
    } else {
        vec![]
    };

    OnboardingLayer {
        index: 5,
        name: "Clearance & Handoff".into(),
        description:
            "Final delta-KYC clearance followed by the commercial handoff from deal into onboarding"
                .into(),
        state,
        progress_pct: progress,
        summary: Some(format!(
            "{approved} of {total} linked CBU(s) cleared and ready for onboarding handoff"
        )),
        forward_verbs: forward,
        revert_verbs: revert,
        blocked_verbs: vec![],
        unreachable_verbs: vec![],
    }
}

// ── CBU Cards ───────────────────────────────────────────────────────

fn build_cbu_cards(cbu_states: &[CbuStateSummary]) -> Vec<CbuStateCard> {
    cbu_states
        .iter()
        .map(|cbu| {
            let progress = compute_cbu_progress(cbu);
            let next = compute_cbu_next_action(cbu);
            let revert = compute_cbu_revert_action(cbu);

            CbuStateCard {
                cbu_id: cbu.cbu_id.clone(),
                cbu_name: cbu.cbu_name.clone(),
                lifecycle_state: cbu.lifecycle_state.clone(),
                progress_pct: progress,
                phases: CbuPhaseStatus {
                    has_case: cbu.has_kyc_case,
                    case_status: cbu.kyc_case_status.clone(),
                    has_screening: cbu.has_screening,
                    screening_complete: cbu.screening_complete,
                    document_coverage_pct: cbu.document_coverage_pct,
                },
                next_action: next,
                revert_action: revert,
            }
        })
        .collect()
}

fn compute_cbu_progress(cbu: &CbuStateSummary) -> u8 {
    // 5 checkpoints: case, screening started, screening complete, docs, approved
    let mut score = 0u8;
    if cbu.has_kyc_case {
        score += 20;
    }
    if cbu.has_screening {
        score += 20;
    }
    if cbu.screening_complete {
        score += 20;
    }
    if cbu.document_coverage_pct.unwrap_or(0.0) >= 1.0 {
        score += 20;
    }
    if cbu.kyc_case_status.as_deref() == Some("APPROVED") {
        score += 20;
    }
    score
}

fn compute_cbu_next_action(cbu: &CbuStateSummary) -> Option<SuggestedVerb> {
    let name = cbu.cbu_name.as_deref().unwrap_or("this CBU");

    if !cbu.has_kyc_case {
        return Some(SuggestedVerb {
            verb_fqn: "kyc-case.create".into(),
            label: "Open KYC Case".into(),
            suggested_utterance: format!("Open a KYC case for {name}"),
            reason: "No KYC case exists for this CBU".into(),
            boost: 0.12,
            direction: VerbDirection::Forward,
            governance_tier: Some("governed".into()),
        });
    }

    if cbu.kyc_case_status.as_deref() == Some("APPROVED") {
        return Some(SuggestedVerb {
            verb_fqn: "deal.request-onboarding".into(),
            label: "Request Onboarding Handoff".into(),
            suggested_utterance: "request onboarding for this deal".into(),
            reason: "Delta KYC cleared — this CBU is ready for deal handoff".into(),
            boost: 0.08,
            direction: VerbDirection::Forward,
            governance_tier: Some("governed".into()),
        });
    }

    if !cbu.has_screening {
        return Some(SuggestedVerb {
            verb_fqn: "screening.run".into(),
            label: "Run Screening".into(),
            suggested_utterance: format!("Run screening for {name}"),
            reason: "KYC case open but no screening started".into(),
            boost: 0.10,
            direction: VerbDirection::Forward,
            governance_tier: Some("governed".into()),
        });
    }

    if cbu.document_coverage_pct.unwrap_or(0.0) < 1.0 && cbu.screening_complete {
        return Some(SuggestedVerb {
            verb_fqn: "document.solicit".into(),
            label: "Request Documents".into(),
            suggested_utterance: format!("Request documents for {name}"),
            reason: "Screening complete, documents incomplete".into(),
            boost: 0.10,
            direction: VerbDirection::Forward,
            governance_tier: Some("governed".into()),
        });
    }

    Some(SuggestedVerb {
        verb_fqn: "kyc-case.read".into(),
        label: "Check Case Status".into(),
        suggested_utterance: format!("Check KYC status for {name}"),
        reason: "Review case progress".into(),
        boost: 0.06,
        direction: VerbDirection::Query,
        governance_tier: Some("governed".into()),
    })
}

/// Compute the composite-level revert action for a CBU.
///
/// This is NOT entity attribute undo — it's state machine rollback.
/// E.g., "Withdraw the case" (INTAKE → WITHDRAWN), "Reopen for discovery"
/// (ASSESSMENT → DISCOVERY via BLOCKED path).
fn compute_cbu_revert_action(cbu: &CbuStateSummary) -> Option<SuggestedVerb> {
    let name = cbu.cbu_name.as_deref().unwrap_or("this CBU");

    if !cbu.has_kyc_case {
        return None; // Nothing to revert
    }

    match cbu.kyc_case_status.as_deref() {
        Some("APPROVED") => Some(SuggestedVerb {
            verb_fqn: "kyc-case.reopen".into(),
            label: "Reopen for Review".into(),
            suggested_utterance: format!("Reopen the case for {name}"),
            reason: "Revert: move approved case back to review".into(),
            boost: 0.0,
            direction: VerbDirection::Revert,
            governance_tier: Some("governed".into()),
        }),
        Some("REVIEW") | Some("ASSESSMENT") | Some("DISCOVERY") => Some(SuggestedVerb {
            verb_fqn: "kyc-case.update-status".into(),
            label: "Withdraw Case".into(),
            suggested_utterance: format!("Withdraw the KYC case for {name}"),
            reason: "Revert: withdraw case from current phase".into(),
            boost: 0.0,
            direction: VerbDirection::Revert,
            governance_tier: Some("governed".into()),
        }),
        Some("INTAKE") => Some(SuggestedVerb {
            verb_fqn: "kyc-case.update-status".into(),
            label: "Withdraw Case".into(),
            suggested_utterance: format!("Withdraw the KYC case for {name}"),
            reason: "Revert: cancel case before processing".into(),
            boost: 0.0,
            direction: VerbDirection::Revert,
            governance_tier: Some("governed".into()),
        }),
        _ => None, // Terminal states (REJECTED, WITHDRAWN, etc.) — can't revert
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn filter_hints(
    hints: &[ScoredVerbHint],
    prefixes: &[&str],
    direction: VerbDirection,
) -> Vec<SuggestedVerb> {
    hints
        .iter()
        .filter(|h| prefixes.iter().any(|p| h.verb_fqn == *p))
        .map(|h| SuggestedVerb {
            verb_fqn: h.verb_fqn.clone(),
            label: verb_fqn_to_label(&h.verb_fqn),
            suggested_utterance: verb_fqn_to_utterance(&h.verb_fqn),
            reason: h.reason.clone(),
            boost: h.boost,
            direction,
            governance_tier: Some("governed".into()),
        })
        .collect()
}

fn filter_blocked(blocked: &[BlockedVerbHint], domain_prefixes: &[&str]) -> Vec<BlockedVerb> {
    blocked
        .iter()
        .filter(|b| domain_prefixes.iter().any(|p| b.verb_fqn.starts_with(p)))
        .map(|b| BlockedVerb {
            verb_fqn: b.verb_fqn.clone(),
            label: verb_fqn_to_label(&b.verb_fqn),
            reason: b.reason.clone(),
            prerequisite: None,
            unblock_utterance: None,
        })
        .collect()
}

fn blocked_layer(index: u8, name: &str, desc: &str, reason: &str) -> OnboardingLayer {
    OnboardingLayer {
        index,
        name: name.into(),
        description: desc.into(),
        state: LayerState::Blocked,
        progress_pct: 0,
        summary: Some(format!("Blocked: {reason}")),
        forward_verbs: vec![],
        revert_verbs: vec![],
        blocked_verbs: vec![],
        unreachable_verbs: vec![],
    }
}

/// Convert "kyc-case.create" → "Open KYC Case" style labels.
fn verb_fqn_to_label(fqn: &str) -> String {
    match fqn {
        "ubo.discover" => "Discover UBO".into(),
        "ownership.trace-chain" => "Trace Ownership Chain".into(),
        "gleif.import-tree" => "Import GLEIF Hierarchy".into(),
        "control.build-graph" => "Build Control Graph".into(),
        "cbu.ensure" => "Confirm CBU".into(),
        "cbu.create" => "Create CBU".into(),
        "kyc-case.create" | "kyc.open-case" => "Open KYC Case".into(),
        "kyc-case.reopen" => "Reopen Case".into(),
        "kyc-case.update-status" => "Update Case Status".into(),
        "screening.run" => "Run Screening".into(),
        "screening.sanctions" => "Run Sanctions Check".into(),
        "screening.pep" => "Run PEP Check".into(),
        "document.solicit" => "Request Documents".into(),
        "document.solicit-set" => "Request Document Set".into(),
        "deal.request-onboarding" => "Request Onboarding Handoff".into(),
        "kyc-case.read" => "Check Case Status".into(),
        "deal.read-record" => "Review Deal".into(),
        _ => {
            let parts: Vec<&str> = fqn.splitn(2, '.').collect();
            if parts.len() == 2 {
                format!(
                    "{} {}",
                    title_case(parts[1].replace('-', " ").as_str()),
                    title_case(parts[0].replace('-', " ").as_str()),
                )
            } else {
                fqn.to_string()
            }
        }
    }
}

/// Convert "kyc-case.create" → "Open a new KYC case" style utterances.
///
/// **Critical:** these phrases MUST resolve through HybridVerbSearcher
/// to the same verb FQN. Use the verb's canonical invocation phrases.
fn verb_fqn_to_utterance(fqn: &str) -> String {
    match fqn {
        "ubo.discover" => "Discover the UBO for this group".into(),
        "ownership.trace-chain" => "Trace the ownership chain".into(),
        "gleif.import-tree" => "Import the corporate hierarchy from GLEIF".into(),
        "control.build-graph" => "Build the control graph".into(),
        "cbu.ensure" => "ensure CBU exists".into(),
        "cbu.create" => "Create a new CBU".into(),
        "kyc-case.create" | "kyc.open-case" => "Open a new KYC case".into(),
        "kyc-case.reopen" => "Reopen the case for review".into(),
        "screening.run" => "Run screening".into(),
        "screening.sanctions" => "Run a sanctions check".into(),
        "screening.pep" => "Run a PEP check".into(),
        "document.solicit" => "Request a document".into(),
        "document.solicit-set" => "Request the full document set".into(),
        "deal.request-onboarding" => "request onboarding for this deal".into(),
        "kyc-case.read" => "Check the KYC case status".into(),
        "deal.read-record" => "Review the deal record".into(),
        _ => fqn.replace(['.', '-'], " "),
    }
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::composite_state::{CbuStateSummary, GroupCompositeState};

    #[test]
    fn test_empty_group_produces_blocked_layers() {
        let composite = GroupCompositeState::default();
        let view = project_onboarding_state(&composite, Some("Test Group"));

        assert_eq!(view.group_name.as_deref(), Some("Test Group"));
        assert_eq!(view.layers.len(), 6);
        assert_eq!(view.active_layer_index, 0); // Layer 0 not started

        // Layer 0: not started (no UBO)
        assert_eq!(view.layers[0].state, LayerState::NotStarted);
        // Layer 1: blocked (no UBO)
        assert_eq!(view.layers[1].state, LayerState::Blocked);
        // Layers 2-5: blocked (no CBUs)
        for layer in &view.layers[2..] {
            assert_eq!(layer.state, LayerState::Blocked);
        }
    }

    #[test]
    fn test_group_with_progress_shows_active_layer_and_revert() {
        let mut composite = GroupCompositeState {
            cbu_count: 2,
            has_ubo_determination: true,
            has_control_chain: true,
            cbu_states: vec![
                CbuStateSummary {
                    cbu_id: "cbu-1".into(),
                    cbu_name: Some("Fund A".into()),
                    lifecycle_state: Some("VALIDATED".into()),
                    has_kyc_case: true,
                    kyc_case_status: Some("APPROVED".into()),
                    has_screening: true,
                    screening_complete: true,
                    document_coverage_pct: Some(1.0),
                },
                CbuStateSummary {
                    cbu_id: "cbu-2".into(),
                    cbu_name: Some("Fund B".into()),
                    lifecycle_state: Some("DISCOVERED".into()),
                    has_kyc_case: false,
                    kyc_case_status: None,
                    has_screening: false,
                    screening_complete: false,
                    document_coverage_pct: None,
                },
            ],
            ..Default::default()
        };
        composite.derive_next_likely_verbs();

        let view = project_onboarding_state(&composite, Some("Allianz"));

        // Layer 0+1: complete
        assert_eq!(view.layers[0].state, LayerState::Complete);
        assert_eq!(view.layers[1].state, LayerState::Complete);
        // Layer 2: in progress (1/2 have cases)
        assert_eq!(view.layers[2].state, LayerState::InProgress);
        assert_eq!(view.layers[2].progress_pct, 50);
        // Active layer = 2 (first non-complete)
        assert_eq!(view.active_layer_index, 2);

        // Layer 2 has revert verbs (case can be withdrawn)
        assert!(!view.layers[2].revert_verbs.is_empty());
        assert_eq!(
            view.layers[2].revert_verbs[0].direction,
            VerbDirection::Revert
        );

        // Layer 5 has revert verbs (approved case can be reopened)
        assert!(!view.layers[5].revert_verbs.is_empty());

        // CBU cards
        assert_eq!(view.cbu_cards.len(), 2);
        assert_eq!(view.cbu_cards[0].progress_pct, 100);
        // Fund A: KYC approved → next action is deal handoff
        assert!(view.cbu_cards[0].next_action.is_some());
        assert_eq!(
            view.cbu_cards[0].next_action.as_ref().unwrap().verb_fqn,
            "deal.request-onboarding"
        );
        assert!(view.cbu_cards[0].revert_action.is_some()); // Can reopen
        assert_eq!(
            view.cbu_cards[0].revert_action.as_ref().unwrap().verb_fqn,
            "kyc-case.reopen"
        );

        assert_eq!(view.cbu_cards[1].progress_pct, 0);
        assert!(view.cbu_cards[1].next_action.is_some()); // Fund B: needs case
        assert_eq!(
            view.cbu_cards[1].next_action.as_ref().unwrap().verb_fqn,
            "kyc-case.create"
        );
        assert!(view.cbu_cards[1].revert_action.is_none()); // Nothing to revert
    }

    #[test]
    fn test_cbu_progress_calculation() {
        let cbu = CbuStateSummary {
            cbu_id: "test".into(),
            cbu_name: None,
            lifecycle_state: None,
            has_kyc_case: true,
            kyc_case_status: Some("REVIEW".into()),
            has_screening: true,
            screening_complete: true,
            document_coverage_pct: Some(0.5),
        };
        // case(20) + screening(20) + screening_complete(20) = 60
        assert_eq!(compute_cbu_progress(&cbu), 60);
    }

    #[test]
    fn test_revert_action_varies_by_case_status() {
        // INTAKE → can withdraw
        let cbu = CbuStateSummary {
            cbu_id: "t".into(),
            cbu_name: Some("Test".into()),
            lifecycle_state: None,
            has_kyc_case: true,
            kyc_case_status: Some("INTAKE".into()),
            has_screening: false,
            screening_complete: false,
            document_coverage_pct: None,
        };
        let revert = compute_cbu_revert_action(&cbu);
        assert!(revert.is_some());
        assert_eq!(revert.unwrap().label, "Withdraw Case");

        // APPROVED → can reopen
        let cbu_approved = CbuStateSummary {
            kyc_case_status: Some("APPROVED".into()),
            ..cbu.clone()
        };
        let revert = compute_cbu_revert_action(&cbu_approved);
        assert!(revert.is_some());
        assert_eq!(revert.unwrap().label, "Reopen for Review");

        // REJECTED → terminal, no revert
        let cbu_rejected = CbuStateSummary {
            kyc_case_status: Some("REJECTED".into()),
            ..cbu.clone()
        };
        assert!(compute_cbu_revert_action(&cbu_rejected).is_none());

        // No case → nothing to revert
        let cbu_no_case = CbuStateSummary {
            has_kyc_case: false,
            kyc_case_status: None,
            ..cbu
        };
        assert!(compute_cbu_revert_action(&cbu_no_case).is_none());
    }

    #[test]
    fn test_suggested_utterances_are_pipeline_aligned() {
        // Verify that suggested utterances are human-readable phrases
        // that should resolve through the intent pipeline.
        // This is a structural test — pipeline alignment testing happens
        // in the calibration harness with real embeddings.
        let utterances = vec![
            ("ubo.discover", "Discover the UBO for this group"),
            ("kyc-case.create", "Open a new KYC case"),
            ("screening.run", "Run screening"),
            ("document.solicit", "Request a document"),
            ("kyc-case.reopen", "Reopen the case for review"),
        ];

        for (fqn, expected) in utterances {
            let actual = verb_fqn_to_utterance(fqn);
            assert_eq!(actual, expected, "Utterance for {fqn} mismatch");
        }
    }
}
