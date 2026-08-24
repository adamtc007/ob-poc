//! D2.0/D2.1 — the evaluation pack's foundation and engine: check
//! applicability, verdict computation, and the run book. Pure, deterministic,
//! no DB (same discipline as `determination.rs`).
//!
//! **The boundary that must not break (D2.0 §2):** this module reads the
//! board (`ControlState`, `TypeRegistryState`, `Option<&FrozenDetermination>`)
//! and produces run/finding records. It has no append in its dependency
//! graph and must never gain one.
//!
//! **D2.1 (2026-08-23) closes the gap D2.0 left open:** D2.0 shipped the run
//! book's envelope (pins, hashes, append-only shape) but no way to fill it —
//! `Check` had no `evaluate()`, the catalogue passed by every decide op was a
//! literal `&[]`, and `findings` was hardcoded `json!([])` at the persist
//! site. `Check::evaluate` (below), `EvaluationCatalogue`, and the one real
//! proof check (`ProvenTypeCheck`, D2.1 §2/§7 Q2) close that gap. The check
//! CATALOGUE'S CONTENTS — what a sanctions, threshold, or jurisdiction check
//! actually tests — remain out of scope (D2.0 §7 Q2, reaffirmed D2.1 §2);
//! `ProvenTypeCheck` exists as the machinery's own end-to-end proof, not as
//! catalogue content.

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::determination::{FrozenDetermination, ProvisionalityReason};
use crate::error::KycError;
use crate::fold::control::{ControlState, StructureClass};
use crate::fold::type_registry::{TypeProofStatus, TypeRegistryState};
use crate::geometry::EntityType;
use crate::types::{EventId, Hash, SubjectId};

// ── §3 Applicability — a closed, typed condition vocabulary (RULED) ────────

/// A relative risk threshold. Typed per D2.0 §3's ruling ("risk rating at or
/// above" is a real condition kind, implemented in Rust) even though no
/// risk-rating value is recorded on the board today (P0 0c board-gap
/// finding) — the taxonomy is real; the data source is a separate,
/// out-of-scope gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// The closed set of applicability condition kinds (D2.0 §3, RULED
/// 2026-08-22). Deliberately NOT an expression language: no parser, no
/// arbitrary predicates. Adding a new kind is a code change (this enum) and
/// a ruling, not a data change. `holds()` is an exhaustive match with no
/// catch-all — a new variant is a compile error until every caller handles it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicabilityCondition {
    /// At least one entity on the board carries this asserted (even merely
    /// alleged — applicability is a scoping question, not a proof question)
    /// entity type.
    EntityTypePresent(EntityType),
    /// At least one jurisdiction fact exists on the board. **Board gap
    /// (P0 0c):** no jurisdiction source exists on the board today — this
    /// always evaluates `false` until Assembly records one; the condition
    /// kind is real, its data source is not.
    JurisdictionPresent,
    /// A risk rating at or above the given threshold is recorded on the
    /// board. **Board gap (P0 0c):** no risk-rating value is recorded on the
    /// board today — always `false` until Assembly records one.
    RiskAtOrAbove(RiskLevel),
    /// The board's folded structure class matches the given class.
    StructureClassPresent(StructureClass),
    /// Always applicable — no board fact required.
    Unconditional,
}

/// Read-only view of the board a check may consult (D2.0 §2: taxonomy,
/// determination, assurance profile, and their pins — never raw assertions).
pub struct BoardSnapshot<'a> {
    pub control: &'a ControlState,
    pub type_registry: &'a TypeRegistryState,
    pub determination: Option<&'a FrozenDetermination>,
}

impl ApplicabilityCondition {
    /// Whether this condition currently holds against the board. Exhaustive
    /// match, no `_ =>` arm — a new `ApplicabilityCondition` variant fails to
    /// compile here until handled explicitly.
    pub fn holds(&self, board: &BoardSnapshot<'_>) -> bool {
        match self {
            ApplicabilityCondition::EntityTypePresent(want) => {
                board.type_registry.types.values().any(|rec| &rec.entity_type == want)
            }
            ApplicabilityCondition::JurisdictionPresent => false,
            ApplicabilityCondition::RiskAtOrAbove(_) => false,
            ApplicabilityCondition::StructureClassPresent(want) => {
                board.control.structure_class.as_ref() == Some(want)
            }
            ApplicabilityCondition::Unconditional => true,
        }
    }
}

