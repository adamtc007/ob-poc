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
/// A verb with a non-trivial precondition:
/// - `kyc_ubo.decide.determination.freeze` → `EntityTypeSupportsStrategy`
///   (T4, 2026-08-28 — was `StructureClassSupported`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precondition {
    // The verification-gate variant RETIRED (EOP-DD-UBO-PROOF-001 §3/§4,
    // T5, 2026-08-28) alongside `kyc_ubo.assert.edge.verification`, its
    // sole reader (K-G7: 0 real committed events under that FQN — full
    // deletion, not an R5 historical keep). "Log a proof before moving to
    // the next state" gated a ratchet that no longer exists — `evidence`
    // now logs a proof unconditionally (still gated by
    // `EdgeActive`/`EdgeExists`, unchanged), and there is no second move
    // left to cite it as a precondition for.
    // `ReconciledProjection` REMOVED (EOP-VS-UBO-GAME-001 T3, §3.3,
    // 2026-08-27) alongside `kyc_ubo.assert.edge.reconciliation`, its sole
    // writer, and the K-14 gate on freeze, its sole reader — "No reconcile.
    // ... a third path to what two moves [connect/disconnect] already do."
    // Unlike `NotAlreadyRegistered`/other T2 retirements, the enum variant
    // itself is deleted (not merely detached from every lexicon entry):
    // the concept it gated (a canonicalisation step before freeze) is
    // dissolved, not superseded by a successor precondition, so there is
    // no reason for the variant to keep compiling.
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
    // `StructureClassified`/`StructureClassSupported` RETIRED
    // (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) alongside the
    // `kyc_ubo.assert.subject.structure-class` verb and its op — fully
    // superseded by `EntityTypeSupportsStrategy` below, no reader remains.
    // Unlike the fold arm (kept R5-historical — 10 real committed events;
    // P0 census — because a stream replay must still populate
    // `ControlState.structure_class` for those events), a precondition is
    // a write-path-only check with no replay role, so there is nothing for
    // the variants to stay declared FOR. Deleted outright, matching the
    // `ReconciledProjection`/`select-strategy`/`compute-fold` precedent.
    /// EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28), replaces `StructureClassSupported`
    /// on `freeze`: the subject's `EntityType` (`TypeRegistryState`, not
    /// `ControlState` — crosses fold axes, same as `MembershipActive`) must
    /// dispatch to `DeterminationDispatch::Strategy(_)`, not
    /// `NotADeterminationSubject` and not unknown. §4 D1: refuses by name.
    EntityTypeSupportsStrategy,
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
    // `ObligationExists`/`SubjectAllTerminal` REMOVED (EOP-DD-UBO-CLEANOUT-001
    // T6 P2, 2026-09-07) with `fold/obligation.rs`/`ObligationState` — both
    // were already permanently unreachable at the real write path (the op
    // layer passed `validate_entry_fqn: None` for the three verbs that
    // declared `ObligationExists`, precisely because it was dead there —
    // see `identity`/`screening`/`risk`'s lexicon entries below, now
    // `vec![]`), and `SubjectAllTerminal` was declared on no live lexicon
    // entry (K-23 is enforced in `ob-poc-kyc-decide`, against
    // `kyc_decision_records`, never this checker — see `evaluation_lexicon()`
    // below).

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
    /// Attached to `kyc_ubo.assert.edge.connect` (T3 merge of the former
    /// `control` + `economic-interest`) — the verb that introduces or
    /// restates an edge's (from, kind, to) triple (TS.5 §6 Q1/Q2).
    /// (`kyc_ubo.assert.edge.nominee-piercing`'s own entry, which also
    /// carried this, was retired TS.6 P2 — piercing now reaches this
    /// precondition via its own `connect` step, same entry as any other
    /// connect call.)
    /// R5/R6: an alleged or untyped endpoint, or an unresolved pipe
    /// classification, ADMITS provisionally — this is the one precondition
    /// that must never fail closed on missing proof (CTN-2e); it fails
    /// closed only on a triple the matrix affirmatively refuses. Produces
    /// `KycError::GeometryRefused`, distinct from `PreconditionFailed`
    /// (TS.5 §5 `geometry_refusal_is_distinguishable_from_stud_refusal`).
    TypeGeometryPermits,

    // ── EOP-VS-UBO-GAME-001 T2 — `place` never carries update semantics ─────
    /// §8 Q1 (RULED 2026-08-27): correcting a block's type is `remove` then
    /// `place`, never a superseding placement — `place` must refuse an
    /// entity that is CURRENTLY on the board (registered and not withdrawn).
    /// An entity that has never been registered, or was registered and has
    /// since been `remove`d (withdrawn), passes: first-time placement and
    /// re-placement after removal are both legal. Reads `target.entity_id`
    /// — vacuous when the probe carries none, same convention as
    /// `EntityRegistered`/`MembershipActive`/`PriorTypeAsserted` above; the
    /// placement generator now probes `place` with a real entity_id for
    /// every board-enumerable candidate (T1's P4 deferral, closed for the
    /// re-placeable population — `placement.rs`).
    NotCurrentlyPlaced,

    // ── 2026-09-07 audit item 2 — the K-8 pierce guard hoisted to the chokepoint ─
    /// §3.4 R7: a rule expressed only in one surface's code is not a rule.
    /// The event payload's `pierced_from` edge id (present only when a
    /// `connect` call is the first half of the `kyc_ubo.assert.edge.nominee-piercing`
    /// macro composition) must reference an edge that EXISTS, is
    /// `EdgeKind::Nominee`, and is currently active — the underlying kind a
    /// nominee arrangement is pierced to reveal is asserted by THIS call's
    /// own `kind`, never carried by the cited edge. Vacuous when the probe
    /// carries no `pierced_from` — same convention as every other
    /// payload-keyed precondition above (a real caller always supplies it
    /// when piercing; absence means either an ordinary non-piercing connect
    /// or an abstract board-preview probe, never a real pierce attempt with
    /// the field missing).
    PiercedFromIsActiveNominee,

    /// TS.4 §3 Ruling B (K-8): a determination may not freeze while ANY
    /// active `EdgeKind::Nominee` edge remains unpierced anywhere in the
    /// subject's control graph — pure `ControlState` scan
    /// (`unpierced_nominee_edges`), independent of `event`/target/payload.
    /// `freeze` has exactly one
    /// live surface (the op — `canonical_event_shape` bails on this FQN by
    /// design, §3.2), so this promotion doesn't close a two-surface
    /// disagreement the way `PiercedFromIsActiveNominee` does; it is done
    /// anyway so the single declared checker stays the sole enforcement
    /// point, fuzzable through one entry point regardless of which surface
    /// (if any, ever) drives it.
    NoUnpiercedNomineeEdges,
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
/// 16 dsl.kyc verbs today (25 originally covered; `select-strategy`,
/// `compute-fold`, and `pierce-nominee`'s own entries all retired TS.6 P2,
/// K-G7 — piercing's precondition pair now reaches the stream via
/// `assert-control`'s and `supersede`'s own entries, unchanged, composed by
/// the `kyc_ubo.assert.edge.nominee-piercing` macro; `kyc.person.approve`/`.reject`
/// retired from this manifest TS.6 P2 and moved to `evaluation_lexicon()` —
/// see the note where they used to live, above): Phase-1/2 determination
/// verbs, preconditions from birth per the K-G7 reintroduction discipline,
/// plus 3 of the original 8 W5 obligation/person verbs still standing here
/// (`kyc_ubo.assert.entity.{identity,screening,risk}` — TS.6 §3 keeps these
/// PERMANENT Assembly facts; `creation`/`satisfaction` DISSOLVED and
/// `waiver` MOVED to `evaluation_lexicon()`, D2.0 §5, 2026-08-22) plus 4 D1
/// type-registry moves (EOP-DD-KYCUBO-TS.1 §3 moves 2/6/7/8 — assert-type,
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
        // `kyc_ubo.assert.subject.register`/`.type` RETIRED (EOP-VS-UBO-GAME-001
        // T2, §3.2) — merged into `place`. Their fold arms remain, historical
        // replay only (R5); `NotAlreadyRegistered` (register's old
        // precondition) stays a real `Precondition` variant so its own
        // `check_preconditions` arm keeps compiling for the enum's
        // exhaustiveness, but no lexicon entry attaches it any more.
        LexiconEntry::build(
            "kyc_ubo.assert.subject.place",
            "Put a group member on the board, in this slot, as this type — merges \
             register + assert-type (§3.2); place never carries update semantics, \
             a currently-placed entity must be `remove`d first (§8 Q1)",
            Taxonomy::Subject,
            smallvec![FoldId::ControlGraph, FoldId::TypeRegistry],
            vec![Precondition::NotCurrentlyPlaced],
            AuthoritySpec::analyst(),
            vec![],
        ),
        // `kyc_ubo.assert.subject.structure-class` LexiconEntry RETIRED
        // (EOP-DD-UBO-DISPATCH-001 T4, 2026-08-28) alongside the verb and
        // its op — the strategy is now derived directly from the subject's
        // EntityType (`dispatch_for_entity_type`), never separately
        // classified. 10 real committed events existed for this FQN before
        // deletion (P0 census); the fold-side parser stays R5-historical
        // for replay (structure_class_from_payload), but no entry survives
        // it here — matching the `reconciliation`/`type-correction`
        // precedent, not the "kept declared, unattached" T6.1(b) shape
        // (a LexiconEntry has no replay role either).
        // EOP-VS-UBO-GAME-001 T3 (§3.2, 2026-08-27): `control` + `economic-
        // interest` MERGED into `connect` — "they differ by kind, and
        // geometry already validates the classified pipe. Their
        // preconditions are the same... This merge passes the test." (P0b
        // confirmed the two precondition sets were byte-identical before
        // merging.) Both former FQNs' fold arms survive in
        // `fold::control` — `control`'s historical-only (4 real events,
        // R5), `economic-interest`'s deleted outright (0 real events,
        // K-G7).
        LexiconEntry::build(
            "kyc_ubo.assert.edge.connect",
            "A link exists, from this block to that one, of this kind (§3.1) — \
             merges control + economic-interest (§3.2)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 1/2 (merged): subject must be registered; no active
            // edge with the same (from, to, kind) may already exist —
            // contradicting claims go through disconnect, never a second
            // assert (K-13). TS.5 R1: the type-geometry layer — TS.1 §1's
            // FIRST constraint, ahead of these positional studs.
            // 2026-09-07 (audit item 2, K-8): PiercedFromIsActiveNominee —
            // hoisted from the op-layer-only hand check (was
            // `UboEdgeConnect::execute`'s own `pierced-from` scan, never run
            // by the workbook path).
            vec![
                Precondition::SubjectRegistered,
                Precondition::NoDuplicateActiveEdge,
                Precondition::TypeGeometryPermits,
                Precondition::PiercedFromIsActiveNominee,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc_ubo.assert.edge.evidence",
            "Log one proof — kind, source, and date (EOP-DD-UBO-PROOF-001 \
             §1/§2) — against a control/economic edge, OR against an \
             entity's asserted type (TS.1 §3 row 4: \"this document/source \
             evidences a TYPE OR A LINKAGE — one verb, two possible \
             targets\"). No ratchet: the proof is added to the assertion's \
             citation set (§4, T5) — the board collects facts, the policy \
             rules on adequacy (§3).",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 3 (edge-scoped: target edge must exist and not be
            // superseded) UNION TS.1 §3 row 4 (entity-scoped: a type must
            // already be asserted and the member must not be withdrawn).
            // Each pair is vacuous for the OTHER call shape (see
            // `check_preconditions`'s `EdgeExists`/`EdgeActive`/
            // `MembershipActive`/`PriorTypeAsserted` arms) — a single event
            // is always exactly one shape, never both, so exactly one pair
            // ever actually evaluates.
            vec![
                Precondition::EdgeExists,
                Precondition::EdgeActive,
                Precondition::PriorTypeAsserted,
                Precondition::MembershipActive,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        // `kyc_ubo.assert.edge.verification` RETIRED (EOP-DD-UBO-PROOF-001
        // §3/§4, T5, 2026-08-28) — "the board collects facts; the policy
        // rules on adequacy." K-G7: 0 real committed events under this FQN.
        LexiconEntry::build(
            "kyc_ubo.assert.edge.retract",
            "Withdraw one previously logged proof by its citation id (§3.1's \
             move table: \"that proof no longer stands — group, citation\"). \
             The citation set shrinks; nothing else changes — there is no \
             status to demote (EOP-DD-UBO-PROOF-001 §4, T5).",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        // EOP-VS-UBO-GAME-001 T3 (§3.2, 2026-08-27): `supersession` renamed
        // `disconnect` — "that link is not on the board" (§3.1). Pure
        // rename, identical shape and preconditions.
        LexiconEntry::build(
            "kyc_ubo.assert.edge.disconnect",
            "That link is not on the board (K-13 supersede-never-delete — edge stays, status flips)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 4: target edge must exist and not already be
            // superseded (double-disconnect would no-op-pollute the stream).
            vec![Precondition::EdgeExists, Precondition::EdgeActive],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        // "kyc_ubo.assert.edge.nominee-piercing" LexiconEntry RETIRED (TS.6 P2, K-G7,
        // 2026-08-22): the verb it covered no longer exists — piercing is
        // now the `kyc_ubo.assert.edge.nominee-piercing` MACRO (config/verb_schemas/
        // macros/ubo.yaml) composing `kyc_ubo.assert.edge.connect` (its own
        // entry above already carries the equivalent precondition set —
        // SubjectRegistered, NoDuplicateActiveEdge, TypeGeometryPermits —
        // plus a new op-layer-only "referenced edge is EdgeKind::Nominee
        // and active" check when its `pierced-from` arg is present, same
        // no-precondition-primitive discipline the retired verb used) +
        // `kyc_ubo.assert.edge.disconnect` (its entry above: EdgeExists, EdgeActive).
        // The decomposition is exact — no precondition was lost or gained.
        //
        // `kyc_ubo.assert.edge.reconciliation` LexiconEntry RETIRED
        // (EOP-VS-UBO-GAME-001 T3, §3.3, 2026-08-27, K-G7 full deletion):
        // "No reconcile ... a third path to what two moves already do."
        // No entry survives it here — see fold/control.rs's former
        // reconciliation fold arm (deleted) for the full reasoning.
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
            "kyc_ubo.decide.determination.freeze",
            "Pin an immutable determination; emits PersonObligation for each resolved person",
            Taxonomy::Control,
            smallvec![FoldId::Determination, FoldId::ObligationGraph],
            // T6.1(c) exemplar (matrix row 8a): StructureClassSupported was
            // freeze's sole structure-class gate (TS.6 P2 removed the
            // separate StrategySelected stud it used to pair with — the
            // strategy IS derived from this same guarded structure_class,
            // so the two studs had become one fact checked twice).
            // EOP-VS-UBO-GAME-001 T3 (§3.3, 2026-08-27): `ReconciledProjection`
            // removed from this pair — the K-14 gate it enforced is
            // dissolved with `reconciliation` itself.
            // EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28): StructureClassSupported
            // itself superseded by EntityTypeSupportsStrategy — the subject's
            // `EntityType` (§2's ratified 21-type mapping) is the dispatch
            // key now, not `structure_class`; see `dispatch_for_entity_type`.
            // 2026-09-07 (audit item 2, P2): NoUnpiercedNomineeEdges hoisted
            // from the op-layer-only hand check (TS.4 §3 Ruling B, K-8).
            vec![
                Precondition::EntityTypeSupportsStrategy,
                Precondition::NoUnpiercedNomineeEdges,
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
        // D2.0 §5 (K-G7, RATIFIED 2026-08-22): `kyc_ubo.assert.obligation.creation`
        // LexiconEntry DISSOLVED. "Nobody hands over an obligation; you run
        // the checks and they fail. The finding is the record." Its
        // precondition (SubjectRegistered) carried no independent
        // information a check couldn't re-derive from the board directly.
        // Reintroduction path: a new verb, in Assembly, with a precondition
        // attached from birth — never bare again (K-G7 discipline).
        LexiconEntry::build(
            "kyc_ubo.assert.entity.identity",
            "Advance the identity verification track of an obligation",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 12 (⊗): target obligation exists. `SubjectNotDecided`
            // retired TS.6 P2 — decisions no longer reach the fact stream at
            // all (see `kyc.person.approve`/`.reject`'s TS.6 note below), so
            // the substrate has nothing left to check it against.
            // `ObligationExists` REMOVED (EOP-DD-UBO-CLEANOUT-001 T6 P2,
            // 2026-09-07) — was already dead at the real write path
            // (`validate_entry_fqn: None`, `kyc_stream_ops.rs`).
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc_ubo.assert.entity.screening",
            "Advance the screening track of an obligation (K-26 — screening gates \
             approval, not determination)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 13 (⊗): same stud as row 12, reused.
            // `ObligationExists` REMOVED — see `identity` above.
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc_ubo.assert.entity.risk",
            "Advance the risk-assessment track of an obligation",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 14 (⊗): same stud as row 12, reused.
            // `ObligationExists` REMOVED — see `identity` above.
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        // D2.0 §5 (K-G7, RATIFIED 2026-08-22): `kyc_ubo.assert.obligation.satisfaction`
        // LexiconEntry DISSOLVED. "Nothing is satisfied; you assert the
        // missing fact and re-run. The new run supersedes the old."
        // Reintroduction path: none named — the run book replaces this
        // capability entirely, not a future verb.
        //
        // `kyc_ubo.assert.obligation.waiver` LexiconEntry DISSOLVED here too
        // (D2.0 §5) and MOVED to `evaluation_lexicon()` below as
        // `kyc_ubo.decide.obligation.waiver` — the deferral this entry used
        // to name ("cannot leave the fact stream until obligation
        // dissolution removes the track model it writes to") is now
        // resolved: obligation-track state (`TrackState::Waived` via this
        // FQN specifically) no longer exists to protect. The one genuine
        // act among the six obligation verbs: a human rules that a failing
        // check does not apply, with reason and authority, citing the run.
        // `kyc.person.approve`/`kyc.person.reject` retired from this
        // manifest TS.6 P2 (K-G7) — renamed `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject`
        // and moved to `evaluation_lexicon()` (`ob-poc-kyc-decide`, no
        // dependency on `ob-poc-kyc-seam`). Their K-23 "decision is final"
        // finality check (`Precondition::SubjectNotDecided`, retired
        // alongside them) moved with them onto `kyc_decision_records`
        // directly — the substrate's pure fold can no longer see decisions
        // at all, by construction, which is the whole point of the split.
        // ── D1 (EOP-DD-KYCUBO-TS.1 §3) — the four new moves ──────────────────
        // `kyc_ubo.assert.subject.type` RETIRED (T2, §3.2) — merged into
        // `place` above. Fold arm remains, historical replay only (R5).
        //
        // `kyc_ubo.assert.subject.type-correction` DISSOLVED (T2, §8 Q1,
        // 2026-08-27) — no entry survives it here. §8 Q1 RULED: correcting
        // a type is `remove` then `place`, two ordinary moves, never a
        // superseding placement or a computed cascade. UNLIKE `type`/
        // `member-withdrawal` above, this verb had 0 real committed events
        // (confirmed by DB query before deletion), so it does NOT keep a
        // historical-replay-only fold arm — it is a full K-G7 deletion:
        // verb, op, fold arm, precondition attachment, and the cascade
        // function (`edges_invalidated_by_correction`) all removed in the
        // same diff. Reintroduction path: none named — the two-move
        // pattern replaces this capability entirely, not a future verb.
        // `kyc_ubo.assert.subject.member-withdrawal` RETIRED (T2, §3.2) —
        // renamed `remove`, identical shape and preconditions. Fold arm
        // remains, historical replay only (R5).
        LexiconEntry::build(
            "kyc_ubo.assert.subject.remove",
            "Take a group member off the board — withdraws the placement, never the \
             entity (§2, absorbs member-withdrawal); the entity remains a group member \
             and can be placed again. Never refused (§3.4 R8, corrected 2026-08-27): any \
             active link touching the block is pruned as a fold-time side effect — nothing \
             propagates past those links, and a block left with no links is an ordinary, \
             legal board member",
            Taxonomy::Subject,
            smallvec![FoldId::TypeRegistry, FoldId::ControlGraph],
            vec![
                Precondition::EntityRegistered,
                Precondition::MembershipActive,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc_ubo.assert.subject.enquiry",
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
/// 3 verbs today: `kyc_ubo.decide.subject.approve`, `kyc_ubo.decide.subject.reject` (renamed from
/// `kyc.person.approve`/`.reject`, TS.6 P2, landed with the structural
/// crate split — `ob-poc-kyc-decide` has no dependency on
/// `ob-poc-kyc-seam`, so this pack has no write path to the fact stream by
/// construction), and `kyc_ubo.decide.obligation.waiver` (moved here from
/// `assembly_lexicon()`, D2.0 §5, 2026-08-22 — the obligation dissolution
/// that entry's own former note was waiting on has now landed).
///
/// All three entries declare `writes: []` and `preconditions: []` —
/// deliberately. None writes to any substrate fold (they write to
/// `kyc_decision_records` instead, entirely outside the pure in-memory
/// `ControlState`/`ObligationState` model this manifest's checker
/// enforces), and their real gating (K-23 `SubjectAllTerminal` for approve;
/// the finality check for all three; a run citation for the verdict, D2.0
/// §5) is a hand-rolled op-layer check against `kyc_decision_records` —
/// the substrate's pure `check_control_preconditions` checker has no DB
/// access and cannot express any of these as a `Precondition` variant.
pub fn evaluation_lexicon() -> LexiconManifest {
    use smallvec::smallvec;
    let entries = vec![
        LexiconEntry::build(
            "kyc_ubo.decide.subject.approve",
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
            "kyc_ubo.decide.subject.reject",
            "Reject a subject (K-23 — decision recorded in kyc_decision_records, \
             never the fact stream, never erased). Rejection deliberately allowed \
             at ANY stage (early rejection is a real compliance outcome).",
            Taxonomy::Obligation,
            smallvec![],
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc_ubo.decide.obligation.waiver",
            "A human rules that a failing check does not apply, with reason and \
             authority, citing the run (D2.0 §5, moved from the retired \
             assert-scoped obligation waiver verb on 2026-08-22). Writes only to \
             kyc_decision_records — never the fact stream.",
            Taxonomy::Obligation,
            smallvec![],
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
    ];

    LexiconManifest::new(entries)
}
