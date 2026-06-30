# KYC/UBO — W1 Substrate & Vertical-Slice Design
### The in-memory build target (no schema)

| | |
|---|---|
| **Document** | EOP-DD-KYCUBO-001 |
| **Type** | Design / build hand-off (in-memory slice; **no schema, no migration, no durable persistence**) |
| **Version** | 0.2 — Post-build reconciliation |
| **Owner** | Adam Cearns |
| **Date** | 2026-06-30 |
| **Binds to** | EOP-VS-KYCUBO-001 v0.6 (V&S, binding); EOP-RP-KYCUBO-001 v0.1 §7–§8 (slice + W1); Addendum A (verification, hardening, ratified decisions). |
| **Status** | **BUILT AND GREEN.** 15/15 slice tests pass. Three corrections from v0.1 are inlined below; see §10 for the full delta log. |

> **What this slice proves — and what it does NOT (H2, read first).** A green slice proves the **semantic** model: verb-event → fold → determination → obligation, with bit-identical replay, on **one structure class (private company), in memory.** It proves **nothing** about durable ordering, concurrent appends, replay against a live partitioned DB, or projection consistency under at-least-once delivery. Those are W1-proper, after the slice. Do not read a green slice as "architecture proven." It is "semantics proven."

> **v0.2 reconciliation note.** Three things in v0.1 diverged from the implementation and are corrected here. They are marked **[v0.2]** at their first occurrence:
> 1. **Crate name.** The slice lives at `rust/crates/ob-poc-kyc-substrate/` (§0, §6).
> 2. **`&'static str` → `String`.** `LexiconEntry.intent`, `AuthoritySpec.required_role`, and `EmitSpec.kind` are owned `String`s, not `&'static str`, so the types derive `Deserialize` cleanly (§3).
> 3. **CI guards.** Two build-time enforcement mechanisms are in place: `#![deny(unreachable_pub)]` in `lib.rs` and `scripts/check_kyc_substrate_deps.sh` (proven red/green, §6a).
>
> A subsequent **tree-reconciliation pass (R1–R8)** then folded in the crate-discipline statement (§0a), the realised structural invariants (§7a), the dep-gate→§12.3 framing (§6a), and the forward-reference to DD-002/D2 re-opening this crate (§0) — applied *before* the W1-proper build so the paper is accurate at the moment D2 (build-order step 1) re-opens the crate. See the "0.2 (build landed)" changelog row.

---

## 0. Build provenance [v0.2]

The slice is implemented as a standalone Rust crate with no DB, no sqlx, no `sem_os_core` (git dep):

```
rust/crates/ob-poc-kyc-substrate/
├── Cargo.toml          # pure deps: anyhow, chrono, serde, serde_json, sha2, hex,
│                       # thiserror, uuid (v4+v5), smallvec
├── src/
│   ├── lib.rs          # #![deny(unreachable_pub)]; public re-exports only
│   ├── types.rs        # SubjectId, EdgeId, PersonId, EntityId, ObligationId,
│   │                   # EventId, VerbFqn, Hash, Principal, AuthorityRef,
│   │                   # TargetBinding, IdemKey
│   ├── event.rs        # IntentEvent (7-arg constructor + builder), CapturedEffect,
│   │                   # KycEventStore trait, InMemoryEventStore
│   ├── lexicon.rs      # LexiconEntry, LexiconManifest, Taxonomy, FoldId,
│   │                   # Precondition, AuthoritySpec, EmitSpec, phase1_lexicon()
│   ├── error.rs        # KycError
│   ├── fold/
│   │   ├── control.rs  # fold_control(), check_control_preconditions(),
│   │   │               # reconciled_economic_edges(), natural_persons_from_events()
│   │   └── obligation.rs # fold_obligations()
│   └── determination.rs  # DeterminationStrategy, OwnershipProngStrategy,
│                          # DeterminationPin, FrozenDetermination, RecoveryPin,
│                          # freeze_determination(), recover_determination_at()
└── tests/
    └── kyc_slice.rs    # 15 tests: 7 exit criteria + K-13 + K-30 + Q7 +
                        # 2 Phase-1 determinism-stress + 2 ec3 variants
```

