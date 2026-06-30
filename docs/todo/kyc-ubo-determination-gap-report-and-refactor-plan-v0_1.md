# KYC/UBO — Current-State Gap Report & Refactoring Plan

| | |
|---|---|
| **Document** | EOP-RP-KYCUBO-001 (companion refactoring plan to EOP-VS-KYCUBO-001) |
| **Type** | Current-state gap report + refactoring plan (no code, schema, or remediation authorised) |
| **Version** | 0.1 — Draft for Adam peer review |
| **Author** | Lead architect (Claude), grounded in a 5-probe codebase + DB survey |
| **Date** | 2026-06-30 |
| **Binds to** | EOP-VS-KYCUBO-001 v0.6 *From Percentage to Determination* (the V&S; treated here as binding). Invariants referenced as K-1…K-35. Workstreams W1–W7 from V&S Appendix B. |
| **Status** | Analysis only. This document does not authorise implementation. It is the §12 / Appendix-B companion the V&S anticipates. |

> **Reading note.** The V&S is verb-first and event-sourced *by design decision*, not by tooling preference. This report does **not** collapse the target into CRUD, ORM, an entity service, or generic event sourcing. Where the current code already does the verb-first thing partially (e.g. per-verb content hashing, the convergence allegation/proof split, dual VOTES/ECONOMIC axes), that is called out as *reusable substrate*, not as "done."

---

## 0. Executive verdict

The platform is **verb-fronted but current-state-of-record**. There is a rich KYC/UBO verb surface (≈60 verbs across `ubo`, `ubo.registry`, `ownership`, `control`, `edge`, `kyc-case`, `kyc`, `investor`, `screening`, `document`) and a real determination-run snapshot table — but **every authoritative mutation writes the destination directly into domain tables, and no ordered semantic-intent stream is the system of record.** That single fact is the centre of gravity of the refactor: it is the gap behind K-15, K-16, K-17, K-33, K-34, K-35 simultaneously, and it cannot be tuned away because it is a system-of-record question, not a performance one (V&S §9).

Three secondary structural gaps sit on top of it:

1. **No structure-class strategy selection (K-4).** The determination engine (`ubo.compute-chains`) runs one ownership-percentage-multiply uniformly over all entities. Prongs exist as a *vocabulary* (`kyc_ubo_registry.ubo_type ∈ {OWNERSHIP, CONTROL, TRUST_ROLE, SMO_FALLBACK, NOMINEE_BENEFICIARY}`) but there is no strategy dispatcher keyed on structure class, and no automatic SMO fallback (K-5).
2. **No first-class obligation graph (K-21, K-22, K-24, K-28).** Obligation is *inferred at query time* from a static `roles.kyc_obligation` mapping; it is never recorded as `role + subject + jurisdiction + exposure + policy + evidence + decisions` with a basis. There is no subject taxonomy and no institutional KYC profile.
3. **No proof ratchet and no lexicon-bound verbs (K-11, K-30).** Edge epistemic status (`alleged → pending → proven → disputed`) exists on `ubo_edges`/`ubo_relationship_verification` but is *settable*, mutated in place, not a governed evidence-cited transition; and verb YAML does not declare governing taxonomy / written fold / authority / emits.

**Good news for sequencing:** the V&S's own §12.3 ("prove the semantic model before hardening tables") is the right path *and* the codebase already has the seams to do it cheaply — one `TransactionScope` choke point per mutation that already carries `principal` + `correlation_id` + `execution_id`; a per-verb content-hash + execution-log substrate; an append-only `sem_reg` reference plane; and an `outbox` for post-commit folds. The first slice is in-memory and additive; nothing needs to be deleted (V&S §12.2: the percentage chain is *demoted, not deleted*).

---

## 1. Current KYC/UBO capability map

### 1.1 Determination engine (the percentage chain)
- **`ubo.compute-chains`** — `rust/crates/sem_os_postgres/src/ops/ubo_compute.rs:52`. In-memory upward adjacency graph over `entity_relationships` (type `ownership`), stack DFS multiplying percentages along each chain (`new_pct = cumulative * pct/100`, ~line 219), cycle detection (`path.contains`), depth cap, hardcoded 5% floor. Persists a frozen run to `ubo_determination_runs` (output/chains snapshots, `code_hash`, `config_version`, `threshold_pct`, `as_of`).
- **SQL economic look-through** — `migrations/031_economic_lookthrough.sql:43` `kyc.fn_compute_economic_exposure()` recursive CTE; bounded depth; economic-only multiply.
- **`ubo.trace-chains`** (`ubo_analysis.rs:30`) — SQL-backed chain trace; annotates `has_control_path` boolean; synthesises prong label *post-hoc* (`OWNERSHIP` / `CONTROL` / `OWNERSHIP_AND_CONTROL`).
- **`ubo.snapshot.capture` / `.diff`** — serialise / set-diff determination runs (the closest thing to "freeze" today).