/// A check's applicability is the set of conditions under which it becomes
/// relevant — composed with OR: "a check declares what makes it applicable"
/// (D2.0 §3) reads as a set of triggers, not a conjunction of prerequisites.
/// `Unconditional` as the sole condition is the always-in-scope case.
pub fn applicability_holds(conditions: &[ApplicabilityCondition], board: &BoardSnapshot<'_>) -> bool {
    conditions.iter().any(|c| c.holds(board))
}

/// A check's full shape (D2.1 §2): an id, its applicability, and — new this
/// tranche — how to actually run it. `evaluate` is machinery, not catalogue
/// content: it turns a board into a `Verdict`, the thing D2.0 shipped no way
/// to produce. The check catalogue's actual CONTENTS (what a sanctions,
/// threshold, or evidence-sufficiency check tests) remain explicitly out of
/// scope (D2.0 §7 Q2 / D2.1 §2) — this trait is what any real check,
/// whenever one is authored, implements against.
pub trait Check {
    fn check_id(&self) -> &str;
    fn applicability(&self) -> &[ApplicabilityCondition];
    /// Run this check against the board and produce an outcome. D2.1 §2/C1:
    /// "a check that cannot be run is not a check" — this single call still
    /// fully determines the outcome, no second pass. Widened 2026-08-24
    /// (D2.0 §4: "a verdict plus findings citing the facts relied on") to
    /// return citations alongside the verdict in the SAME call, rather than
    /// adding a second `cites()` method — a second call against the same
    /// board would reopen exactly the "risk of [two things] disagreeing"
    /// C1 was ratified to close off, one layer over (verdict vs cites
    /// instead of verdict vs reason). Ruled by Adam over widening `Verdict`
    /// itself (touches ~39 existing `Verdict::Pass`/`Fail`/`Unevaluable`
    /// sites and blurs D2.0 §4's own separation of "a verdict" from
    /// "findings citing") and over a second trait method.
    fn evaluate(&self, board: &BoardSnapshot<'_>) -> CheckOutcome;
}

/// A check's one-call result: the verdict, plus the real `EventId`s it
/// relied on to reach it. Citing nothing is coherent exactly when the
/// verdict's own data already explains the absence (`Unevaluable` — see
/// `ProvenTypeCheck::evaluate`); a `Pass` or `Fail` over a non-empty board
/// always rests on a real, citable fact.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub verdict: Verdict,
    pub cites: Vec<EventId>,
}

/// D2.1 §2 C3: "a real catalogue type the run consults, replacing the
/// literal `&[]`." Holds check instances behind `dyn Check` so a catalogue
/// can mix different concrete check types (today it holds exactly one — see
/// `default_catalogue`). The catalogue's CONTENTS are D2.0 §7 Q2 / D2.1 §2's
/// explicit OUT-of-scope line; this type is the machinery that would run
/// whatever contents a future, separately-governed tranche adds.
pub struct EvaluationCatalogue(Vec<Arc<dyn Check + Send + Sync>>);

impl EvaluationCatalogue {
    pub fn new(checks: Vec<Arc<dyn Check + Send + Sync>>) -> Self {
        Self(checks)
    }

    pub fn checks(&self) -> &[Arc<dyn Check + Send + Sync>] {
        &self.0
    }

    /// The one real check that exists today (D2.1 §2's "one real check, end
    /// to end — as the proof the machinery works, not as catalogue
    /// content"; §7 Q2 RULED it is `ProvenTypeCheck`, below). Every decide
    /// op consults this catalogue instead of the D2.0-era literal `&[]`.
    pub fn default_catalogue() -> Self {
        Self(vec![Arc::new(ProvenTypeCheck)])
    }
}