> **[v0.2 — forward reference, R6] D2 re-opens this crate.** DD-002 (W1-proper) ratified D2 = *version-dispatch the fold now*. Its build-order **step 1** adds a content-addressed `FoldRegistry` (keyed by `lexicon_hash`) **inside `ob-poc-kyc-substrate`** — `fold_control`/`fold_obligations` gain the registry and dispatch per event. The crate stays pure (zero sqlx; dep-gate unaffected). This inventory will gain `fold/registry.rs` (or equivalent) when that lands; flagged here so the "done" crate's contents stay accurate at the moment the build re-opens it.

**Workspace:** added to `rust/Cargo.toml` members list under `crates/ob-poc-authoring`.

---

## 0a. Crate discipline [v0.2]

The slice is the **engine** in a three-tier inward-dependency stack. The durable I/O lives in a separate **membrane** crate (`ob-poc-kyc-store`, authored in DD-002), never in the engine:

```
ob-poc-types          (leaf — IntentEvent shape, Principal*, AuthorityRef, taxonomy types)
      ▲
ob-poc-kyc-substrate  (ENGINE — pure: folds, determination, the KycEventStore TRAIT) ← zero sqlx
      ▲
ob-poc-kyc-store      (MEMBRANE — impl KycEventStore over Postgres; DD-002; the ONLY sqlx crate)
      ▲
sem_os_postgres / dsl-runtime   (seam — append at the TransactionScope choke point)
```

Five rules govern the stack:

1. **The engine has zero I/O deps.** No `sqlx`, no DB, no async-runtime coupling. Enforced by the §6a dep-gate, not by convention.
2. **The store trait is defined in the engine, implemented in the membrane.** `KycEventStore` is the engine's contract; `InMemoryEventStore` satisfies it for the slice, and `ob-poc-kyc-store` (DD-002) satisfies it over Postgres behind the same trait — no caller changes.
3. **Demotion is extraction, not reference.** The percentage chain (V&S §12.2) is re-implemented as `OwnershipProngStrategy` *inside* the engine, pure, fed reconciled edges — not called out to `ubo_compute.rs`. (Done in the slice.)
4. **The `Principal` coupling resolves into `ob-poc-types`.** *Pending — W1-proper.* The slice currently carries a **local** `Principal` in `types.rs`; W1-proper hoists it to `ob-poc-types` so engine, membrane, and seam share one definition (the `*` on the sketch marks this). Until then the engine's `Principal` is its own type.
5. **Restricted public surface from birth.** `#![deny(unreachable_pub)]` + an explicit re-export allowlist in `lib.rs`; no `pub use *`. (Done — §6a.)

---

## 1. Ratified decisions (the fixed constraints)

These are settled (Addendum A §3.1) and are the load-bearing constraints of the design below.

- **Q4 — Person KYC** = **parallel per-obligation tracks → approval gate.** Person overall-state is a *fold* over obligation-track states; approval gates on all-required-terminal. (Drives the obligation fold, §4.2.)
- **Q5 — Node status** = **terminal natural-person status is verb-set; intermediate-entity resolution is a derived, checkpointed fold.** (Drives the control fold, §4.1.)
- **Q6 — Ordering & replay** = **per-subject ordering domain** (not global); replay pins *policy + lexicon-manifest + reference-snapshot + import-runs + graph-hash + frozen `as_of`*; **time is an input, never a wall-clock read inside a verb**; **external lookups are captured as evidence events at first execution and replayed from capture, never re-called.** (Drives the event contract §2 and freeze §5.)
- **Q7 — Lexicon evolution** = **content-addressed verbs + a whole-lexicon manifest hash;** semantic change = new hash = new verb identity; stream migration is a governed **re-fold**, never in-place reinterpretation. (Drives the lexicon entry §3 and freeze pin §5.)

---

## 2. The verb-event contract

The intent-native record. In the slice it is an in-memory append-only log keyed by `subject_root`; in W1-proper the identical shape becomes the durable `kyc_intent_events`. **It is the only writer of authoritative state in the slice's scope.**