### 1.2 Control / ownership graph
- **`control_edges`** (`migrations/022_control_edges.sql`) — typed edges (`HOLDS_SHARES`, `HOLDS_VOTING_RIGHTS`, `APPOINTS_BOARD`, `EXERCISES_INFLUENCE`, `IS_SENIOR_MANAGER`, `IS_SETTLOR/TRUSTEE/PROTECTOR/BENEFICIARY`, …) with BODS/GLEIF/PSC mappings auto-derived via trigger. **No epistemic-status column.**
- **`entity_relationships`** — the raw assertion plane; `relationship_type`, `percentage`, `effective_from/to`, `replaces_relationship_id`, `confidence ∈ {ASSERTED, EVIDENCED, VERIFIED}`, `import_run_id`. Append + `effective_to` + replace-chain.
- **`ubo_edges`** + **`proofs`** (`migrations/202412_ubo_convergence.sql`) — the modern convergence model: edge carries `alleged_*` vs `proven_*`, `status ∈ {alleged, pending, proven, disputed}`, `resolution_type ∈ {allegation_corrected, proof_accepted, waived}`. `proofs` carries `valid_from/until`, `status ∈ {pending, valid, expired, dirty, superseded, rejected}`.
- **`ubo_relationship_verification`** (renamed from `cbu_relationship_verification`, 2026-06-22) — per-relationship `status ∈ {unverified, alleged, pending, proven, disputed, waived}`, alleged vs observed %. **Mutated in place.**
- **`cbu_board_controller`** + **`board_control_evidence`** — *derived* board controller (method/confidence/score + JSON explanation). Refreshed by DELETE-and-recompute.
- **`ubo_determination_runs`** — frozen run snapshots (see 1.1).
- **`kyc_ubo_registry`** / `ubo_registry` — UBO candidate→approved lifecycle (`status ∈ {CANDIDATE, IDENTIFIED, PROVABLE, PROVED, REVIEWED, APPROVED, WAIVED, REJECTED, EXPIRED}`), `ubo_type` prong vocabulary, waiver fields.

### 1.3 Obligation / case / subject side
- **`cases`** — single linear FSM `INTAKE → DISCOVERY → ASSESSMENT → REVIEW → {APPROVED, REJECTED, BLOCKED, WITHDRAWN, …}`. Carries `subject_entity_id` (distinct from `cbu_id`).
- **`entity_workstreams`** — *one per entity per case* (not per obligation), with boolean flags `identity_verified / ownership_proved / screening_cleared / evidence_complete`.
- **`roles.kyc_obligation`** — static `{FULL_KYC, SIMPLIFIED, RECORD_ONLY, NO_KYC}` mapping; obligation **inferred on the fly** via `cbu_entity_roles → roles`.
- **`screenings`** — per-workstream, typed (`SANCTIONS/PEP/ADVERSE_MEDIA/…`), disposition states; feeds tollgate/approval, not determination (K-26 broadly honoured).
- **Evidence**: `kyc_ubo_evidence`, `ubo_evidence`, `cbu_evidence` — status sets `{REQUIRED, REQUESTED, RECEIVED, VERIFIED, REJECTED, WAIVED, EXPIRED}` but **no ratchet** (states jump).
- **Tollgate**: `tollgate_definitions` + `tollgate_thresholds` (configurable, `is_blocking`) + `tollgate_evaluations` (pass/fail, overridable). Gates explicit `kyc-case.close`.
- **`kyc_clearance_mandates`** (`migrations/20260616_…ledger.sql`) — `entity × role × product → clearance_status`. The nearest existing thing to obligation-with-context; deal/product-scoped only.
- **`kyc_decisions`** — final decision with frozen `evaluation_snapshot`.

### 1.4 Reference data already present
GLEIF/BODS/PSC alignment tables (`bods_*`), `document_types` (with applicability + embeddings), `issuer_control_config` (thresholds per issuer/as-of), `investor_role_profiles` (look-through policy, `is_ubo_eligible`). These are mostly **DDL-baked taxonomies** (CHECK constraints), not generated reference-plane objects.

