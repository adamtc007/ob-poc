//! Blocker detection — Phase 3.4 (C-07 / C-08 / C-09).
//!
//! The frontier (Phase 3.3) shows what's open. Blockers are deeper
//! conditions that prevent the frontier from advancing — missing
//! preconditions, cross-workspace state misalignment, pending
//! remediation, unsanctioned drafts. The detector runs alongside
//! the frontier engine; both attach to the goal frame.
//!
//! ## Spike scope (Phase 3.4)
//!
//! - Pure compute over the pack manifest + frontier + constellation
//!   snapshot. No IO.
//! - Three kinds emit today:
//!   - `RequiredQuestionUnanswered` — for each open
//!     `RequiredQuestion` in the frontier, emit a blocker with a
//!     remediation hint pointing at the candidate verbs.
//!   - `UnsanctionedDraft` — emit when a proposed verb FQN sits
//!     outside the pack allowlist. Belt-and-braces: the
//!     constrained-composition guard in `PlanningLoop::propose_draft`
//!     rejects this upstream, but the blocker shape is useful for
//!     post-hoc audit reading.
//!   - `EmptyConstellation` — informational marker when the
//!     hydrator returned an empty snapshot. Phase 4 retires this
//!     once real hydration lands.
//! - Cross-workspace + pending-remediation kinds (the C-08 / C-09
//!   tail of the trio) ship the `BlockerKind` variants without
//!   detector logic. Phase 4 wires them once `sem_os_mcp` exposes
//!   the cross-workspace + remediation surfaces.

use serde::{Deserialize, Serialize};

use crate::constellation::ConstellationSnapshot;
use crate::frontier::{Frontier, FrontierItemKind};
use crate::index::SessionIndex;

/// What kind of blocker was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    /// A required pack question has no answer in the snapshot.
    RequiredQuestionUnanswered,
    /// A proposed verb FQN sits outside the pack allowlist (or is
    /// on the denylist). Belt-and-braces — the planning loop
    /// rejects this upstream.
    UnsanctionedDraft,
    /// The hydrator returned an empty constellation snapshot.
    /// Informational; retires when Phase 4 lands real hydration.
    EmptyConstellation,
    /// (Phase 4) Cross-workspace state misalignment — variant
    /// reserved here so consumers can match it without a new
    /// schema migration when the detector lands.
    CrossWorkspaceState,
    /// (Phase 4) A pending remediation event blocks progress.
    PendingRemediation,
}

/// One blocker entry on the goal frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    pub kind: BlockerKind,
    /// Slot / item id the blocker pertains to (frontier `key`,
    /// verb FQN, snapshot tag, …). Free-form string.
    pub blocked_item: String,
    /// Human-readable description.
    pub description: String,
    /// Sanctioned verb FQNs that would remediate the blocker.
    /// Empty when no candidate exists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_remediations: Vec<String>,
}

/// Output of [`BlockerDetector::detect`]. Wrapping `Vec<Blocker>` in
/// a struct leaves space for Phase 4 to add aggregate counters
/// without breaking JSON shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockerReport {
    pub blockers: Vec<Blocker>,
}

impl BlockerReport {
    pub fn is_empty(&self) -> bool {
        self.blockers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blockers.len()
    }

    pub fn by_kind(&self, kind: BlockerKind) -> impl Iterator<Item = &Blocker> {
        self.blockers.iter().filter(move |b| b.kind == kind)
    }
}

/// Pure synchronous blocker detector.
pub struct BlockerDetector;