/// The in-scope set for a board: every check whose applicability holds,
/// computed fresh — never stored (D2.0 §3's one invariant regardless).
pub fn in_scope_check_ids(catalogue: &EvaluationCatalogue, board: &BoardSnapshot<'_>) -> Vec<String> {
    catalogue
        .checks()
        .iter()
        .filter(|c| applicability_holds(c.applicability(), board))
        .map(|c| c.check_id().to_string())
        .collect()
}

/// Evaluate every in-scope check against the board and produce the run's
/// findings (D2.1 §2 C3 — replaces the D2.0-era hardcoded `json!([])`).
/// Uses the SAME applicability filter as `in_scope_check_ids`, so a check
/// that is in scope always has a matching finding and vice versa — the two
/// cannot drift apart because both read the same predicate over the same
/// catalogue and board.
pub fn evaluate_checks(
    catalogue: &EvaluationCatalogue,
    board: &BoardSnapshot<'_>,
    subject: SubjectId,
) -> Vec<Finding> {
    catalogue
        .checks()
        .iter()
        .filter(|c| applicability_holds(c.applicability(), board))
        .map(|c| {
            let outcome = c.evaluate(board);
            Finding { check_id: c.check_id().to_string(), subject, verdict: outcome.verdict, cites: outcome.cites }
        })
        .collect()
}

// ── D2.1 §2/§7 Q2 — the one real proof check ────────────────────────────────

/// "Every entity on the board has a proven type" (D2.1 §7 Q2, RULED).
/// Board-only, no compliance input — exercises all three verdicts from pure
/// board state:
///
/// - **Pass**: every registered, non-withdrawn entity's type is
///   `TypeProofStatus::Proved` (vacuously true on an empty board — the
///   founding property, D2.0 §1, applies here too: nothing to fail on is a
///   legitimate Pass, not a special case).
/// - **Unevaluable**: some entity's type is `Alleged` (asserted but not yet
///   proved — TS.3 §2a's own provisionality distinction) or entirely absent
///   (no type asserted at all — D2.0 §4's `FactAbsent`, a different,
///   "unresolved" state from `Alleged`).
/// - **Fail**: a WITHDRAWN member (`TypeRegistryState::withdrawn_members`)
///   whose type was never proved. Withdrawal (TS.1 move 6) is a real,
///   board-only fact meaning no further evidence will ever arrive for that
///   entity within this determination — its type proof is now permanently
///   stuck, a genuine "cannot be proven going forward" derived purely from
///   board state, distinct from "not yet proven" for an active member.
///
/// Aggregates worst-first across `registered_entity_ids` (a `BTreeSet`, so
/// iteration order — and therefore which offending entity a Fail/Unevaluable
/// verdict names — is deterministic): any qualifying Fail wins over any
/// qualifying Unevaluable wins over Pass.
pub struct ProvenTypeCheck;

impl Check for ProvenTypeCheck {
    fn check_id(&self) -> &str {
        "board.every-entity-has-a-proven-type"
    }

    fn applicability(&self) -> &[ApplicabilityCondition] {
        // Always in scope — this check is the machinery's own end-to-end
        // proof, not conditional catalogue content (D2.1 §2).
        &[ApplicabilityCondition::Unconditional]
    }