---

## 2. Existing write paths and mutation paths

```
user/agent input → orchestrator_v2.process()        (single choke point)
  → verb resolved → ObPocVerbExecutor.execute_verb   (verb_executor_adapter.rs:151)
    → PgTransactionScope::begin()                     (sequencer_tx.rs:37)
      → SemOsVerbOp::execute(args, ctx, scope)        (ops/mod.rs:995)
          • CRUD verbs    → direct INSERT/UPDATE/DELETE on domain tables
          • plugin verbs  → custom SQL + optional event emit + optional outbox row
      → commit / rollback                              (atomic per verb)
  → rehydrate_tos()  (READ live tables → hydrated_state)   ← fold is a re-read, not a replay
  → append_trace_enriched()  (session_trace, telemetry, post-hoc, no args/authority)
```

**Findings that matter for the refactor:**
- **One transaction seam per mutation**, already abstracted as `&mut dyn TransactionScope`. This is the single interception point W1 needs. (`sequencer_tx.rs`, `verb_executor_adapter.rs:151-248`.)
- **`VerbExecutionContext` already carries `principal` (actor+roles), `correlation_id`, `execution_id`** (`dsl-runtime/src/execution.rs:48`) — but **none of it is persisted** with the mutation.
- **Two mutation idioms coexist**: CRUD verbs write directly (`ubo.add-ownership`, `kyc-case.assign`, `ownership.right.*`, `control.add`); plugin verbs add choreography + advisory events. There is **no "record intent → apply → record outcome"** pattern anywhere.
- **Destructive paths exist on authoritative data**: `ubo.delete-relationship` (hard `DELETE`), in-place `UPDATE` on `ubo_relationship_verification`, DELETE-and-recompute on `cbu_board_controller`. These violate K-34 head-on.

---

## 3. Existing persistence model: append / update / projection / event / audit

| Plane | Tables | Append-only? | Authoritative? | Verdict vs V&S §9 |
|---|---|---|---|---|
| **Authoritative current-state** | `cases`, `entity_workstreams`, `entity_relationships`, `control_edges`, `ubo_edges`, `ubo_relationship_verification`, `kyc_ubo_registry`, … | Mixed (some `effective_to`/replace; some in-place UPDATE; some hard DELETE) | **YES — system of record** | ✗ This is "noun-state-as-record" — the thing the V&S replaces |
| **Frozen snapshots** | `ubo_determination_runs`, `kyc_decisions.evaluation_snapshot`, `case_evaluation_snapshots` | Yes | Yes (point reads) | ◐ Partial K-18: has `code_hash`/`config_version`/`as_of`; **missing graph hash, lexicon version, full import-run pin** |
| **Convergence/proof** | `ubo_edges` (alleged vs proven), `proofs` | Edge unique-per-type; proof append + dirty | Yes | ◐ Has the allegation→proof *shape* but status is settable, not a governed ratchet (K-11) |
| **Reference plane** | `sem_reg` snapshots, `dsl_verbs` (per-verb `compiled_hash`/`yaml_hash`/`compiler_version`), `dsl_execution_log`/`dsl_idempotency` (`verb_hash`, `verb_names`, `get_verb_config_at_execution()`) | Append-only, content-addressed | Governance | ◐ Strong K-31 substrate *for verbs*; **not extended to KYC subject/control/obligation taxonomies** (those are DDL CHECK constraints) |
| **Telemetry / observability** | `case_events`, `event_log`, `events`, `intent_events`, `ubo_assertion_log`, `session_trace` | Yes | **No** | ✗ Records *that* something happened, not the authoritative *intent before state moved*; no args+authority+causation atomic with mutation |
| **Effect dispatch** | `outbox` (FOR UPDATE SKIP LOCKED, `idempotency_key`, effect_kind) | Append (pending→done) | Side-effects | ✓ Reusable as the post-commit fold/projection dispatcher (W6) |

**Bottom line:** there is *no* authoritative append-only semantic-intent stream. The append-only tables that exist are telemetry; the authoritative tables are current-state. K-16 is unmet at the architectural level.

---

## 4. Existing DSL / verb coverage (against the V&S seed lexicon)

Verbs *exist and are content-addressed per verb*, but **not in the §8.1 normative shape** (no `governing taxonomy`, no declared `writes`-fold, no `authority` beyond ad-hoc `role_guard`, no `emits`). Mapping the V&S Appendix-A seed surface:

