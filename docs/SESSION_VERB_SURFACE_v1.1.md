# Session Verb Surface — Architecture, Capabilities & Remediation Plan

> **Version:** 1.1 (peer-reviewed)  
> **Date:** 28 February 2026  
> **Status:** Draft — Post Peer Review  
> **Authors:** Adam TC / Claude Opus 4.6  
> **Platform:** ob-poc (BNY Mellon Enterprise Onboarding)  
> **Commit:** `e440bfd` (74-verb management agent surface)  
> **Reviewer:** External peer review (v1.0 → v1.1 delta)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 28 Feb 2026 | Adam TC / Claude Opus 4.6 | Initial draft from codebase audit of commit `e440bfd` |
| 1.1 | 28 Feb 2026 | Adam TC / Claude Opus 4.6 | Peer review patches: dual fingerprint story, `VerbSurfaceFailPolicy`, ABAC pipeline position, multi-reason exclusions, embedding search mitigation. Two new gaps (G7, G8), three new risks, updated acceptance criteria. See §9.2 for full change log. |

---

## 1. Executive Summary

The ob-poc platform manages **642 DSL verbs** across **125 YAML domain files**, providing a comprehensive command language for institutional client onboarding, KYC screening, entity management, and semantic registry governance. These verbs are the platform's operational surface — every user action, agent decision, and workflow transition is expressed through verb invocation.

The platform has implemented world-class *post-resolution* governance: SemReg's Context-Constrained Intent Resolution (CCIR) pipeline evaluates each verb against ABAC policies, entity-kind subject constraints, taxonomy membership, governance tiers, and trust classes. A cryptographic fingerprint (`AllowedVerbSetFingerprint`) protects against TOCTOU races, and AgentMode gating enforces Research/Governed boundaries.

The most recent commit (`e440bfd`) substantially advanced the architecture by adding: (a) a `build_verb_profiles` function that produces structured `VerbProfile` payloads from the ContextEnvelope, (b) a React VerbBrowser component for progressive-disclosure verb navigation, (c) SemReg-sourced `/commands` and `/options` routing, and (d) pre-constrained verb search via `with_allowed_verbs()` in the IntentPipeline.

However, a key architectural gap remains: the platform lacks a **SessionVerbSurface** — a first-class type that composes all governance layers into a single, queryable, time-varying set that answers the question: *"What can I do right now, given my session state, loaded CBUs, active entity, workflow phase, and role?"* This paper defines the vision, audits current capabilities, and provides a prioritised remediation plan.

---

## 2. Vision & Scope

### 2.1 Vision Statement

> At every point in a session, agents and users should have immediate, deterministic access to the exact set of verbs that are legal, contextually relevant, and semantically ranked for their current state — without issuing a command, receiving an error, or consulting documentation.

The verb surface is the platform's contract with its users. Like a compiler's type system prevents invalid programs, the verb surface should prevent invalid interactions. A user should never select a verb that will be rejected, and an agent should never propose an action the governance stack will deny.

### 2.2 Design Principles

**Principle 1: Proactive over Reactive.** Governance should shape what is offered, not reject what is attempted. The user experience moves from "try → fail → retry" to "see → choose → succeed".

**Principle 2: Single Source of Truth.** One function computes the verb surface. The MCP tool, the React UI, the `/commands` response, and the IntentPipeline pre-filter all consume the same output. No parallel code paths, no divergence.

**Principle 3: Deterministic & Dual-Fingerprintable.** The verb surface at time T is a pure function of session state at time T. Two fingerprints track staleness at different layers: `semreg_fingerprint` (CCIR-internal, for TOCTOU and audit) and `surface_fingerprint` (final visible surface, for UI refresh and agent awareness). See §2.4 Safety Invariants.

**Principle 4: Composable Governance.** The surface computation composes existing layers (SemReg CCIR, AgentMode, ABAC, lifecycle state, workflow phase) — it does not replace or duplicate them. Each layer is a filter in a pipeline.

**Principle 5: Progressive Narrowing.** The verb count decreases as context sharpens. Session start exposes ~20 verbs; loading a CBU adds ~80; opening a KYC case adds ~120 but only in that domain; focusing an entity in lifecycle state "draft" narrows to ~40 valid transitions.

**Principle 6: Fail-Safe by Default.** No code path may return verbs from the full RuntimeVerbRegistry unless a `VerbSurfaceFailPolicy` explicitly permits it. Governance failure narrows the surface, it does not expand it.

### 2.3 Scope Boundaries

| In Scope | Out of Scope |
|----------|-------------|
| SessionVerbSurface Rust type & computation | Replacing SemReg's CCIR pipeline |
| session_verb_surface MCP tool | Custom RBAC/ABAC engine (uses existing) |
| Pre-filter integration into IntentPipeline | Verb execution engine changes |
| `/commands` wired to live surface | New verb domain definitions |
| React command palette + verb picker | BPMN workflow engine modifications |
| Surface delta notifications on state change | Agent loop orchestration changes |
| Lifecycle-state-aware verb narrowing | Cross-session verb surface federation |
| Dual fingerprint system | Multi-tenant verb isolation |
| VerbSurfaceFailPolicy | Replacing existing fail-open/fail-closed SemReg policy |

