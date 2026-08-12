//! Placement-set generator — EOP-PLAN-KYCUBO-KIT-001 T2 (closes KIT-3).
//!
//! KYC-native board/move types mirroring the `semantic-decision-contracts`
//! gameboard discipline (T0.1 rec c — mirror, do not adopt: its types are
//! graph-specific `GraphRevision`/`GraphDeltaPreview` anchors, ours are
//! event-stream/precondition-native): content-addressed move identity,
//! canonical ordering, an explicit abstention candidate, and a single
//! content hash for the whole placement set.
//!
//! Pure: `(state, lexicon, subject) -> PlacementSet`. No I/O, no store, no
//! clock read — the precondition checker never reads `as_of`, so a fixed
//! probe timestamp keeps this generator side-effect-free by construction.
//!
//! **Scope (T2, precondition-tier only).** A move is admitted iff the
//! verb's `LexiconEntry.preconditions` hold against the folded
//! `ControlState` (`check_control_preconditions`, the same oracle the write
//! path uses). Verbs with an empty precondition list are therefore always
//! admitted — the K-G5 "geometry-free" gap from T0.3's pack-closure audit.
//! Tightening that geometry (real domain preconditions per verb family) is
//! T6 scope, gated by T0.3's disposition register, not this generator.
//! Likewise the universe enumerated here is `lexicon.entries` — the 12
//! verbs `phase1_lexicon()` declares — not the full 22-verb dsl.kyc pack;
//! the K-G6 declaration-drift gap (10 undeclared verbs) is a lexicon-closure
//! problem, not a placement-set problem, and is out of this module's scope.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::event::IntentEvent;
use crate::fold::control::{check_control_preconditions, ControlState};
use crate::lexicon::LexiconManifest;
use crate::types::{AuthorityRef, EdgeId, Hash, Principal, SubjectId, TargetBinding, VerbFqn};

/// Canonical candidate id for "none of these moves apply" — always present
/// in a `PlacementSet`. Mirrors
/// `semantic-decision-contracts::ABSTENTION_CANDIDATE_ID`.
pub const NONE_OF_THE_ABOVE: &str = "abstain.none_of_the_above";

/// Fixed probe timestamp. `check_control_preconditions` never reads
/// `as_of` — this exists only so `IntentEvent::new` has a value, not to
/// carry meaning; using a clock read here would make this generator
/// impure for no reason.
fn probe_as_of() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).expect("epoch is representable")
}

/// Content-addressed identity of one legal move: `"{verb_fqn}::{target}"`.
/// Two moves with the same verb and target always collide to the same id
/// (idempotent enumeration; also the sort key for canonical ordering).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MoveId(pub String);

/// One legal move on the board: a verb bound to a specific target,
/// admitted because its lexicon preconditions hold against the folded
/// state (or because it declares none — K-G5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalMove {
    pub move_id: MoveId,
    pub verb_fqn: VerbFqn,
    pub target: TargetBinding,
}

/// The full set of legal moves at one folded state, canonically ordered
/// and content-hashed. Same `(subject, state, lexicon)` ⇒ bit-identical
/// `PlacementSet` (K-16/33 determinism discipline, applied one layer up
/// from the fold).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementSet {
    pub subject: SubjectId,
    /// Ordered by `move_id` (`BTreeMap` iteration order — no `HashMap`
    /// anywhere in this module). Always includes the abstention move.
    pub moves: Vec<LegalMove>,
    /// Content hash over the ordered move-id list.
    pub board_hash: Hash,
}

impl PlacementSet {
    pub fn abstain_move_id() -> MoveId {
        MoveId(NONE_OF_THE_ABOVE.to_string())
    }

    /// True iff `verb_fqn` appears in the placement set bound to `target`.
    pub fn admits(&self, verb_fqn: &str, target: &TargetBinding) -> bool {
        self.moves
            .iter()
            .any(|m| m.verb_fqn.as_str() == verb_fqn && &m.target == target)
    }
}

/// Verbs whose target is a specific edge in the folded state, rather than
/// the subject itself. Mirrors the `TargetBinding` shape each verb's
/// dispatch handler actually reads (`ob-poc/src/domain_ops/kyc_stream_ops.rs`);
/// duplicated here (not derived from `LexiconEntry`, which carries no arg
/// schema — CLAUDE.md's documented K-G6-adjacent gap) rather than guessed.
fn is_edge_scoped(verb_fqn: &str) -> bool {
    matches!(
        verb_fqn,
        "ubo.edge.verify" | "ubo.edge.attach-evidence" | "ubo.edge.supersede"
    )
}

fn move_id_for(verb_fqn: &str, target: &TargetBinding) -> MoveId {
    let target_key = match target.edge_id {
        Some(EdgeId(id)) => format!("edge:{id}"),
        None => "subject".to_string(),
    };
    MoveId(format!("{verb_fqn}::{target_key}"))
}

fn probe_event(subject: SubjectId, verb_fqn: &str, target: TargetBinding) -> IntentEvent {
    IntentEvent::new(
        subject,
        verb_fqn,
        Principal::test_analyst(),
        AuthorityRef("placement-probe".into()),
        target,
        serde_json::Value::Null,
        probe_as_of(),
    )
}

fn board_content_hash(moves: &[LegalMove]) -> Hash {
    let ids: Vec<&str> = moves.iter().map(|m| m.move_id.0.as_str()).collect();
    Hash::of_json(&serde_json::json!({ "moves": ids }))
}

/// Enumerate the legal placement set for `subject` given its folded
/// `state` and the `lexicon`'s registered verb set.
///
/// For edge-scoped verbs, one candidate move is probed per edge present in
/// `state.edges` (including superseded edges — K-13 supersede-never-delete
/// means they remain addressable targets; precondition checks, not this
/// enumeration, are what should eventually exclude them, per T6). For all
/// other verbs, one subject-scoped candidate is probed.
pub fn enumerate_placement_set(
    subject: SubjectId,
    state: &ControlState,
    lexicon: &LexiconManifest,
) -> PlacementSet {
    let mut candidates: BTreeMap<MoveId, LegalMove> = BTreeMap::new();

    for entry in lexicon.entries.values() {
        let fqn = entry.fqn.as_str();
        let targets: Vec<TargetBinding> = if is_edge_scoped(fqn) {
            state
                .edges
                .keys()
                .map(|edge_id| TargetBinding::for_edge(subject, *edge_id))
                .collect()
        } else {
            vec![TargetBinding::for_subject(subject)]
        };

        for target in targets {
            let probe = probe_event(subject, fqn, target.clone());
            if check_control_preconditions(entry, state, &probe).is_ok() {
                let id = move_id_for(fqn, &target);
                candidates.insert(
                    id.clone(),
                    LegalMove {
                        move_id: id,
                        verb_fqn: entry.fqn.clone(),
                        target,
                    },
                );
            }
        }
    }

    let abstain_id = PlacementSet::abstain_move_id();
    candidates.insert(
        abstain_id.clone(),
        LegalMove {
            move_id: abstain_id,
            verb_fqn: VerbFqn(NONE_OF_THE_ABOVE.to_string()),
            target: TargetBinding::default(),
        },
    );

    // BTreeMap<MoveId, _> iteration is already canonically (move_id-)sorted.
    let moves: Vec<LegalMove> = candidates.into_values().collect();
    let board_hash = board_content_hash(&moves);

    PlacementSet {
        subject,
        moves,
        board_hash,
    }
}
