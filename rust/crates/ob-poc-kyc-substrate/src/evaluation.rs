//! D2.0 — the evaluation pack's foundation: check-applicability and the run
//! book. Pure, deterministic, no DB (same discipline as `determination.rs`).
//!
//! **The boundary that must not break (D2.0 §2):** this module reads the
//! board (`ControlState`, `TypeRegistryState`, `Option<&FrozenDetermination>`)
//! and produces run/finding records. It has no append in its dependency
//! graph and must never gain one.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::determination::{FrozenDetermination, ProvisionalityReason};
use crate::error::KycError;
use crate::fold::control::{ControlState, StructureClass};
use crate::fold::type_registry::TypeRegistryState;
use crate::geometry::EntityType;
use crate::types::{EntityId, EventId, Hash, SubjectId};

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

/// A check's declared shape: an id and its applicability. The check
/// catalogue's actual contents (what a sanctions/threshold/evidence check
/// tests) are explicitly out of scope here (D2.0 §7 Q2) — this trait is the
/// machinery a real check will implement against.
pub trait Check {
    fn check_id(&self) -> &str;
    fn applicability(&self) -> &[ApplicabilityCondition];
}

/// The in-scope set for a board: every check whose applicability holds,
/// computed fresh — never stored (D2.0 §3's one invariant regardless).
pub fn in_scope_check_ids<C: Check>(checks: &[C], board: &BoardSnapshot<'_>) -> Vec<String> {
    checks
        .iter()
        .filter(|c| applicability_holds(c.applicability(), board))
        .map(|c| c.check_id().to_string())
        .collect()
}

// ── §4 The run book ─────────────────────────────────────────────────────────

/// Three verdicts, not two (D2.0 §4). `Unevaluable` is first-class: a check
/// whose facts are absent has not failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Unevaluable,
}

/// Why a check could not be evaluated. Reuses the assurance profile's own
/// provisionality distinctions (TS.3 §2a) rather than inventing a parallel
/// vocabulary, plus the one genuinely new case D2.0 §4 implies: the fact
/// this check needs simply isn't on the board at all (distinct from
/// "on the board but unproven").
#[derive(Debug, Clone)]
pub enum UnevaluableReason {
    /// The fact this check needs has not been recorded on the board at all.
    FactAbsent { what: String },
    /// The fact exists but rests on unproven ground (TS.3 §2a distinctions).
    Provisional(ProvisionalityReason),
}

/// What a check's evaluation concluded about one subject, citing the facts
/// it relied on (K-35-style traceability — never an unsourced verdict).
#[derive(Debug, Clone)]
pub struct Finding {
    pub check_id: String,
    pub subject: EntityId,
    pub verdict: Verdict,
    /// `Some` iff `verdict == Unevaluable`.
    pub unevaluable_reason: Option<UnevaluableReason>,
    /// Free-text detail for `Pass`/`Fail` (what was checked, what it found).
    pub detail: Option<String>,
    /// Events relied on for this verdict.
    pub cites: Vec<EventId>,
}

impl Finding {
    pub fn pass(check_id: impl Into<String>, subject: EntityId, cites: Vec<EventId>) -> Self {
        Self {
            check_id: check_id.into(),
            subject,
            verdict: Verdict::Pass,
            unevaluable_reason: None,
            detail: None,
            cites,
        }
    }

    pub fn fail(
        check_id: impl Into<String>,
        subject: EntityId,
        detail: impl Into<String>,
        cites: Vec<EventId>,
    ) -> Self {
        Self {
            check_id: check_id.into(),
            subject,
            verdict: Verdict::Fail,
            unevaluable_reason: None,
            detail: Some(detail.into()),
            cites,
        }
    }

    pub fn unevaluable(check_id: impl Into<String>, subject: EntityId, reason: UnevaluableReason) -> Self {
        Self {
            check_id: check_id.into(),
            subject,
            verdict: Verdict::Unevaluable,
            unevaluable_reason: Some(reason),
            detail: None,
            cites: vec![],
        }
    }
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
    pub trigger: String,
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
    pub trigger: String,
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
    pub fn new(run_id: Uuid, pins: RunPins, findings: Vec<Finding>) -> Result<Self, KycError> {
        if pins.trigger.trim().is_empty() {
            return Err(KycError::IncompleteRun { reason: "trigger is empty".into() });
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
