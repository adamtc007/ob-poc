//! The super-user IDE session — `EOP-PLAN-KYCUBO-KIT-001` T4 (closes KIT-10/11).
//!
//! Normative spec: `docs/todo/EOP-DD-KYCUBO-KIT-T4_Super-User-IDE-Session-Design_v0.1.md`
//! (internal version 0.2, "Review edits applied"). This module composes T1
//! (`render_intent_event_to_sexpr`), T2 (`enumerate_placement_set`/`PlacementSet`)
//! and T3 (`preview`) — all in `ob-poc-kyc-substrate`, read-only here — plus the
//! real DSL parser (`dsl_parser::parse`) and the real governed append chokepoint
//! (`ob_poc_kyc_seam::append_in_scope`). It adds no new mechanism: recognition is
//! tier-0 deterministic parse-and-membership-test (§6), commit is the workbook
//! driving N ordinary governed verb appends in one transaction (§5).
//!
//! **Zero-inference, by construction.** No model/LLM/agent import anywhere in
//! this file — recognition fails on a parser diagnostic or on frontier
//! placement-set non-membership, never on a ranked "closest guess". See the
//! `zero_inference_assertion` import-allowlist gate in `tests/kyc_workbook.rs`.
//!
//! **Scope fence (design §7).** No entity-handle resolver: target/payload UUID
//! fields must be typed as UUID-literal strings; a bare symbol or qualified
//! name in a slot that isn't one of the five target-binding fields is rejected
//! with [`RecognitionError::UnsupportedValue`], not guessed at. No new
//! persistence: `KycWorkbook` is session-scoped, in-memory only.
//!
//! **Deltas from the design doc's pseudocode** (recorded per the doc's own
//! "record deltas, don't silently absorb" discipline):
//! - **No DB-backed kit loader.** The design's `load_current_lexicon_manifest`/
//!   `load_current_lexicon_manifest_hash` presumed a DB round-trip. Traced
//!   `ob-poc-kyc-store::manifest::publish_assembly_manifest`: it always publishes
//!   *from* `assembly_lexicon()` (code → DB, one-way, audit/provenance only) and
//!   `PgKycEventStore::append` never reads `kyc_lexicon_manifest` back for
//!   enforcement — so there is no scenario where the DB's manifest differs
//!   from the running binary's `assembly_lexicon()`. The kit pin/drift check
//!   (KIT-10) compares `assembly_lexicon().hash` in-process at open and at
//!   commit; zero lines land in `ob-poc-kyc-store`/`ob-poc-kyc-seam` for this.
//! - **`WorkbookError` is local, not `StoreError::KitDrift`.** Keeps the
//!   kit-drift/recognition error surface entirely in this module rather than
//!   widening a lower crate's error enum for an app-layer concern.
//! - **`sexpr_to_intent_event_draft` returns a local `ParsedMove`, not
//!   `ob_poc_kyc_seam::IntentEventDraft`.** The seam's `IntentEventDraft`
//!   requires `authority`/`lexicon_hash` at construction (execution-identity
//!   stamping fields a parse-only mirror of `render_intent_event_to_sexpr`
//!   has no business inventing) and is designed for the execution-identity
//!   stamping flow, not a bare parse-to-shape step. `stage()` builds the real
//!   `IntentEvent` via `IntentEvent::new` directly once `ParsedMove` plus the
//!   caller-supplied principal/authority/as_of are all in hand.
//! - **T1 (`EOP-VS-UBO-GAME-001` §3.4 R6, this tranche):** `sexpr_to_parsed_move`
//!   used to decide target-vs-payload itself, by a fixed rule (five
//!   hyphenated slot names always go to the target, everything else to the
//!   payload) — with zero per-verb knowledge, so a verb whose fold reads a
//!   target-slot-named field back out of the *payload* (`register`'s
//!   `entity-id`, among 12 others) silently never worked through this
//!   surface. That branch is deleted; every slot now flows into one flat
//!   args map, and `ob_poc_kyc_seam::canonical_event_shape` — the SAME
//!   function the op layer calls — decides target vs payload. Two
//!   surfaces, one decision.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use dsl_parser::{RawAtom, RawValue};
use dsl_runtime::TransactionScope;

