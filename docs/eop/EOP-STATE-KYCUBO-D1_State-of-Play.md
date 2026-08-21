# EOP-STATE-KYCUBO-D1 — State of Play
### The first thing any session reads. Updated at the close of every tranche, without exception.

| | |
|---|---|
| **Document** | EOP-STATE-KYCUBO-D1 |
| **Updated** | 2026-08-21 |
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
| EOP-VS-CONTAINERS-001 v0.2 | `docs/eop/` (**untracked — see §5**) | Containers V&S: anchor ⊕ asserted edges ⊕ isolated state; scope from caller identity; status computed not stored; three movements away from verified; snapshot split; append-only by grant |
| EOP-DD-KYCUBO-TS.0 (v0.2 internal) | `docs/todo/` | Entity types as blocks; 17-pipe vocabulary; 7-phase construction sequence; §5 mis-levelling fix (nominee/state-owned/branch are not types) |
| EOP-DD-KYCUBO-TS.1 (v0.3 internal) | `docs/todo/` | The board: type→linkage matrix (§2 target, §2a source), nine moves, group-level-down scope, board-as-basket-of-references |
| EOP-DD-KYCUBO-TS.2 | `docs/todo/` | Pipe convergence: read-time classification, grow `EdgeKind` and derive `Pipe`, reject unparseable wire values |

**Standing laws worth restating, because they keep being rediscovered:** unknown is not a third state — *unknown is alleged*. Status is computed from evidence and horizon, never stored. Config declares which and where; code defines what a rule means. The entity type decides the links.

## 3. Landed

**Committed** — `0cb150b2 feat(kyc): D1 assembly-board type geometry (EOP-DD-KYCUBO-TS.1)`: `geometry.rs` with the 22×17 matrix, `enumerate_placement_set` re-keyed so type geometry gates ahead of the positional studs.

**Uncommitted, working tree** (the pack-wiring, convergence and corrective tranches — see §5):
- Four moves DB-executable: `kyc.subject.{assert-type, correct-type, withdraw-member, record-enquiry}`; verb universe 21→25; new `Precondition::EntityRegistered`; `correct-type` cascade computes invalidated edges.
- `EdgeKind` grown by six (officer, mandate, membership, statutory, employment, containment); wire values 11→17; `pipe_of(edge_kind, target_type)` the sole derivation path.
- Pipe-14 matrix correction; `matrix_is_exactly_known` re-pinned 530→546.
- `PipeClassification.pipe` is `Option<Pipe>` — no fabricated `NonVotingShares`.
- Teeth: `edge_kind_strategy_admission_is_exactly_known`, `op_layer_only_studs_are_exactly_known`, plus the ten TS.1 geometry gates and seven TS.2 gates.

## 4. Next — one tranche, prompt written, verified not started

**The whitelist guard and third-fold stud promotion.** Verified absent from the tree on 2026-08-21: `check_preconditions` still takes only control and obligation state; `reconciled_control_edges` still filters by exclusion; none of `determination_is_unchanged_by_whitelisting`, `new_edge_kinds_are_not_traversed_as_control`, `no_stud_is_duplicated` exist.

**Why it matters:** `reconciled_control_edges` excludes only `EconomicInterest` and `Nominee`, so four strategies sweep in all six new `EdgeKind` variants undifferentiated. Containment (a scoping boundary), Employment, MembershipRights and StatutoryAuthority would be walked as generic control. Those kinds became assertable in the convergence tranche, so this is a **live hazard**, not a latent one: silent over-admission producing wrong determinations.

**Shape:** convert the exclusion filter to an explicit whitelist of exactly the kinds traversed today (exhaustive match, no catch-all, so a new variant is a compile error rather than a silent admission); widen `check_preconditions` to take `&TypeRegistryState` and promote the two hand-duplicated studs, deleting both copies. **The central gate is a before/after determination equivalence proof with a perturbation showing it can fail** — which requires a stable tree (§5).

## 5. Tree cleanup plan (blocks §4's equivalence proof)

**Observed 2026-08-21** on `feat/ws-2a-authoring-plane-pin`: 85 modified, 14 deleted, 7 untracked — at least four unrelated workstreams sharing one branch. §4's central gate is a *before/after determination equivalence* proof, and "unchanged" means little when the baseline is shifting. Separate before running it.

**Stream A — KYC D1 (ours). Commit first, alone.**
```
rust/crates/ob-poc-kyc-substrate/src/{fold/control.rs, geometry.rs, lexicon.rs, lib.rs, placement.rs}
rust/crates/ob-poc-kyc-substrate/tests/{placement.rs, ts1_assembly_board.rs}
rust/crates/ob-poc-kyc-substrate/tests/ts2_pipe_convergence.rs        (untracked — add)
rust/src/domain_ops/kyc_stream_ops.rs
rust/config/verbs/kyc/dsl-kyc.yaml
rust/tests/kyc_pack_closure.rs
rust/tests/{kyc_ts1_moves_live.rs, kyc_ts2_convergence_live.rs}       (untracked — add)
```
Suggested message: `feat(kyc): D1 moves, pipe convergence and geometry corrections (TS.1 v0.3, TS.2)`. Three untracked test files are part of this stream and must be `git add`ed, not left behind — they are the tranche's evidence.

**Stream B — docs. Commit second, trivially.**
`docs/eop/EOP-VS-CONTAINERS-001_Constructed-Containers_v0.1.md` (untracked) and this file. The TS documents cite the containers V&S, so leaving it untracked breaks the citation chain for anyone reading cold.

**Stream C — instrument-matrix / trading-profile / custody retirement. Not ours; commit or stash separately.**
Deletions of `config/verbs/custody/{instruction-profile,trade-gateway}.yaml` and their `.dsl` sources, `migrations/202501_instruction_gateway.sql`, `tests/scenarios/instruction_gateway_dag_test.dsl`, `tests/trading_matrix_materialize_test.rs`; untracked `migrations/20260820_drop_{cbu_gateway_connectivity,trading_profile_materializations}.sql` and `tests/trading_matrix_active_document_shape_test.rs`; plus the bulk of the modified `config/sem_os_seeds/**` and packs.
**Check before committing:** the long-standing `every_declared_verb_has_a_registered_op` red (27 vs 25) has been dismissed as unrelated across several tranches. Verb YAML is visibly being deleted in this stream — that red is now a plausible consequence, not background noise, and should be re-examined against this stream rather than carried forward again.

**Stream D — `.sqlx` cache. Decide deliberately.**
Seven deleted `rust/.sqlx/query-*.json`. These are sqlx offline query caches; deleting them by hand can break offline builds and is a likely contributor to at least one carried red. Either regenerate (`cargo sqlx prepare`) or restore — do not commit a half-deleted cache.

**Order:** A → B → C → D, then re-run the kyc-scoped regression on the clean tree and record the true red set here. Only then run §4.

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
- Three carried reds, each verified pre-existing by `git stash`: `every_declared_verb_has_a_registered_op`, `select_strategy_blocked_end_to_end`, `w3_w5_w6_obligation_lifecycle_end_to_end`. §5 Streams C and D are the prime suspects for the first and possibly the third.

## Change Log
| Date | Note |
|---|---|
| 2026-08-21 | Created after a sync failure: the same tranche receipts were pasted three times and neither side could say from chat alone what had landed. Records the D1/D2 frame, the four ratified documents, what is committed versus working-tree, the one outstanding tranche (verified not started against the tree, not against recollection), a four-stream tree cleanup plan, and the parked set with each item's home. Standing rule established: a tranche closes by updating this file in the same diff. |