| V&S target verb | Status | Nearest current | Note |
|---|---|---|---|
| `kyc.subject.register` | **ABSENT** | `kyc-case.create` | Case-centric, not subject-centric |
| `kyc.subject.classify-structure` | **ABSENT** | `entity.create` subtype | No structure-class concept |
| `kyc.subject.link-to-cbu-role` | **ABSENT** | `cbu.assign-role` (structural) | Not obligation-emitting |
| `ubo.edge.assert-control` | **ABSENT** | `control.add` | Direct upsert, no epistemic status |
| `ubo.edge.assert-economic-interest` | **ABSENT** | `ubo.add-ownership` | No economic-vs-control edge split at verb grain |
| `ubo.edge.attach-evidence` | **ABSENT** | `:evidence-doc-id` arg | Evidence is an arg, not a ratchet step |
| `ubo.edge.verify` | **ABSENT** | `ubo.registry.advance` | Registry promotes; edge has no verify verb |
| `ubo.edge.supersede` | ◐ near | `ubo.convergence-supersede` | Exists; not `ubo.edge`-scoped; not always non-destructive |
| `ubo.edge.pierce-nominee` | **ABSENT** | `ubo.trace-chains` (implicit) | No explicit pierce / terminal-at-nominee ban |
| `ubo.edge.reconcile-conflict` | ◐ near | `ownership.reconcile` | Exists as *analysis*, not a determination precondition |
| `ubo.determination.select-strategy` | **ABSENT** | — | No structure-class strategy dispatch |
| `ubo.determination.compute-fold` | ◐ near | `ubo.compute-chains` | Monolithic; consumes raw (not reconciled) edges |
| `ubo.determination.apply-smo-fallback` | **ABSENT** | `ubo.waive-verification` | No auto-SMO |
| `ubo.determination.freeze` | ◐ near | `ubo.snapshot.capture` | Frozen run exists; under-pinned (K-18) |
| `kyc.obligation.create/satisfy/defer/waive/expire/reopen` | **ABSENT** | role-derived | No obligation as first-class object |
| `kyc.person.assert/verify-identity/screen/assess-risk/approve` | **ABSENT** | workstream flags + `screening.*` | Identity is a boolean, not a track |
| `kyc.entity.*` (institutional profile) | **ABSENT** | `entity_regulatory_profiles` fragments | No consolidated profile |

**Lifecycle FSM verbs that *do* exist and are reusable**: `kyc-case.*` (12), `ubo.registry.*` (6, with a real allegation→approved ratchet), `ownership.reconcile*` (cross-source reconciliation + findings), `screening.*`, `document.solicit` (durable). These become *consumers/projections* of the new stream, not the system of record.

---

## 5. Where the percentage chain is retained as the ownership-prong strategy (V&S §12.2)

The current chain code is **demoted, not deleted**. Concretely:

1. **Wrap, don't rewrite.** `ubo.compute-chains` (`ubo_compute.rs`) and `kyc.fn_compute_economic_exposure()` become the body of an **`ownership_prong_strategy`** — one `DeterminationStrategy` among several, selected by structure class.
2. **Feed it reconciled, verified edges.** Today it consumes raw `entity_relationships`; the target makes `ubo.edge.reconcile-conflict` (canonical projection) a **precondition** so the chain multiplies a single non-conflicting economic representation — this is exactly the fix for the >100% double-count defect (K-14, Success Criterion 3).
3. **Its output feeds `ubo.determination.compute-fold`, it is not the determination.** The fold composes ownership-prong candidates with control-by-other-means and SMO-fallback results, records **basis/prong per person**, and only then `ubo.determination.freeze` pins it.
4. **Structure classes that map to it directly:** *private company* and *multi-tier holding group* (economic ≈ control), and any class where policy mandates an economic threshold test. For *LP/PE fund, LLP, trust, foundation, SICAV/AIF, cooperative, nominee* the ownership prong is **not primary** — control/role prongs lead (V&S §6.4), and the chain contributes only the economic axis where one exists.
5. **Differential safety net.** A golden differential test (old `compute-chains` vs new `ownership_prong_strategy` over the same private-company fixtures) proves the demotion is behaviour-preserving before it is wired in (W7).

This is the lowest-risk bridge: the engine keeps running; it just stops being *the* answer and becomes *one prong's* answer.

---

## 6. Gap analysis against K-1…K-35