```rust
/// Append-only, intent-native. Ordering domain = per `subject_root` (Q6), NOT global.
/// `state = fold(events_for(subject_root))`. Replay is re-running the fold.
pub struct IntentEvent {
    pub id: EventId,                       // UUID v4 — PK in the durable table
    pub seq: u64,                          // dense, per subject_root — ordering domain (Q6)
    pub subject_root: SubjectId,           // the determination root this stream belongs to
    pub verb_fqn: VerbFqn,
    pub lexicon_hash: Hash,                // exact verb definition this event was written against (Q7)
    pub actor: Principal,                  // from VerbExecutionContext.principal
    pub authority: AuthorityRef,           // object-capability that licensed this move (K-17, K-35)
    pub target: TargetBinding,             // the edge / node / obligation acted on
    pub payload: serde_json::Value,
    pub payload_hash: Hash,
    pub idempotency_key: IdemKey,
    pub causation_id: Option<EventId>,     // the event that caused this one
    pub correlation_id: Uuid,             // from VerbExecutionContext.correlation_id
    pub as_of: DateTime<Utc>,             // FROZEN clock — an input, never now() inside a verb (Q6)
    pub captured_effects: Vec<CapturedEffect>, // external-lookup results captured for replay (Q6)
}
```

**Constructor note.** `IntentEvent::new` takes the 7 required fields (`subject_root`, `verb_fqn`, `actor`, `authority`, `target`, `payload`, `as_of`) and defaults `seq=0`, `idempotency_key=random`, `lexicon_hash=default`. Use builder methods `.with_seq()`, `.with_lexicon_hash()`, `.with_idempotency_key()` to override. The store overwrites `seq` on append; the builder methods exist for tests and for explicit seam wiring.

Three properties the type enforces by construction:

1. **No state without semantic cause (K-35).** Every fold transition is keyed off an `IntentEvent`; the fold has no other input. There is no path to state that does not pass through an event carrying `actor` + `authority` + `target`.
2. **Deterministic replay (K-16, K-18, K-33).** `as_of` is frozen into the event; `captured_effects` holds any external-lookup result. Re-running the fold over the same events yields bit-identical state. **No verb calls `now()` or an external service during replay.**
3. **Effects dispatch once (H5).** On *first* apply, effects are emitted via the existing `outbox` (idempotency_key, effect_kind). **Replay folds state only; it never re-dispatches.** This is the line that keeps `document.solicit`/screening from re-firing on recovery.

---

## 3. The lexicon-entry contract (§8.1 shape)

Every Phase-1 verb declares its binding. A verb that does not declare governing taxonomy + writes-fold + authority is rejected at load (K-30 lint, gap-report Test 9).

```rust
pub struct LexiconEntry {
    pub fqn: VerbFqn,
    pub intent: String,               // [v0.2] owned String, not &'static str — needed for Deserialize
    pub governing_taxonomy: Taxonomy, // Subject | Control | Obligation  (K-30)
    pub writes: SmallVec<[FoldId; 2]>,// which fold(s) it mutates        (K-30, K-32)
    pub preconditions: Vec<Precondition>, // e.g. EvidenceCited for `verify`  (K-11)
    pub authority: AuthoritySpec,         // object-capability              (K-17)
    pub emits: Vec<EmitSpec>,             // e.g. `freeze` emits obligations
    pub hash: Hash,                       // content address of this entry   (Q7)
}

pub struct AuthoritySpec {
    pub required_role: String,   // [v0.2] owned String (was &'static str)
    pub interactive_only: bool,
}

pub struct EmitSpec {
    pub kind: String,            // [v0.2] owned String (was &'static str)
}

/// Whole-lexicon version (Q7). manifest = SHA-256 over sorted concatenation of entry hashes.
pub struct LexiconManifest {
    pub hash: Hash,
    pub entries: BTreeMap<String, LexiconEntry>,  // key = FQN string; [v0.2] String not VerbFqn
}
```

> **Why `String` not `&'static str` [v0.2].** The lexicon entries are constructed at runtime (not as static constants) and must derive `Deserialize` for replay-pinning against stored snapshots. `&'static str` cannot derive `Deserialize` without explicit lifetime work; `String` is the correct owned type and has no semantic cost here — the hash is the content-address, not the string's identity.

The slice populates entries for the 12 Phase-1/2 verbs. The two with non-trivial bindings:

```
ubo.edge.verify
  governing_taxonomy: Control
  writes:            [ControlGraph]
  preconditions:     [EvidenceCited]      // prior attach-evidence for this edge must exist — else REJECT
  authority:         { required_role: "senior_analyst", interactive_only: false }
  emits:             []
  # There is deliberately NO "set status" verb. Edge status is a FOLD OUTPUT,
  # never settable. That is what makes the ratchet structural (K-11).

ubo.determination.freeze
  governing_taxonomy: Control
  writes:            [Determination, ObligationGraph]   // pins AND emits
  preconditions:     [ReconciledProjection, StrategySelected]
  authority:         { required_role: "senior_analyst", interactive_only: false }
  emits:             [PersonObligation, EntityObligation] // resolved persons → subjects (Q4)
```