### 2.4 Safety Invariants

These invariants are governance-critical and must hold across all code paths:

**SI-1: No ungoverned expansion.** If SemReg is unavailable or returns an empty set, the surface must not fall back to the full 642-verb RuntimeVerbRegistry. The `VerbSurfaceFailPolicy` controls behaviour:

```rust
pub enum VerbSurfaceFailPolicy {
    /// Default. Surface contains only always-safe domains:
    /// session.*, agent.*, view.* (~30 verbs).
    FailClosed,
    /// Dev/test only. Gated by config flag `VERB_SURFACE_FAIL_OPEN=true`.
    /// Full registry returned with `governance_tier: "ungoverned"` tagging.
    /// Must never be enabled in production.
    FailOpen { tag_ungoverned: bool },
}
```

**SI-2: Dual fingerprints never conflated.** `semreg_fingerprint` is the SHA-256 from CCIR's `AllowedVerbSetFingerprint` (hash of sorted allowed FQNs only). `surface_fingerprint` is the SHA-256 of the final visible surface including all post-CCIR filters. They are distinct fields, tracked independently. UI/agent delta notifications key off `surface_fingerprint`, not `semreg_fingerprint`.

**SI-3: Exclusion reasons are additive.** A verb excluded from the surface may be excluded for multiple simultaneous reasons (e.g., AgentMode + lifecycle state + workflow phase). The `ExcludedVerb` type carries `Vec<SurfacePrune>`, not a single reason.

### 2.5 Actors & Stakeholders

**Onboarding Agents (LLM-driven):** Need pre-filtered verb candidates so semantic search returns only legally invocable verbs. Reduces false matches, eliminates "verb not allowed" errors, and improves intent resolution accuracy.

**Human Operators:** Need a command palette (`/commands`, VerbBrowser) that shows exactly what they can do, with explanations for why some verbs are unavailable. Progressive disclosure prevents information overload.

**Compliance Officers:** Need audit evidence that verb surfaces were correctly computed. Dual fingerprints, PruneReason annotations, and IntentTrace provenance provide the compliance trail.

**Platform Engineers:** Need a composable, testable abstraction. SessionVerbSurface should be constructible in unit tests with mock session state, verifiable without a running database.

---

## 3. Current Capabilities Audit

This section provides a factual audit of the codebase as of commit `e440bfd`. Each component is assessed against its contribution to the verb surface problem. Status is: **IMPLEMENTED** (complete and wired), **PARTIAL** (exists but gaps remain), or **MISSING** (not yet built).

### 3.1 Capability Matrix

| Component | Status | What It Does | Verb Surface Role |
|-----------|--------|-------------|-------------------|
| `UnifiedVerbRegistry` (verb_registry.rs) | IMPLEMENTED | Singleton holding 642 parsed YAML verbs. Query by domain or FQN. | Phone book. Knows every verb that exists. No session awareness. |
| `SemReg CCIR Pipeline` (context_resolution.rs) | IMPLEMENTED | 9-step resolution: subject match, tier filter, **ABAC** (Step 5), taxonomy, rank scoring. Produces `VerbCandidate[]`. | Core governance filter. Entity-scoped, evidence-mode-aware. Returns allowed verbs + rank scores. |
| `ContextEnvelope` (context_envelope.rs) | IMPLEMENTED | Carries `allowed_verbs`, `pruned_verbs` (with `PruneReason`), fingerprint, evidence gaps, verb contracts. | Rich governance output. Has everything needed for surface computation. |
| `AgentMode Gating` (agent_mode.rs) | IMPLEMENTED | Research vs Governed mode. Prefix-based domain allow/deny. Applied in orchestrator Stage A.3. | Binary mode filter. Blocks publish verbs in Research; blocks authoring in Governed. |
| `with_allowed_verbs()` (intent_pipeline.rs) | IMPLEMENTED | Pre-constrains HybridVerbSearcher to SemReg-approved verbs before semantic search. | Critical: search only returns legal verbs. Eliminates "verb not allowed" after match. |
| `build_verb_profiles()` (agent_routes.rs) | IMPLEMENTED | Builds `VerbProfile[]` from ContextEnvelope + AgentMode filter. Falls back to full registry if SemReg empty. | Produces structured payload for UI. **Falls back to full registry if SemReg empty — violates SI-1.** |
| `/commands` → SemReg (agent_routes.rs) | IMPLEMENTED | Slash commands route through `resolve_options()` → `resolve_sem_reg_verbs()`. | User-facing verb discovery now uses live governance. |
| `VerbBrowser` (React) (VerbBrowser.tsx) | IMPLEMENTED | Progressive disclosure: domain cards → verb list → arg details. Click inserts s-expr into chat. | First UI for verb discovery. Domain-grouped, searchable. |
| `SemOs Workflow Selection` (agent_routes.rs) | IMPLEMENTED | Decision packet with 4 workflow choices mapping to `stage_focus` values. | Sets workflow context that should narrow verb surface. |
| `IntentTrace` (orchestrator.rs) | IMPLEMENTED | Captures pre/post filter verb candidates, SemReg policy, `agent_mode_blocked`, fingerprint. | Complete audit trail AFTER resolution. Not queryable BEFORE. |
| `Verb Prominence` (context_resolution.rs) | IMPLEMENTED | Rank scoring from ViewDef domain match, entity-kind boost, taxonomy, relationships. | Orders verbs by relevance. Domain match = 0.8, cross-domain = 0.3, entity-kind boost +0.15. |
| `Governance Gates` (sem_os_core/gates/) | IMPLEMENTED | 4 technical gates including `verb_surface_disclosure` (I/O surface completeness). | Ensures verb contracts declare all attributes they read/write. |
| `SessionVerbSurface` type | MISSING | No first-class type composing all layers into single queryable surface. | The missing abstraction this paper addresses. |
| `session_verb_surface` MCP tool | MISSING | No MCP tool returning context-aware verb set. | Agent cannot ask "what verbs are available to me right now?" |
| Lifecycle State Narrowing | MISSING | Verb YAML has `lifecycle.requires_states` but not aggregated into valid-transition queries. | Entity in state "draft" should only show draft-valid transitions. |
| Surface Delta Notifications | MISSING | No event emitted when verb surface changes. | Agent unaware surface changed. |
| Workflow → Verb Filter | PARTIAL | `stage_focus` is set but not threaded to verb filtering in CCIR pipeline. | Selecting "KYC" workflow should narrow to kyc.* + session.* domains. |
| `VerbSurfaceFailPolicy` | MISSING | No explicit fail-policy. `build_verb_profiles()` silently falls back to full registry. | Governance gap: SI-1 violation. |