Legend: **MET** / **◐ PARTIAL** (substrate exists, invariant not enforced) / **✗ ABSENT** / **⚠ VIOLATED** (current code does the opposite).

### Determination
| K | Invariant | Status | Evidence / gap |
|---|---|---|---|
| K-1 | Basis mandatory | ◐ | `ubo_type` exists but prong is synthesised post-hoc, not recorded as mandatory determination basis |
| K-2 | Axes independent | ◐ | VOTES/ECONOMIC kept separate in `ownership_snapshots`; but `compute-chains` collapses to one % multiply and does not reconcile two trees at the determination |
| K-3 | Control not multiplied | ◐ | Economic correctly multiplied; control-by-other-means not *propagated as control* — it is reduced to a `has_control_path` boolean, so control-prong determination is underdeveloped rather than wrong |
| K-4 | Structure selects strategy | ✗ | Uniform engine; no structure-class dispatch |
| K-5 | No silent absence (SMO) | ✗ | `SMO_FALLBACK` is a registry type; no automatic fallback when ownership+control empty |
| K-6 | Thresholds sourced | ◐ | `issuer_control_config` per issuer/as-of; verb default 25; not governed per jurisdiction × risk × structure-class in the reference plane |
| K-7 | Pooled investors route out | ◐ | `investor_role_profiles.lookthrough_policy` + `is_ubo_eligible`; routing is data, not a governed determination rule |
| K-8 | Nominees pierced | ◐ | `NOMINEE_BENEFICIARY` type; no explicit pierce verb / no terminal-at-nominee prohibition |

### Graph & state machine
| K | Invariant | Status | Evidence / gap |
|---|---|---|---|
| K-9 | Edges typed control claims | ◐ | `control_edges` typed; validity partly modelled; **proof rule per edge type not formalised**; epistemic status not on the edge |
| K-10 | Non-register control instrument-cited | ◐ | `evidence_hint`/`source_document_id` exist; citation not enforced for control-by-other-means |
| K-11 | Proof is a ratchet | ⚠ | Status fields settable + `ubo_relationship_verification` mutated in place; advance is not an evidence-cited governed transition |
| K-12 | Node-status discipline | ✗ | No terminal-person-verb-set vs intermediate-derived-fold distinction |
| K-13 | Supersede-never-delete | ⚠ | `entity_relationships` supersedes well; but `ubo.delete-relationship` hard-deletes and `cbu_board_controller` DELETE-recomputes |
| K-14 | Reconcile before determination | ◐ | `ownership.reconcile` exists but as analysis, **not a precondition** to the fold |

### Persistence & governance
| K | Invariant | Status | Evidence / gap |
|---|---|---|---|
| K-15 | Verbs sole mutator | ◐ | Verbs are the main path; CRUD verbs + SQL triggers (`set_bods_interest_type`) + derived-table recompute mutate outside any intent record |
| K-16 | Verb stream is SoR | ✗ | **Current-state-in-tables is SoR.** No ordered authoritative intent stream |
| K-17 | Intent recorded | ◐ | `case_events`/`session_trace`/`intent_events` are telemetry; not args+authority atomic with mutation |
| K-18 | Determinations immutable + reproducible | ◐ | `ubo_determination_runs` pins `code_hash`/`config_version`/`as_of`; **missing graph hash, lexicon version, full import-run set** |
| K-19 | Substrate rigid, structure soft | ◐ | Referential integrity solid; KYC taxonomies are DDL CHECK constraints, not generated projections |
| K-20 | Structural change governed in reference plane | ✗ | New structure class / control-means / obligation type = migration today |

### KYC obligation
| K | Invariant | Status | Evidence / gap |
|---|---|---|---|
| K-21 | Obligation role-based (recorded basis) | ◐ | `roles.kyc_obligation` static; basis never recorded per obligation |
| K-22 | Identity ⊥ obligation | ✗ | Identity is a boolean flag on a single per-entity workstream; not a reusable identity record supporting many obligations |
| K-23 | Determination ⊥ approval | MET-ish | Case approval separate from UBO determination; tollgate gates on both — but not modelled as parallel obligation terminality |
| K-24 | CBU role drives obligation selection | ◐ | `kyc_clearance_mandates` (entity×role×product) exists for deals; not generalised to structure×role×jurisdiction×exposure×risk |
| K-25 | Evidence reusable only via provenance | ◐ | Evidence links + `proofs.valid_from/until`; reuse not gated by provenance/validity-window rule |
| K-26 | Screening gates approval not determination | MET | Screening feeds tollgate/approval only |
| K-27 | Retail/person KYC first-class | ◐ | `investors` lifecycle exists; person is "entity + workstream," not a first-class subject |
| K-28 | Institutional KYC profile first-class | ✗ | No consolidated profile; fragmented across workstreams/evidence/regulatory_profiles |

