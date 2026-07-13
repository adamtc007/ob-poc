# EOP-DESIGN-CONTROLPLANE-T11.2 — `CapabilityInvocation` / Keyed-Door Pattern, Part A: `OrchestratorContext` Split

### Basis: EOP-PLAN-CONTROLPLANE-002 v0.1 Tranche T11.2; architect ruling 2026-07-13 ("T11.2 first, pattern-before-instances")
### Status: DRAFT for ratification — design only, no code.

## 0. Sequencing note

Architect ruling (2026-07-13): T11.2 goes ahead of the `dsl_v2/` per-file trace, because T11.2 defines the keyed-door pattern that any later extraction must conform to — doing an extraction trace first would scope work against a door shape that doesn't exist yet. Within T11.2, the `OrchestratorContext` capability/metadata split goes first (Part A, this doc), because it's already the named blocker on T11.1b/slice-2's step 4, and its shape constrains what the `CapabilityInvocation` type needs to carry. The three named capability keyed-door targets (`HybridVerbSearcher`, `constellation_runtime`, `GleifClient` — Part B) follow once Part A's projection pattern is validated against a real caller.

Per the newly-ratified standing rule (ledger, 2026-07-13): this doc is built from a **per-file field census** of `OrchestratorContext` and its three external construction sites, not a directory-level assumption.

## 1. The problem, restated precisely