---

## 4. The two folds (one stream, two projections)

Both are pure functions over the per-subject event stream. `state = fold(events)`.

### 4.1 Control & determination fold (Q5)

```rust
pub struct ControlState {
    pub edges: BTreeMap<EdgeId, EdgeState>,              // status DERIVED, never stored as settable
    pub terminal_persons: BTreeMap<PersonId, TerminalStatus>, // VERB-SET (Q5)
    pub structure_class: Option<StructureClass>,         // set by classify-structure
    pub reconciliation_event_id: Option<EventId>,        // K-14 precondition witness
    pub selected_strategy: Option<String>,               // K-4
    pub smo_person_id: Option<PersonId>,                 // K-5 fallback
    pub smo_event_id: Option<EventId>,                   // always Some when smo_person_id is Some
    // ... registration metadata
}
```

- **Edge status is derived** from the sequence of events touching an edge: `assert` → `Asserted`; `attach-evidence` → `Evidenced`; `verify` (precondition: Evidenced first) → `Verified`; `supersede` → `Superseded` (never removed — K-13). Nothing sets status; the fold computes it.
- **Terminal natural-person status** (`Approved`, `Waived`) is set by an explicit decision verb (Q5).
- **Intermediate-entity resolution** is a derived-and-checkpointed fold over upward edges and stop-conditions (natural person / SMO / waiver); checkpointed at freeze (Q5). The `smo_event_id` field carries K-35 traceability: it is always `Some` when `smo_person_id` is `Some` (set together in `fold_control`); using `EventId::new()` as a fallback here is forbidden by the fold-path determinism invariant.

**Fold-path determinism invariant.** Both `fold/control.rs` and `determination.rs` carry a module-doc INVARIANT line:

> No `HashMap`/`HashSet`, no `Uuid::new_v4`/`EventId::new`, no `Utc::now()`, no `SystemTime::now()`, and no float-to-string in any hashed payload inside this module. Violating any of these breaks bit-identical replay (Q6, K-16/18/33).

All collections in the fold and DFS paths are `BTreeMap`/`BTreeSet` with explicitly sorted adjacency lists. Enforced by the Phase-1 determinism-stress tests (`phase1_fold_determinism_stress_graph_hash` and `_determination_hash`).

### 4.2 Obligation fold (Q4)

```rust
pub struct ObligationState {
    pub obligations: BTreeMap<ObligationId, ObligationTracks>, // PARALLEL tracks (Q4)
    pub subjects: BTreeMap<SubjectId, SubjectRollup>,          // fold over its obligations
}

pub struct ObligationTracks {           // tracks advance independently, converge at approval
    pub obligation_id: ObligationId,
    pub basis: ObligationBasis,         // role + jurisdiction + cbu_role + source_event_id (K-21)
    pub identity:  TrackState,
    pub screening: TrackState,
    pub risk:      TrackState,
    pub originating_event_id: EventId,  // K-35 traceability
}

pub struct ObligationBasis {
    pub role: String,
    pub jurisdiction: Option<String>,
    pub cbu_role: Option<String>,
    pub source_event_id: EventId,       // the event that established this basis (K-35)
}
```

- Obligations are **emitted by `freeze`** (each resolved person becomes a subject with identity + screening obligations) and by explicit `kyc.obligation.create`.
- A subject's overall KYC state is a **fold over its obligation tracks**; approval gates on all-required-terminal (Q4, K-23).
- The same person under multiple bases (shareholder + director) folds into **one `SubjectId`** with **distinct `ObligationId`s**, each carrying its own `ObligationBasis` (K-21/K-22, exit criterion 7).

---

## 5. Determination: demote, compose, freeze

```rust
pub trait DeterminationStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve(
        &self,
        edges: &[ReconciledEconomicEdge],   // reconciled, active economic edges (K-14)
        subject_entity_id: EntityId,
        natural_persons: &BTreeSet<PersonId>, // BTreeSet for deterministic DFS (invariant)
        threshold_pct: f64,
    ) -> Vec<ProngCandidate>;
}

/// The demoted percentage chain (V&S §12.2).
/// Feeds on reconciled, verified economic edges — structural fix for >100% double-count (K-14).
/// One prong's answer, not the determination. W7 owed: live differential vs ubo.compute-chains.
pub struct OwnershipProngStrategy;
```

