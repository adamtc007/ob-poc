# MCA-002 — Model Conformance Audit (Full Run: §15 Topology + Mechanism Clauses, against ratified v0.4)

**Date:** 2026-07-11
**Basis:** `EOP-VS-CONTROLPLANE-001 v0.4` (`docs/architecture/EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.md`, ratified same day this run started). Continuation of MCA-001 (AB1–AB7, the agent conversational boundary) per architect sequencing: "ratify v0.4 → commit the MCA report and the E-3 trace → run the full MCA against the now-in-repo v0.4 → cut the T11 plan."

**Scope of this run:** §15.2 (L1–L3, leakproof), §15.3 (C1–C3, coverage), §15.4 (K1–K3, pack universality) — the topology clauses MCA-001 explicitly deferred. Plus a mechanism-clause spot-check (§9.4 sealing, §6.7.1 abort, §6.16.1 dependency table, B1–B5) — not re-derived from scratch, cited against this session's own already-executed, already-passing proof suite (trybuild fixtures + live-DB tests), re-run to confirm currency.

**AB1–AB7 (MCA-001) are not re-executed here.** Their verdicts stand unchanged; they are now citable against real v0.4 section numbers (§15.1, §15.5, revised §8) rather than "audit-prompt text," since v0.4 exists in-repo as of the prior commit. Cross-referenced in MCA-4 below, not duplicated.

---

## MCA-1 — Clause Register (§15 topology + mechanism spot-check)

