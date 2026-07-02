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

/// Build the canonical `LexiconManifest` for the ~10 Phase-1/2 verbs.
/// This is the normative lexicon for the vertical slice (V&S Appendix A,
/// phases 1–2).
pub fn phase1_lexicon() -> LexiconManifest {
    use smallvec::smallvec;

    let entries = vec![
        // ── Phase 1 — substrate verbs ────────────────────────────────────────
        LexiconEntry::build(
            "kyc.subject.register",
            "Bring a subject into KYC scope, recording the basis for obligation",
            Taxonomy::Subject,
            smallvec![FoldId::ObligationGraph],
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "kyc.subject.classify-structure",
            "Set structure class, driving both determination strategy and obligation set",
            Taxonomy::Subject,
            smallvec![FoldId::ControlGraph, FoldId::ObligationGraph],
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.assert-control",
            "Claim a control edge (voting, board, GP statutory, etc.)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.assert-economic-interest",
            "Claim an economic-interest edge (shareholding percentage)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![],
            AuthoritySpec::analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.attach-evidence",
            "Cite documentary proof for a control or economic edge",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![],
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
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            "ubo.edge.reconcile-conflict",
            "Canonicalise conflicting source edges before determination (K-14)",
            Taxonomy::Control,
            smallvec![FoldId::ControlGraph],
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        // ── Phase 2 — determination verbs ────────────────────────────────────
        LexiconEntry::build(
            "ubo.determination.select-strategy",
            "Choose the determination strategy keyed on the subject structure class (K-4)",
            Taxonomy::Control,
            smallvec![FoldId::Determination],
            vec![],
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
            vec![],
            AuthoritySpec::senior_analyst(),
            vec![],
        ),
        LexiconEntry::build(
            // The pivot verb: pins determination AND emits person obligations.
            "ubo.determination.freeze",
            "Pin an immutable determination; emits PersonObligation for each resolved person",
            Taxonomy::Control,
            smallvec![FoldId::Determination, FoldId::ObligationGraph],
            vec![
                Precondition::ReconciledProjection,
                Precondition::StrategySelected,
            ],
            AuthoritySpec::senior_analyst(),
            vec![EmitSpec::person_obligation(), EmitSpec::entity_obligation()],
        ),
    ];

    LexiconManifest::new(entries)
}