### 3.2 Data Flow: Current State

**Stage 0 — Session Context Assembly.** The orchestrator builds an `OrchestratorContext` from the `UnifiedSession`, including: pool, verb_searcher, agent_mode, scope (loaded CBUs), sem_os_client, and actor.

**Stage 1 — SemReg Resolution.** `resolve_sem_reg_verbs()` calls the CCIR pipeline in `sem_os_core`. The pipeline evaluates all published verb contracts against the current subject (entity kind, taxonomy memberships, relationships), applying **ABAC at Step 5**, tier, trust class, and evidence mode filters. Output: `ContextEnvelope` with `allowed_verbs` HashSet, `VerbCandidateSummary[]`, `pruned_verbs`, and SHA-256 fingerprint.

**Stage 2 — Pre-Constrained Search.** The IntentPipeline is constructed with `with_allowed_verbs(envelope.allowed_verbs)`. When `HybridVerbSearcher.search()` runs, it filters results against this set AFTER normalisation but BEFORE returning candidates. Semantic search still computes similarity scores against all 642 embeddings, but only SemReg-approved verbs survive.

**Stage 3 — Post-Filter Safety Net.** The orchestrator re-applies the AllowedSet filter (belt-and-suspenders), then applies AgentMode gating (Stage A.3). Blocked verbs are logged to `IntentTrace.agent_mode_blocked_verbs`.

**Stage 4 — Verb Selection & DSL Generation.** The top-ranked surviving candidate is selected. The LLM generates a DSL s-expression. The verb executor validates lifecycle preconditions at execution time.

> **Observation:** The governance is reactive — applied after the user speaks. The `/commands` and VerbBrowser paths run the same pipeline (`resolve_options` → `resolve_sem_reg_verbs`), producing a correct verb set, but this computation is triggered by explicit user request, not maintained as a session-resident, always-current surface.

---

## 4. Gap Analysis

### 4.1 Gap G1: No SessionVerbSurface Abstraction

**Severity: P0** | **Impact:** All consumers

There is no Rust type that represents the composed, session-aware verb surface. The function `f(session_state, agent_mode, loaded_cbus, active_entity, workflow_phase, actor_role) → Set<VerbDef>` does not exist as a named, testable, cacheable unit. Instead, the verb set is computed ad-hoc in three places: (a) `build_verb_profiles()`, (b) `generate_options_from_envelope()`, and (c) the orchestrator's pre-filter + post-filter chain. These share the same underlying `ContextEnvelope` but diverge in fallback behaviour and output format.

**Risk:** Divergence between what `/commands` shows and what the orchestrator allows.

### 4.2 Gap G2: Verb Surface Not Session-Resident

**Severity: P0** | **Impact:** Agent, UI, Audit

The verb surface is computed on-demand, not maintained as part of the session's live state. The agent doesn't know the surface changed after a CBU load unless it explicitly re-queries. No diff, no event, no auto-refresh.

**Risk:** Stale UI state. Agent proposes verbs from an expired surface.

### 4.3 Gap G3: Lifecycle State Not Aggregated

**Severity: P1** | **Impact:** Entity transitions