use ob_poc_kyc_seam::{append_in_scope, canonical_event_shape, map_principal};
use ob_poc_kyc_store::{AppendOutcome, PgKycEventStore, StoreError};
use ob_poc_kyc_substrate::{
    check_preconditions, entity_type_from_wire, enumerate_placement_set, assembly_lexicon, preview,
    render_intent_event_to_sexpr, AuthorityRef, ControlState, FoldRegistry,
    IntentEvent, KycError, LexiconManifest, MoveId, ObligationState,
    SubjectId, TargetBinding, TypeRegistryState, VerbFqn,
};

/// Mirrors `placement.rs`'s own private `PLACE` const — `place` is the one
/// verb `stage()` matches differently (see that method's doc comment).
const PLACE: &str = "kyc_ubo.assert.subject.place";
use sem_os_core::principal::Principal as RuntimePrincipal;

// ── Errors ───────────────────────────────────────────────────────────────────

/// Tier-0 recognition failure — a parser diagnostic or a frontier
/// non-membership, never a ranked guess (§6/§8 zero-inference).
#[derive(Debug, thiserror::Error)]
pub enum RecognitionError {
    #[error("DSL parse failed for {text:?}: {diagnostics}")]
    Parse { text: String, diagnostics: String },
    #[error("expected exactly one top-level DSL statement, got {0}")]
    NotSingleAtom(usize),
    #[error("unsupported slot value: {0}")]
    UnsupportedValue(String),
    #[error("slot {slot:?} value {value:?} is not a valid UUID literal")]
    InvalidUuid { slot: String, value: String },
    #[error("typed subject {typed} does not match the open workbook's subject {workbook}")]
    SubjectMismatch { typed: Uuid, workbook: Uuid },
    #[error(
        "{verb_fqn} is not currently legal against the frontier state; \
         currently legal moves: {legal:?}"
    )]
    NotCurrentlyLegal { verb_fqn: String, legal: Vec<String> },
}

