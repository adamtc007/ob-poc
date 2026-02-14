# Policy Loom — Phased Implementation Plan

**Source Spec**: `docs/PENDING_Policy_Loom_v0.4.md`
**Task Breakdown**: `docs/POLICY_LOOM_TODO.md`
**Target Codebase**: `ob-poc` (Rust / SQLx / PostgreSQL / Semantic OS)
**Baseline**: Semantic OS Phases 0-9 complete (migrations 078-086, 13 ObjectType variants, ~32 MCP tools, 12-step Context Resolution)

---

## Context

The Policy Loom turns static policy documents into first-class, versioned, testable, explainable control plane artifacts within the Semantic OS. The TODO doc defines 11 layers (L0-L10). This plan consolidates them into 5 executable implementation phases, each delivering a self-contained, testable increment that builds on the existing `sem_reg` infrastructure.

**Key codebase facts**:
- No `src/loom/` module exists yet (clean slate)
- `ObjectType` enum at `rust/src/sem_reg/types.rs:58-97` has 13 variants
- `PolicyRuleBody` at `rust/src/sem_reg/policy_rule.rs` is the existing atomic rule type
- Latest migration: `086_sem_reg_phase9.sql` (next = 087)
- `sem_reg` module is NOT feature-gated (always compiled)
- Existing patterns: typed body structs, JSONB storage, `RegistryService` publish/resolve, `SnapshotStore` INSERT-only

---

## Phase Summary

| Phase | Name | Layers | Deliverables | Migration |
|-------|------|--------|-------------|-----------|
| **1** | Foundation Types & Document Model | L0+L1+L2 | Core enums, PolicySectionRef, PolicyDocFamily/Version, active version constraint | `087_loom_phase1.sql` |
| **2** | Coverage, Directives & Interpretations | L3+L4 | CoverageSpec, 7 directive types, PolicyInterpretationBody, lifecycle validation | `088_loom_phase2.sql` |
| **3** | Merge Engine, Compilation & Action Linkage | L5+L6 | Merge semantics, PolicyRuleSet, deterministic compilation, ActionPolicyLinkage, PolicyProvenance | `089_loom_phase3.sql` |
| **4** | Test Fixtures, Publish Gates & Evaluation | L7+L8 | PolicyTestFixture, fixture inheritance, 7 publish gates, evaluation pipeline, PolicyEvaluationTrace | `090_loom_phase4.sql` |
| **5** | Runbook Compilation, MCP Tools & Impact Analysis | L9+L10 | Runbook compiler, 10 MCP tools, impact analysis, coverage metrics | `091_loom_phase5.sql` |

---

## Phase 1 — Foundation Types & Document Model (L0+L1+L2)

**Files to create**:
- `rust/src/loom/mod.rs`
- `rust/src/loom/types.rs`
- `rust/src/loom/section_ref.rs`
- `rust/src/loom/policy_doc.rs`
- `migrations/087_loom_phase1.sql`

**Files to modify**:
- `rust/src/lib.rs` — add `pub mod loom;`
- `rust/src/sem_reg/types.rs` — add 5 ObjectType variants + Display arms

### Step 1.1 — Module scaffold & core enums
- Create `rust/src/loom/` directory, `mod.rs` with submodule declarations
- Wire `pub mod loom;` into `rust/src/lib.rs`
- Define all core enums in `types.rs`:
  - `ActionRef` (Dsl/Java/Bpmn/Custom) — `#[serde(tag = "type")]` for JSONB
  - `ComparisonOp`: Gte|Gt|Lte|Lt|Eq|InSet
  - `TimeoutAction`: Escalate|Cancel|Extend
  - `ExceptionEffect`: Exclude|Override|Substitute
  - `LinkageType`: Precondition|EvidenceSource|ParameterSource|SecurityDirective|TimeoutSource|EscalationTrigger
  - `InterpretationStatus`: Draft|Reviewed|Approved|Disputed
  - `PolicyStatus`: Draft|Active|Superseded|Retired
  - `GateVerdict`: Deny>Escalate>PermitWithMasking>Permit — implement `Ord` for severity ordering
  - `MergeResult`: Value(f64)|GovernanceFinding|UnionOfSets
  - `LoomError` enum
- Unit tests: GateVerdict ordering, serde round-trips

### Step 1.2 — PolicySectionRef & section index
- `PolicySectionRef` (anchor_id, positional fallback, quote_excerpt_hash, display_label)
- `PositionalRef` (doc_content_hash, page, paragraph_index)
- `SectionIndexEntry` (anchor_id, display_label, page, paragraph_index, quote_excerpt_hash)
- `compute_excerpt_hash()` — SHA-256 of first 100 chars
- `resolve_section_ref()` — anchor first, positional fallback
- `check_section_drift()` — compare excerpt hashes
- Unit tests: resolution priority, drift detection