Verb YAML definitions include `lifecycle.requires_states` and `lifecycle.transitions_to` fields. These are evaluated at execution time by the verb executor — not at surface-computation time. No aggregation of "given entity X in state Y, which verbs are valid transitions?"

**Risk:** User sees verbs that will fail at execution because the entity is in the wrong lifecycle state.

### 4.4 Gap G4: Workflow Phase Not Threaded to Verb Filtering

**Severity: P1** | **Impact:** Progressive narrowing

`session.context.stage_focus` is set by workflow selection but not passed into the CCIR pipeline as a filter. The ContextEnvelope is unaware of workflow phase.

**v1.1 note:** Workflow filtering should be applied *before* CCIR to reduce the candidate set that CCIR evaluates — both a performance optimisation and a correctness improvement.

**Risk:** Selecting "KYC" workflow still shows all 642 verbs (filtered by SemReg entity context only, not by workflow domain scope).

### 4.5 Gap G5: No MCP Tool for Agent Surface Query

**Severity: P1** | **Impact:** Agent autonomy

The existing `registry.verb-surface` MCP tool (`sem_reg_verb_surface`) returns inputs/outputs/preconditions for a *single named verb* (requires `verb_fqn` parameter). No MCP tool returns the *session-wide* verb surface.

**Risk:** Agents must rely on `/commands` text parsing or embedded context.

### 4.6 Gap G6: Search Hits Against Full Embedding Space

**Severity: P2** | **Impact:** Search quality

While `with_allowed_verbs()` post-filters search results, the BGE-small-en-v1.5 embedding search still computes similarity against all 642 verb embeddings. A high-scoring match against an unavailable verb is discarded, potentially hiding a lower-scoring but legal verb.

**v1.1 mitigation (cheaper alternative before sub-index):** When filtered results are sparse (e.g., `allowed_verbs.len() < topK * 0.3`), double the effective `topK` before filtering. This is a single-line change to `HybridVerbSearcher::search()` and handles most recall loss without index reconstruction.

**Risk:** Reduced search recall on narrowed surfaces.

### 4.7 Gap G7: Fingerprint Ambiguity *(v1.1 — new)*

**Severity: P0** | **Impact:** UI refresh, Agent awareness

The existing `AllowedVerbSetFingerprint` is computed from sorted allowed FQNs only (SHA-256 in `context_envelope.rs`). It does not incorporate AgentMode, workflow phase, lifecycle state, or loaded CBU scope. A workflow phase change that filters verbs via a post-CCIR layer would *not* change the SemReg fingerprint, even though the visible surface changed.

**Risk:** UI keying refresh off `semreg_fingerprint` would miss surface changes from non-SemReg layers.

### 4.8 Gap G8: Fail-Open Accidental Disclosure *(v1.1 — new)*

**Severity: P0** | **Impact:** Governance integrity

`build_verb_profiles()` (line ~2060, agent_routes.rs) falls back to the full `RuntimeVerbRegistry` when SemReg returns an empty verb set, with `preconditions_met: true` and `governance_tier: "operational"` defaults. Similarly, `generate_options_from_envelope()` has the same fallback. These are silent governance bypasses — the user sees all 642 verbs with no indication that governance was not applied.

**Risk:** A verb shown as available in the full-registry fallback may be denied when the orchestrator's stricter fail-closed policy kicks in.

---

## 5. Target Architecture

### 5.1 SessionVerbSurface Type

The central new type is `SessionVerbSurface`, defined in `rust/src/agent/verb_surface.rs`.

```rust
pub struct SessionVerbSurface {
    /// Verbs the user/agent can invoke right now.
    pub available: Vec<SurfaceVerb>,
    /// Verbs excluded with per-layer reasons.
    pub excluded: Vec<ExcludedVerb>,
    /// CCIR-internal fingerprint (hash of SemReg allowed FQNs only).
    /// Used for TOCTOU checking and audit provenance.
    pub semreg_fingerprint: AllowedVerbSetFingerprint,
    /// Final surface fingerprint (hash of available FQNs + filter context).
    /// Used for UI refresh and agent staleness detection.
    pub surface_fingerprint: SurfaceFingerprint,
    /// Snapshot of session state at computation time.
    pub session_snapshot: SessionSnapshot,
    /// Available verbs grouped by domain.
    pub by_domain: BTreeMap<String, Vec<SurfaceVerb>>,
    /// Available verbs grouped by category.
    pub by_category: BTreeMap<String, Vec<SurfaceVerb>>,
    /// When this surface was computed.
    pub computed_at: chrono::DateTime<Utc>,
    /// Active workflow phase (if any).
    pub workflow_phase: Option<String>,
    /// The fail-policy that was in effect.
    pub fail_policy: VerbSurfaceFailPolicy,
}
```