/// Workbook-level failure — recognition, validation, kit drift, or a real
/// store/append error. Local to this module (see the module doc's delta
/// note): the KYC substrate/store/seam crates are unmodified by T4.
#[derive(Debug, thiserror::Error)]
pub enum WorkbookError {
    #[error(
        "kit drift: this workbook pinned lexicon {pinned}, the running binary's kit is now \
         {live} — re-open the workbook to pick up the current kit (KIT-10: never a silent \
         substitution)"
    )]
    KitDrift { pinned: String, live: String },
    #[error(transparent)]
    Recognition(#[from] RecognitionError),
    #[error(transparent)]
    Kyc(#[from] KycError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

// ── Types (design §4) ────────────────────────────────────────────────────────

/// One recognised, not-yet-committed candidate move — the output of tier-0
/// recognition against the frontier placement set, never a guess.
#[derive(Debug, Clone)]
pub struct StagedMove {
    pub event: IntentEvent,
    /// The canonical resolved re-render (T0.2/§6 step 4) — what gets
    /// committed IS what `render_intent_event_to_sexpr` produces from the
    /// fully-built event, not necessarily the verbatim text typed.
    pub source_text: String,
    /// Which frontier `PlacementSet` candidate this was recognised as.
    pub legal_move: MoveId,
}

/// The staged sequence for one subject, open against one loaded history,
/// validated against ONE pinned kit for the whole session (KIT-10).
#[derive(Debug)]
pub struct KycWorkbook {
    pub subject: SubjectId,
    /// Loaded at open; never mutated in-session.
    pub committed: Vec<IntentEvent>,
    /// Append-only until commit or discard.
    pub staged: Vec<StagedMove>,
    /// Captured at open — all in-session recognition and validation use this
    /// pin, never a per-call parameter (KIT-10: session = kit ⊕ snapshot).
    pub kit: LexiconManifest,
    pub kit_hash: String,
}

// ── Parse-only mirror of `render_intent_event_to_sexpr` (§6 step 2) ────────────

/// The shape recognition extracts from one parsed atom, before caller-supplied
/// identity (principal/authority/as_of) turns it into a real `IntentEvent`.
struct ParsedMove {
    verb_fqn: VerbFqn,
    target: TargetBinding,
    payload: serde_json::Value,
}

fn parse_uuid_slot(slot: &str, value: &RawValue) -> Result<Uuid, RecognitionError> {
    match value {
        RawValue::StringLit(s) => Uuid::parse_str(s).map_err(|_| RecognitionError::InvalidUuid {
            slot: slot.to_string(),
            value: s.clone(),
        }),
        other => Err(RecognitionError::UnsupportedValue(format!(
            "slot {slot:?}: expected a UUID string literal, got {other:?}"
        ))),
    }
}

/// Reduce one parsed payload value back to JSON. Mirrors `render_value` in
/// `render.rs`, reversed. Only the literal forms `render_value` ever
/// produces are accepted — `Symbol`/`QualifiedName`/`SlotRef`/template forms
/// are symbolic/pre-resolution handles, out of scope for T4 (design §7).
fn raw_value_to_json(value: &RawValue) -> Result<serde_json::Value, RecognitionError> {
    match value {
        RawValue::StringLit(s) => Ok(serde_json::Value::String(s.clone())),
        RawValue::IntLit(n) => Ok(serde_json::Value::from(*n)),
        RawValue::FloatLit(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .ok_or_else(|| RecognitionError::UnsupportedValue(format!("non-finite float: {f}"))),
        RawValue::BoolLit(b) => Ok(serde_json::Value::Bool(*b)),
        RawValue::List(items) => items
            .iter()
            .map(raw_value_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        RawValue::Map(entries) => entries
            .iter()
            .map(|(k, v)| raw_value_to_json(v).map(|jv| (k.clone(), jv)))
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(serde_json::Value::Object),
        other => Err(RecognitionError::UnsupportedValue(format!(
            "{other:?} is a symbolic/template form — T4 accepts UUID-literal DSL text only; \
             entity-handle resolution is out of scope for this tranche"
        ))),
    }
}

/// Map one parsed top-level atom to a `ParsedMove`.
///
/// T1 (R6): this function no longer decides target-vs-payload itself. Every
/// slot — including the five that used to be hard-routed to the target
/// (`subject-id`/`edge-id`/`entity-id`/`person-id`/`obligation-id`) — flows
/// into one flat args map, keyed exactly as typed. `subject-id`, if typed,
/// is checked against `workbook_subject` here (a workbook-specific
/// concern, not a shape concern) and then included in the args map like
/// everything else, so `canonical_event_shape` sees the same input the op
/// layer would build from identical DSL text. `canonical_event_shape` —
/// the SAME function the op layer calls via `stream_append` — decides
/// which declared arguments become the target and which become the
/// payload, and under what key names.
fn sexpr_to_parsed_move(
    atom: &RawAtom,
    workbook_subject: SubjectId,
) -> Result<ParsedMove, RecognitionError> {
    if let Some(name) = &atom.name {
        return Err(RecognitionError::UnsupportedValue(format!(
            "bare atom name {name:?} not expected on a governed verb call"
        )));
    }

    let mut args = serde_json::Map::new();
    for (slot, value) in &atom.slots {
        if slot == "subject-id" {
            let id = parse_uuid_slot(slot, value)?;
            if id != workbook_subject.0 {
                return Err(RecognitionError::SubjectMismatch {
                    typed: id,
                    workbook: workbook_subject.0,
                });
            }
        }
        args.insert(slot.clone(), raw_value_to_json(value)?);
    }
    // subject-id defaults to the workbook's own subject if the caller
    // omitted it — canonical_event_shape's `subject: SubjectId` parameter
    // carries this, not the args map, so no default-injection is needed
    // here; the SubjectMismatch check above already covers the only case
    // where an explicit value matters (it disagreeing with the workbook).

    let (target, payload, _minted_edge_id) =
        canonical_event_shape(&atom.kind, workbook_subject, &serde_json::Value::Object(args))
            .map_err(|e| RecognitionError::UnsupportedValue(format!("{}: {e}", atom.kind)))?;

    Ok(ParsedMove { verb_fqn: VerbFqn(atom.kind.clone()), target, payload })
}

// ── The session model (design §5) ───────────────────────────────────────────

/// Fold `committed ++ staged` over the pinned kit — the re-run-whole
/// reconstruction (KIT-4) shared by `validate()` and `stage()`'s frontier
/// computation. Pure; no store dependency. Returns all three folds
/// (T6.1(a) extended to the type-registry axis, TS.1 D1 tranche;
/// Phase 2 of the tree-cleanup follow-up tranche, EOP-STATE-KYCUBO-D1 §4,
/// promoted `MembershipActive`/`PriorTypeAsserted` into real
/// `Precondition`s, so `check_preconditions` now genuinely reads
/// `TypeRegistryState` — `preview()` folds and validates against it
/// directly, so this is a thin pass-through, not a second independent fold).
fn folded_state(
    committed: &[IntentEvent],
    staged: &[StagedMove],
    kit: &LexiconManifest,
) -> Result<(ControlState, ObligationState, TypeRegistryState), KycError> {
    let candidates: Vec<IntentEvent> = staged.iter().map(|m| m.event.clone()).collect();
    preview(committed, &candidates, kit)
}

/// Open a workbook: load the committed history and pin the current kit
/// in-process (see the module doc's delta note — no DB read for the kit).
/// A subject with zero committed events is the empty-baseplate path with no
/// special-casing: `folded_state` on an empty `committed`/`staged` pair is
/// `ControlState::default()` (T2/T3's own `empty_state()` fixtures already
/// exercise this).
pub async fn open_workbook(
    conn: &mut PgConnection,
    subject: SubjectId,
) -> Result<KycWorkbook, StoreError> {
    let committed = PgKycEventStore::load_events(conn, subject).await?;
    let kit = assembly_lexicon();
    let kit_hash = kit.hash.to_hex();
    Ok(KycWorkbook {
        subject,
        committed,
        staged: Vec::new(),
        kit,
        kit_hash,
    })
}

impl KycWorkbook {
    /// The Repl's job: re-run-whole over the staged workbook (T2∘T3
    /// composed). Called after every stage, not just before commit. Returns
    /// all three folds (T6.1(a), extended to the type-registry axis).
    pub fn validate(&self) -> Result<(ControlState, ObligationState, TypeRegistryState), KycError> {
        folded_state(&self.committed, &self.staged, &self.kit)
    }

    /// Recognise `text` against the **frontier** placement set — the state
    /// after every already-staged move, not committed-only state (the
    /// review-fixed bug: committed-only would wrongly refuse a dependent
    /// chain's 2nd/3rd move). Rejection carries the parser diagnostic
    /// verbatim, or the currently-legal move listing — never a guess.
    ///
    /// **`place` is matched differently from every other move** (2026-09-07,
    /// audit item 3). Every other verb's frontier candidate names a
    /// concrete `TargetBinding` the board already knows about (an existing
    /// edge, an existing registered entity) — `parsed.target` either equals
    /// one of those or it doesn't. `place` candidates are type-level
    /// (`LegalMove::proposed_entity_type` — see `placement.rs`): the board
    /// offers "this TYPE may be added", never a pre-enumerated entity id,
    /// because a brand-new entity has no id yet for the board to know
    /// about. So `place` matches on the TYPE the caller's text asserts,
    /// then — because the type-level frontier probe couldn't evaluate
    /// `NotCurrentlyPlaced` without a concrete id (vacuous when probed
    /// abstractly, `fold::control::check_preconditions`'s documented
    /// convention) — re-runs the real precondition oracle against the
    /// caller's ACTUAL id once it is known. Bounded to this one verb: no
    /// other move's matching changes.
    pub fn stage(
        &mut self,
        text: &str,
        principal: &RuntimePrincipal,
        authority: AuthorityRef,
        as_of: DateTime<Utc>,
    ) -> Result<&StagedMove, WorkbookError> {
        let (source_file, diagnostics) = dsl_parser::parse(text);
        if diagnostics.has_errors() {
            return Err(RecognitionError::Parse {
                text: text.to_string(),
                diagnostics: format!("{diagnostics:?}"),
            }
            .into());
        }
        if source_file.atoms.len() != 1 {
            return Err(RecognitionError::NotSingleAtom(source_file.atoms.len()).into());
        }
        let parsed = sexpr_to_parsed_move(&source_file.atoms[0], self.subject)?;

        let (frontier, frontier_obligation, frontier_type_registry) =
            folded_state(&self.committed, &self.staged, &self.kit)?;
        let placement_set = enumerate_placement_set(
            self.subject,
            &frontier,
            &frontier_obligation,
            &frontier_type_registry,
            &self.kit,
        );

        let entry = self.kit.get(parsed.verb_fqn.as_str());
        let actor = map_principal(principal);
        let event = IntentEvent::new(
            self.subject,
            parsed.verb_fqn.clone(),
            actor,
            authority,
            parsed.target.clone(),
            parsed.payload.clone(),
            as_of,
        )
        .with_lexicon_hash(self.kit.hash);

        let legal_move = if parsed.verb_fqn.as_str() == PLACE {
            let entity_type = event
                .payload
                .get("entity_type")
                .and_then(|v| v.as_str())
                .and_then(entity_type_from_wire);
            let matched = entity_type.and_then(|et| {
                placement_set
                    .moves
                    .iter()
                    .find(|m| m.verb_fqn == parsed.verb_fqn && m.proposed_entity_type == Some(et))
            });
            let move_id = matched
                .map(|m| m.move_id.clone())
                .ok_or_else(|| RecognitionError::NotCurrentlyLegal {
                    verb_fqn: parsed.verb_fqn.as_str().to_string(),
                    legal: placement_set
                        .moves
                        .iter()
                        .map(|m| m.move_id.0.clone())
                        .collect(),
                })?;
            // The frontier confirmed the TYPE is legal; now check the REAL
            // entity id isn't already placed (`NotCurrentlyPlaced` — vacuous
            // at the type-level probe above, real here against the actual id).
            if let Some(e) = entry {
                check_preconditions(e, &frontier, &frontier_obligation, &frontier_type_registry, &event)?;
            }
            move_id
        } else {
            placement_set
                .moves
                .iter()
                .find(|m| m.verb_fqn == parsed.verb_fqn && m.target == parsed.target)
                .map(|m| m.move_id.clone())
                .ok_or_else(|| RecognitionError::NotCurrentlyLegal {
                    verb_fqn: parsed.verb_fqn.as_str().to_string(),
                    legal: placement_set
                        .moves
                        .iter()
                        .map(|m| m.move_id.0.clone())
                        .collect(),
                })?
        };

        let source_text = render_intent_event_to_sexpr(&event, entry);

        self.staged.push(StagedMove {
            event,
            source_text,
            legal_move,
        });
        Ok(self.staged.last().expect("just pushed"))
    }

    /// Replay through the real governed append, verb by verb, in one
    /// transaction — not a bespoke append mechanism (design §5). Consumes
    /// the workbook: nothing partial escapes a rejected commit, and there is
    /// no reuse-after-commit/reuse-after-reject footgun.
    pub async fn commit(
        self,
        scope: &mut dyn TransactionScope,
        registry: &FoldRegistry,
    ) -> Result<Vec<AppendOutcome>, WorkbookError> {
        // KIT-10 kit-drift check, in-process (see module doc delta note).
        let live_hash = assembly_lexicon().hash.to_hex();
        if live_hash != self.kit_hash {
            return Err(WorkbookError::KitDrift {
                pinned: self.kit_hash.clone(),
                live: live_hash,
            });
        }

        // Whole-chain fail-fast against the scope's own connection: the
        // workbook may have gone stale since the last `validate()` call
        // (another session wrote to this subject meanwhile). Not redundant
        // with `append_in_scope`'s own per-event re-fold (K-14) — this turns
        // "commit N legal moves then fail on N+1 mid-transaction" into one
        // clean upfront error.
        let fresh_committed = PgKycEventStore::load_events(scope.executor(), self.subject).await?;
        let candidates: Vec<IntentEvent> = self.staged.iter().map(|m| m.event.clone()).collect();
        preview(&fresh_committed, &candidates, &self.kit)?;

        let kit = &self.kit;
        let mut outcomes = Vec::with_capacity(self.staged.len());
        for staged in &self.staged {
            let outcome = append_in_scope(
                scope,
                registry,
                &staged.event,
                &staged.source_text,
                |control: &ControlState, obligation: &ObligationState, type_registry: &TypeRegistryState| {
                    let entry = kit.get(staged.event.verb_fqn.as_str()).ok_or_else(|| {
                        KycError::UnknownVerb(staged.event.verb_fqn.clone())
                    })?;
                    check_preconditions(entry, control, obligation, type_registry, &staged.event)
                },
            )
            .await?;
            outcomes.push(outcome);
        }
        Ok(outcomes)
    }
}