### Lexicon & functional orientation
| K | Invariant | Status | Evidence / gap |
|---|---|---|---|
| K-29 | Verbs primary; data aligned | ◐ | Platform is verb-first, but KYC structs exist without a governing-verb declaration |
| K-30 | Every verb declares its binding | ✗ | No `governing taxonomy` / `writes`-fold / `authority` / `emits` in verb YAML |
| K-31 | Lexicon governed, versioned, content-addressed; replay pins it | ◐ | Per-verb `compiled_hash` + `dsl_execution_log.verb_hash` + `get_verb_config_at_execution()` is strong substrate; **no whole-lexicon version; replay does not pin it** |
| K-32 | State carries no behaviour | ◐ | Behaviour leaks into SQL triggers, recursive CTEs, derived tables |

### Point-in-time & audit causality
| K | Invariant | Status | Evidence / gap |
|---|---|---|---|
| K-33 | Point-in-time KYC mandatory | ✗ | Cannot reconstruct full as-of subject/obligation/evidence/determination state; in-place updates erase prior |
| K-34 | Destructive mutation not authoritative | ⚠ | Hard DELETE + in-place UPDATE on authoritative tables |
| K-35 | No state without semantic cause | ✗ | State reachable via direct CRUD / triggers / recompute with no originating verb event |

**Scorecard:** 0 fully-clean of the 35 are "MET and enforced" beyond K-23/K-26 (broadly honoured). ~17 PARTIAL (real reusable substrate), ~11 ABSENT, ~4 VIOLATED. The PARTIALs are the leverage: most of the target's *data shapes* exist; what is missing is the *stream-as-record inversion* and the *enforcement* (ratchet, strategy dispatch, obligation as object, lexicon binding).

---

## 7. Minimum vertical slice (proves the architecture)

Per V&S §12.3–§12.4 and Appendix A Phase 1–2: **prove the semantic model in memory before hardening any table.** The slice deliberately does *not* start with schema.

**Slice scope — one structure class (private company) end to end:**

1. **Verb-event contract** (in-memory): `{ verb_fqn, lexicon_hash, actor, authority, target_bindings, payload, payload_hash, idempotency_key, causation_id, correlation_id, seq, ts }`.
2. **Lexicon-entry contract** (§8.1 shape): FQN, intent, args, governing taxonomy, writes-fold, reads, preconditions, authority, emits — for the ~10 Phase-1 verbs only.
3. **In-memory fold/replay**: append events → fold to a minimal control graph + a minimal obligation graph; replay must be bit-identical.
4. **Phase-1 verbs over the stream**: `kyc.subject.register`, `kyc.subject.classify-structure`, `ubo.edge.assert-control`, `ubo.edge.assert-economic-interest`, `ubo.edge.attach-evidence`, `ubo.edge.verify` (ratchet-enforced), `ubo.edge.reconcile-conflict`.
5. **Phase-2 determination**: `ubo.determination.select-strategy` (picks `ownership_prong_strategy` for private company), `ubo.determination.compute-fold` (wraps the demoted percentage chain over *reconciled* edges), `ubo.determination.apply-smo-fallback` (never-empty), `ubo.determination.freeze` (pins policy + graph hash + lexicon version), `kyc.obligation.create` (emitted by freeze).
6. **Append/supersede replay proof**: supersede an edge, re-freeze, recover the *prior* determination at its point in time.

**Exit criterion of the slice:** for a private-company fixture, the new path produces a determination differentially equal to today's `compute-chains` for the ownership prong, *plus* a recorded basis, *plus* a frozen replayable determination, *plus* an emitted obligation — all from an ordered intent stream, with **no schema migration yet** (in-memory store behind the same `SemOsVerbOp`/`TransactionScope` interface so persistence drops in later).

This is the smallest thing that proves K-11, K-14, K-15–K-18, K-30, K-33–K-35 at once on one class, before any table is touched.

---

## 8. Ordered workstreams

Mapped to V&S Appendix B (W1–W7). Ordering follows §12.3 (semantics → in-memory folds → persistence). Each is a review gate, not a ticket list.