```rust
/// Final surface fingerprint — includes all filter context, not just FQNs.
pub struct SurfaceFingerprint(pub String);

impl SurfaceFingerprint {
    pub fn compute(
        available_fqns: &[String],
        agent_mode: &AgentMode,
        stage_focus: Option<&str>,
        focused_entity: Option<(&Uuid, &str)>,  // (id, lifecycle_state)
        loaded_cbu_ids: &[Uuid],
    ) -> Self {
        let mut hasher = Sha256::new();
        let mut sorted_fqns = available_fqns.to_vec();
        sorted_fqns.sort();
        for fqn in &sorted_fqns {
            hasher.update(fqn.as_bytes());
            hasher.update(b"\n");
        }
        hasher.update(format!("mode:{}", agent_mode).as_bytes());
        if let Some(sf) = stage_focus {
            hasher.update(format!("workflow:{}", sf).as_bytes());
        }
        if let Some((id, state)) = focused_entity {
            hasher.update(format!("entity:{}:{}", id, state).as_bytes());
        }
        let mut sorted_cbus = loaded_cbu_ids.to_vec();
        sorted_cbus.sort();
        for cbu in &sorted_cbus {
            hasher.update(format!("cbu:{}", cbu).as_bytes());
        }
        Self(format!("v1:{:x}", hasher.finalize()))
    }
}
```

```rust
pub struct SurfaceVerb {
    pub fqn: String,
    pub domain: String,
    pub description: String,
    pub sexpr_signature: String,
    pub args: Vec<VerbArgProfile>,
    pub rank_score: f64,
    pub governance_tier: GovernanceTier,
    pub preconditions_met: bool,
    pub surface_reason: SurfaceReason,
}

pub enum SurfaceReason {
    AlwaysAvailable,                       // session.*, agent.*, view.*
    ScopeMatch { cbu_ids: Vec<Uuid> },     // CBU loaded
    WorkflowPhase { phase: String },       // stage_focus match
    EntityFocus { entity_kind: String },   // entity-kind constraint
    LifecycleValid { from_state: String }, // valid transition
    SemRegAllowed { view_fqn: String },    // view-based prominence
}
```

```rust
/// A verb excluded from the surface, with ALL reasons it was excluded.
/// A single verb may be excluded by multiple layers simultaneously.
pub struct ExcludedVerb {
    pub fqn: String,
    pub domain: String,
    pub prune_reasons: Vec<SurfacePrune>,
}

/// A single prune decision from one governance layer.
pub struct SurfacePrune {
    /// Which layer excluded this verb.
    pub layer: PruneLayer,
    /// Machine-readable reason code.
    pub reason_code: String,
    /// Human-readable explanation (for UI tooltips).
    pub reason_display: String,
    /// Optional: what the layer expected.
    pub expected: Option<String>,
    /// Optional: what it actually got.
    pub got: Option<String>,
}

pub enum PruneLayer {
    AgentMode,
    WorkflowPhase,
    SemRegCCIR,
    LifecycleState,
    ActorGating,   // Non-SemReg ABAC (always-available verbs only)
    FailPolicy,    // Verb excluded because fail-closed is active
}
```

```rust
pub struct SessionSnapshot {
    pub session_id: Uuid,
    pub agent_mode: AgentMode,
    pub stage_focus: Option<String>,
    pub loaded_cbu_ids: Vec<Uuid>,
    pub focused_entity_id: Option<Uuid>,
    pub focused_entity_state: Option<String>,
    pub actor_role: String,
    // Note: No PII. No names, emails, or user identifiers beyond role.
}
```

```rust
pub enum VerbSurfaceFailPolicy {
    /// Default. Surface contains only always-safe domains:
    /// session.*, agent.*, view.* (~30 verbs).
    FailClosed,
    /// Dev/test only. Gated by `VERB_SURFACE_FAIL_OPEN=true`.
    /// Full registry returned with governance_tier = "ungoverned".
    /// MUST NOT be enabled in production.
    FailOpen { tag_ungoverned: bool },
}
```

### 5.2 Computation Pipeline

Each layer narrows the previous layer's output.

| Step | Layer | Input | Output |
|------|-------|-------|--------|
| 1 | RuntimeVerbRegistry | All 642 YAML-parsed verbs | Full verb universe as base set |
| 2 | AgentMode Gate | Base set + current mode | Mode-filtered set (~500–600) |
| 3 | Workflow Phase Filter | Mode-filtered + `stage_focus` | Phase-scoped set (~100–200). **Applied before CCIR to reduce candidate set.** |
| 4 | SemReg CCIR | Phase-scoped + subject (entity kind, taxonomy, relationships). **Includes ABAC at its Step 5.** | ContextEnvelope with `allowed_verbs` + rank scores |
| 5 | Lifecycle State | SemReg-allowed + entity current state | Transition-valid subset (~20–80) |
| 6 | Non-SemReg Actor Gating | Always-available verbs (`session.*`, `agent.*`, `view.*`) that bypass CCIR | Actor-authorized subset of non-SemReg verbs. **No-op for CCIR-sourced verbs** — ABAC already applied at Step 4. |
| 7 | FailPolicy Check | Combined set from Steps 4+5+6 | If SemReg unavailable/empty and `FailPolicy = FailClosed`, restrict to always-safe domains. If `FailOpen(DevOnly)`, include full registry tagged `"ungoverned"`. |
| 8 | Ranking & Grouping | Final set + ViewDef prominence + taxonomy boosts | Ranked, domain-grouped `SessionVerbSurface` with dual fingerprints |