    fn evaluate(&self, board: &BoardSnapshot<'_>) -> CheckOutcome {
        for &entity in &board.control.registered_entity_ids {
            let withdrawn = board.type_registry.is_withdrawn(entity);
            if withdrawn && !matches!(board.type_registry.proof_of(entity), Some(TypeProofStatus::Proved)) {
                // D2.0 §4: "a fail verdict is a finding, citing the facts
                // it rests on." Cites the withdrawal event that actually
                // CAUSED this Fail (`withdrawal_event_id_of`, the same
                // discipline as `proof_event_id_of` — the real cause, not
                // an assertion), plus whatever type-proof fact exists for
                // this entity (an Alleged assertion, if one was ever made
                // before withdrawal). When `proof_of` is `None` (no
                // assertion ever made), the assertion half is empty and the
                // verdict's own `detail` string explains why — but the
                // withdrawal citation is always present, since withdrawal
                // is a precondition of reaching this branch at all.
                let mut cites: Vec<EventId> =
                    board.type_registry.withdrawal_event_id_of(entity).into_iter().collect();
                cites.extend(board.type_registry.originating_event_id_of(entity));
                return CheckOutcome {
                    verdict: Verdict::Fail {
                        detail: format!(
                            "entity {} was withdrawn from the group with no proven type on record — \
                             its type can no longer be proven (no further evidence will arrive for a \
                             withdrawn member)",
                            entity.0
                        ),
                    },
                    cites,
                };
            }
        }
        let mut cites = Vec::new();
        for &entity in &board.control.registered_entity_ids {
            if board.type_registry.is_withdrawn(entity) {
                continue;
            }
            match board.type_registry.proof_of(entity) {
                Some(TypeProofStatus::Proved) => {
                    // The fact this entity's contribution to a Pass rests
                    // on: the event that PROVED its type (`attach-evidence`),
                    // not `originating_event_id_of` (the type ASSERTION —
                    // stays pinned to the pre-proof `assert-type` event even
                    // after evidence flips `proof` to `Proved`; citing it
                    // here would point at the allegation, not the proof).
                    cites.extend(board.type_registry.proof_event_id_of(entity));
                    continue;
                }
                Some(TypeProofStatus::Alleged) => {
                    // This Unevaluable rests on a real, resolvable fact:
                    // the assertion event that made the type Alleged in
                    // the first place — the same accessor and the same
                    // reasoning as the Fail path above (an Alleged
                    // assertion IS a fact the verdict rests on, not
                    // nothing). Distinct from the `None` arm below, where
                    // there is genuinely no event to cite.
                    return CheckOutcome {
                        verdict: Verdict::Unevaluable {
                            reason: UnevaluableReason::Provisional(ProvisionalityReason::AllegedType { entity }),
                        },
                        cites: board.type_registry.originating_event_id_of(entity).into_iter().collect(),
                    };
                }
                None => {
                    // Citing nothing here IS coherent — no assertion was
                    // ever made, so there is no event to point at; the
                    // verdict's own `FactAbsent` reason explains why.
                    return CheckOutcome {
                        verdict: Verdict::Unevaluable {
                            reason: UnevaluableReason::FactAbsent {
                                what: format!("entity {} has no type asserted at all", entity.0),
                            },
                        },
                        cites: vec![],
                    };
                }
            }
        }
        // A Pass over a non-empty board cites every entity's proving
        // event; a Pass over an EMPTY board (no registered entities —
        // vacuously true) cites nothing, by construction, since the loop
        // above never ran. This is the exact distinction P3 makes
        // self-evident in the persisted record.
        CheckOutcome { verdict: Verdict::Pass, cites }
    }
}

// ── §4 The run book ─────────────────────────────────────────────────────────

/// Three verdicts, not two (D2.0 §4). `Unevaluable` is first-class: a check
/// whose facts are absent has not failed.
///
/// **D2.1 §2/C1/C6:** `Fail` and `Unevaluable` now carry their own data —
/// `evaluate()`'s single call fully determines the outcome, so a persisted
/// `Unevaluable` verdict always carries the SAME reason that drove it (C6
/// `unevaluable_carries_its_reason`), never a reason recomputed separately
/// and at risk of disagreeing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Fail { detail: String },
    Unevaluable { reason: UnevaluableReason },
}

/// Why a check could not be evaluated. Reuses the assurance profile's own
/// provisionality distinctions (TS.3 §2a) rather than inventing a parallel
/// vocabulary, plus the one genuinely new case D2.0 §4 implies: the fact
/// this check needs simply isn't on the board at all (distinct from
/// "on the board but unproven").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnevaluableReason {
    /// The fact this check needs has not been recorded on the board at all.
    FactAbsent { what: String },
    /// The fact exists but rests on unproven ground (TS.3 §2a distinctions).
    Provisional(ProvisionalityReason),
}