impl BlockerDetector {
    /// Compute the blocker set from index + frontier + snapshot.
    ///
    /// Phase 3.4 emits three deterministic kinds (see module doc).
    /// Phase 4 wires cross-workspace + remediation detectors.
    pub fn detect(
        index: &SessionIndex,
        frontier: &Frontier,
        snapshot: &ConstellationSnapshot,
        proposed_verb_fqn: Option<&str>,
    ) -> BlockerReport {
        let mut blockers = Vec::new();

        // RequiredQuestionUnanswered — one per open required question.
        for item in &frontier.items {
            if item.kind == FrontierItemKind::RequiredQuestion && !item.satisfied {
                blockers.push(Blocker {
                    kind: BlockerKind::RequiredQuestionUnanswered,
                    blocked_item: item.key.clone(),
                    description: item.description.clone(),
                    // Spike heuristic: any candidate verb might
                    // unblock; Phase 4 narrows by precondition.
                    candidate_remediations: frontier.candidate_verbs.clone(),
                });
            }
        }

        // UnsanctionedDraft — flag if the proposed verb sits outside
        // the allowlist. Upstream guard in propose_draft already
        // rejects, but recording here gives audit a stable shape.
        if let Some(verb) = proposed_verb_fqn {
            if !index.is_verb_sanctioned(verb) {
                blockers.push(Blocker {
                    kind: BlockerKind::UnsanctionedDraft,
                    blocked_item: verb.to_string(),
                    description: format!(
                        "verb '{verb}' is not sanctioned by pack '{}'",
                        index.pack.id
                    ),
                    candidate_remediations: frontier.candidate_verbs.clone(),
                });
            }
        }

        // EmptyConstellation — informational. Phase 4 retires.
        if snapshot.is_empty() {
            blockers.push(Blocker {
                kind: BlockerKind::EmptyConstellation,
                blocked_item: index.pack.id.clone(),
                description: "constellation snapshot is empty — falling back to pack allowlist"
                    .to_string(),
                candidate_remediations: Vec::new(),
            });
        }

        BlockerReport { blockers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constellation::{ConstellationSnapshot, EntityStateDTO};
    use crate::frontier::FrontierEngine;
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;
    use std::collections::HashMap;

    fn manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: blocker detector fixture
invocation_phrases: []
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
  - cbu.attach-product
forbidden_verbs:
  - cbu.delete
required_questions:
  - field: jurisdiction
    prompt: Which jurisdiction?
  - field: vehicle_type
    prompt: What vehicle?
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done:
  - "CBU created"
progress_signals: []
"#
    }

    fn make_index() -> SessionIndex {
        let (pack, pack_hash) = load_pack_from_bytes(manifest_yaml()).unwrap();
        SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        }
    }

    #[test]
    fn empty_snapshot_emits_question_and_empty_constellation_blockers() {
        let index = make_index();
        let snapshot = ConstellationSnapshot::empty();
        let frontier = FrontierEngine::compute(&index, &snapshot);
        let report = BlockerDetector::detect(&index, &frontier, &snapshot, Some("cbu.create"));

        // 2 required questions + EmptyConstellation = 3 blockers.
        assert_eq!(report.len(), 3);
        assert_eq!(
            report
                .by_kind(BlockerKind::RequiredQuestionUnanswered)
                .count(),
            2
        );
        assert_eq!(report.by_kind(BlockerKind::EmptyConstellation).count(), 1);
        assert_eq!(report.by_kind(BlockerKind::UnsanctionedDraft).count(), 0);
    }

    #[test]
    fn unsanctioned_draft_emits_blocker() {
        let index = make_index();
        let snapshot = ConstellationSnapshot {
            entity_states: vec![EntityStateDTO {
                entity_id: "cbu:a".to_string(),
                entity_kind: "cbu".to_string(),
                state: "draft".to_string(),
                attributes: HashMap::from([
                    ("jurisdiction".to_string(), serde_json::json!("LU")),
                    ("vehicle_type".to_string(), serde_json::json!("SICAV")),
                ]),
            }],
            hydrated_at: Utc::now(),
        };
        let frontier = FrontierEngine::compute(&index, &snapshot);
        let report = BlockerDetector::detect(&index, &frontier, &snapshot, Some("cbu.delete"));

        // No question blockers (both answered); no empty-constellation;
        // one UnsanctionedDraft (cbu.delete is on the denylist).
        assert_eq!(report.by_kind(BlockerKind::UnsanctionedDraft).count(), 1);
        assert_eq!(
            report
                .by_kind(BlockerKind::RequiredQuestionUnanswered)
                .count(),
            0
        );
        assert_eq!(report.by_kind(BlockerKind::EmptyConstellation).count(), 0);
    }

    #[test]
    fn fully_satisfied_snapshot_with_sanctioned_draft_emits_nothing() {
        let index = make_index();
        let snapshot = ConstellationSnapshot {
            entity_states: vec![EntityStateDTO {
                entity_id: "cbu:a".to_string(),
                entity_kind: "cbu".to_string(),
                state: "draft".to_string(),
                attributes: HashMap::from([
                    ("jurisdiction".to_string(), serde_json::json!("LU")),
                    ("vehicle_type".to_string(), serde_json::json!("SICAV")),
                ]),
            }],
            hydrated_at: Utc::now(),
        };
        let frontier = FrontierEngine::compute(&index, &snapshot);
        let report = BlockerDetector::detect(&index, &frontier, &snapshot, Some("cbu.create"));
        assert!(report.is_empty(), "expected zero blockers, got {report:?}");
    }
}
