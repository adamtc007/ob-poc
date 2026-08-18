# EOP-PLAN-GAMEBOARD-001 — Board Convergence and Gate Remediation
### Tranche plan: one declaration surface, one evaluator, replayable legality

| | |
|---|---|
| **Document** | EOP-PLAN-GAMEBOARD-001 |
| **Version** | 0.1 — for ratification |
| **Owner** | Adam Cearns |
| **Binds to** | EOP-SA-OBP-001 v0.6; EOP-VS-KYCUBO-KIT-001 v0.2; EOP-PLAN-KYCUBO-KIT-001 v0.6 (the kit programme continues in parallel) |
| **Grounded in** | The 2026-08-18 cross-repo alignment review (C1–C7 register) **and** the 2026-08-18 verification pass — single-executor, raw-output, both defects CONFIRMED LIVE against a running DB |
| **Status** | Draft. R0–R4 build-ready on ratification. R5 is design-first (gate: its own design doc). |

---

## §0 The ratified frame (2026-08-18)

**DAG declares · evaluator enforces · board = declaration ⊗ state.** These are three named things, deliberately. A DAG is data; it cannot enforce. The evaluator is the code that reads the declaration against a state and returns admit/refuse. The board is the evaluated position — one per entity, per moment. This vocabulary is load-bearing: the verified `cross_slot_constraints` defect is *perfect declaration with no evaluator*, and if "the DAG is the enforcer" were the frame, that defect class would be unsayable.

**One core; SemOS is DAG++ — same source, additive metadata.** One node/edge/transition structure, one loader, one validator, one enforcement path. SemOS packs/graphs extend that structure with metadata (presentation, ranking, narration, authoring affordances); they never restate it. Two structures meaning the same thing is the machine that produced six drift instances.

**Metadata-blindness is an invariant, not a convention.** The enforcement path must never read a SemOS-only metadata field. The moment admit/refuse branches on metadata, one structure has two behaviours. Toothed in R0.

**One declaration surface, two predicate kinds.** A verb declares, in one place, both its *structural* legality (transition exists, ordering, slot states, cross-slot/cross-workspace constraints) and its *domain* legality (studs over the entity's own folded state: `SubjectRegistered`, `EdgeActive`, `StructureClassSupported`, …). One loader, one evaluator, one verdict, one reason. Two predicate KINDS is not two surfaces.

**Config declares which/where; code defines what a rule means.** Stud variants stay typed Rust. This boundary is why the drift class is nameable; widening the declarative surface never widens the semantic surface.

**Legality reads state through a provider, not a table.** The evaluator takes a state provider: `current` (Postgres slot state) and `at(T, axis)` (bitemporal fold) are two implementations of one interface. This is what keeps C7 (replayable legality) buildable — a mutable slot column can never answer "what was legal in March"; the fold can.

---

## §1 Verified facts this plan stands on (execution evidence, 2026-08-18)

1. **`cross_slot_constraints` is dead config.** 49 entries, 8+ DAG files, all `severity: error`, **zero runtime consumers** in either repo. Live consequence proven: a real CBU (VALIDATION_PENDING, zero evidence rows, NULL commercial client) reached VALIDATED through the real CRUD dispatch path. *(Independently confirmed by grep on the tree: many YAML hits, no Rust hits.)*
2. **`requires_states` domain-key resolution is broken system-wide.** All 5 structurally-reachable transitions on a live `entity_workstream` (SCREEN) refused with `no_slot_mapping` — not a business refusal — through the real chokepoint with production config loaded. With `OB_POC_LIFECYCLE_GATE_MODE=fail-open` all 5 pass unconditionally. **Scope: 60 of 87 `requires_states`-declaring verbs**, i.e. any hyphenated/multi-word domain key; `cbu`/`deal` work by accident (domain string == registered self-slot).
3. **The declarations themselves are unvalidated.** `screening.run` declares `requires_states: [PENDING, VERIFY, workstream_open]`; the entity's real state is `SCREEN`. **Fixing the key mapping will not restore function — it exposes a second layer of stale declarations.** This is why R3 gates R1's enforcement.
4. **Legality is not time-parameterised anywhere.** No legality function in either repo takes a time/axis parameter. State is replayable (T5 landed `recover_determination_bitemporal`); the legal move SET at a past moment is not.
5. **Two evaluators exist in ob-poc today.** `DagRegistry`+`GateChecker` (Postgres slot state, per-candidate probe) and the kit's `enumerate_placement_set` (folded event stream, true enumerator). R0/R5 converge them; the verified defects are all on the first.
6. **Teeth already pinning defects 1–2:** `rust/tests/domain_pack_config_qualification.rs` — `cross_slot_constraints_declared_but_zero_consumers_is_the_known_gap`, `requires_states_domain_key_resolution_gap_is_exactly_known`. Both RED-honest, both proven sensitive by perturb-and-revert. They flip from pinning gaps to pinning closure as R1/R2 land.

---

## §2 Tranches

### R0 — Core convergence (one structure, one loader, one evaluator)
**Scope:** collapse the duplicated declaration/evaluation machinery onto one core: a single node/edge/transition model, one loader, one validator, one enforcement entry point. SemOS pack/graph metadata attaches to that structure additively — same source, no restatement. Existing consumers adapt to the core; the core adopts nothing domain-specific.
**Gate tests (RED first):** `enforcement_is_metadata_blind` — the admit/refuse path's inputs provably exclude SemOS-only metadata fields (structural: the evaluator's signature/type surface, not a grep); `one_loader_one_structure` — every declaration surface loads through the single loader (no second parse path); `evaluator_is_single_entry` — every admit/refuse verdict in the system routes through one function (the chokepoint made provable, closing the class that produced defects 1–2); existing DAG/gate/kit tests all still green.
**Fences:** no behaviour changes to any rule; no fixes to defects 1–2 (those are R1/R2); no new config surface. Pure convergence.
**Routing:** Sonnet, after a recon table of every current declaration/evaluation path.

