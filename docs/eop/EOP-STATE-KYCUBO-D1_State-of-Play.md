# EOP-STATE-KYCUBO-D1 — State of Play
### The first thing any session reads. Updated at the close of every tranche, without exception.

| | |
|---|---|
| **Document** | EOP-STATE-KYCUBO-D1 |
| **Updated** | 2026-08-21 (tree cleanup) |
| **Purpose** | One place holding what is ratified, what has landed, what is next, and what is deliberately parked — so no session has to reconstruct it from chat history. |
| **Standing rule** | A tranche is not complete until this file is updated in the same diff. An out-of-date state file has caused real cost on this programme before (`docs/todo/control-plane/INDEX.md` went two versions stale while existing precisely to prevent that). |

---

## 1. The frame

**D1 — establish the facts.** Assemble the alleged set of entities and connections; prove what can be proved; construct the best possible taxonomy. **D2 — assess the facts.** Judge the taxonomy against regulation, BNY policy, legal barriers.

**D2 needs no completeness gate.** It may run against a board in any state; it simply fails, and *the failure is the work list*. That is why D1 comes first and why nothing in D1 knows policy exists. The interface is one-directional: D2 receives the taxonomy at (T, axis, scope) with its determination, assurance profile and pins, and never reaches back into raw assertions.

**Everything in this document is D1.**

## 2. Ratified law (do not re-litigate; amend by version bump)

| Document | Location | State |
|---|---|---|
| EOP-VS-CONTAINERS-001 v0.2 | `docs/eop/` (committed `63b2e2d7`) | Containers V&S: anchor ⊕ asserted edges ⊕ isolated state; scope from caller identity; status computed not stored; three movements away from verified; snapshot split; append-only by grant |
| EOP-DD-KYCUBO-TS.0 (v0.2 internal) | `docs/todo/` | Entity types as blocks; 17-pipe vocabulary; 7-phase construction sequence; §5 mis-levelling fix (nominee/state-owned/branch are not types) |
| EOP-DD-KYCUBO-TS.1 (v0.3 internal) | `docs/todo/` | The board: type→linkage matrix (§2 target, §2a source), nine moves, group-level-down scope, board-as-basket-of-references |
| EOP-DD-KYCUBO-TS.2 | `docs/todo/` | Pipe convergence: read-time classification, grow `EdgeKind` and derive `Pipe`, reject unparseable wire values |

**Standing laws worth restating, because they keep being rediscovered:** unknown is not a third state — *unknown is alleged*. Status is computed from evidence and horizon, never stored. Config declares which and where; code defines what a rule means. The entity type decides the links.

## 3. Landed

**Committed:**
- `0cb150b2 feat(kyc): D1 assembly-board type geometry (EOP-DD-KYCUBO-TS.1)`: `geometry.rs` with the 22×17 matrix, `enumerate_placement_set` re-keyed so type geometry gates ahead of the positional studs.
- `b3000a61 feat(kyc): D1 moves, pipe convergence and geometry corrections (TS.1 v0.3, TS.2)` (Stream A, tree cleanup 2026-08-21):
  - Four moves DB-executable: `kyc.subject.{assert-type, correct-type, withdraw-member, record-enquiry}`; verb universe 21→25; new `Precondition::EntityRegistered`; `correct-type` cascade computes invalidated edges.
  - `EdgeKind` grown by six (officer, mandate, membership, statutory, employment, containment); wire values 11→17; `pipe_of(edge_kind, target_type)` the sole derivation path.
  - Pipe-14 matrix correction; `matrix_is_exactly_known` re-pinned 530→546.
  - `PipeClassification.pipe` is `Option<Pipe>` — no fabricated `NonVotingShares`.
  - Teeth: `edge_kind_strategy_admission_is_exactly_known`, `op_layer_only_studs_are_exactly_known`, plus the ten TS.1 geometry gates and seven TS.2 gates.
- `63b2e2d7 docs: containers V&S, D1 state-of-play, TS design docs` (Stream B, tree cleanup 2026-08-21): `EOP-VS-CONTAINERS-001` and this file added to the tree.

**Still uncommitted, working tree** — Streams C and D (see §5), not ours; held pending separate ownership decision.

## 4. Next — one tranche, prompt written, verified not started

**The whitelist guard and third-fold stud promotion.** Verified absent from the tree on 2026-08-21: `check_preconditions` still takes only control and obligation state; `reconciled_control_edges` still filters by exclusion; none of `determination_is_unchanged_by_whitelisting`, `new_edge_kinds_are_not_traversed_as_control`, `no_stud_is_duplicated` exist.

**Why it matters:** `reconciled_control_edges` excludes only `EconomicInterest` and `Nominee`, so four strategies sweep in all six new `EdgeKind` variants undifferentiated. Containment (a scoping boundary), Employment, MembershipRights and StatutoryAuthority would be walked as generic control. Those kinds became assertable in the convergence tranche, so this is a **live hazard**, not a latent one: silent over-admission producing wrong determinations.