/// What a check's evaluation concluded, citing the facts it relied on
/// (K-35-style traceability — never an unsourced verdict). `subject` is the
/// run's own `SubjectId` — the UBO-group determination root (D2.0 §7 Q3: "a
/// run is UBO-group level... findings tagged per subject") — not an
/// individual board `EntityId`; a check that names a SPECIFIC offending
/// entity does so inside its `Verdict`'s own data (see `ProvenTypeCheck`),
/// which is always available regardless of whether the board has any
/// entities on it at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub check_id: String,
    pub subject: SubjectId,
    pub verdict: Verdict,
    /// Events relied on for this verdict.
    pub cites: Vec<EventId>,
}

impl Finding {
    pub fn pass(check_id: impl Into<String>, subject: SubjectId, cites: Vec<EventId>) -> Self {
        Self { check_id: check_id.into(), subject, verdict: Verdict::Pass, cites }
    }

    pub fn fail(check_id: impl Into<String>, subject: SubjectId, detail: impl Into<String>, cites: Vec<EventId>) -> Self {
        Self { check_id: check_id.into(), subject, verdict: Verdict::Fail { detail: detail.into() }, cites }
    }

    pub fn unevaluable(check_id: impl Into<String>, subject: SubjectId, reason: UnevaluableReason) -> Self {
        Self { check_id: check_id.into(), subject, verdict: Verdict::Unevaluable { reason }, cites: vec![] }
    }
}

/// D2.1 §7 Q3 (RULED): "a run is an act in a session, not a background job
/// ... Sage triggers the permission and the REPL runs it against the
/// database. No side doors, except in test mode." A run's trigger must name
/// BOTH which verb ran it (`verb_fqn`, D2.0 §4's "who or what triggered it")
/// AND the session that acted (`session_id`) — "who or what" is incomplete
/// without knowing WHO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunTrigger {
    pub verb_fqn: String,
    pub session_id: Uuid,
}

/// A UBO-group-level evaluation run (D2.0 §7 Q3, RULED): one board hash, one
/// moment, findings tagged per subject. Append-only — re-running creates a
/// new run; this one, once built, is never mutated (§4).
#[derive(Debug, Clone)]
pub struct EvaluationRun {
    pub run_id: Uuid,
    pub subject_root: SubjectId,
    pub board_state_hash: Hash,
    pub evaluation_pack_version_hash: Hash,
    pub valid_time: DateTime<Utc>,
    pub knowledge_time: DateTime<Utc>,
    pub trigger: RunTrigger,
    pub in_scope_check_ids: Vec<String>,
    pub findings: Vec<Finding>,
}

/// Everything a run must pin (D2.0 §4/§6 `run_pins_are_complete`). Assembled
/// by the caller (who has the timing/trigger context) and validated by
/// `EvaluationRun::new`, which REFUSES to construct a run missing any pin —
/// a falsifiable gate, not a NOT NULL column nothing can violate.
pub struct RunPins {
    pub subject_root: SubjectId,
    pub board_state_hash: Hash,
    pub evaluation_pack_version_hash: Hash,
    pub valid_time: DateTime<Utc>,
    pub knowledge_time: DateTime<Utc>,
    pub trigger: RunTrigger,
    pub in_scope_check_ids: Vec<String>,
}