### R1 — Gate chokepoint: bypass trace, then fix
**Phase 0 (the open question, answered before any fix):** for a sample of the 60 affected verbs, trace EVERY write path that can reach their target tables — gated dispatch, `GenericCrudExecutor`, `dsl_v2`, direct store calls — and show which production actually uses. Three possible answers, each branching the tranche:
- **(a) traffic goes through the gate** → fix domain-key resolution (map every real domain key, or normalise at one place), plus the literal-string comparison bug.
- **(b) a bypass exists** → the tranche pivots: closing the bypass outranks fixing the gate. This would be **instance seven** of the drift class and the most serious finding yet — a chokepoint that isn't one.
- **(c) the flows are genuinely dead** → the fix is still made, but the tranche additionally reports which declared surface is unexercised (a K-G2-shaped finding).
**Gate tests (RED first):** the existing `requires_states_domain_key_resolution_gap_is_exactly_known` tooth flips from pinning the gap to pinning closure (conscious edit, red→green evidence); `every_declared_domain_key_resolves` (all 87, not just the 60); a live-path test proving a real transition on a real entity is admitted for the right reason and refused for the right reason.
**HARD GATE:** enforcement of the repaired gate does not go live until **R3** has validated the declarations (fact 3). Landing the fix without R3 trades a known-broken gate for an unpredictable one.
**Routing:** Sonnet for Phase 0 + the fix; the branch choice is mechanical (the trace decides it).

### R2 — `cross_slot_constraints`: staged report-only → enforce
**Ruled:** staged. **Stage 1** — index and evaluate all 49 constraints, log every violation with entity/rule/severity, **refuse nothing**. Run against real data; produce the violation census. **Stage 2** (separate, after Adam reads the census) — flip to enforcing, `severity: error` refuses.
**Rationale:** 49 dormant `severity: error` rules going live in one step would start refusing operations that have been quietly non-compliant for an unknown period. The census is the ratification input for stage 2 — and is itself a compliance artifact worth having.
**Gate tests (RED first):** stage 1 — `every_declared_cross_slot_constraint_is_evaluated` (the dead-config tooth flips to pinning closure), `report_only_refuses_nothing` (structural: no refusal path wired in stage 1); stage 2 — per-rule admit/refuse pairs, and the `cbu.confirm` case from fact 1 provably refused.
**Routing:** Sonnet both stages; the census is Adam's ratification gate between them.

### R3 — Declaration validation register (rulings tranche; gates R1's enforcement)
**Why:** fact 3. The wiring bug has been masking whether the declared geometry is even correct. `screening.run`'s declared states don't include the state its entity actually occupies — and nothing has ever exercised that comparison.
**Artifact:** a register, one row per affected verb: declared `requires_states` → the states actually reachable for that entity type → verdict (CORRECT / STALE / WRONG-VOCABULARY / UNKNOWN) → proposed correction. Drafted by Opus from the DAG + live state vocabulary; **ratified by Adam** (these are domain rulings, same pattern as the stud-geometry matrix); encoded by Sonnet.
**Gate tests (RED first):** `declared_states_are_real_states` — every value in every `requires_states` list exists in that slot's actual state vocabulary (this alone would have caught the `workstream_open` case); the per-verb pairs for corrected rows.
**Routing:** Opus drafts · Adam ratifies · Sonnet encodes.