**Shape:** convert the exclusion filter to an explicit whitelist of exactly the kinds traversed today (exhaustive match, no catch-all, so a new variant is a compile error rather than a silent admission); widen `check_preconditions` to take `&TypeRegistryState` and promote the two hand-duplicated studs, deleting both copies. **The central gate is a before/after determination equivalence proof with a perturbation showing it can fail** — which requires a stable tree (§5).

## 5. Tree cleanup — executed 2026-08-21

**Observed** on `feat/ws-2a-authoring-plane-pin`: 85 modified, 14 deleted, 7 untracked — at least four unrelated workstreams sharing one branch. §4's central gate is a *before/after determination equivalence* proof, and "unchanged" means little when the baseline is shifting. Backup branch `backup/pre-cleanup-2026-08-21` cut at the start, pointing at pre-cleanup HEAD. One pre-existing, unrelated stash (`stash@{0}`, "pre-merge stale dsl-source/verbs generator output") found and left untouched.

**Stream A — KYC D1 (ours). Committed: `b3000a61`.**
Exactly the 13 files planned (7 modified `ob-poc-kyc-substrate` + `kyc_stream_ops.rs` + `dsl-kyc.yaml` + `kyc_pack_closure.rs`, plus 3 new test files `ts2_pipe_convergence.rs`/`kyc_ts1_moves_live.rs`/`kyc_ts2_convergence_live.rs`). `Cargo.lock` deliberately left unstaged — its 1-line diff is not a product of this stream.

**Stream B — docs. Committed: `63b2e2d7`.**
Exactly 2 files: `EOP-VS-CONTAINERS-001_Constructed-Containers_v0.1.md` and this file (pre-update version). No other `docs/eop/` or `docs/todo/EOP-*` file was modified or untracked — confirmed by explicit listing before staging.

**Stream C — instrument-matrix / trading-profile / custody retirement. Investigated, NOT committed.**
Full file list: 8 deletions (`config/verbs/custody/{instruction-profile,trade-gateway}.yaml` + their `.dsl` sources, `migrations/202501_instruction_gateway.sql`, `tests/scenarios/instruction_gateway_dag_test.dsl`, `tests/trading_matrix_materialize_test.rs`), 3 untracked (`migrations/20260820_drop_{cbu_gateway_connectivity,trading_profile_materializations}.sql`, `tests/trading_matrix_active_document_shape_test.rs`), plus ~55 modified `config/sem_os_seeds/**`, `config/packs/**`, `config/verb_schemas/**`, verb YAML, dsl-source, and crate files.

*27-vs-25 verdict (the question this phase existed for): **genuinely unrelated to Stream C.*** `every_declared_verb_has_a_registered_op` reads exactly three sources — `config/verbs/kyc/dsl-kyc.yaml`, `config/verbs/kyc/dsl-kyc-obligation.yaml`, `src/domain_ops/kyc_stream_ops.rs` — none of which Stream C touches. The extra two registered-but-undeclared ops are `screening.complete`/`screening.review-hit`, real ops registered in `kyc_stream_ops.rs` since the 2026-08-17 W5 screening-hook wiring (predates this tranche); their YAML lives in `config/verbs/screening.yaml`, a file `declared_verb_universe()`'s const list never included. Root cause is a pre-existing test-scoping gap in `kyc_pack_closure.rs` itself, compounded by Stream A's own count pin (25) going stale the moment D1 added 4 new ops (→27). Not caused by, and structurally cannot be caused by, verb-YAML deletions in a disjoint file set.

Self-consistency: looks complete, not half-finished. Both new migrations cite `EOP-PLAN-MANDATE-FIX-001` (F1, F3/B2) by name with rationale (`cbu_gateway_connectivity` was an orphan table since `trade_gateways` was never created; `trading_profile_materializations` had zero real writes because `trading-profile.materialize` could never satisfy its own parse). That plan doc exists on disk. `SQLX_OFFLINE=true cargo check -p ob-poc --lib` builds clean with Stream C still uncommitted — no dangling references.

**Stream D — `.sqlx` cache. Investigated, NOT committed.**
Seven deletions. `SQLX_OFFLINE=true cargo check --workspace --all-targets` fails with 35 errors in two disjoint groups: **6 errors** in `crates/ob-poc-web/src/{process_registry.rs,routes/forms.rs}` map exactly to 6 of the 7 deleted cache entries (`process_definitions`, `dsl_workflow_instance`, `dsl_pending_wait` ×2, `dsl_event_queue`, `form_schemas`) — those queries are still live in source, so this is a genuine offline-build break, not stale-entry cleanup. **29 unrelated pre-existing errors** in `crates/sem_os_postgres/src/integration_tests/capital_ownership_integration.rs` (share-class/capital test queries never `sqlx prepare`d for the test target) — a separate, much larger, pre-existing gap. The 7th deleted entry (`SELECT ssi_id, ssi_name...`) is correctly superseded: `custody.rs` (Stream C, uncommitted) rewrites that exact query into a JOIN — this one entry's correctness is coupled to Stream C landing, not independent.
Recommendation (not executed): regenerate the 6 process_registry/forms entries via `cargo sqlx prepare`; hold the 1 custody.rs entry with Stream C.