impl EvaluationRun {
    /// Construct a run, refusing if any pin is incomplete. `run_id` is
    /// minted by the caller (durable-store layer owns id generation, same
    /// split as `EventId`/`SubjectId` elsewhere in this crate).
    ///
    /// An empty `in_scope_check_ids` is NOT refused — it is a legitimate
    /// computed value (zero applicable checks against today's catalogue,
    /// or a board nothing is applicable to yet), distinct from a pin that
    /// was never recorded at all.
    ///
    /// D2.1 §7 Q3: a run with no session origin is refused — the trigger's
    /// `session_id` must not be the nil UUID, same discipline as
    /// `subject_root`.
    pub fn new(run_id: Uuid, pins: RunPins, findings: Vec<Finding>) -> Result<Self, KycError> {
        if pins.trigger.verb_fqn.trim().is_empty() {
            return Err(KycError::IncompleteRun { reason: "trigger.verb_fqn is empty".into() });
        }
        if pins.trigger.session_id.is_nil() {
            return Err(KycError::IncompleteRun {
                reason: "trigger.session_id is the nil UUID — a run must be an act in a session (D2.1 §7 Q3)".into(),
            });
        }
        if pins.subject_root.0.is_nil() {
            return Err(KycError::IncompleteRun { reason: "subject_root is the nil UUID".into() });
        }
        Ok(Self {
            run_id,
            subject_root: pins.subject_root,
            board_state_hash: pins.board_state_hash,
            evaluation_pack_version_hash: pins.evaluation_pack_version_hash,
            valid_time: pins.valid_time,
            knowledge_time: pins.knowledge_time,
            trigger: pins.trigger,
            in_scope_check_ids: pins.in_scope_check_ids,
            findings,
        })
    }

    /// The current work list: failing and unevaluable findings of THIS run.
    /// Derived, never stored (D2.0 §4) — callers always derive from the
    /// LATEST run in a run history, never from a cached field.
    pub fn work_list(&self) -> Vec<&Finding> {
        self.findings.iter().filter(|f| !matches!(f.verdict, Verdict::Pass)).collect()
    }

    /// Whether this run is stale against the board's current hash (D2.0 §4
    /// `staleness_is_hash_comparison`) — a pure comparison, no flag anywhere.
    pub fn is_stale(&self, current_board_hash: Hash) -> bool {
        self.board_state_hash != current_board_hash
    }
}

/// The work list over a run HISTORY: the latest run's failing/unevaluable
/// findings — a query over the sequence, never a stored status field
/// (D2.0 §6 `work_list_is_derived_from_latest_run`). `runs` is assumed
/// ordered oldest-to-newest (append-only, §4).
pub fn work_list_from_history(runs: &[EvaluationRun]) -> Vec<&Finding> {
    match runs.last() {
        Some(latest) => latest.work_list(),
        None => vec![],
    }
}

/// Deterministic content hash of the board (`ControlState` + type registry +
/// determination, if any) at (T, axis, scope) — D2.0 §4's "board state
/// hash". Hand-rolled canonical strings, sorted, hashed — same idiom as
/// `placement::board_content_hash` / `DeterminationPin::compute_graph_hash`,
/// deliberately not a full `serde_json` dump of these structs (neither
/// derives `Serialize` today, and a canonical-string hash keeps this
/// function independent of their derive surface).
pub fn board_state_hash(board: &BoardSnapshot<'_>) -> Hash {
    let mut edge_parts: Vec<String> = board
        .control
        .edges
        .values()
        .map(|e| {
            format!(
                "{}|{:?}|{}->{}|{:?}|{:?}",
                e.id.0, e.kind, e.from.0, e.to.0, e.status, e.percentage
            )
        })
        .collect();
    edge_parts.sort();

    let mut type_parts: Vec<String> = board
        .type_registry
        .types
        .iter()
        .map(|(id, rec)| format!("{}|{:?}|{:?}", id.0, rec.entity_type, rec.proof))
        .collect();
    type_parts.sort();

    let registered_entities: BTreeSet<String> =
        board.control.registered_entity_ids.iter().map(|e| e.0.to_string()).collect();

    let structure_class = board.control.structure_class.as_ref().map(|c| format!("{c:?}"));
    let determination_hash = board.determination.map(|d| d.determination_hash.to_hex());

    Hash::of_json(&serde_json::json!({
        "edges": edge_parts,
        "types": type_parts,
        "structure_class": structure_class,
        "registered": board.control.registered,
        "registered_entities": registered_entities,
        "determination_hash": determination_hash,
    }))
}
