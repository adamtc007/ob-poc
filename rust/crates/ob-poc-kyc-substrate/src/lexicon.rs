//! Lexicon-entry contract (§3 of EOP-DD-KYCUBO-001 / V&S §8.1).
//!
//! Every Phase-1/2 verb declares governing taxonomy, writes-fold, authority,
//! preconditions, and emits.  A verb without these fields fails the K-30 lint
//! (gap-report Test 9).  The whole-lexicon manifest hash (Q7) pins replay:
//! a semantic change to a verb = new hash = new verb identity.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::types::{Hash, VerbFqn};

// ── Taxonomy ──────────────────────────────────────────────────────────────────

/// The three governed reference-plane taxonomies (V&S §7.1).
/// Every verb declares exactly one governing taxonomy (K-30).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Taxonomy {
    /// Who or what can carry a KYC obligation (§5.1).
    Subject,
    /// Control-edge graph: typed claims with proof rules (§7.2).
    Control,
    /// Subject → role → obligation → evidence → decision (§7.3).
    Obligation,
}

// ── Fold identifier ───────────────────────────────────────────────────────────

/// Identifies a fold/projection this verb mutates.  A verb may write 1–2 folds
/// (e.g. `freeze` writes both `Determination` and `ObligationGraph`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoldId {
    ControlGraph,
    ObligationGraph,
    Determination,
    /// `fold::type_registry::TypeRegistryState` (EOP-DD-KYCUBO-TS.1 §3
    /// moves 2/6/7/8) — a third, independent fold axis (T6.1(a) extended).
    TypeRegistry,
}

// ── Precondition ─────────────────────────────────────────────────────────────