> **W7 note (important).** `OwnershipProngStrategy` is a pure-Rust re-implementation of the percentage-multiply chain, verified against a hand-authored private-company fixture (`ec1_ownership_prong_differential_equality`). That test is a **fixture differential** — it proves the algorithm is equivalent on the declared fixture, NOT a live-oracle differential against the running `ubo.compute-chains` DB verb. The oracle proof (run both paths against the live DB on the same entities; assert equality) is **W7**, still owed before the demotion is considered proven end-to-end.

Flow for the private-company slice:

1. `ubo.edge.reconcile-conflict` → canonical economic projection (precondition to fold, K-14).
2. `ubo.determination.select-strategy` → picks `OwnershipProngStrategy` for *private company*.
3. `ubo.determination.compute-fold` → composes ownership-prong candidates; **records basis/prong per person** (K-1).
4. `ubo.determination.apply-smo-fallback` → if empty, an SMO person (never silence, K-5).
5. `ubo.determination.freeze` → pins the determination and emits obligations:

```rust
pub struct DeterminationPin {         // the K-18 close
    pub policy_version: String,
    pub lexicon_manifest_hash: Hash,  // Q7 — whole-lexicon version
    pub reference_snapshot_id: Uuid,
    pub import_run_ids: BTreeSet<Uuid>,
    pub graph_content_hash: Hash,     // SHA-256 of reconciled economic edges (K-14 witness)
    pub as_of: DateTime<Utc>,         // frozen clock from the freeze event's as_of (Q6)
}
```

**`RecoveryPin`** — helper struct grouping the four pin params for `recover_determination_at` (kept under 8 args per clippy `too_many_arguments`):

```rust
pub struct RecoveryPin<'a> {
    pub policy_version: &'a str,
    pub lexicon_manifest_hash: Hash,
    pub reference_snapshot_id: Uuid,
    pub import_run_ids: BTreeSet<Uuid>,
}
```

**SMO provenance invariant.** `smo_event_id` is always `Some` when `smo_person_id` is `Some` (set together in `fold_control`). `recover_determination_at` uses a match that panics on `(Some(pid), None)` rather than generating a random UUID — a random UUID in the originating event ID would break replay determinism (Q6) and lose K-35 traceability.

---

## 6. The interface seam (so persistence drops in later)

The pure engine crate **`ob-poc-kyc-substrate`** (zero sqlx, zero DB — §0a) sits behind the **existing** `SemOsVerbOp` / `TransactionScope` seam (`dsl-runtime/src/execution.rs:48–64` carries `principal`/`correlation_id`/`execution_id`). The slice's job at the seam: **record-intent → apply-fold → record-outcome, all inside the verb transaction.** Because `InMemoryEventStore` implements the engine's `KycEventStore` trait, the durable Postgres implementation — **`ob-poc-kyc-store`** writing `kyc_intent_events` (the membrane crate, DD-002) — replaces the in-memory store behind the same trait in W1-proper, with no caller change.

**H1 boundary.** The slice does **not** yet eliminate the authoritative-write bypass paths (`set_bods_interest_type` trigger, `fn_compute_economic_exposure` CTE, `cbu_board_controller` recompute). Those are W1-proper scope (K-15 cannot close until they route through the stream). Within the slice's scope, the event store is the only writer.

**H6 note.** No strangler hybrid exists yet. The cutover rule — *a table is authoritative until its fold is differential-equal, then cut over atomically per class* — applies when persistence lands.

### 6a. CI enforcement [v0.2]

Two build-time mechanisms are in place for the slice crate:

**`#![deny(unreachable_pub)]`** in `src/lib.rs`. All `pub` items in the crate are reachable through the `pub mod` chain; the lint is currently a no-op but acts as a ratchet — any `pub` item added inside a `pub(crate)` module will fail the build.

**`rust/scripts/check_kyc_substrate_deps.sh`** — dep-gate CI script. Runs `cargo tree -p ob-poc-kyc-substrate` and exits 1 if any of `{sqlx, tokio-postgres, sem_os_postgres, dsl-runtime}` appear in the transitive dep tree. Proven red/green:

```
# With sqlx in [dependencies]:
FAIL: forbidden dep 'sqlx' found in ob-poc-kyc-substrate dep tree  → exit 1

# After removal:
PASS: no forbidden deps in ob-poc-kyc-substrate  → exit 0
```