### Step 1.3 — Policy document family & version
- `PolicyDocFamilyBody` — validate family_id dot-notation `^[a-z][a-z0-9]*(\.[a-z][a-z0-9_]*)+$`
- `PolicyDocVersionBody` with section_index, jurisdiction_scope, effective dates, status
- `ApprovalRecord` struct
- Active version constraint: at most one Active per (family_id, jurisdiction_scope)
- `extract_section_index()` stub

### Step 1.4 — ObjectType extension
- Add to `rust/src/sem_reg/types.rs`: PolicyDocFamily, PolicyDocVersion, PolicyInterpretation, PolicyRuleSet, PolicyTestFixture
- Update Display impl

### Step 1.5 — Database migration (087)
- Extend `sem_reg.object_type` enum
- Create `sem_reg.policy_status` + `sem_reg.interpretation_status` enums
- Create `policy_doc_families` + `policy_doc_versions` tables
- Unique partial index for active version constraint
- SQLx CRUD operations

### Step 1.6 — Integration tests
- Version lifecycle, activation scope constraint, family_id validation, drift detection

**Gate**: Types compile, serde round-trips pass, active version constraint enforced. ~28%

---

## Phase 2 — Coverage, Directives & Interpretations (L3+L4)

**Files to create**:
- `rust/src/loom/coverage.rs`
- `rust/src/loom/directives.rs`
- `rust/src/loom/interpretation.rs`
- `migrations/088_loom_phase2.sql`

### Step 2.1 — CoverageSpec & applicability
- `CoverageSpec` (domain_tags, entity/evidence/action/jurisdiction/risk/product scopes, exclusions, is_default_for_domain, specificity_rank)
- `ExclusionPredicate`, `CaseProfile`
- `CoverageSpec::matches()` — empty scope = matches all, check exclusions
- `select_applicable_policies()` — filter + sort by specificity_rank desc
- Default-for-domain fallback

### Step 2.2 — All 7 directive types
Each carries applicability_predicate, section_ref, rationale:
- `EvidenceDirective`, `ThresholdDirective`, `DeadlineDirective`, `HandlingDirective`, `EscalationDirective`, `ExceptionClause`, `PolicyDefinition`

### Step 2.3 — PolicyInterpretationBody & lifecycle
- Full struct with all directive vecs + governance fields
- Lifecycle: Disputed requires dispute_reason, blocks compilation
- `diff_interpretations()` — per-directive-type structural diff

### Step 2.4 — Migration (088) + CRUD
### Step 2.5 — Integration tests

**Gate**: Directives compile + serde, lifecycle enforced, CoverageSpec matching works. ~48%

---

## Phase 3 — Merge Engine, Compilation & Action Linkage (L5+L6)

**Files to create**:
- `rust/src/loom/merge.rs`
- `rust/src/loom/ruleset.rs`
- `rust/src/loom/compilation.rs`
- `rust/src/loom/linkage.rs`
- `migrations/089_loom_phase3.sql`

**Files to modify**:
- `rust/src/sem_reg/policy_rule.rs` — add optional `policy_provenance` field (`#[serde(default)]`)

### Step 3.1 — Merge semantics engine
- `merge_thresholds()` — direction-aware (Gte/Gt->min, Lte/Lt->max, Eq->conflict, InSet->union)
- Evidence: union. Deadlines: most-stringent. Handling: most-restrictive. Verdicts: severity max. Escalations: union.
- ExceptionClause override/substitute handling
- Structural conflict detection -> GovernanceFinding + DisambiguationPrompt
- Appendix A test vectors

### Step 3.2 — PolicyProvenance & ActionPolicyLinkage
- Extend existing `PolicyRuleBody` with optional provenance (backward-compat via `#[serde(default)]`)

### Step 3.3 — PolicyRuleSet & deterministic compilation
- `compile_ruleset()` — Approved guard, directive->rule+linkage, deterministic output
- ActionRef resolution against verb registry

### Step 3.4 — Migration (089): rulesets, linkages, provenance tables
### Step 3.5 — Integration tests

**Gate**: Merge correct for all Appendix A scenarios, compilation deterministic, provenance chain complete. ~68%

---

## Phase 4 — Test Fixtures, Publish Gates & Evaluation (L7+L8)

**Files to create**:
- `rust/src/loom/fixtures.rs`
- `rust/src/loom/gates.rs`
- `rust/src/loom/evaluation.rs`
- `rust/src/loom/trace.rs`
- `migrations/090_loom_phase4.sql`