/// Ratchet ordering and prior-state requirements checked *before* appending the
/// event.  Enforcement is in `check_preconditions()`; the fold never enforces
/// them (the fold just applies; the write path guards).
///
/// Two verbs with non-trivial preconditions (§3):
/// - `ubo.edge.verify`       → `EvidenceCited`
/// - `ubo.determination.freeze` → `ReconciledProjection` + `StructureClassSupported`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precondition {
    /// A prior `ubo.edge.attach-evidence` event must exist in the stream for
    /// the same `target.edge_id` (K-11).  Enforces: evidence before verify.
    EvidenceCited,
    /// A prior `ubo.edge.reconcile-conflict` event must exist in the stream
    /// (K-14).  Enforces: reconcile before fold.
    ReconciledProjection,
    // `StrategySelected` RETIRED (TS.6 P2, K-G7) alongside
    // `ubo.determination.select-strategy` — `StructureClassSupported` alone
    // now gates strategy readiness, since the strategy is DERIVED from the
    // same guarded `structure_class`, not separately asserted. Reintroduce
    // only if a structure class is ever ratified with more than one
    // legitimate strategy to choose between.

    // ── T6.1(b) — EOP-DD-KYCUBO-KIT-T6 §2 variant inventory ────────────────
    // Machinery only in this tranche: every variant below is evaluable, but
    // only `StructureClassSupported` is attached to a lexicon entry today
    // (the T6.1(c) exemplar, matrix rows 6a/8a). The other 9 ship unattached;
    // T6.2–T6.4 attach them per the ratified matrix.
    /// `ControlState.registered` must be true (rows 1,2,5,6,10,11).
    SubjectRegistered,
    /// `ControlState.registered` must be false (row 9 — no double-registration).
    NotAlreadyRegistered,
    /// `ControlState.structure_class` must be `Some` (row 6).
    StructureClassified,
    /// `ControlState.structure_class` must be a member of the pinned
    /// implemented-strategy set (rows 6a/8a — the T6.1(c) fail-closed guard).
    StructureClassSupported,
    /// No active edge may already exist with the same (from, to, kind) as the
    /// event payload — contradicting claims go through `supersede`, never a
    /// second assert (rows 1,2; K-13).
    NoDuplicateActiveEdge,
    /// The edge named by `target.edge_id` must exist in the control graph
    /// (rows 3,4).
    EdgeExists,
    /// The edge named by `target.edge_id` must exist and not be superseded
    /// (rows 3,4).
    EdgeActive,
    /// The obligation named by the event payload's `obligation_id` must exist
    /// (rows 12–16).
    ObligationExists,
    /// The subject's obligation rollup must be `AllTerminal`
    /// (row 17 — the K-23 approval gate).
    SubjectAllTerminal,

    // ── D1 (EOP-DD-KYCUBO-TS.1 §3) — the four new moves ─────────────────────
    /// `target.entity_id` must be in `ControlState.registered_entity_ids`
    /// (TS.1 §3 rows 2/6/7: "entity exists" for assert-type/withdraw-member/
    /// correct-type). ControlState-only — ONE new ratchet-safe stud, not a
    /// checker-signature change. A probe with no `entity_id` (placement's
    /// generic per-entry probing) is vacuously satisfied, mirroring
    /// `NoDuplicateActiveEdge`/`NotAlreadyRegistered`.
    EntityRegistered,

    // ── D1 tree-cleanup follow-up, Phase 2 (EOP-STATE-KYCUBO-D1 §4/§7) ──────
    // Promotes the two studs that were hand-duplicated in the op layer
    // (`src/domain_ops/kyc_stream_ops.rs`) and the board preview
    // (`placement.rs::type_registry_candidates`) into real `Precondition`
    // primitives, evaluated by the single `check_preconditions` checker —
    // `TypeRegistryState`-only, mirroring `EntityRegistered`'s vacuous-when-
    // probed convention exactly.
    /// `target.entity_id` must not be withdrawn in `TypeRegistryState`
    /// (TS.1 §3 row 6 — `withdraw-member`'s "membership must be active").
    MembershipActive,
    /// `target.entity_id` must already have a type asserted in
    /// `TypeRegistryState` (TS.1 §3 row 7 — `correct-type`'s "a type was
    /// already asserted"; nothing to correct otherwise, that's
    /// `assert-type`'s job).
    PriorTypeAsserted,

    // ── EOP-DD-KYCUBO-TS.5 R1 — geometry becomes a precondition ─────────────
    /// TS.1 §2/§2a's type→linkage matrix, checked against the event's
    /// endpoints and (classified) pipe (`crate::geometry::check_type_geometry`).
    /// Attached to `ubo.edge.assert-control` and `ubo.edge.assert-economic-
    /// interest` — the verbs that introduce or restate an edge's (from,
    /// kind, to) triple (TS.5 §6 Q1/Q2). (`ubo.edge.pierce-nominee`'s own
    /// entry, which also carried this, was retired TS.6 P2 — piercing now
    /// reaches this precondition via its own `assert-control` step, same
    /// entry as any other assert-control call.)
    /// R5/R6: an alleged or untyped endpoint, or an unresolved pipe
    /// classification, ADMITS provisionally — this is the one precondition
    /// that must never fail closed on missing proof (CTN-2e); it fails
    /// closed only on a triple the matrix affirmatively refuses. Produces
    /// `KycError::GeometryRefused`, distinct from `PreconditionFailed`
    /// (TS.5 §5 `geometry_refusal_is_distinguishable_from_stud_refusal`).
    TypeGeometryPermits,
}

// ── Authority spec ────────────────────────────────────────────────────────────

/// Object-capability requirement for invoking a verb (K-17, K-30).
/// In the slice this is a role string; in W1-proper it maps to the ABAC model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoritySpec {
    pub required_role: String,
    pub interactive_only: bool,
}

impl AuthoritySpec {
    pub fn analyst() -> Self {
        Self {
            required_role: "analyst".into(),
            interactive_only: false,
        }
    }
    pub fn senior_analyst() -> Self {
        Self {
            required_role: "senior_analyst".into(),
            interactive_only: false,
        }
    }
    pub fn compliance_officer() -> Self {
        Self {
            required_role: "compliance_officer".into(),
            interactive_only: true,
        }
    }
}