`OrchestratorContext` (`rust/src/agent/orchestrator.rs:63-117`, 24 fields) is constructed at 4 sites (`orchestrator.rs`'s own tests, `sequencer.rs`, `agent/harness/stub.rs`, `api/agent_service.rs`) and passed by reference into every orchestrator function, including the ~15 functions the T11.1b/slice-2 trace confirmed are pure interpretation logic (`run_sage_stage`, `run_coder_stage`, `route`, the `data_management_rewrite` family, `dsl_similarity`, `build_journey_pipeline_result`, `build_journey_selection_decision`, etc.). Those functions only ever read a handful of metadata fields off `ctx` — but the *signature* requires the whole struct, which includes four live capability handles. Moving the pure functions to `ob-poc-agent` today would hand agent-tier code those handles directly (an L1 violation), or duplicate `OrchestratorContext` across two crates (the "no duplicated logic" rule this session already applied to the legality mint).

## 2. Field census

| Field | Type | Class |
|---|---|---|
| `pool` | `PgPool` | **Capability handle** |
| `verb_searcher` | `Arc<HybridVerbSearcher>` | **Capability handle** |
| `lookup_service` | `Option<LookupService>` | **Capability handle** — wraps `Arc<dyn EntityLinkingService>` + `Arc<HybridVerbSearcher>` internally (verified: `src/lookup/service.rs:61-70`), not plain data despite the name |
| `policy_gate` | `Arc<PolicyGate>` | **Capability handle** |
| `sem_os_client` | `Option<Arc<dyn SemOsClient>>` | **Capability handle** |
| `actor` | `ActorContext` | Agent-tier data |
| `session_id` | `Option<Uuid>` | Agent-tier data |
| `case_id` | `Option<Uuid>` | Agent-tier data |
| `dominant_entity_id` | `Option<Uuid>` | Agent-tier data |
| `source` | `UtteranceSource` | Agent-tier data |
| `sage_engine` | `Option<Arc<dyn SageEngine>>` | Agent-tier engine handle — `SageEngine` is itself agent-tier (lives in `ob_poc_sage`/`ob-poc-agent`), not a capability in the L1 sense |
| `nlci_compiler` | `Option<Arc<dyn IntentCompiler>>` | Agent-tier engine handle — `IntentCompiler` is `crate::semtaxonomy_v2`, already moved to `ob-poc-agent` in T11.1b/slice 1 |
| `pre_sage_entity_kind` / `_name` / `_confidence` | `Option<String>`/`Option<f64>` | Agent-tier data |
| `recent_sage_intents` | `Vec<RecentIntent>` | Agent-tier data (`RecentIntent` is `crate::sage::RecentIntent`, itself re-exported from `ob_poc_sage`) |
| `discovery_selected_domain` / `_family` / `_constellation` | `Option<String>` | Agent-tier data |
| `discovery_answers` | `HashMap<String, String>` | Agent-tier data |
| `session_cbu_ids` | `Option<Vec<Uuid>>` | Agent-tier data |
| `agent_mode` | `AgentMode` | **CP-authoritative data** — feeds the legality mint's `fail_policy`/mode-gating directly |
| `goals` | `Vec<String>` | **CP-authoritative data** — threaded into `ContextResolutionRequest.goals`, part of the legality resolution request |
| `stage_focus` | `Option<String>` | **CP-authoritative data** — per the T11.1b/slice-2 design law's own §3 rule 3 ("the AB5 field-split follows this same line: scope/stage_focus are CP-side data because they feed a verdict") |
| `scope` | `Option<ScopeContext>` | **CP-authoritative data** — same rule; `ScopeContext` itself is confirmed plain data (`client_group_id`/`client_group_name`/`persona`, `src/mcp/scope_resolution.rs:143-150`), but its *use* is legality-determining |

**Three classes, not two.** Capability handles (5 fields) must never be held by agent-tier code. CP-authoritative data (4 fields: `agent_mode`, `goals`, `stage_focus`, `scope`) is plain data but its legality-relevant use is CP-tier by the design law already ratified for the legality grant — agent-tier code may still need to *read* these for non-legality interpretation purposes (e.g. `stage_focus` for journey routing), but must not use them to compute a verdict itself. Everything else (15 fields) is unambiguously agent-tier data.

## 3. Proposed shape: a projection, not a restructure

`OrchestratorContext` itself is **not restructured**. It is constructed at 4 call sites outside `orchestrator.rs` (`sequencer.rs`, `agent/harness/stub.rs`, `api/agent_service.rs`, plus its own test module) — splitting its literal field layout would touch all of them for no behavioral gain, and `OrchestratorContext` legitimately needs to stay CP-tier-resident regardless (it's constructed by code that already holds the real capability handles at startup/session-scope).

Instead, mirroring the `LegalityGrant` pattern already landed in T11.1b/slice 2: a new, narrow, `Clone`-able projection type, built once per turn from `&OrchestratorContext`, is what the pure-interpretation functions take instead of the full context.

```rust
/// Agent-tier projection of OrchestratorContext — everything the confirmed-
/// clean interpretation functions need, none of the capability handles.
/// Crosses into ob-poc-agent; OrchestratorContext itself never does.
pub struct AgentTurnContext {
    pub actor: ActorContext,
    pub session_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub dominant_entity_id: Option<Uuid>,
    pub source: UtteranceSource,
    pub sage_engine: Option<Arc<dyn SageEngine>>,
    pub nlci_compiler: Option<Arc<dyn IntentCompiler>>,
    pub pre_sage_entity_kind: Option<String>,
    pub pre_sage_entity_name: Option<String>,
    pub pre_sage_entity_confidence: Option<f64>,
    pub recent_sage_intents: Vec<crate::sage::RecentIntent>,
    pub discovery_selected_domain: Option<String>,
    pub discovery_selected_family: Option<String>,
    pub discovery_selected_constellation: Option<String>,
    pub discovery_answers: HashMap<String, String>,
    pub session_cbu_ids: Option<Vec<Uuid>>,
    // CP-authoritative fields carried read-only, per §2's third class —
    // present so agent-tier code can route/format, NOT to recompute legality.
    // Doc-commented as advisory-only, same posture as LegalityGrant's hint.
    pub agent_mode: AgentMode,
    pub goals: Vec<String>,
    pub stage_focus: Option<String>,
    pub scope: Option<ScopeContext>,
}

impl OrchestratorContext {
    pub(crate) fn agent_turn_context(&self) -> AgentTurnContext { /* field-by-field clone */ }
}
```

Capability handles never appear in `AgentTurnContext` — the type itself is the enforcement mechanism the design law's rule 4 asks for (grep-provable: `AgentTurnContext` has no `PgPool`/`Arc<HybridVerbSearcher>`/`Arc<PolicyGate>`/`Arc<dyn SemOsClient>` field, full stop).

## 4. Relationship to `CapabilityInvocation` (Part B)

This doc only resolves the *context* half of the L1 problem — what agent-tier code is allowed to hold as ambient state. It does not yet resolve the *call* half: today, `resolve_sem_reg_verbs`/`mint_legality_grant` etc. still call `ctx.sem_os_client`/`ctx.pool` directly from within `orchestrator.rs`, which is fine (that code stays CP-tier-resident), but the three named capabilities from T11.1b/slice 1 (`HybridVerbSearcher`, `constellation_runtime`, `GleifClient`) are consumed by code that — once `dsl_v2/`'s eventual split or any future agent-tier extraction happens — may need to *request* a capability call without holding the handle. That's `CapabilityInvocation` proper (Part B of this design), deliberately not drafted in this doc: per the architect's sequencing note, `AgentTurnContext`'s shape should land and prove itself against a real caller (recommend: retrofit it onto the already-moved `run_sage_stage`/`run_coder_stage` as the first consumers) before the second, harder half (a capability *request* type, not just a context *read* type) is designed.

## 5. Status

Draft, not ratified. No code. Awaiting review of the field census (§2) and the projection-not-restructure shape (§3) before implementation, and a decision on whether Part B (`CapabilityInvocation` proper) is scoped now or after Part A proves itself against a real move.