### 5.3 Integration Points

**5.3.1 Session State Change Hook.** Every mutation that could affect the verb surface triggers recomputation: CBU load/unload, entity focus change, lifecycle transition, workflow phase selection, AgentMode toggle. The session stores the most recent `SessionVerbSurface` and its `surface_fingerprint`. On recomputation, if `surface_fingerprint` differs, a `VerbSurfaceDelta` is emitted.

**5.3.2 IntentPipeline Pre-Filter.** The orchestrator passes the session's current surface FQN set to `IntentPipeline::with_allowed_verbs()`. This replaces calling `resolve_sem_reg_verbs()` inline — the surface is already computed and cached.

**5.3.3 MCP Tool: session_verb_surface.** New MCP tool returns the current `SessionVerbSurface` as JSON. Parameters: `domain_filter` (optional), `format` (summary|full|fqns_only), `include_excluded` (bool).

**5.3.4 React UI: Command Palette.** VerbBrowser switches to calling `/api/session/:id/verb-surface`. On WebSocket `verb_surface_changed`, it auto-refreshes. Unavailable verbs shown greyed with multi-reason PruneReason tooltips.

**5.3.5 ChatResponse Payload.** Every `ChatResponse` includes `surface_fingerprint` (not `semreg_fingerprint`). UI compares against cached fingerprint; on mismatch, refreshes VerbBrowser.

**5.3.6 Dual Fingerprint Contract.**

| Fingerprint | Source | Hash Inputs | Used For |
|-------------|--------|-------------|----------|
| `semreg_fingerprint` | `AllowedVerbSetFingerprint::compute()` in context_envelope.rs | Sorted allowed FQNs only | TOCTOU protection, audit provenance, SemReg-internal staleness |
| `surface_fingerprint` | `SurfaceFingerprint::compute()` in verb_surface.rs | Sorted available FQNs + agent_mode + stage_focus + focused_entity + loaded_cbu_ids | UI refresh trigger, agent cache invalidation, delta notifications |

---

## 6. Remediation Plan

Four phases, each independently testable and deployable. Effort estimates assume a single developer with Claude Opus assistance.

### 6.1 Phase Overview

| Phase | Deliverable | Priority | Effort | Gaps | Dependency |
|-------|------------|----------|--------|------|------------|
| 1 | SessionVerbSurface type + `compute()` + FailPolicy | P0 | 2–3 days | G1, G7, G8 | None |
| 2 | session_verb_surface MCP tool + `/commands` rewire | P0 | 1–2 days | G2, G5 | Phase 1 |
| 3 | Workflow phase threading + lifecycle narrowing | P1 | 2–3 days | G3, G4 | Phase 1 |
| 4 | React command palette + delta notifications | P2 | 3–4 days | G2, G6 | Phase 2 |

### 6.2 Phase 1: Foundation — SessionVerbSurface Type

**Objective:** Introduce the core type, `compute()` function, and `VerbSurfaceFailPolicy`.

**Deliverables:**
- New file: `rust/src/agent/verb_surface.rs` with all types from §5.1.
- `compute_session_verb_surface(ctx: &OrchestratorContext, session: &UnifiedSession) → SessionVerbSurface` composing: RuntimeVerbRegistry → AgentMode → SemReg CCIR → ranking.
- Unit tests with mock `ContextEnvelope` payloads verifying filter composition, fingerprint stability, and fail-policy behaviour.
- Integration into orchestrator's `handle_utterance()`: replace inline `resolve_sem_reg_verbs()` + post-filter with `surface.available` as pre-filter source.
- Retire `build_verb_profiles()` full-registry fallback — replace with FailPolicy-governed behaviour.

**Acceptance Criteria:**
1. `surface_fingerprint` computed from identical inputs is deterministic.
2. `surface_fingerprint` changes when AgentMode or stage_focus changes, even if `semreg_fingerprint` is unchanged.
3. Research mode excludes `publish.*` verbs; Governed mode excludes authoring exploration verbs.
4. No CBU loaded → only `session.*`, `agent.*`, `view.*` domains (≤30 verbs).
5. **No code path returns verbs from the full registry unless `FailPolicy = FailOpen(DevOnly)`.**
6. `ExcludedVerb` entries carry all applicable `SurfacePrune` reasons (multi-reason).
7. Existing orchestrator tests pass unchanged.

### 6.3 Phase 2: Exposure — MCP Tool & /commands Rewire

**Objective:** Make `SessionVerbSurface` queryable by agents and users.

**Deliverables:**
- `session_verb_surface` MCP tool handler returning JSON-serialised `SessionVerbSurface`.
- Rewire `build_verb_profiles()` to consume `SessionVerbSurface.available`.
- Rewire `generate_options_from_envelope()` to consume `SessionVerbSurface` — eliminate parallel RuntimeVerbRegistry fallback.
- Add `surface_fingerprint` to `ChatResponse`.

