# Sage ACP rip-first remediation — completion memo

> **Status:** Retrospective. Records the evidence behind the
> green-light Adam gave on 2026-05-13 that the V&S §11 hard gate
> ("SemOS rip-first remediation is complete") was satisfied at
> the point Sage ACP implementation began.
>
> **Audience:** Reviewers who later challenge the gate. This memo
> is the one-page response.
>
> **Authority:** Architectural acknowledgement, not new
> implementation. No code changes accompany this document.

## 1 — The gate

V&S §11 ("Assumptions and Dependencies", hard dependency #1):

> SemOS rip-first remediation is complete. The 8-phase Codex
> execution plan (Phases 0–8) must be at "done" before Sage ACP
> work begins. Building a runtime over a substrate with **14
> broken Class A verbs, 18 closure gaps, and a disconnected
> discovery pipeline** is the textbook second-system trap.

Three concrete failure modes the gate calls out:

1. **Class A verbs** — the foundational mutation surface the
   Sage drafter would compose against. Broken verbs at this
   tier mean drafts are unrunnable.
2. **Closure gaps** — paths through the substrate where a
   reachable state has no sanctioned transition out. Drafts
   land in dead ends.
3. **Discovery pipeline** — the chain from "user utterance" to
   "scoped verb surface at state". A disconnect here means the
   drafter cannot honour the constrained-composition guarantee
   (V&S §6.3 / O8).

## 2 — The work that closed the gate

The ob-poc team did not run an explicit 8-phase "Codex plan" by
that name. The remediation landed under multiple concurrent
plans whose commits cumulatively address every failure mode V&S
names. The mapping below is retrospective — work-by-work, not
by phase number.

### 2.1 Class A verb tier

| Remediation | Evidence |
|---|---|
| `SemOsVerbOp` single-trait plugin-verb execution; all 567 prior `CustomOperation` ops migrated YAML-first | Phase 5c-migrate (80 slices, commits referenced in memory file `project_semos_plugin_op_relocation.md`); `CustomOperation` trait, `inventory` registry, `dispatch_plugin_via_execute_json`, `dsl-runtime-macros` proc-macro, `register_custom_op` and `verify_plugin_verb_coverage*` helpers all **deleted** in slice #80 |
| Verb metadata coverage at 60.9% three-axis (758 / 1245 verbs as of Tranche 2 closure) | `project_tranche_2_closure.md`; commit `1a194d40` ("v1.2 Catalogue Platform Refinement: Tranche 1 + 2 + pub API surface cleanup") |
| `VerbExecutionPort` trait in `sem_os_core` + `PgCrudExecutor` in `sem_os_postgres` covering 12 of 14 CRUD operations; production startup activated | SemOS Execution Port Phases 0–3 |
| Catalogue Platform v1.3 cross-workspace constraints, derived-state evaluator, cascade planner, `transition_args` metadata on `VerbConfig` (87 verbs declaring it), `GatePipeline` bundle wired into orchestrator + `ob-poc-web::main` | `docs/todo/catalogue-platform-refinement-v1_3.md`; status table in `CLAUDE.md` |

Net effect: the "14 broken Class A verbs" are no longer in a
broken posture — they execute through the SemOS-owned
transaction scope under the same gate pipeline that protects
governed mutations, and their three-axis metadata is curated
against a YAML-first source of truth.

### 2.2 Closure gaps

| Remediation | Evidence |
|---|---|
| Cross-workspace state consistency: 10 phases (P1–P10), shared-atom registry + lifecycle FSM, shared-fact versioning, workspace-fact refs with staleness propagation, remediation events with FSM, external-call idempotency envelope, provider capabilities, compensation records | Commits `c7c00caa` (P1–P6) and `7a91559c` (P7–P10); `docs/annex-cross-workspace-state-consistency.md` |
| DAG reachability sweep — every authored verb traced to a sanctioned transition before it can run; failures land as `unreachable` rather than silently as orphans | Commit `2f17f290` ("[dag-reachability] complete SemOS hygiene remediation") |
| 11 canonical DAG taxonomies authored across CBU + KYC + Deal + InstrumentMatrix + BookingPrincipal + LifecycleResources + ProductMaintenance + 4 infrastructure workspaces | `rust/config/sem_os_seeds/dag_taxonomies/`; `CLAUDE.md` status table |
| Two-tier attribute model + ECIR removal — internal/external attribute split + dead-code removal on the noun-index path that was producing phantom closure suggestions | Commit `6c1eb361`; commit `de638862` |

Net effect: the substrate now provides operationally-active
state for every workspace the agent reasons about, and the
state-graph is reachability-verified.

### 2.3 Discovery pipeline

| Remediation | Evidence |
|---|---|
| `SessionVerbSurface` — single authoritative `ContextEnvelope` computed once per turn through a 7-step pipeline (Registry → AgentMode → Scope+Workflow → SemReg CCIR → Lifecycle → FailPolicy → Rank+CompositeStateBias) with `vs1:` and `v1:` dual fingerprints and FailClosed default | `rust/src/agent/verb_surface.rs`; integration at Stage 2.5 in `orchestrator_v2`; MCP tool + REST endpoint shipped |
| Constrained composition guarantee enforced server-side — pre-constrained verb search threads allowed verbs into `HybridVerbSearcher`; pruned verbs carry structured `PruneReason` | `SemOsContextEnvelope` replaced `SemRegVerbPolicy`; commit history under "PolicyGate" |
| ScenarioIndex (Tier -2A, 0.97) + MacroIndex (Tier -2B, 0.96) + ConstellationVerbIndex (Tier -0.5) — three governed-recipe layers above the verb floor, all hashed/versioned/lifecycle-FSM'd | Commit `a3231e1c` (ConstellationVerbIndex landing); CLAUDE.md "intent pipeline" surface |
| Sage ACP capability §9 v2 items 8–11 closed mid-build:<br>• Item 8: `dsl-lsp` ob-poc dep cut (commit `354bb385`)<br>• Item 9: shared-analyser consolidation into `dsl-runtime` (slices 1–7)<br>• Item 10: `sem_os_mcp` server (commits `2d2a8749` → `86fc6769`)<br>• Item 11: Coder → Drafter rename (commits `ca5ffef6`, `f91ee7b7`, `fc068cff`, `c0f21477`) | `Sage ACP capability plan §9.4` references |

Net effect: the user-utterance-to-scoped-verb-surface chain has
no remaining disconnect. The MCP-fronted knowledge surface
(`sem_os_mcp`) is the authoritative discovery transport; both
in-process and subprocess deployments share the same
`McpTransport` shape (commits `0a0b5a9c`, `e088a0d6`,
`0c32dc36`).

## 3 — Standing posture (the §O7 schema-authority claim)

V&S §O7 names "structural no-drift enforcement" as a success
criterion. As of 2026-05-13 the audit gate enforcing it is
green:

- `cargo run -p xtask -- audit` — 22 canonical
  schema-authority names tracked, 22 known mirrors held at
  status quo. Three high-priority near-duplicates (`src/sem_reg/
  {entity_type_def, relationship_type_def, verb_contract}.rs`)
  collapsed to `sem_os_core::*` re-exports in commits
  `0276851d`, `548f9b2d`, `f710808e`.
- `sem_os_core` is the sole home for all DAG primitives, FSM
  transition primitives, verb contracts, entity-type lifecycle,
  and relationship types.

## 4 — Standing posture (replay-grade audit)

V&S §6.5 / §13 names byte-equality replay across BYOK providers
and the audit emission surface. As of 2026-05-13:

- `RunbookEnvelope` JSON shape + SHA-256 hashing (commit
  `36de3297`); `xtask runbook-envelope-determinism-check`
  baseline (commit `555ed133`) green against a 6-fixture
  corpus.
- `OtlpAuditSink` + `MultiAuditSink` (commit `9bb46921`) emit
  every planning round-trip to a local JSONL sink plus an
  optional OTLP collector.
- `byok-conformance-check` corpus + harness (commits
  `19fc6c95`, `fc931feb`) — Anthropic and OpenAI both wired;
  cross-provider conformance asserted whenever both real-API
  runs match.

## 5 — Open caveats

These items do not block the gate but reviewers should know
about them:

1. **The original Codex 8-phase plan was not run by that name.**
   The team's remediation landed under concurrent named plans
   (Tranche 1 + 2 Catalogue Platform Refinement; Three-Plane
   §9.4; Cross-Workspace State Consistency; SemOS Execution
   Port). This memo's §2 mapping is the retrospective bridge.
2. **Verb-resolution accuracy parity (V&S O5, ≥83%) is still
   a separate workstream.** The baseline measurement lives in
   `acp-pack-context-parity` Phase 2; Sage ACP work does not
   wait on it, but the V&S §5 success matrix will close that
   row when the baseline measurement lands.
3. **`DerivationResult` write path** — V&S §11 names this as a
   hard dependency. It was restored under the derived-attribute
   persistence work (D0–D12 in `CLAUDE.md`'s feature ledger);
   the canonical two-table model + staleness propagation +
   `v_cbu_derived_values` projection view are operational.

## 6 — Sign-off

- Adam Cearns confirmed green on 2026-05-13. The Sage ACP
  capability plan §0 ("Locked decisions") records the
  acknowledgement: *"Rip-first gate satisfied? ✅ Yes —
  user-confirmed."*
- This memo is the artefact a future reviewer can land on if
  they ask "what was the basis for that green-light?". The
  evidence above is reproducible by walking `git log` against
  the named commits and reading the listed annexes.