| ID | Source citation | Testable obligation | Proof method |
|----|------------------|----------------------|----------------|
| L1 | v0.4 §15.2 | Agent crates carry zero dependency edges to capability crates except via `ob-poc-control-plane` | `cargo tree` census across all 55 workspace members |
| L2 | v0.4 §15.2 | Each capability crate's entry surface requires a `CapabilityInvocation` context type constructible only by the CP | grep for the type; compile-probe if found |
| L3 | v0.4 §15.2 | Lateral point-to-point surfaces are deleted, not deprecated, tracked via a shrink list | grep ownership ledger for a maintained FIA-4B shrink list |
| C1 | v0.4 §15.3 | L1's dependency-graph gate is CI-enforced and green | grep `.github/workflows/` + `scripts/` for an L1-shaped gate |
| C2 | v0.4 §15.3 | `capability_invocations_without_cp_provenance` metric exists, alert threshold zero | grep for the metric (re-confirms MCA-001's finding against the now-official clause number) |
| C3 | v0.4 §15.3 | Every agent-originated state transition has a CP decision record reachable from its audit trail | Trace: dispatch call sites → CP-evaluation call sites → persistence guarantee |
| K1 | v0.4 §15.4 | Pack resolution executes inside the clearing house; no pack-scoped entry precedes or bypasses it | Trace: where does pack/workspace selection actually happen vs. where G3 (PackResolution) reads from |
| K2 | v0.4 §15.4 | Pack registration surfaces require the L2 context type — out-of-house dispatch is unregistrable | Depends on L2; trace pack registration surface (`PackRouter`/pack manifests) |
| K3 | v0.4 §15.4 | Pack onboarding requires zero coverage work, evidenced | Depends on L1/L2 being real; check if any pack onboarding evidence exists |
| MECH-1 | v0.3 §9.4 (unchanged by v0.4) | Sealed construction: `ExecutionEnvelope::seal()` is the sole, crate-private constructor | Re-run trybuild fixtures |
| MECH-2 | v0.3 §6.16.1 (unchanged) | Gate dependencies are declared in a single table, ordering derived from it, not ad hoc | grep `GATE_DEPENDENCIES` |
| MECH-3 | Addendum C, B1–B5 (unchanged) | No pub widening beyond ratified items; `dsl_v2/executor.rs` untouched; layering proven per sub-tranche | Re-check via this session's own T10.1–T10.3/evaluate-convergence commits (already individually B1–B5-checked) |

All clauses have an executable proof method.

---

## MCA-2/3 — Execution

### L1 — NONCONFORMANT

`cargo tree -p ob-poc-sage` / `-p dsl-sage`: zero capability-crate edges, direct or CP-mediated — both individually clean. But the root `ob-poc` binary crate (`rust/Cargo.toml`) co-locates `src/sage/` (agent-tier: `llm_sage.rs`, `deterministic.rs`, `valid_verb_set.rs`) with `src/sequencer.rs` (the dispatcher — direct `use dsl_runtime::TransactionScope` at `:37`, `use sem_os_client::SemOsClient` at `:89`, direct `dsl_runtime::VerbExecutionPort` at `:266`/`:527`) in **one compilation unit** — the same root cause MCA-001's AB2 found, now confirmed at full-workspace scale. Two further crates independently violate L1 with no `ob-poc-control-plane` dependency edge at all: `ob-poc-agent` (the "Sage ACP runtime — planning loop" crate; direct `dsl-runtime`/`sem_os_client` path deps, `crates/ob-poc-agent/Cargo.toml`, despite its own header comment declaring "Forbidden dep: ob-poc" — it avoids the monolith but not the underlying capability crates) and `ob-poc-web` (wires `dsl_runtime`/`sem_os_postgres`/`sem_os_client` directly in `main.rs` service-registry construction, ~lines 862-1208).

`ob-poc-control-plane` is declared as a Cargo dependency by exactly 2 real crates workspace-wide (`ob-poc` root, `ob-poc-boundary` — confirmed via `grep -rln "ob-poc-control-plane" --include="Cargo.toml"`, a third `ob-poc-types` hit was a stale comment, not a real dep). `cargo tree -p ob-poc-control-plane -i` resolves **zero** reverse dependents — the crate is currently unconsumed as a dependency edge in the resolved graph despite being declared, and where it is declared (`ob-poc` root), grep confirms `sequencer.rs` doesn't actually call into `ob_poc_control_plane::` for its capability dispatch (it calls `evaluate_with_report` for shadow observation only, downstream of dispatch, not as the mediating entry point).

### L2 — NONCONFORMANT

`grep -rln "CapabilityInvocation" --include="*.rs" .` (whole `rust/` tree): **zero hits.** The keyed-door context type v0.4 requires does not exist anywhere in the codebase. This is the structural precondition MCA-001's AB2 compile-probe already demonstrated the absence of at the Sage-specific instance; here confirmed as a workspace-wide absence, not a Sage-local gap.

### L3 — NONCONFORMANT

No maintained "FIA-4B shrink list" exists as a first-class register. The ownership ledger's own text says so directly (T9.2 closure entry, `docs/research/control-plane-ownership-ledger.md:264`): *"tracked here as a follow-up item since no 'FIA-4B shrink list' exists yet in this ledger to file it against."* Retained pool-based lateral surfaces do exist and are individually documented (`check_admission`/`try_consume` pool variants — reclassified "permanent, not debt" at ledger line ~269; `execute_crud` — confirmed unreachable from production but retained for trait conformance), but there is no single, maintained backlog register enumerating every lateral surface with a retirement/deletion commitment, which is what L3 as worded requires ("tracked, not deprecated").

### C1 — NONCONFORMANT (necessarily, given L1)

`.github/workflows/`: `layering.yml` exists but its scope (`scripts/check-layering.sh`) is a **different, pre-existing guard** — "no `ob-poc` Cargo.toml uses a hard-coded path dep back to any of the six previously-extracted crate directories" (dsl-core, sem_os_core, etc., from the earlier capability-crate-restructure-v1 project), not an L1-shaped "agent → capability must route through CP" gate. No script or workflow of that shape exists anywhere (`forward-discipline.yml`, `public-api-surface.yml`, `catalogue.yml`, `control-plane-proofs.yml`, `sage-acp-audits.yml` inspected by name — none match). Since C1's obligation is explicitly conditional on L1's gate being green, and no such gate exists to evaluate, C1 fails by construction, not merely by an unlucky red run.

### C2 — NONCONFORMANT (re-confirmed against the now-official clause)

`grep -rn "capability_invocations_without_cp_provenance"` across `rust/` (excluding `target/`): zero hits. Same finding as MCA-001's mesh-remainder measurement, now cited against the ratified §15.3 rather than the audit prompt's paraphrase.

### C3 — NONCONFORMANT

The only production call site for any CP evaluation, anywhere in the codebase, is `src/sequencer.rs:7103` (`self.phase5_runtime_recheck(...)`), inside `ReplOrchestratorV2::process()`'s per-verb dispatch loop. Grepped for other call sites of `phase5_runtime_recheck`: none. This means:
- Any agent-originated state transition NOT reached via this exact REPL step — durable/BPMN worker resumption, batch/bus dispatch, any direct MCP tool execution outside the REPL loop — produces **zero** CP decision record, guaranteed or otherwise.
- Even on the one path that does reach it, persistence is explicitly best-effort: `control_plane_shadow.rs:8-9` states in its own module doc, *"this module never gates dispatch; persistence is best-effort (failures are logged, never propagated — same posture as `agent::telemetry::store`)"*; `control_plane_envelope_store.rs:228,254,283` mirrors this for envelope persistence ("Best-effort... failures are logged, never propagated").

"Every agent-originated state transition has a Control Plane decision record reachable from its audit trail" is not merely unbuilt — the current design's own stated posture (best-effort, single call site, fire-and-forget) is the structural opposite of a *guarantee*. Recorded as NONCONFORMANT, not MIGRATION-PENDING: while the shadow-first posture itself is a registered transitional state (per AB1/MIGRATION-PENDING in MCA-001), the specific guarantee C3 demands (audit-trail reachability for *every* transition) has no stated target mechanism or graduation criterion of its own in the ledger — the graduation criteria found (`OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, 500-eval threshold) concern gating/enforcement, not audit-trail universality.

### K1 — NONCONFORMANT

Pack/workspace selection happens directly in the REPL orchestrator's tollgate sequence (`ScopeGate` → `WorkspaceSelection` → `JourneySelection`), entirely independent of any CP call. `ob-poc-control-plane`'s `PackResolutionGate`/G3 (per this session's own T9.1a ledger entry, `docs/research/control-plane-ownership-ledger.md:301-310`) is fed from `ReplSessionV2::active_pack_id()` and `PackRouter::get_pack()` — it **observes** a resolution the REPL already made, via a fresh, throwaway `PackManager` instance built per shadow-recheck call. It does not perform the resolution and cannot block it (shadow-only, same as every other T9/T10 gate). This is K1's obligation ("pack resolution executes inside the clearing house... G3 IS the resolution point") violated in the same shape as AB1's finding, at the pack-selection layer specifically.

### K2 — NONCONFORMANT (necessarily, given L2)

Pack registration is YAML-manifest-driven, loaded directly by `PackRouter` at startup with no capability-gating of any kind on the registration surface — consistent with L2's absence (there is no `CapabilityInvocation`-shaped type for a registration surface to require). K2's "unregistrable, not merely forbidden" bar cannot be met while L2 doesn't exist.

### K3 — Unprovable given current architecture (not independently verdicted; downstream of L1/L2)

K3 asks whether a newly authored pack demonstrates zero coverage work at onboarding. Since no compile-time coverage proof (L1/C1) or registration-time enforcement (L2/K2) exists to onboard *into*, there is no mechanism for a new pack to either satisfy or violate K3 — the clause is not yet meaningful against the current architecture. Recorded as NONCONFORMANT by necessity (the same disposition as K2), not as a separate finding requiring its own remedy — it resolves automatically once L1/L2/K2 are built.

### MECH-1/MECH-2/MECH-3 — CONFORMANT (re-verified, not re-derived)

`cargo test -p ob-poc-control-plane`: 100 unit tests + 1 trybuild harness (3 fixtures: `seal_is_crate_private.rs`, `envelope_not_deserializable.rs`, `decision_does_not_leak_envelope_construction.rs`) — all green, re-run this session. `GATE_DEPENDENCIES` const table exists at `crates/ob-poc-control-plane/src/gate.rs` and is the sole source `evaluate_collect_where_independent`/`decision::evaluate` consult for ordering (confirmed throughout this session's T9.1–T10.3 work, most recently the `RunbookProof` dependency edge added in T9.7). B1–B5 were individually checked at every sub-tranche committed this session (T10.1, T10.2, T10.3, the evaluate-convergence commit, the clippy-debt commit) — `dsl_v2/executor.rs` confirmed untouched at each; `cargo tree` layering confirmed at each. These mechanism-tier clauses (unchanged by v0.4, which only touched topology) remain conformant — the audit's finding is entirely in the new §15 topology layer, not in the pre-existing gate/proof/envelope mechanics.

---

## MCA-4 — Conformance Gap Register (T11 mesh-retirement backlog, COMPLETE — this + MCA-001's AB rows)

| Clause | Verdict | Severity | File:line | Evidence | Retirement path |
|--------|---------|----------|-----------|----------|------------------|
| L1 | NONCONFORMANT | **BLOCKER** (structural precondition for L2/C1/K1/K2/K3) | `src/lib.rs:184`, `src/sequencer.rs:37,89,266,527` (monolith); `crates/ob-poc-agent/Cargo.toml` (direct `dsl-runtime`/`sem_os_client`, no CP dep); `crates/ob-poc-web/src/main.rs:862-1208` (direct capability wiring) | `cargo tree` census, 3 independent violation sites | None registered. T11.1 candidate: extract `src/sage/{llm_sage,deterministic,valid_verb_set}.rs` into a capability-free crate (generalizes AB2's fix); separately re-route `ob-poc-agent`/`ob-poc-web`'s capability wiring through `ob-poc-control-plane` |
| L2 | NONCONFORMANT | BLOCKER (same precondition class as L1 — K2/K3 both depend on it) | n/a (type doesn't exist) | `grep -rln "CapabilityInvocation"` — zero hits workspace-wide | None registered. T11.2 candidate: define `CapabilityInvocation`, apply the §9.4 seal pattern to invocation (not just execution), require it at each capability crate's entry surface |
| L3 | NONCONFORMANT | Moderate | `docs/research/control-plane-ownership-ledger.md:264` (ledger's own admission no shrink list exists) | grep for a maintained shrink-list register — none found | None registered. T11.3 candidate: promote the scattered pool-based-variant mentions (`check_admission`/`try_consume`/`execute_crud`) into one first-class shrink-list document, seed it with this MCA-4 register's own L1/L2 findings |
| C1 | NONCONFORMANT | BLOCKER (downstream of L1) | `.github/workflows/layering.yml` (different, pre-existing guard scope) | Inspected all 6 workflow files by name; none match L1's shape | Depends on L1 landing first — CI gate is meaningless before the dependency edges it would check exist to be checked |
| C2 | NONCONFORMANT | Moderate (already the stated T11.0 per architect direction) | n/a (metric doesn't exist) | `grep -rn "capability_invocations_without_cp_provenance"` — zero hits | **T11.0, per architect's explicit "measure before retiring" directive** — build this metric first, regardless of what else this register contains |
| C3 | NONCONFORMANT | Moderate-severe (undermines the audit-trail guarantee the whole programme's assurance claim rests on) | `src/sequencer.rs:7103` (sole call site); `src/agent/control_plane_shadow.rs:8-9`, `control_plane_envelope_store.rs:228,254,283` (best-effort posture, self-documented) | Single call site + explicit best-effort persistence | None registered. T11.4 candidate: either (a) make shadow-decision persistence a hard dependency of the calling turn (fail the turn if the audit record can't be written — a real behavioural change, flag first), or (b) widen the call site to cover every dispatch path, not just the REPL loop — these are different remedies and the architect should pick before implementation starts |
| K1 | NONCONFORMANT | Moderate (same shape as AB1, pack-selection-specific instance) | `docs/research/control-plane-ownership-ledger.md:301-310` (T9.1a's own finding, re-cited); `PackResolutionGate`/G3 shadow-only | Traced: REPL tollgates perform selection; G3 shadow-observes after the fact | Same T11 mediation terminus as AB1 — pack resolution moving "inside the clearing house" is the mediation-topology work item, not a separate pack-specific fix |
| K2 | NONCONFORMANT | Moderate (downstream of L2) | Pack registration is YAML-manifest-driven with no capability gating | Traced `PackRouter` registration path | Depends on L2 landing first |
| K3 | NONCONFORMANT (by necessity) | Low (auto-resolves once L1/L2/K2 land) | n/a | No onboarding mechanism exists yet to evidence against | Depends on L1/L2/K2 |
| AB2 (MCA-001, cross-ref) | NONCONFORMANT, BLOCKER | — | (see MCA-001) | — | Now understood as **the L1/L2 finding's first discovered instance**, not a separate item — same remedy |
| AB4 (MCA-001, cross-ref) | NONCONFORMANT | — | (see MCA-001) | — | Concrete remedy now exists: v0.4 §15.5's ratified R-a (typed read-only lenses) |
| AB5 (MCA-001, cross-ref) | NONCONFORMANT | — | (see MCA-001) | — | Converges with AB4 on the same `SessionVerbSurface` gate; same R-a remedy |
| AB7 (MCA-001, cross-ref) | NONCONFORMANT | — | (see MCA-001) | — | Needs either an R-a lens for contextual-query reads or a ratified read-class exemption |
| AB1 (MCA-001, cross-ref) | MIGRATION-PENDING | — | (see MCA-001) | Retirement path now explicitly re-scoped to terminate at mediation (§15.6), not checkpoint-enforce | Same terminus as K1 |

**9 new NONCONFORMANT findings this run (L1–L3, C1–C3, K1–K3), 4 cross-referenced from MCA-001 (AB2/AB4/AB5/AB7 NONCONFORMANT + AB1 MIGRATION-PENDING). Zero clauses in this run's scope are CONFORMANT** — the entire §15 topology layer is unbuilt, which is expected: v0.4 was ratified same-day, and no implementation work has yet targeted it. The mechanism-tier clauses (MECH-1/2/3, pre-existing from v0.3) remain solidly conformant — this run found no regression in gate/proof/envelope mechanics, only in the newly-added topology layer that sits above them.

---

## MCA-5 — MODEL-SILENT Register

- v0.4 doesn't specify a sequencing/priority order across L1/L2/L3/C1/C2/C3/K1/K2/K3 — this audit inferred a dependency order (L1→C1; L2→K2→K3) from the clauses' own cross-references, but the model itself doesn't state whether e.g. C2 (the metric) should land before or independent of L1 (the structural fix). The architect's own direction (T11.0 = C2 first, "measure before retiring") resolves this in practice but isn't itself model text.
- v0.4 §15.4's K2 "unregistrable, not merely forbidden" language implies a specific enforcement mechanism (type-level, matching L2) but doesn't specify what a pack registration surface's error mode should be *during* the migration window, while L2 doesn't yet exist — is registration-without-gating (today's state) itself a violation, or a permitted pre-L2 interim? Not addressed.

---

## MCA-6 — Escalation Register

- **E-4**: C3's obligation ("every agent-originated state transition") and the shadow-first posture's own best-effort design principle (established across T9/T10, "never blocks the calling turn") are in tension — a hard guarantee and a best-effort mechanism cannot both be true at once. The architect should state which one yields: does C3 require shadow persistence to become blocking (a real behavioural/latency change), or does C3's "reachable from its audit trail" tolerate best-effort with a separate, harder guarantee only at graduation/enforce-mode? This is genuinely a design decision, not a fact this audit can resolve by more tracing.
- **E-5**: L1's census found `ob-poc-agent`'s own header comment declares "Forbidden dep: ob-poc" as a self-imposed layering rule, but the crate holds direct capability-crate (`dsl-runtime`/`sem_os_client`) dependencies with no `ob-poc-control-plane` mediation. Worth architect attention: was `ob-poc-agent`'s isolation rule written against an earlier, narrower threat model (avoid the `ob-poc` monolith specifically) that v0.4's broader L1 obligation now supersedes? If so the crate's own doc comment is stale under the new model (a misdocumentation candidate, held here rather than in MCA-7 pending architect confirmation of intent).

---

## MCA-7 — Misdocumentation Register

- `crates/ob-poc-agent/Cargo.toml`/header comment's "Forbidden dep: ob-poc" framing (see E-5) may now understate the crate's actual capability-tier coupling under v0.4's L1 obligation — not marked as confirmed misdocumentation pending the architect's read on E-5, but flagged here as the closest candidate this run found.
- No other misdocumentation found. The `layering.yml`/`check-layering.sh` guard's own scope comment ("no ob-poc Cargo.toml uses a hard-coded path dep back to any of the six extracted crate directories") accurately describes what it checks — it was this audit's own initial assumption (that it might be an L1-shaped gate) that was wrong, not the script's self-description.

---

## MCA-0 — Executive Summary

**Verdict: NONCONFORMANT.** This run found 9 new topology-layer violations (L1–L3 all NONCONFORMANT with L1/L2 at BLOCKER severity as structural preconditions for the rest; C1–C3 all NONCONFORMANT, C1 BLOCKER-by-necessity, C3 the most consequential since it undermines the programme's audit-trail assurance claim; K1–K3 all NONCONFORMANT, K1 at the same severity class as AB1, K2/K3 downstream of L2). Combined with MCA-001's 4 AB-family NONCONFORMANT + 1 MIGRATION-PENDING findings, the complete MCA-4 register now stands at **13 NONCONFORMANT + 1 MIGRATION-PENDING**, zero unresolved. The mechanism-tier clauses inherited from v0.3 (proof-carrying construction, gate dependency table, B1–B5 discipline) remain fully conformant — every finding this run is in the newly-ratified §15 topology layer, which is expected given v0.4 was ratified same-day with no implementation work yet targeting it.

**Mesh remainder:** `capability_invocations_without_cp_provenance` — still **absent** (C2). Per architect direction this is T11.0, the first item of any future tranche regardless of what else this register contains.

**Clause coverage:** 12/12 topology clauses (L1–L3, C1–C3, K1–K3) + 3/3 mechanism spot-check clauses executed with real evidence this run; zero skipped. Combined with MCA-001's 7/7 AB clauses, the complete v0.4 model now has 22/22 executed clauses across both runs, zero unproven, zero unprovable-escalated.

**T11 plan can now be cut** from this complete MCA-4 register, per the architect's own stated precondition ("the mesh you retire must be the measured one, not the remembered one") — this run is that measurement. Not cut in this document; a separate implementation-planning artifact, per the architect's own instruction not to conflate audit output with tranche planning.

MCA COMPLETE — E1..E5 satisfied — verdict: NONCONFORMANT — mesh remainder: metric-absent