**Acceptance Criteria:**
1. `/commands` output matches VerbBrowser content exactly (same data source).
2. Agent can call `session_verb_surface` tool and receive structured JSON with domain grouping.
3. `ChatResponse` includes `surface_fingerprint`; sequential identical requests return same fingerprint.
4. **Zero code paths fall back to full RuntimeVerbRegistry outside `FailOpen(DevOnly)`.**

### 6.4 Phase 3: Narrowing — Workflow & Lifecycle Integration

**Objective:** Thread workflow phase and entity lifecycle state into surface computation.

**Deliverables:**
- `stage_focus` as pre-CCIR filter. Map values to domain allow-lists: `"semos-kyc"` → `[kyc, screening, evidence, entity, session, agent, view]`.
- Lifecycle state aggregation: verbs with unmet `lifecycle.requires_states` moved to `ExcludedVerb` with `PruneLayer::LifecycleState` and expected/got fields.
- `entity_lifecycle_transitions(entity_id)` function returning valid next-states.
- xtask lint rule: every verb with `behavior != plugin` must define `lifecycle.requires_states`. CI enforcement.

**Acceptance Criteria:**
1. KYC workflow → ≤150 verbs (from 642).
2. Entity in "draft" state → only draft-valid transitions; verbs requiring "approved" excluded with `expected: "approved"`, `got: "draft"`.
3. Workflow + CBU + entity focus → ≤80 verbs (triple narrowing).

### 6.5 Phase 4: UI & Reactivity — Command Palette & Deltas

**Objective:** Reactive UI updates and visual feedback on surface changes.

**Deliverables:**
- `GET /api/session/:id/verb-surface` endpoint.
- WebSocket `verb_surface_changed` event: `{ old_fingerprint, new_fingerprint, added: [], removed: [] }`.
- VerbBrowser: auto-refresh, greyed unavailable verbs with multi-reason tooltips.
- Embedding recall mitigation: adaptive `topK` when `allowed_verbs.len() < topK * 0.3`. Measure recall. Evolve to bitset-filtered ANN if insufficient.

**Acceptance Criteria:**
1. CBU load → VerbBrowser auto-refresh within 500ms.
2. Unavailable verbs: multi-reason PruneReason tooltips.
3. Adaptive `topK` shows measurable recall improvement on narrowed surfaces.

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Surface computation latency | Medium | Medium | Cache in session. CCIR ~15ms + filters ~2ms. Recompute on state-change only. |
| Fallback divergence (3 parallel paths) | High | High | Phase 2 eliminates all parallel paths. Single `compute_session_verb_surface()`. |
| SemReg unavailability | Low | High | `FailClosed` restricts to always-safe domains. No silent expansion. |
| Stale surface after concurrent mutation | Medium | Low | `surface_fingerprint` comparison per ChatResponse. Re-resolve on mismatch. |
| Lifecycle YAML coverage gaps | Medium | Medium | Phase 3 xtask lint rule with CI enforcement. |
| Fingerprint ambiguity *(v1.1)* | High | High | Dual fingerprints: `semreg_fingerprint` for CCIR/audit, `surface_fingerprint` for UI/agent. Never conflated. |
| Fail-open accidental disclosure *(v1.1)* | Medium | High | `VerbSurfaceFailPolicy` with explicit `FailOpen(DevOnly)` gated by env flag. CI check: no production config sets `VERB_SURFACE_FAIL_OPEN`. |
| Redundant ABAC passes *(v1.1)* | Medium | Low | Step 6 documented as "non-SemReg actor gating only". CCIR-sourced verbs skip it. Assert in tests. |
| Embedding recall on narrow surfaces | Medium | Medium | Adaptive `topK` first. Bitset-filtered ANN second. Sub-index rebuild only as last resort. |

---

## 8. Success Metrics

| Metric | Current | Target | Phase |
|--------|---------|--------|-------|
| "Verb not allowed" errors per session | ~2–4 per complex session | 0 | 1 |
| Verb surface code paths | 3 parallel | 1 (`SessionVerbSurface`) | 2 |
| Time to answer "what can I do?" | ~500ms (full resolve) | <50ms (cached read) | 2 |
| Verbs visible: empty session | 642 | ≤30 | 1 |
| Verbs visible: after workflow | 642 | ≤200 | 3 |
| Verbs visible: entity focus | ~200 | ≤80 | 3 |
| UI staleness after state change | Manual refresh | <500ms | 4 |
| Surface audit trail | IntentTrace (post-hoc) | Per-response dual fingerprint + delta log | 2 |
| Silent governance bypasses | 2 code paths | **0** | 1 |
| Fingerprint false-negatives | Possible | **0** (dual fingerprint) | 1 |

---

## 9. Appendices

### 9.1 Verb Domain Inventory