| Phase | Workstream | Scope | Satisfies | Depends on |
|---|---|---|---|---|
| **W0** | *Decision pack* (this doc's sign-off) | Resolve V&S §15 open questions that block the contracts (esp. Q4 person-lifecycle shape, Q5 node-status fork, Q6 replay determinism boundary, Q7 lexicon versioning, Q9 read-model). No code. | — | V&S sign-off |
| **W1** | Verb-stream substrate | Append-only intent event (FQN, lexicon hash, actor, authority, target bindings, payload+hash, idempotency, causation/correlation, ordering). Intercept at the existing `TransactionScope` seam: record-intent → apply → record-outcome **inside** the verb transaction. No authoritative mutation outside it. | K-15, K-16, K-17, K-33, K-34, K-35 | W0 |
| **W2** | Reference-plane lexicon | Extend verb YAML + `dsl_verbs` to the §8.1 entry shape (governing taxonomy, writes-fold, authority, emits); introduce a **whole-lexicon version**; make replay pin it. Reuse existing per-verb `compiled_hash` substrate. | §8, K-30, K-31 | W1 |
| **W3** | Subject taxonomy & KYC subject model | Subject as anything-carrying-obligation (person-as-customer / -as-UBO / -as-controller / -as-SMO / -as-related; entity-as-customer / -as-intermediate; investor/subscriber). Role-based **basis recording**; CBU/Deal role link. `kyc.subject.*`, `kyc.role.*`. | §5, K-21–K-24, K-27 | W1, W2 |
| **W4** | Control graph & UBO determination | Typed control edges with per-type proof rules; proof ratchet (`assert → attach-evidence → verify`); reconcile-before-fold; structure-class strategy dispatch; ownership/control/SMO prongs; `freeze`. **Demote the percentage chain to `ownership_prong_strategy`.** | §6, §7.2, K-1–K-14, K-18 | W1, W2, (W7 wrap) |
| **W5** | Obligation graph | `subject → role → obligation → evidence → decision`; obligations emitted by `determination.freeze`; per-obligation lifecycle as parallel tracks; person-level + entity-profile folds; screening/risk hooks (consume existing `screening.*`/tollgate). | §7.3–§7.5, K-23, K-26, K-28 | W3, W4 |
| **W6** | Projections & analyst read model | Current determination / obligation / subject-profile projections rebuilt as **non-authoritative folds** over the stream; freshness/hash; dispatch via existing `outbox`. Existing `cases`/`entity_workstreams`/`ubo_edges` become projections. | §9, K-34, V&S Q9 | W1, W4, W5 |
| **W7** | Migration of current UBO code | Wrap `ubo.compute-chains` behind `ownership_prong_strategy`; consume reconciled, verified economic edges; feed `compute-fold`; differential old/new over private-company fixtures. | §12.2, K-14 | runs *with* W4 |

**Critical path:** W0 → W1 → W2 → W4(+W7) on one structure class (= the §7 slice) → W3 → W5 → W6, then widen W4 across the §6.4 structure-class table one class at a time.

---

## 9. Files likely affected (no edits authorised — impact map only)

**Determination engine (demotion target, W4/W7):**
- `rust/crates/sem_os_postgres/src/ops/ubo_compute.rs` — wrap as `ownership_prong_strategy`
- `rust/crates/sem_os_postgres/src/ops/ubo_analysis.rs`, `ubo_graph.rs`, `ubo_registry.rs`, `ownership.rs` — become prong/fold inputs and stream-projections
- `migrations/031_economic_lookthrough.sql` (`fn_compute_economic_exposure`), `022_control_edges.sql`, `013_capital_structure_ownership.sql` — economic axis under the ownership prong

**Verb-stream seam (W1) — the single highest-leverage area:**
- `rust/src/sequencer_tx.rs`, `rust/src/sequencer.rs`, `rust/src/sem_os_runtime/verb_executor_adapter.rs` — record-intent/outcome inside the txn
- `rust/crates/dsl-runtime/src/execution.rs` (`VerbExecutionContext`) — surface actor/authority into the event
- `rust/crates/sem_os_postgres/src/ops/mod.rs` (`SemOsVerbOp`) — contract unchanged; gains stream emission
- New migration: append-only `kyc_intent_events` (authoritative) — *Phase W1, after in-memory slice proves out*

**Lexicon (W2):**
- `rust/config/verbs/*.yaml` (esp. `ubo.yaml`, `ownership.yaml`, `control.yaml`, `edge.yaml`, `kyc/*.yaml`) — §8.1 entry shape
- `docs/verb-definition-spec.md` — normative shape extension
- `rust/migrations/20260104_execution_verb_hashes.sql`, `dsl_verbs` compiler — whole-lexicon version

**Subject/obligation (W3/W5):**
- `rust/config/verbs/kyc/*.yaml`, `rust/config/packs/kyc-case.yaml`, `rust/config/sem_os_seeds/state_machines/kyc_case_lifecycle.yaml`
- `migrations/202412_kyc_case_builder.sql`, `20260616_add_context_dependent_kyc_ledger.sql`, tollgate migrations
- New: subject taxonomy + obligation graph (reference-plane, **not** new DDL CHECK constraints — governed objects per K-20)

**Projections (W6):**
- `rust/src/outbox/*` (drainer/consumers), `rehydrate_tos` path in `sequencer.rs` — re-point folds to the stream

**Reference plane / governance (W2/W3/K-20):**
- `rust/src/sem_reg/*`, `sem_os_*` crates — host subject/control/obligation taxonomies + lexicon as governed objects

---

## 10. Tests to add **before** any code change (RED-first)

These encode the V&S success criteria as executable gates; they should fail against `main` today and pass only when the slice lands. External harnesses, public API only (per the repo's test-boundary rule).

1. **Differential ownership prong** — old `ubo.compute-chains` vs new `ownership_prong_strategy` over private-company fixtures must be equal (proves demotion is safe; Success Criterion 2, W7).
2. **>100% / conflict reconciliation gate** — a determination over conflicting source edges must reconcile first and must never sum >100% economic (K-14, Criterion 3). *Should fail today.*
3. **Proof ratchet enforcement** — `ubo.edge.verify` without a cited evidence event must be rejected; status cannot be set directly (K-11, Criterion 7). *Should fail today (status is settable).*
4. **Structure-class strategy selection** — a fund-LP fixture must surface controlling principals via the control prong and must **not** attribute control to passive LPs (K-4, Criterion 2). *Should fail today (no strategy dispatch).*
5. **SMO never-empty** — any determination yielding no ownership/control UBO must return an SMO person or an explicit authorised waiver, never silence (K-5, Criterion 1). *Should fail today.*
6. **Point-in-time recovery / replay determinism** — append → supersede → re-freeze; recover the prior determination bit-identically at its `as_of`, pinned to lexicon version + graph hash (K-16, K-18, K-31, K-33, Criterion 5). *Should fail today.*
7. **No authoritative state without semantic cause** — assert that every row in the new authoritative determination/obligation projections has an originating intent-event id (K-35, Criterion 11). *Should fail today.*
8. **Obligation basis + multi-role fold** — the same natural person as 30% shareholder + director + signatory folds into one consolidated case with three distinct basis-obligations (K-21, K-22, Criterion 8). *Should fail today (obligation inferred, no basis).*
9. **Lexicon-entry binding lint** — every KYC/UBO verb YAML declares governing taxonomy + writes-fold + authority + emits; CI lint fails otherwise (K-30, Criterion 10).
10. **Destructive-mutation guard** — a source-scanning/integration test asserting no hard `DELETE` / in-place authoritative `UPDATE` on determination/obligation tables outside a supersede verb (K-34).

---

## Appendix — open questions this plan inherits from V&S §15 (must close in W0)

- **Q4** Person KYC lifecycle: confirm *parallel per-obligation tracks → approval gate* (this plan assumes it; W5 depends on it).
- **Q5** Node-status fork: terminal-node verb-set vs intermediate derived-and-checkpointed (W4 fold design).
- **Q6** Replay determinism boundary: what beyond policy + import + graph hash + lexicon version must be pinned (clock? ordering? external lookups?) — sizing for `freeze` and Test 6.
- **Q7** Lexicon evolution vs replay: content-addressed/frozen verbs as the mechanism; how a semantic change to a verb versions without breaking historical streams (W2).
- **Q9** Read model: confirm projections are explicitly non-authoritative (W6; lets us keep `cases`/`entity_workstreams`/`ubo_edges` as folds).
- **Q3 / Q10 / Q13** Pooled-vehicle cutoff (K-7), trusts relevant-persons-vs-UBOs, acting-in-concert — affect strategy/obligation rules but not the substrate; can lag W1–W2.