**Remaining after this tranche:** Streams C and D sit in the working tree, untouched, pending a separate commit/ownership decision — not blocking §4 any further than "don't attribute their noise to KYC work."

## 5a. Regression baseline with A+B committed (Streams C/D untouched)

kyc-scoped regression, exactly the 20 named `rust/tests/kyc_*.rs` binaries: **17/20 clean, 3 failing** — `every_declared_verb_has_a_registered_op`, `select_strategy_blocked_end_to_end`, `w3_w5_w6_obligation_lifecycle_end_to_end`. Identical to the previously-carried set; none new, none healed by the Stream A/B commits. `ob-poc-kyc-substrate`'s own crate suite (11 TS.1 + 7 TS.2 tests): 18/18 green.

## 6. Parked (not forgotten; each has a home)

| Item | Where it lives | Gate |
|---|---|---|
| Assurance profile, policy versioning, clearance | `EOP-DD-KYCUBO-ASSURANCE-001` (to write) | D2 — starts after D1 |
| The 49 `cross_slot_constraints` | Gameboard plan R2 | **Correction: these are D2 policy rules.** Wiring them into D1 dispatch would re-commit the conflation |
| The nine `enforce_*` predicates | Gameboard plan | D2 judgments currently inside D1's dispatch path — they move out, not get declared where they sit |
| Gameboard R1 (bypass trace) / R3 (declaration register) | `EOP-PLAN-GAMEBOARD-001` | R1's fix is hard-gated on R3 |
| Control-plane graduation (G0–G7, GM, GW) | `docs/todo/control-plane/`, plan v0.5 | Separate programme, separately owned. `DagProofInput`/`decide` widening is unowned by either |
| TS strategy semantics (which kinds are control; fund pivot; trust/foundation role enumeration; nominee piercing) | TS.2 fund design exists; others unwritten | Downstream of §4's whitelist |
| Java DOP port | `EOP-VS-CONTAINERS-001` | Parked by ruling: out of scope for all Rust changes |

## 7. Known open items inside D1

- Six new `EdgeKind` variants are assertable but not consumed by any determination traversal — pinned by `edge_kind_strategy_admission_is_exactly_known`; §4 converts this from silent over-admission to deliberate exclusion.
- Two studs hand-duplicated in the op layer and the board preview — pinned by `op_layer_only_studs_are_exactly_known`; §4 retires the duplication.
- The differential oracle in `tests/placement.rs` excludes the four new moves — documented, mirrors the `is_edge_scoped` precedent, still a coverage hole.
- **True red set (verified 2026-08-21 with only Stream A+B committed, §5a): exactly 3, unchanged from every prior tranche.**
  - `every_declared_verb_has_a_registered_op` — **confirmed genuinely unrelated to Stream C** (§5): a pre-existing `kyc_pack_closure.rs` scoping gap (`screening.*` ops declared in `config/verbs/screening.yaml`, outside the test's const source list) compounded by Stream A's own stale count pin (25, now 27). Not caused by verb-YAML deletion.
  - `select_strategy_blocked_end_to_end` — cause not yet localized this tranche; still open.
  - `w3_w5_w6_obligation_lifecycle_end_to_end` — cause not yet localized this tranche; still open.
  - Streams C and D remain uncommitted and are no longer suspected causes of the first red; whether either contributes to the second or third is still unverified.

## Change Log
| Date | Note |
|---|---|
| 2026-08-21 | Created after a sync failure: the same tranche receipts were pasted three times and neither side could say from chat alone what had landed. Records the D1/D2 frame, the four ratified documents, what is committed versus working-tree, the one outstanding tranche (verified not started against the tree, not against recollection), a four-stream tree cleanup plan, and the parked set with each item's home. Standing rule established: a tranche closes by updating this file in the same diff. |
| 2026-08-21 (tree cleanup) | Executed the §5 plan: Stream A committed (`b3000a61`), Stream B committed (`63b2e2d7`), Streams C and D investigated and left uncommitted pending separate ownership decisions. Closed the open question on the 27-vs-25 red: confirmed genuinely unrelated to Stream C's verb-YAML deletions, root-caused to a pre-existing `kyc_pack_closure.rs` scoping gap plus Stream A's own stale count pin. Recorded the true post-cleanup red set (§7, §5a) — same 3 as before, none new, none healed. Backup branch `backup/pre-cleanup-2026-08-21` cut before any staging. |
