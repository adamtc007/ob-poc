//! Frontier computation + gap analysis — Phase 3.3 (C-05 / C-06).
//!
//! Given a session's pack manifest + constellation snapshot, the
//! frontier engine identifies what the pack expects ("definition of
//! done", progress signals, required questions) and pairs each
//! still-open item with the sanctioned verbs that would close it.
//!
//! ## Spike scope (Phase 3.3)
//!
//! - Pure compute over already-loaded DTOs — no IO.
//! - "Satisfied" detection is intentionally narrow: the spike's
//!   constellation snapshots are empty, so every frontier item
//!   surfaces as open. Phase 4 (with real `sem_os_mcp` hydration)
//!   feeds satisfaction predicates over the entity-state bag; Phase
//!   3.4 (blocker detection) extends with deeper analysis.
//! - Gap analysis is the simplest heuristic that respects
//!   constrained composition: every sanctioned verb in the pack
//!   allowlist is a candidate. Phase 4's
//!   `ActiveVerbsAtState` query refines this to the substrate's
//!   session-aware surface.
//!
//! The output [`Frontier`] is consumed by Phase 3.5 (motivation
//! prompt template) and surfaces on the goal frame so the editor /
//! audit reader can inspect the agent's view of "what's left".

use serde::{Deserialize, Serialize};

use crate::constellation::ConstellationSnapshot;
use crate::index::SessionIndex;

/// Source of one [`FrontierItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierItemKind {
    /// `definition_of_done` entry from the pack manifest.
    DefinitionOfDone,
    /// `progress_signals` entry from the pack manifest.
    ProgressSignal,
    /// `required_questions` entry from the pack manifest.
    RequiredQuestion,
}

/// One open frontier item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierItem {
    pub kind: FrontierItemKind,
    /// Identifier (acceptance text, signal name, or question field).
    pub key: String,
    /// Human-readable description / question prompt.
    pub description: String,
    /// Whether the snapshot indicates this item is already
    /// satisfied. Spike returns `false` for every item because the
    /// stub hydrator returns an empty snapshot.
    pub satisfied: bool,
}

/// Output of [`FrontierEngine::compute`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontier {
    /// Every item the pack expects, satisfied flag set per
    /// [`is_satisfied`] (Phase 4 widens the predicate set).
    pub items: Vec<FrontierItem>,
    /// Verb FQNs that could advance the frontier. Spike returns
    /// the full pack allowlist (minus denylist hits) so the
    /// constrained-composition guard holds.
    pub candidate_verbs: Vec<String>,
}

impl Frontier {
    /// Count of items still open (`!satisfied`).
    pub fn open_count(&self) -> usize {
        self.items.iter().filter(|i| !i.satisfied).count()
    }

    /// Whether every item is satisfied.
    pub fn is_complete(&self) -> bool {
        !self.items.is_empty() && self.open_count() == 0
    }
}

/// Pure compute engine — no IO, no trait, no async. Called by the
/// planning loop after hydration.
pub struct FrontierEngine;

impl FrontierEngine {
    /// Build the frontier from the pack manifest + the constellation
    /// snapshot.
    pub fn compute(index: &SessionIndex, snapshot: &ConstellationSnapshot) -> Frontier {
        let manifest = &index.pack;
        let mut items = Vec::new();

        for criterion in &manifest.definition_of_done {
            items.push(FrontierItem {
                kind: FrontierItemKind::DefinitionOfDone,
                key: criterion.clone(),
                description: criterion.clone(),
                satisfied: is_satisfied_definition(criterion, snapshot),
            });
        }
        for signal in &manifest.progress_signals {
            items.push(FrontierItem {
                kind: FrontierItemKind::ProgressSignal,
                key: signal.signal.clone(),
                description: signal.description.clone(),
                satisfied: is_satisfied_signal(&signal.signal, snapshot),
            });
        }
        for question in &manifest.required_questions {
            items.push(FrontierItem {
                kind: FrontierItemKind::RequiredQuestion,
                key: question.field.clone(),
                description: question.prompt.clone(),
                satisfied: is_satisfied_question(&question.field, snapshot),
            });
        }

        let candidate_verbs: Vec<String> = manifest
            .allowed_verbs
            .iter()
            .filter(|v| !manifest.forbidden_verbs.contains(v))
            .cloned()
            .collect();

        Frontier {
            items,
            candidate_verbs,
        }
    }
}

