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
}

// ── Precondition ─────────────────────────────────────────────────────────────

/// Ratchet ordering and prior-state requirements checked *before* appending the
/// event.  Enforcement is in `check_preconditions()`; the fold never enforces
/// them (the fold just applies; the write path guards).
///
/// Two verbs with non-trivial preconditions (§3):
/// - `ubo.edge.verify`       → `EvidenceCited`
/// - `ubo.determination.freeze` → `ReconciledProjection` + `StrategySelected`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precondition {
    /// A prior `ubo.edge.attach-evidence` event must exist in the stream for
    /// the same `target.edge_id` (K-11).  Enforces: evidence before verify.
    EvidenceCited,
    /// A prior `ubo.edge.reconcile-conflict` event must exist in the stream
    /// (K-14).  Enforces: reconcile before fold.
    ReconciledProjection,
    /// A prior `ubo.determination.select-strategy` event must exist in the
    /// stream (K-4).  Enforces: strategy before fold.
    StrategySelected,

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
    /// The subject's obligation rollup must not already be
    /// `Approved`/`Rejected` (rows 12–18; K-23: decision is final).
    SubjectNotDecided,
    /// The subject's obligation rollup must be `AllTerminal`
    /// (row 17 — the K-23 approval gate).
    SubjectAllTerminal,
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

// ── Phase-1/2 verb entries ────────────────────────────────────────────────────