**Files to modify**:
- `rust/src/sem_reg/gates.rs` — integrate Loom publish gates into existing framework
- `rust/src/sem_reg/context_resolution.rs` — policy-aware case resolution

### Step 4.1 — PolicyTestFixture + evaluation + inheritance + coverage matrix
### Step 4.2 — 7 publish gates (5 Hard + 2 ReportOnly) integrated with existing gate framework
### Step 4.3 — Full evaluation pipeline + PolicyEvaluationTrace + Context Resolution integration
### Step 4.4 — Migration (090): fixtures + governance_findings tables
### Step 4.5 — Integration tests (Appendix A fixtures F-1 through F-4)

**Gate**: All gates wired, fixtures enforce regression, evaluation produces full trace. ~88%

---

## Phase 5 — Runbook Compilation, MCP Tools & Impact Analysis (L9+L10)

**Files to create**:
- `rust/src/loom/runbook.rs`
- `rust/src/loom/mcp_tools.rs`
- `rust/src/loom/impact.rs`
- `migrations/091_loom_phase5.sql`

**Files to modify**:
- `rust/src/sem_reg/agent/mcp_tools.rs` — register 10 new tool specs + dispatch

### Step 5.1 — Runbook compilation (evidence gaps -> AgentPlan steps with provenance)
### Step 5.2 — 10 MCP tool handlers (describe_policy_doc, describe_interpretation, policy_coverage, policy_evaluate, evidence_gaps, compile_runbook, policy_impact, policy_test_run, policy_diff, policy_drift_check)
### Step 5.3 — Impact analysis (graph traversal over Loom artifacts), coverage metrics, dot-notation discovery
### Step 5.4 — Integration tests

**Gate**: All tools operational, impact analysis works, runbook compilation produces traceable plans. 100%

---

## Cross-Cutting Concerns

| Concern | Rule |
|---------|------|
| Serde | All types round-trip JSONB. `#[serde(tag = "type")]` for enums. |
| Snapshots | Use existing snapshot store infrastructure. |
| ABAC | Handling directives -> SecurityLabel fields. |
| Determinism | No randomness/LLM in compilation chain. |
| Backward compat | `#[serde(default)]` when extending existing structs. |
| Errors | `LoomError` enum for all failure modes. |

---

## Verification

After each phase:
1. `cargo build` — all types compile
2. `cargo test` — unit + integration tests pass
3. Migration applied successfully to local PostgreSQL
4. Serde round-trip tests pass for all new types

After all phases:
- End-to-end: Appendix A example flows from PolicyDocVersion through interpretation, compilation, fixture gate, evaluation, to runbook with full provenance chain
- All 10 MCP tools respond correctly
- Impact analysis on v12->v13 identifies affected actions
- Point-in-time reconstruction from DecisionRecord back to legal text + section

---

## New Files Summary

| File | Purpose |
|------|---------|
| `rust/src/loom/mod.rs` | Module root |
| `rust/src/loom/types.rs` | Core enums (ActionRef, ComparisonOp, GateVerdict, etc.) |
| `rust/src/loom/section_ref.rs` | PolicySectionRef, drift detection |
| `rust/src/loom/policy_doc.rs` | PolicyDocFamily/Version, active version constraint |
| `rust/src/loom/coverage.rs` | CoverageSpec, CaseProfile, applicability matching |
| `rust/src/loom/directives.rs` | 7 directive types |
| `rust/src/loom/interpretation.rs` | PolicyInterpretationBody, lifecycle, diff |
| `rust/src/loom/merge.rs` | Merge semantics engine |
| `rust/src/loom/ruleset.rs` | PolicyRuleSetBody |
| `rust/src/loom/compilation.rs` | Deterministic compilation pipeline |
| `rust/src/loom/linkage.rs` | ActionPolicyLinkage, PolicyProvenance |
| `rust/src/loom/fixtures.rs` | PolicyTestFixture, evaluation, inheritance |
| `rust/src/loom/gates.rs` | 7 Loom publish gates |
| `rust/src/loom/evaluation.rs` | Full evaluation pipeline |
| `rust/src/loom/trace.rs` | PolicyEvaluationTrace |
| `rust/src/loom/runbook.rs` | Runbook compilation |
| `rust/src/loom/mcp_tools.rs` | 10 MCP tool handlers |
| `rust/src/loom/impact.rs` | Impact analysis, coverage metrics |
| `migrations/087_loom_phase1.sql` | Foundation tables + enums |
| `migrations/088_loom_phase2.sql` | Interpretations table |
| `migrations/089_loom_phase3.sql` | Rulesets + linkages tables |
| `migrations/090_loom_phase4.sql` | Fixtures + findings tables |
| `migrations/091_loom_phase5.sql` | Final indices/views |