// ── Emit spec ────────────────────────────────────────────────────────────────

/// Events produced when a verb fires (K-30, Q4).
/// `freeze` emits `PersonObligation` and `EntityObligation` for each resolved
/// person/entity, feeding the obligation graph (§3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitSpec {
    pub kind: String,
}

impl EmitSpec {
    pub fn person_obligation() -> Self {
        Self {
            kind: "PersonObligation".into(),
        }
    }
    pub fn entity_obligation() -> Self {
        Self {
            kind: "EntityObligation".into(),
        }
    }
}

// ── Lexicon entry ─────────────────────────────────────────────────────────────

/// One verb in the governed, versioned, content-addressed lexicon (V&S §8.1,
/// K-29–K-32).  The `hash` is the content address of this definition; replay
/// pins it (Q7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconEntry {
    pub fqn: VerbFqn,
    pub intent: String,
    /// Which taxonomy this verb governs (K-30).
    pub governing_taxonomy: Taxonomy,
    /// Folds mutated by this verb (K-30, K-32).
    pub writes: SmallVec<[FoldId; 2]>,
    /// Prior-state requirements checked before appending the event (K-11, K-14).
    pub preconditions: Vec<Precondition>,
    /// Who may invoke (K-17).
    pub authority: AuthoritySpec,
    /// Events emitted by this verb (K-30; Q4: `freeze` emits obligations).
    pub emits: Vec<EmitSpec>,
    /// Content address of this entry (Q7).
    pub hash: Hash,
}

impl LexiconEntry {
    /// Build + compute the content hash from the canonical JSON representation.
    pub fn build(
        fqn: impl Into<VerbFqn>,
        intent: &str,
        governing_taxonomy: Taxonomy,
        writes: SmallVec<[FoldId; 2]>,
        preconditions: Vec<Precondition>,
        authority: AuthoritySpec,
        emits: Vec<EmitSpec>,
    ) -> Self {
        let fqn = fqn.into();
        // Hash the stable fields (not `hash` itself — avoid circularity).
        let canonical = serde_json::json!({
            "fqn": fqn.as_str(),
            "intent": intent,
            "governing_taxonomy": governing_taxonomy,
            "writes": writes,
            "preconditions": preconditions,
            "authority": authority,
            "emits": emits,
        });
        let hash = Hash::of_json(&canonical);
        Self {
            fqn,
            intent: intent.to_owned(),
            governing_taxonomy,
            writes,
            preconditions,
            authority,
            emits,
            hash,
        }
    }
}

// ── Whole-lexicon manifest ────────────────────────────────────────────────────

/// Sorted set of lexicon entries; `hash` = SHA-256 of the sorted
/// concatenation of entry hashes (Q7 whole-lexicon version).
/// Replay of a frozen determination pins this manifest (K-18, K-31).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconManifest {
    pub hash: Hash,
    pub entries: BTreeMap<String, LexiconEntry>,
}

impl LexiconManifest {
    pub fn new(entries: Vec<LexiconEntry>) -> Self {
        let map: BTreeMap<String, LexiconEntry> =
            entries.into_iter().map(|e| (e.fqn.0.clone(), e)).collect();
        // Manifest hash = SHA-256 of sorted entry hashes concatenated.
        let mut bytes = Vec::new();
        for entry in map.values() {
            bytes.extend_from_slice(&entry.hash.0);
        }
        let hash = Hash::of(&bytes);
        Self { hash, entries: map }
    }

    pub fn get(&self, fqn: &str) -> Option<&LexiconEntry> {
        self.entries.get(fqn)
    }
}

// ── Assembly pack — the verbs that build the board ─────────────────────────────