### R4 — Time-parameterised legality (closes C7; the distinctive capability)
**Scope:** the evaluator takes a **state provider** — `current` (Postgres slot state) and `at(T, axis)` (bitemporal fold) as two implementations of one interface — plus a **ruleset version pin** so a past answer is reproducible: declaration version (content hash), evaluator version, and the kit/manifest hash the kit programme already computes. Completes the `DeterminationPin` gap T5 flagged (viewer, axis, kit-version, constructor-version).
**Gate tests (RED first):** `legal_set_at_past_time_is_reproducible` — enumerate at (T, axis), mutate present state, re-enumerate at the same (T, axis), assert bit-identical; `legality_pins_its_ruleset` — a declaration change makes a past enumeration explicitly version-mismatched rather than silently different; `provider_swap_is_transparent` — same declaration + equivalent state ⇒ same verdict across providers.
**Note:** buildable only because legality reads a provider, not a table (§0). On mutable slot columns alone this tranche is impossible.
**Routing:** Sonnet; the provider interface is a half-page design for sign-off first.

### R5 — Declaration convergence: studs into the DAG surface (DESIGN-FIRST)
**Scope:** migrate the lexicon-declared domain studs (the 18 ratified matrix rows + the 10 typed variants from T6.1) into the single DAG declaration surface, so one file declares both predicate kinds for a verb. Stud *variants* stay typed Rust (§0 boundary) — only the declaration moves.
**Gate:** this tranche does **not** start from this plan. It needs its own design doc first, because the migration ripples into the lexicon manifest hash, the closure teeth, and T4's session kit-pinning — all of which currently key off the lexicon. Design doc answers: what the merged declaration looks like per verb; what the pin/hash becomes; how the teeth follow; migration order and whether both surfaces coexist during it.
**Gate tests (RED first, once designed):** `one_declaration_surface` (no verb declares legality in two places); the full existing stud pairs re-run green post-migration; manifest/pin tests updated consciously.
**Routing:** Opus design doc · Adam ratifies · Sonnet migrates.

---

## §3 Dependency graph

```
R0 (core convergence) ──┬─► R1 Phase 0 (bypass trace) ─► R1 fix ──┐
                        │                                          ├─► enforcement live
                        │   R3 (declaration register) ─────────────┘   (R1 HARD-GATED on R3)
                        ├─► R2 stage 1 (report-only) ─► census ─► R2 stage 2 (enforce)
                        └─► R4 (provider + time axis + ruleset pin)
R5 (studs → DAG) : design doc FIRST; independent of R1–R4; do not start from this plan
```
Parallel-safe: R2, R3, R4 against each other. R1's *fix* waits on R3; R1's *trace* does not.
The kit programme (EOP-PLAN-KYCUBO-KIT-001 v0.6: T6.2–6.4, TS.0–4, T7) continues in parallel and is unaffected except by R5, which is design-gated.

## §4 Out of scope (fenced)
bpmn-lite changes (its realization is the reference, not the patient); the SLM/ramp work (charter-gated, kit plan); `SqlPredicateResolver`'s silent-fail-on-unparseable (recorded as a known gap — a one-line loudness fix, but it belongs to whoever owns that resolver, not to a convergence tranche); any fix landing without its tooth.

## Change Log
| Version | Date | Note |
|---|---|---|
| 0.1 | 2026-08-18 | Initial plan. Frame ratified: DAG declares · evaluator enforces · board = declaration ⊗ state; one core with SemOS as DAG++ (same source, additive metadata); metadata-blind enforcement as an invariant; one declaration surface carrying two predicate kinds; config declares which/where, code defines meaning; legality reads a state provider, not a table. Six tranches: R0 core convergence, R1 gate chokepoint (bypass-trace-then-branch, hard-gated on R3), R2 cross_slot_constraints staged report-only→enforce, R3 declaration validation register (rulings), R4 time-parameterised legality closing C7, R5 stud→DAG convergence (design-first). Grounded in the verification pass: both defects CONFIRMED LIVE, scope 60-of-87 verbs, declarations themselves unvalidated. |