This gate **is the enforcement mechanism for V&S §12.3 (prove the semantics before hardening tables).** "Semantics-before-schema" is not a guideline the builder is trusted to honour — it is structurally impossible for the engine to acquire a DB dependency without the build failing. The gate must be added to the CI pipeline alongside `cargo test -p ob-poc-kyc-substrate` before W1-proper work begins, and (per §0a rule 1) it keeps the engine pure even as DD-002 re-opens the crate for the D2 `FoldRegistry`.

---

## 7. Exit criteria — state after slice build

All 15 tests pass. The 7 design exit criteria map as follows:

| EC | Test | Invariants | Status |
|---|---|---|---|
| 1 — Fixture differential | `ec1_ownership_prong_differential_equality` | Criterion 2 | ✅ **FIXTURE ONLY** (W7 oracle still owed) |
| 2 — Reconcile before fold | `ec2_conflicting_edges_fail_without_reconcile`, `ec2_reconciled_edges_do_not_exceed_100_percent` | K-14, Criterion 3 | ✅ |
| 3 — Proof ratchet | `ec3_verify_without_evidence_is_rejected`, `ec3_verify_after_evidence_succeeds` | K-11, Criterion 7 | ✅ |
| 4 — SMO never-empty | `ec4_smo_fallback_when_no_ubos_found`, `ec4_freeze_without_candidates_or_smo_fails` | K-5, Criterion 1 | ✅ |
| 5 — Replay determinism | `ec5_replay_determinism_after_supersede` | K-16/18/33, Criterion 5 | ✅ |
| 6 — K-35 traceability | `ec6_every_candidate_has_originating_event_id` | K-35, Criterion 11 | ✅ |
| 7 — Multi-role fold | `ec7_multi_role_person_one_subject_two_obligations` | K-21/22, Q4, Criterion 8 | ✅ |

Additional tests added during cleanup:

| Test | Purpose |
|---|---|
| `k13_superseded_edges_remain_in_fold` | Supersede-never-delete (K-13) |
| `k30_all_phase1_verbs_declare_governing_taxonomy` | K-30 binding lint |
| `q7_lexicon_manifest_hash_is_stable` | Stable across calls (Q7) |
| `phase1_fold_determinism_stress_graph_hash` | Fold-path determinism invariant — graph layer |
| `phase1_fold_determinism_stress_determination_hash` | Fold-path determinism invariant — determination layer |

**EC1 scope caveat (important).** `ec1` asserts that `OwnershipProngStrategy` on a hand-authored private-company fixture produces the same candidates as the percentage-multiply algorithm documented in `ubo_compute.rs`. It does **not** prove the live DB verb and the new strategy agree on real entity data. That is the W7 oracle test — run both against the same entities in the live DB, assert identical candidate sets. W7 must be green before the demotion (§12.2 of the V&S) is called proven.

---

## 7a. Realised structural invariants (shipped properties, not builder guidance) [v0.2, R7]

These are **enforced by construction** in the shipped crate — properties of the code, not rules a builder must remember. They are recorded here as realised so W1-proper inherits them as facts and the D2 re-open (§0 forward-reference) preserves them:

1. **No "set status" verb — edge status is unconstructable except as a fold output (K-11).** There is no verb, method, or field setter that writes `EdgeStatus`. It is *always* computed by `fold_control()` from the event sequence (`Asserted → Evidenced → Verified → Superseded`). The proof ratchet is structural because the alternative is not expressible in the API.
2. **No `now()` inside a verb — `as_of` is always a parameter (Q6).** No fold or determination function reads a wall clock; `as_of` enters only as a frozen field on `IntentEvent`. Replay re-runs the same inputs and cannot diverge on time.
3. **The determinism class is closed (the cleanup-pass invariant).** Inside `fold/control.rs`, `fold/obligation.rs`, and `determination.rs`: no `HashMap`/`HashSet` (only `BTreeMap`/`BTreeSet`), no `Uuid::new_v4`/`EventId::new`, no `Utc::now()`/`SystemTime::now()`, no float-to-string in any hashed payload. Module-doc INVARIANT lines state it; the two `phase1_fold_determinism_stress_*` tests enforce it (re-fold the same stream → bit-identical graph hash and determination hash). A reflexive `HashMap` or random-ID addition would fail those tests, not merely warn.