/// Build the canonical Assembly-pack `LexiconManifest` (TS.6 §1: "a pack is a
/// capability" — this pack builds the board of truth, writes only the fact
/// stream, and has no write path to any decision record).
///
/// 20 dsl.kyc verbs today (25 originally covered; `select-strategy`,
/// `compute-fold`, and `pierce-nominee`'s own entries all retired TS.6 P2,
/// K-G7 — piercing's precondition pair now reaches the stream via
/// `assert-control`'s and `supersede`'s own entries, unchanged, composed by
/// the `ubo.edge.pierce-nominee` macro; `kyc.person.approve`/`.reject`
/// retired from this manifest TS.6 P2 and moved to `evaluation_lexicon()` —
/// see the note where they used to live, above): Phase-1/2 determination
/// verbs, preconditions from birth per the K-G7 reintroduction discipline,
/// plus 6 of the original 8 W5 obligation/person verbs (T6.0 closure,
/// 2026-08-12 — `kyc.obligation.waive` stays here too, deferred per its own
/// TS.6 note, pending obligation dissolution D2.0) plus 4 D1 type-registry
/// moves (EOP-DD-KYCUBO-TS.1 §3 moves 2/6/7/8 — assert-type,
/// withdraw-member, correct-type, record-enquiry). This is the normative
/// lexicon for the vertical slice (V&S Appendix A, phases 1–2) plus the W5
/// obligation lifecycle plus the D1 type-registry axis.
///
/// Formerly `assembly_lexicon()` — renamed TS.6 P1 when the pack split made
/// "phase1" the wrong axis to name it by; "assembly vs evaluation" is the
/// capability split that now matters (§1).
pub fn assembly_lexicon() -> LexiconManifest {
    use smallvec::smallvec;

    let entries = vec![
        // ── Phase 1 — substrate verbs ────────────────────────────────────────
        LexiconEntry::build(
            "kyc.subject.register",
            "Bring a subject into KYC scope, recording the basis for obligation",
            Taxonomy::Subject,
            smallvec![FoldId::ObligationGraph],
            // T6 row 9 CLOSED (2026-08-17, corrects EOP-DD-KYCUBO-KIT-T6
            // §5, see its §6 amendment): the ratified bare
            // `NotAlreadyRegistered` (`!state.registered`) would have
            // blocked every multi-person determination — `register`'s real
            // production usage fires MULTIPLE `kyc.subject.register`
            // events sharing one subject_root, differentiated by payload
            // `entity_id` (one call registers the subject entity itself,
            // one more per natural-person candidate — see
            // `kyc_m3_remediation.rs`'s real production-shaped fixtures).
            // §5 rejected a keyed-check amendment on the assumption it
            // would need a new parameterised `Precondition` variant,
            // breaking the matrix's niladic-variant property; it doesn't —
            // `NotAlreadyRegistered` stays a bare unit variant and the
            // check reads `event.payload`'s `entity_id` against
            // `ControlState.registered_entity_ids`, the same pattern
            // `NoDuplicateActiveEdge` already uses (`fold/control.rs`).
            vec![Precondition::NotAlreadyRegistered],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.subject.classify-structure",
            "Set structure class, driving both determination strategy and obligation set",
            Taxonomy::Subject,
            smallvec![FoldId::ControlGraph, FoldId::ObligationGraph],
            // T6.3 row 10: subject must be registered. Reclassification
            // stays legal (last-wins, per the current fold) — deliberately
            // NOT freezing the class; reclassify is a real workflow.
            vec![Precondition::SubjectRegistered],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.assert-control",
            "Claim a control edge (voting, board, GP statutory, etc.)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 1: subject must be registered; no active edge with the
            // same (from, to, kind) may already exist — contradicting claims
            // go through supersede, never a second assert (K-13). TS.5 R1:
            // the type-geometry layer — TS.1 §1's FIRST constraint, ahead of
            // these positional studs.
            vec![
                Precondition::SubjectRegistered,
                Precondition::NoDuplicateActiveEdge,
                Precondition::TypeGeometryPermits,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.assert-economic-interest",
            "Claim an economic-interest edge (shareholding percentage)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 2: same two studs as row 1, reused. TS.5 R1/§6 Q1:
            // geometry checked against the CLASSIFIED pipe (pipe_of), not a
            // raw kind — economic-interest edges carry no `kind` field.
            vec![
                Precondition::SubjectRegistered,
                Precondition::NoDuplicateActiveEdge,
                Precondition::TypeGeometryPermits,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.attach-evidence",
            "Cite documentary proof for a control or economic edge",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 3: target edge must exist and not be superseded.
            vec![Precondition::EdgeExists, Precondition::EdgeActive],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            // The non-trivial one: requires EvidenceCited (K-11).
            "ubo.edge.verify",
            "Ratchet edge to Verified state; requires evidence previously attached",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![Precondition::EvidenceCited],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.supersede",
            "Retire an edge and replace with a new one (supersede-never-delete, K-13)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 4: target edge must exist and not already be
            // superseded (double-supersede would no-op-pollute the stream).
            vec![Precondition::EdgeExists, Precondition::EdgeActive],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        // "ubo.edge.pierce-nominee" LexiconEntry RETIRED (TS.6 P2, K-G7,
        // 2026-08-22): the verb it covered no longer exists — piercing is
        // now the `ubo.edge.pierce-nominee` MACRO (config/verb_schemas/
        // macros/ubo.yaml) composing `ubo.edge.assert-control` (its own
        // entry above already carries the equivalent precondition set —
        // SubjectRegistered, NoDuplicateActiveEdge, TypeGeometryPermits —
        // plus a new op-layer-only "referenced edge is EdgeKind::Nominee
        // and active" check when its `pierced-from` arg is present, same
        // no-precondition-primitive discipline the retired verb used) +
        // `ubo.edge.supersede` (its entry below: EdgeExists, EdgeActive).
        // The decomposition is exact — no precondition was lost or gained.
        LexiconEntry::build(
            "ubo.edge.reconcile-conflict",
            "Canonicalise conflicting source edges before determination (K-14)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 5: subject must be registered. Ratified WITHOUT the
            // matrix's optional "≥1 active economic edge" amendment — kept
            // callable early, deliberately, per the T6 ratification note.
            vec![Precondition::SubjectRegistered],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        // ── Phase 2 — determination verbs ────────────────────────────────────
        // TS.6 P2 (K-G7): `ubo.determination.select-strategy` RETIRED —
        // "strategy follows from entity type" (TS.0 §1). The explicit
        // confirmation step was redundant: `classify-structure` alone
        // already determines the strategy unambiguously for every
        // implemented class (11 classes, 8 arms, TOTAL —
        // `strategy_for_structure_class`, pinned by
        // `implemented_class_split_matches_strategy_arms`). Reintroduction
        // path: if a structure class is ever ratified with more than one
        // legitimate strategy to choose between, select-strategy (or an
        // equivalent explicit-choice verb) comes back — that ambiguity
        // does not exist today.
        //
        // TS.6 P2 (K-G7, RATIFIED 2026-08-22): `ubo.determination.compute-fold`
        // RETIRED — "a derivation dressed as a verb" (TS.6 §5a). It was a
        // pure read (fold the stream, return a summary, append nothing) that
        // happened to carry `freeze`'s own precondition pair
        // (`ReconciledProjection`, `StructureClassSupported`) purely so the
        // pair could be demonstrated/enforced pre-freeze; `freeze` already
        // declares the identical pair independently, so nothing needed
        // building to "preserve the gate elsewhere" — there was nowhere
        // else it needed to go. Reintroduction path: if a genuine read-only
        // preview/projection capability is ever needed again, it should be
        // a query API, not a `dsl.kyc` fact-stream verb (this is exactly
        // the shape `enumerate_placement_set`/`preview` already serve).
        // TS.6 §5 (K-G7, RATIFIED): `ubo.determination.apply-smo-fallback`
        // LexiconEntry RETIRED 2026-08-22. SMO is PULLED on exhaustion by
        // the traversal (`determination.rs` walks OfficerAppointment edges
        // into the frontier and emits `Prong::SmoFallback` straight into
        // `candidates`, TS.3 §4a) — asserting it was a second way to WRITE
        // an answer the system already computes, which could contradict the
        // traversal. Its precondition pair (ReconciledProjection,
        // StructureClassSupported) was never unique to it: `freeze` declares
        // the identical pair independently, so nothing was lost.
        // Reintroduction path: a query/preview API, never a fact-stream verb.
        LexiconEntry::build(
            // The pivot verb: pins determination AND emits person obligations.
            "ubo.determination.freeze",
            "Pin an immutable determination; emits PersonObligation for each resolved person",
            Taxonomy::Control,
            smallvec![FoldId::Determination, FoldId::ObligationGraph],
            // T6.1(c) exemplar (matrix row 8a): StructureClassSupported is
            // now freeze's sole structure-class gate (TS.6 P2 removed the
            // separate StrategySelected stud it used to pair with — the
            // strategy IS derived from this same guarded structure_class,
            // so the two studs had become one fact checked twice).
            vec![
                Precondition::ReconciledProjection,
                Precondition::StructureClassSupported,
            ],
            AuthoritySpec::senior_analyst(),
            vec![EmitSpec::person_obligation(), EmitSpec::entity_obligation()],
        ),
        // ── T6.0 — obligation/person family entries (K-G6 closure) ──────────
        // Entries only (T0.3-ratified split from T6): no preconditions
        // authored here — that stays T6.1+. `governing_taxonomy: Obligation`
        // for all 8 — the Subject→role→obligation→evidence→decision chain
        // (§7.3) this taxonomy names explicitly includes the decision step,
        // which is what `person.approve`/`person.reject` are.
        LexiconEntry::build(
            "kyc.obligation.create",
            "Create an obligation for a subject under a stated role basis (K-21, K-35)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 11 (⊗ cross-fold, the motivating case for the T6.1
            // unified checker): subject must be registered.
            vec![Precondition::SubjectRegistered],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "assert.identity",
            "Advance the identity verification track of an obligation",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 12 (⊗): target obligation exists. `SubjectNotDecided`
            // retired TS.6 P2 — decisions no longer reach the fact stream at
            // all (see `kyc.person.approve`/`.reject`'s TS.6 note below), so
            // the substrate has nothing left to check it against.
            vec![Precondition::ObligationExists],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "assert.screening",
            "Advance the screening track of an obligation (K-26 — screening gates \
             approval, not determination)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 13 (⊗): same stud as row 12, reused.
            vec![Precondition::ObligationExists],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "assert.risk",
            "Advance the risk-assessment track of an obligation",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 14 (⊗): same stud as row 12, reused.
            vec![Precondition::ObligationExists],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.obligation.satisfy",
            "Mark all tracks on an obligation as satisfied (full KYC clearance)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 15 (⊗): same stud as row 12, reused.
            vec![Precondition::ObligationExists],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.obligation.waive",
            "Waive an obligation (all tracks set to Waived with a recorded reason). \
             TS.6 §4 names this the future `decide.waive` — deferred: waive still \
             mutates live obligation-track fold state (`TrackState::Waived`, feeding \
             `derive_subject_state`'s AllTerminal computation), so it cannot leave \
             the fact stream until obligation dissolution (D2.0, unbuilt) removes \
             the track model it writes to. Moving it now would silently break \
             terminal-state derivation for any subject relying on a waiver.",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 16 (⊗): same stud as row 12, reused. Waive's extra
            // sensitivity is an AUTHORITY question (AuthoritySpec), a
            // separate ruling — not a stud, per the ratified matrix.
            vec![Precondition::ObligationExists],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        // `kyc.person.approve`/`kyc.person.reject` retired from this
        // manifest TS.6 P2 (K-G7) — renamed `decide.approve`/`decide.reject`
        // and moved to `evaluation_lexicon()` (`ob-poc-kyc-decide`, no
        // dependency on `ob-poc-kyc-seam`). Their K-23 "decision is final"
        // finality check (`Precondition::SubjectNotDecided`, retired
        // alongside them) moved with them onto `kyc_decision_records`
        // directly — the substrate's pure fold can no longer see decisions
        // at all, by construction, which is the whole point of the split.
        // ── D1 (EOP-DD-KYCUBO-TS.1 §3) — the four new moves ──────────────────
        LexiconEntry::build(
            "kyc.subject.assert-type",
            "Assert this entity is of this type (TS.1 move 2) — always Alleged; \
             supersedes any prior type assertion (last-wins)",
            Taxonomy::Subject,
            smallvec![FoldId::TypeRegistry],
            vec![Precondition::EntityRegistered],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.subject.correct-type",
            "Correct a wrongly-asserted type (TS.1 move 7) — triggers the §4 cascade: \
             affected linkages demote and flag for re-assertion, determination marked \
             stale; never silently deletes",
            Taxonomy::Subject,
            smallvec![FoldId::TypeRegistry],
            vec![Precondition::EntityRegistered, Precondition::PriorTypeAsserted],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.subject.withdraw-member",
            "This entity is not, or is no longer, a group member (TS.1 move 6) — \
             flags membership, never deletes (TS.1 §2c basket-of-references)",
            Taxonomy::Subject,
            smallvec![FoldId::TypeRegistry],
            vec![Precondition::EntityRegistered, Precondition::MembershipActive],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.subject.record-enquiry",
            "Record sources consulted and searches run (TS.1 move 8) — completeness \
             is a diligence assertion, never a proof (TS.0 §6.3)",
            Taxonomy::Subject,
            smallvec![FoldId::TypeRegistry],
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
    ];

    LexiconManifest::new(entries)
}

// ── Evaluation pack — checks and verdicts ──────────────────────────────────────

/// Build the canonical Evaluation-pack `LexiconManifest` (TS.6 §1/§4).
///
/// 2 verbs today: `decide.approve`, `decide.reject` (renamed from
/// `kyc.person.approve`/`.reject`, TS.6 P2, landed with the structural
/// crate split — `ob-poc-kyc-decide` has no dependency on
/// `ob-poc-kyc-seam`, so this pack has no write path to the fact stream by
/// construction). `decide.waive` (renamed from `kyc.obligation.waive`)
/// stays in `assembly_lexicon()` for now — see that entry's own note; it is
/// a live Assembly-fold mutation until obligation dissolution (D2.0)
/// removes the obligation-track model, not yet a pure verdict.
///
/// Both entries declare `writes: []` and `preconditions: []` — deliberately.
/// Neither writes to any substrate fold (they write to `kyc_decision_records`
/// instead, entirely outside the pure in-memory `ControlState`/
/// `ObligationState` model this manifest's checker enforces), and their
/// real gating (K-23 `SubjectAllTerminal` for approve; the finality check
/// for both) is a hand-rolled op-layer check against `kyc_decision_records`
/// and the (read-only) obligation fold — the substrate's pure
/// `check_control_preconditions` checker has no DB access and cannot
/// express either check as a `Precondition` variant.
pub fn evaluation_lexicon() -> LexiconManifest {
    use smallvec::smallvec;
    let entries = vec![
        LexiconEntry::build(
            "decide.approve",
            "Approve a subject once all obligations are terminal (K-23 gate). \
             Writes only to kyc_decision_records — never the fact stream \
             (TS.6 §1 structural split).",
            Taxonomy::Obligation,
            smallvec![],
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "decide.reject",
            "Reject a subject (K-23 — decision recorded in kyc_decision_records, \
             never the fact stream, never erased). Rejection deliberately allowed \
             at ANY stage (early rejection is a real compliance outcome).",
            Taxonomy::Obligation,
            smallvec![],
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
    ];

    LexiconManifest::new(entries)
}