/// Spike satisfaction predicate for `definition_of_done` entries.
/// Snapshots from the stub hydrator are empty so this returns
/// `false`. Phase 4 swaps for a pack-defined predicate evaluator.
fn is_satisfied_definition(_criterion: &str, _snapshot: &ConstellationSnapshot) -> bool {
    false
}

/// Spike satisfaction predicate for `progress_signals`. Same as
/// above — always false until Phase 4.
fn is_satisfied_signal(_signal: &str, _snapshot: &ConstellationSnapshot) -> bool {
    false
}

/// Spike satisfaction predicate for `required_questions`. A
/// question is "satisfied" when the entity-state bag has a non-
/// empty attribute with the same key.
fn is_satisfied_question(field: &str, snapshot: &ConstellationSnapshot) -> bool {
    snapshot
        .entity_states
        .iter()
        .any(|entity| entity.attributes.contains_key(field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constellation::{ConstellationSnapshot, EntityStateDTO};
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;
    use std::collections::HashMap;

    fn manifest_yaml() -> &'static [u8] {
        br#"
id: book-setup
name: Book Setup
version: "0.1"
description: frontier engine fixture
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
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done:
  - "CBU created"
  - "All products attached"
progress_signals:
  - signal: cbu_id_present
    description: A CBU id has been assigned.
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
    fn empty_snapshot_leaves_every_item_open() {
        let frontier = FrontierEngine::compute(&make_index(), &ConstellationSnapshot::empty());
        assert_eq!(frontier.items.len(), 4); // 2 DoD + 1 signal + 1 question
        assert_eq!(frontier.open_count(), 4);
        assert!(!frontier.is_complete());

        // Candidate verbs: allowlist minus denylist.
        assert_eq!(
            frontier.candidate_verbs,
            vec!["cbu.create", "cbu.attach-product"]
        );
    }

    #[test]
    fn snapshot_with_question_attribute_marks_question_satisfied() {
        let snapshot = ConstellationSnapshot {
            entity_states: vec![EntityStateDTO {
                entity_id: "cbu:abc".to_string(),
                entity_kind: "cbu".to_string(),
                state: "draft".to_string(),
                attributes: HashMap::from([("jurisdiction".to_string(), serde_json::json!("LU"))]),
            }],
            hydrated_at: Utc::now(),
        };
        let frontier = FrontierEngine::compute(&make_index(), &snapshot);
        let question_item = frontier
            .items
            .iter()
            .find(|i| i.kind == FrontierItemKind::RequiredQuestion)
            .unwrap();
        assert!(question_item.satisfied, "jurisdiction attribute present");
        // DoD + signal still open.
        assert_eq!(frontier.open_count(), 3);
    }

    #[test]
    fn empty_pack_yields_empty_frontier() {
        let yaml = br#"
id: empty-pack
name: Empty
version: "0.1"
description: ""
invocation_phrases: []
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs: []
forbidden_verbs: []
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#;
        let (pack, pack_hash) = load_pack_from_bytes(yaml).unwrap();
        let index = SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        };
        let frontier = FrontierEngine::compute(&index, &ConstellationSnapshot::empty());
        assert!(frontier.items.is_empty());
        assert!(frontier.candidate_verbs.is_empty());
        // `is_complete` returns false when there are no items —
        // a pack with no acceptance criteria is "not done" because
        // the agent has nothing to prove.
        assert!(!frontier.is_complete());
    }
}