The single most expensive temptation to retrofit — and the one these three make impossible — is intent-from-state-delta. Because status, time, and ordering are all derived from the event stream and nothing else, there is no path by which authoritative state moves without a traceable originating event (K-35).

---

## 8. Out of slice scope (deferred, by design)

Durable persistence; concurrent appends / ordering under load; replay against a live DB; projection consistency under at-least-once outbox (W6); the other ten structure classes (widen W4 one class at a time, gated on golden fixtures — Addendum A §3.2); elimination of the trigger/CTE/recompute bypass paths (W1-proper, H1); screening/risk *execution* (the slice records the obligation + disposition hooks only); the W7 oracle differential.

---

## 9. W1-proper sequencing (what comes next)

1. **Wire the seam** (`sequencer_tx.rs`, `verb_executor_adapter.rs`): record-intent → apply-fold → record-outcome inside the verb transaction. `KycEventStore` already abstracts the store; this is wiring `InMemoryEventStore` behind the existing `SemOsVerbOp` execution path.
2. **Add `kyc_intent_events` migration**: same shape as `IntentEvent`; replaces `InMemoryEventStore` behind the trait with no caller change.
3. **Eliminate H1 bypass paths** one at a time (`set_bods_interest_type` trigger, `fn_compute_economic_exposure` CTE as a standalone determination, `cbu_board_controller` DELETE-recompute), each with a differential test proving K-15 closes.
4. **W7 oracle test**: run `OwnershipProngStrategy` and `ubo.compute-chains` on the same live-DB entities; assert identical candidate sets before the demotion is declared proven.
5. **Add `check_kyc_substrate_deps.sh` to CI pipeline** alongside `cargo test -p ob-poc-kyc-substrate`.

---

## 10. Change log

| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-06-30 | Initial slice design. Inlines ratified Q4–Q7. Verb-event contract (per-subject ordering, frozen clock, captured effects); lexicon-entry contract + manifest; control fold (derived edge status, Q5) + obligation fold (parallel tracks, Q4); demoted ownership-prong strategy + freeze pin (closes K-18). Seam reuse + H1/H2/H6 boundaries. Exit criteria mapped to gap-report RED tests. In-memory only; no schema authorised. |
| 0.2 | 2026-06-30 | **Post-build reconciliation.** Three corrections from v0.1: (1) crate name `ob-poc-kyc-substrate` + build provenance (§0); (2) `&'static str → String` on `LexiconEntry.intent`, `AuthoritySpec.required_role`, `EmitSpec.kind` with rationale; (3) CI guards: `deny(unreachable_pub)` + dep-gate script with proven red/green (§6a). Additional deltas: `IntentEvent::new` is 7-arg constructor + builder methods (not 10-arg); `DeterminationStrategy::resolve` takes `&[ReconciledEconomicEdge]` + `&BTreeSet<PersonId>` (not `&ReconciledControlGraph`); `RecoveryPin<'_>` groups 4 pin params to satisfy clippy; fold-path determinism invariant stated and enforced (no HashMap/HashSet/random IDs in fold/determination modules); `ControlState` fields documented including `smo_event_id` + fold invariant; `ObligationBasis` and `ObligationTracks` fully documented; EC1 explicitly scoped as fixture-differential with W7 oracle still owed; W1-proper sequencing added (§9); 15-test suite documented with additions beyond the 7 original ECs. |
| 0.2 (build landed) | 2026-06-30 | **Slice landed green** on `codex/phase-1-5-governance-closure`: 15/15 tests passing, `cargo clippy -p ob-poc-kyc-substrate` clean (zero warnings, zero `#[allow]`), dep-gate proven red-then-green by adding/removing `sqlx`. Recorded as a build event (not just a design), per the project's "every decision is a recorded event" discipline. **Tree-reconciliation pass (R1–R8 checklist):** R1 crate named in §6; R2 crate-discipline §0a (inward dep sketch + 5 rules; rule 4 `Principal`-hoist marked pending); R3/R4 already current (15 tests; `String` types); R5 dep-gate framed as the V&S §12.3 semantics-before-schema enforcement; R6 forward-reference to the DD-002/D2 `FoldRegistry` re-opening this crate (§0); R7 structural invariants recorded as realised/unconstructable (§7a); R8 this line. Applied before the W1-proper build so the paper is accurate at the moment D2 (build-order step 1) re-opens the crate. |