/// Build the canonical `LexiconManifest` for all 21 dsl.kyc verbs: 13
/// Phase-1/2 determination verbs (incl. `ubo.edge.pierce-nominee`, TS.4 =
/// K-8, preconditions from birth per the K-G7 reintroduction discipline)
/// plus 8 W5 obligation/person verbs (T6.0 closure, 2026-08-12). This is
/// the normative lexicon for the vertical slice (V&S Appendix A, phases
/// 1–2) plus the W5 obligation lifecycle.
pub fn phase1_lexicon() -> LexiconManifest {
    use smallvec::smallvec;

    let entries = vec![
        // ── Phase 1 — substrate verbs ────────────────────────────────────────
        LexiconEntry::build(
            "kyc.subject.register",
            "Bring a subject into KYC scope, recording the basis for obligation",
            Taxonomy::Subject,
            smallvec![FoldId::ObligationGraph],
            // T6.3 row 9 HALTED (2026-08-12), not shipped: the ratified
            // matrix's `NotAlreadyRegistered` (`!state.registered`, a bare
            // stream-level boolean) is incompatible with `register`'s real
            // production usage — `kyc_stream_ops.rs`'s own
            // `UboDeterminationFreeze` resolves natural-person candidates
            // via `natural_persons_from_events`, which scans for MULTIPLE
            // `kyc.subject.register` events sharing one subject_root,
            // differentiated by payload `entity_id` (one call registers the
            // subject entity itself, one more per natural-person
            // candidate — see `kyc_m3_remediation.rs`'s
            // `m3_1_freeze_differential_matches_ownership_prong_strategy` /
            // `m4_control_prong_strategy_resolves_gp_statutory_control`,
            // both real production-shaped fixtures, not test-only
            // workarounds). Attaching this stud as ratified would block
            // every multi-person determination. Left geometry-free pending
            // a matrix amendment (row 9 needs a KEYED check — "this
            // (subject_root, entity_id) pair must not already be
            // registered" — not the niladic boolean the matrix's own §2
            // inventory claims suffices; out of this tranche's scope).
            vec![],
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
            // go through supersede, never a second assert (K-13).
            vec![
                Precondition::SubjectRegistered,
                Precondition::NoDuplicateActiveEdge,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.assert-economic-interest",
            "Claim an economic-interest edge (shareholding percentage)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            // T6.2 row 2: same two studs as row 1, reused.
            vec![
                Precondition::SubjectRegistered,
                Precondition::NoDuplicateActiveEdge,
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
        LexiconEntry::build(
            // TS.4 = K-8 (EOP-DD-KYCUBO-KIT-TS0 §2.6): supersede the target
            // nominee edge + assert the disclosed nominator's underlying
            // edge in ONE governed event. Preconditions FROM BIRTH (the
            // K-G7 reintroduction discipline): subject registered; target
            // edge exists and is active (matrix rows 3/4 vocabulary). The
            // "target is actually EdgeKind::Nominee" check has no
            // precondition primitive — enforced op-layer, fail-closed
            // (kyc_stream_ops.rs::UboEdgePierceNominee).
            "ubo.edge.pierce-nominee",
            "Pierce a nominee arrangement: supersede the nominee edge and assert the \
             disclosed nominator's underlying edge (K-8, K-13)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![
                Precondition::SubjectRegistered,
                Precondition::EdgeExists,
                Precondition::EdgeActive,
            ],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
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
        LexiconEntry::build(
            "ubo.determination.select-strategy",
            "Choose the determination strategy keyed on the subject structure class (K-4)",
            Taxonomy::Control,
            smallvec![FoldId::Determination],
            // T6.3 row 6 (remainder of the T6.1(c) 6a exemplar): subject
            // registered -> structure classified -> structure class
            // supported, in that documented order. StructureClassSupported
            // (6a) is the fail-closed guard converting a silently-wrong
            // determination for the 6 unimplemented classes into an error;
            // SubjectRegistered/StructureClassified are the ordering studs
            // added in T6.3.
            vec![
                Precondition::SubjectRegistered,
                Precondition::StructureClassified,
                Precondition::StructureClassSupported,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.determination.compute-fold",
            "Fold the reconciled control graph to UBO candidates with basis/prong (K-1)",
            Taxonomy::Control,
            smallvec![FoldId::Determination],
            vec![
                Precondition::ReconciledProjection,
                Precondition::StrategySelected,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.determination.apply-smo-fallback",
            "Record SMO where ownership+control resolution is empty (K-5, never silent)",
            Taxonomy::Control,
            smallvec![FoldId::Determination],
            // T6.3 row 7: reuses the two EXISTING variants that gate
            // compute-fold/freeze — zero new machinery. Prevents SMO
            // fallback firing before the determination stage is set up.
            vec![
                Precondition::ReconciledProjection,
                Precondition::StrategySelected,
            ],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            // The pivot verb: pins determination AND emits person obligations.
            "ubo.determination.freeze",
            "Pin an immutable determination; emits PersonObligation for each resolved person",
            Taxonomy::Control,
            smallvec![FoldId::Determination, FoldId::ObligationGraph],
            // T6.1(c) exemplar (matrix row 8a): StructureClassSupported ADDED
            // to freeze's existing two — defense in depth at the terminal
            // verb; the guard holds even if select-strategy is bypassed.
            vec![
                Precondition::ReconciledProjection,
                Precondition::StrategySelected,
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
            "kyc.obligation.update-identity",
            "Advance the identity verification track of an obligation",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 12 (⊗): target obligation exists; subject not
            // already decided (K-23: decision is final).
            vec![
                Precondition::ObligationExists,
                Precondition::SubjectNotDecided,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.obligation.update-screening",
            "Advance the screening track of an obligation (K-26 — screening gates \
             approval, not determination)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 13 (⊗): same two studs as row 12, reused.
            vec![
                Precondition::ObligationExists,
                Precondition::SubjectNotDecided,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.obligation.update-risk",
            "Advance the risk-assessment track of an obligation",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 14 (⊗): same two studs as row 12, reused.
            vec![
                Precondition::ObligationExists,
                Precondition::SubjectNotDecided,
            ],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.obligation.satisfy",
            "Mark all tracks on an obligation as satisfied (full KYC clearance)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 15 (⊗): same two studs as row 12, reused. Prevents
            // satisfying dead/decided work.
            vec![
                Precondition::ObligationExists,
                Precondition::SubjectNotDecided,
            ],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.obligation.waive",
            "Waive an obligation (all tracks set to Waived with a recorded reason)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 16 (⊗): same two studs as row 12, reused. Waive's
            // extra sensitivity is an AUTHORITY question (AuthoritySpec),
            // a separate ruling — not a stud, per the ratified matrix.
            vec![
                Precondition::ObligationExists,
                Precondition::SubjectNotDecided,
            ],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.person.approve",
            "Approve a subject once all obligations are terminal (K-23 gate)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 17 (⊗ — the highest-value row in this tranche): all
            // required obligation tracks terminal (THE K-23 GATE, closes
            // the DD-003 finding that approve was ungated at the checker
            // layer) + subject not already decided.
            vec![
                Precondition::SubjectAllTerminal,
                Precondition::SubjectNotDecided,
            ],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.person.reject",
            "Reject a subject (K-23 — decision recorded on the stream, never erased)",
            Taxonomy::Obligation,
            smallvec![FoldId::ObligationGraph],
            // T6.4 row 18 (⊗): subject not already decided ONLY. Rejection
            // deliberately allowed at ANY stage (early rejection is a real
            // compliance outcome) — no SubjectAllTerminal stud here, per
            // the ratified matrix.
            vec![Precondition::SubjectNotDecided],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
    ];

    LexiconManifest::new(entries)
}