| Functional Area | Domains | Example Verbs | Workflow Phase(s) |
|----------------|---------|---------------|-------------------|
| Core Onboarding | ~8 | `cbu.*`, `entity.*`, `document.*` | Onboarding, KYC |
| KYC & Screening | ~6 | `kyc.*`, `screening.*`, `ubo.*` | KYC |
| Financial Structures | ~10 | `fund-vehicle.*`, `share-class.*`, `holding.*` | Onboarding, Data Management |
| Session & Navigation | ~3 | `session.*`, `view.*`, `agent.*` | All (always available) |
| SemReg Operations | ~7 | `registry.*`, `changeset.*`, `governance.*` | Stewardship |
| Data Management | ~5 | `taxonomy.*`, `policy.*`, `attribute.*` | Data Management, Stewardship |
| Compliance & Audit | ~4 | `audit.*`, `evidence.*`, `requirement.*` | KYC, Stewardship |

### 9.2 File References

| File Path | Role in Verb Surface |
|-----------|---------------------|
| `rust/src/dsl_v2/verb_registry.rs` | UnifiedVerbRegistry (642 verbs, OnceLock singleton) |
| `rust/src/agent/orchestrator.rs` | `handle_utterance`: SemReg → pre-filter → post-filter → AgentMode |
| `rust/src/mcp/intent_pipeline.rs` | `with_allowed_verbs()` pre-constraint on HybridVerbSearcher |
| `rust/src/mcp/verb_search.rs` | HybridVerbSearcher: 6-tier search with SemReg filter |
| `rust/src/api/agent_routes.rs` | `build_verb_profiles()`, `generate_options_from_envelope()`, `/commands` |
| `rust/src/api/agent_service.rs` | `resolve_options()` bridge to `resolve_sem_reg_verbs()` |
| `rust/src/agent/context_envelope.rs` | ContextEnvelope: `allowed_verbs`, `pruned_verbs`, fingerprint |
| `rust/crates/sem_os_core/src/context_resolution.rs` | CCIR: 9-step resolution with ABAC at Step 5 |
| `rust/crates/sem_os_core/src/gates/technical.rs` | Gate 4: `check_verb_surface_disclosure` |
| `rust/src/sem_reg/agent/mcp_tools.rs` | `handle_verb_surface` (single-verb I/O, not session-wide) |
| `ob-poc-ui-react/src/features/chat/components/VerbBrowser.tsx` | React verb palette with progressive disclosure |
| `rust/config/verbs/` | 125 YAML files defining 642 verbs with lifecycle constraints |

### 9.3 Peer Review Change Log (v1.0 → v1.1)

| Section | Change | Rationale |
|---------|--------|-----------|
| §2.2 P3 | "Fingerprintable" → "Dual-Fingerprintable" | SemReg fingerprint ≠ surface fingerprint |
| §2.2 P6 | Added Principle 6: Fail-Safe by Default | No code path may expand surface on governance failure |
| §2.4 | Added Safety Invariants SI-1, SI-2, SI-3 | Codify non-negotiable governance contracts |
| §3.1 CCIR row | Added ABAC clarification | Prevent duplicate ABAC implementation |
| §3.1 `build_verb_profiles` | Added SI-1 violation note | Flag existing governance bypass |
| §3.1 | Added `VerbSurfaceFailPolicy` as MISSING | New gap from review |
| §4.4 | Note: workflow filter before CCIR | Performance + correctness |
| §4.6 | Added adaptive topK mitigation | Cheaper than sub-index |
| §4.7 | New: Gap G7 Fingerprint Ambiguity | Single fingerprint misses non-SemReg changes |
| §4.8 | New: Gap G8 Fail-Open Disclosure | Silent fallback to full registry |
| §5.1 | Dual fingerprints: `semreg_fingerprint` + `surface_fingerprint` | Distinct hash inputs, distinct uses |
| §5.1 | `ExcludedVerb.prune_reasons: Vec<SurfacePrune>` | Multi-reason exclusion |
| §5.1 | Added `VerbSurfaceFailPolicy` type | Explicit fail-policy |
| §5.1 | Added `SessionSnapshot` with PII note | Confirm no PII captured |
| §5.2 Step 3 | Workflow filter moved before CCIR | Reduce candidate set |
| §5.2 Step 6 | Renamed: "Non-SemReg Actor Gating" | No-op for CCIR verbs |
| §5.2 Step 7 | Added FailPolicy Check | Explicit governance boundary |
| §5.3.6 | Added Dual Fingerprint Contract table | Document what each fingerprint covers |
| §6.2 AC5 | Added fail-policy acceptance criterion | No ungoverned expansion |
| §6.2 AC6 | Added multi-reason criterion | `ExcludedVerb` carries `Vec<SurfacePrune>` |
| §6.3 AC4 | Added zero-fallback criterion | Enforces path removal |
| §7 | 3 new risks | Fingerprint ambiguity, fail-open disclosure, redundant ABAC |
| §8 | 2 new metrics | Governance bypasses (target: 0), fingerprint false-negatives (target: 0) |
