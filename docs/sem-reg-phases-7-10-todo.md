# Semantic OS — Phases 7–10 Implementation TODO

**Wiring the Semantic Registry into Agent MCP + ob-poc Runtime**

Version: 1.0 — February 2026  
Author: Adam / Lead Solution Architect  
Reference: `semantic-os-v1.1.md`, `semantic-os-implementation-todo-v2.md`  
Prerequisite: Phases 0–6 complete (see review below)

---

## Phases 0–6 Review: What Exists

### Inventory (6,487 lines Rust, 683 lines SQL, 22 source files)

| Phase | Status | Artifacts | Assessment |
|-------|--------|-----------|------------|
| **0 — Snapshot Infra** | ✅ Complete | `types.rs` (285 LOC), `store.rs` (314 LOC), migration `078` (181 LOC) | Solid. Single `sem_reg.snapshots` table with JSONB `definition`. `resolve_active`, `resolve_at`, `publish_snapshot`, `load_history`, `count_active`, `find_active_by_definition_field` all implemented. Unique index on `(object_type, object_id) WHERE effective_until IS NULL` enforces single-active invariant. `SnapshotMeta`, `SnapshotRow`, all core enums. |
| **1 — Core Registries** | ✅ Complete | `attribute_def.rs` (148), `entity_type_def.rs` (142), `verb_contract.rs` (233), `registry.rs` (430), `scanner.rs` (708), migrations `079` | 12 typed registry methods via macro (publish/resolve/resolve_by_fqn for each body type). Scanner reads `dsl_core::config::loader` YAML verb defs and bootstraps VerbContract, EntityTypeDef, AttributeDef snapshots. `ScanReport` tracks published/skipped counts. |
| **2 — Taxonomy/Views** | ✅ Complete | `taxonomy_def.rs` (113), `membership.rs` (128), `view_def.rs` (160), migration `081` | `TaxonomyDefBody`, `TaxonomyNodeBody`, `MembershipRuleBody` (with `MembershipKind` enum), `ViewDefBody`. All stored as JSONB in the universal snapshots table. |
| **3 — Policy/Evidence** | ✅ Complete | `policy_rule.rs` (124), `evidence.rs` (138), `document_type_def.rs` (87), `observation_def.rs` (88), `abac.rs` (284), migration `082` | ABAC engine is a pure function: `evaluate_abac(actor, label) → AccessDecision`. Handles classification, jurisdiction, purpose intersection, PII masking. PolicyRule, EvidenceRequirement, DocumentTypeDef, ObservationDef body types. |
| **4 — Security Framework** | ✅ Complete | `security.rs` (675) | `compute_inherited_label(&[SecurityLabel]) → SecurityLabel` with most-restrictive confidentiality, most-restrictive residency, union of handling controls, intersection of purpose tags. `validate_verb_security_compatibility` prevents label laundering. |
| **5 — Derivation Specs** | ✅ Complete | `derivation_spec.rs` (232), `derivation.rs` (391), migration `083` | `DerivationSpecBody` with `FunctionRef` expression (MVP — no AST interpreter). `DerivationFunctionRegistry` for Rust function dispatch. Evaluation pins snapshot IDs, computes inherited SecurityLabel, handles null semantics and freshness rules. |
| **6 — Publish Gates** | ✅ Complete | `gates.rs` (899), `gates_technical.rs` (511), `gates_governance.rs` (303) | Full gate framework with `GateMode::Enforce` / `ReportOnly`. Technical gates: proof rule, security label validation, type correctness, orphan detection, derivation cycle detection, evidence grade enforcement, derivation type compatibility. Governance gates: taxonomy membership, stewardship, evidence grade policy. `ExtendedPublishGateResult` with structured `GateFailure` output. |
| **CLI** | ✅ Complete | `xtask/src/sem_reg.rs` (545) | `cargo x sem-reg {stats, attr-describe, attr-list, verb-describe, verb-list, scan, gate-check}` |

### Key Observations for Phase 7–10 Planning

1. **No Context Resolution exists yet.** There is no `ContextResolutionRequest`/`Response` type, no resolution logic, no view-to-verb-to-attribute ranking pipeline. Phase 7 builds this from scratch.

2. **No agent control plane exists.** No `AgentPlan`, `PlanStep`, `DecisionRecord`, `EscalationRecord`, `DisambiguationPrompt`. No MCP tool definitions. Phase 8 builds this from scratch.

3. **No MCP wiring exists.** The registry is CLI-only. The agent has no programmatic access to the semantic OS. Phase 8 is the phase that makes everything prior *usable*.

4. **No lineage graph exists.** `derivation.rs` can evaluate individual derivations but there is no `derivation_edges` table, no `RunRecord`, no impact analysis or provenance chain queries. Phase 9 builds this.

5. **No embedding pipeline exists.** No `SemanticText` generation, no `EmbeddingRecord` table, no staleness tracking. Phase 9 connects to existing ob-poc Candle/BGE infrastructure.

6. **ABAC is complete but untested against MCP actor contexts.** The pure function exists; it needs integration testing with real agent/analyst actor types flowing through MCP tool calls.

7. **Scanner wiring is verb-YAML-only.** Steps 3–6 from the onboarding plan (schema cross-reference, entity inference, seeding, orphan classification) are not implemented — the scanner reads DSL YAML verb configs only. This affects Phase 7/8 data quality (registry may be sparsely populated).

8. **`RegistryService::publish_with_gates` exists** but only calls basic gates. The full extended gate pipeline (`gates_technical` + `gates_governance`) needs explicit wiring through the publish path.

---

## Phase 7 — Context Resolution API

**Goal**: The single query contract that serves agent, UI, CLI, and governance.  
**Why it matters**: Without this, every consumer reimplements its own ad-hoc registry querying. Context Resolution is the semantic OS's "system call" interface.  
**Estimated scope**: ~1,200–1,500 LOC Rust, 1 migration

### 7.1 Types — `context_resolution.rs` (NEW)

Create `rust/src/sem_reg/context_resolution.rs`:

- [ ] `EvidenceMode` enum: `Strict | Normal | Exploratory | Governance`
- [ ] `ContextResolutionRequest` struct:
  - `subject`: enum `SubjectRef { CaseId(Uuid), EntityId(Uuid), DocumentId(Uuid), TaskId(Uuid), ViewId(Uuid) }`
  - `intent`: `Option<String>` (NL — used later for embedding ranking in Phase 9)
  - `actor`: `ActorContext` (reuse from `abac.rs`)
  - `goals`: `Vec<String>` (e.g. `["resolve_ubo", "collect_proof"]`)
  - `constraints`: `ResolutionConstraints { jurisdiction: Option<String>, risk_posture: Option<String>, thresholds: HashMap<String, serde_json::Value> }`
  - `evidence_mode`: `EvidenceMode`
  - `point_in_time`: `Option<DateTime<Utc>>` (defaults to now)
- [ ] `ContextResolutionResponse` struct:
  - `as_of_time`: `DateTime<Utc>` — the point-in-time resolved
  - `resolved_at`: `DateTime<Utc>` — when computation occurred
  - `applicable_views`: `Vec<RankedView>` (snapshot-pinned)
  - `candidate_verbs`: `Vec<VerbCandidate>` (ranked, with precondition status, tier, trust, `usable_for_proof` flag, `verb_snapshot_id`)
  - `candidate_attributes`: `Vec<AttributeCandidate>` (ranked, missing/required flags, tier, trust, `attribute_snapshot_id`)
  - `required_preconditions`: `Vec<PreconditionStatus>` with remediation hints
  - `disambiguation_questions`: `Vec<DisambiguationPrompt>` (empty if unambiguous)
  - `evidence`: `EvidenceSummary { positive: Vec<...>, negative: Vec<...> }`
  - `policy_verdicts`: `Vec<PolicyVerdict>` (each with `policy_snapshot_id`, regulatory ref)
  - `security_handling`: `AccessDecision` (reuse from `abac.rs`)
  - `governance_signals`: `Vec<GovernanceSignal>`
  - `confidence`: `f64` (0.0–1.0)
- [ ] Supporting sub-types: `RankedView`, `VerbCandidate`, `AttributeCandidate`, `PreconditionStatus`, `PolicyVerdict`, `GovernanceSignal`

### 7.2 Resolution Engine — `context_resolution.rs` continued

Implement `pub async fn resolve_context(pool: &PgPool, req: &ContextResolutionRequest) -> Result<ContextResolutionResponse>`:

- [ ] **Step 1**: Determine snapshot epoch — use `point_in_time` or `Utc::now()`
- [ ] **Step 2**: Resolve subject — look up entity type from subject ref. For `CaseId`, query case table → entity type + jurisdiction + state. For `EntityId`, query entity instance → entity type. For `ViewId`, load view directly
- [ ] **Step 3**: Select applicable `ViewDef`s — query active ViewDef snapshots, score by taxonomy overlap between view's `taxonomy_slices` and subject's entity type memberships. Rank by overlap score
- [ ] **Step 4**: For top ViewDef, extract `verb_surface`, `attribute_prominence`, `taxonomy_slices`
- [ ] **Step 5**: Filter verbs by:
  - Taxonomy membership overlap with view slices
  - Precondition evaluability (can we check them?)
  - ABAC verdict (call `evaluate_abac` for each verb's security label against actor)
  - Tier/trust filtering per evidence_mode (Strict/Normal: exclude Operational unless `view.includes_operational`)
- [ ] **Step 6**: Filter attributes similarly (taxonomy overlap + ABAC + tier/trust)
- [ ] **Step 7**: Rank verbs and attributes by ViewDef prominence weights
- [ ] **Step 8**: Evaluate preconditions for top N candidate verbs against current entity state
- [ ] **Step 9**: Evaluate PolicyRules for top candidates — produce `PolicyVerdict`s with snapshot refs
- [ ] **Step 10**: Compute composite `AccessDecision` for the response
- [ ] **Step 11**: Generate governance signals — scan for: unowned governed objects, unclassified governed objects, stale evidence (observations past freshness window), approaching retention deadlines
- [ ] **Step 12**: Compute confidence score (deterministic heuristic):
  - `view_match_score` × 0.30
  - `precondition_satisfiable_pct` × 0.25
  - `required_inputs_present_pct` × 0.30
  - `abac_permit_pct` × 0.15
  - If confidence < 0.5 or multiple views within 0.1: generate disambiguation prompts

### 7.3 Trust-Aware Filtering

- [ ] `Strict` / `Normal`: Governed + Proof/DecisionSupport primary. Operational only if `view.includes_operational == true`. All Operational candidates tagged `usable_for_proof = false`
- [ ] `Exploratory`: All tiers, all trust classes, annotated
- [ ] `Governance`: Coverage metrics focus — stewardship gaps, classification gaps, stale evidence, policy attachment status

### 7.4 Point-in-Time Resolution

- [ ] When `point_in_time` is set, all snapshot lookups use `SnapshotStore::resolve_at` instead of `resolve_active`
- [ ] ViewDefs, VerbContracts, AttributeDefs, PolicyRules, Memberships — all resolved at the specified time
- [ ] Response carries `as_of_time` to make the temporal scope explicit

### 7.5 CLI Integration

- [ ] `cargo x sem-reg ctx-resolve --subject <id> --subject-type <case|entity|view> --actor <agent|analyst|governance> --mode <strict|normal|exploratory|governance> [--as-of <ISO8601>]`
- [ ] Human-readable output: top view, ranked verbs (with precondition status), ranked attributes, governance signals
- [ ] JSON output (`--json`) for machine consumption

### 7.6 Migration

- [ ] Migration `084_sem_reg_phase7.sql`: Convenience views for context resolution queries (e.g. `v_active_memberships_by_subject`, `v_verb_precondition_status`). No new tables — context resolution is a computed projection over existing snapshots.

### Phase 7 Gate

- [ ] Context Resolution returns correct, tier-aware, snapshot-pinned responses for:
  - UBO Discovery view with `goal = resolve_ubo`
  - Sanctions Screening view with `goal = screening_check`
  - Proof Collection view with `goal = collect_proof`
- [ ] Point-in-time resolution returns historical snapshots
- [ ] Confidence score differentiates ambiguous vs. clear contexts
- [ ] All snapshot IDs in response are valid and pinned to the correct epoch

**→ IMMEDIATELY proceed to Phase 8. Progress: ~85%.**

---

## Phase 8 — Agent Control Plane + MCP Tool Integration

**Goal**: The agent can plan, decide, execute, escalate, and record — all through MCP tools backed by the Semantic OS. **This is the phase that makes everything prior usable.**  
**Estimated scope**: ~2,000–2,500 LOC Rust, 1 migration, MCP tool handler registration

### 8.1 Agent Control Plane Types — `agent/` module (NEW)

Create `rust/src/sem_reg/agent/`:

- [ ] **`mod.rs`** — module root
- [ ] **`plans.rs`** — `AgentPlan`, `PlanStep`
  - `AgentPlan`: `plan_id` (UUID), `case_id`, `goal` (String), `context_resolution_ref` (UUID — snapshot-pinned ContextResolution invocation), `steps: Vec<PlanStep>`, `assumptions: Vec<String>`, `risk_flags: Vec<String>`, `security_clearance: AccessDecision`, `created_at`, `approved_by: Option<String>`, `status` (enum: Draft/Active/Completed/Failed/Cancelled)
  - `PlanStep`: `step_id` (UUID), `verb_id` (UUID), `verb_snapshot_id` (UUID — pinned to exact contract version), `params: serde_json::Value`, `expected_postconditions: Vec<String>`, `fallback_steps: Vec<Uuid>`, `depends_on_steps: Vec<Uuid>`, `governance_tier_of_outputs: GovernanceTier`, `status` (Pending/Running/Completed/Failed/Skipped)
- [ ] **`decisions.rs`** — `DecisionRecord`
  - `decision_id` (UUID), `plan_id: Option<Uuid>`, `context_ref` (UUID), `chosen_action: String`, `alternatives_considered: Vec<AlternativeAction>`, `evidence_for: Vec<EvidenceRef>`, `evidence_against: Vec<EvidenceRef>`, `negative_evidence: Vec<NegativeEvidenceRef>`, `policy_verdicts: Vec<PolicyVerdict>` (snapshot-pinned), `security_handling: AccessDecision`, `tier_trust_annotations: serde_json::Value`, **`snapshot_manifest: HashMap<Uuid, Uuid>`** (object_id → snapshot_id — the complete provenance chain), `confidence: f64`, `escalation_flag: bool`, `created_at`
- [ ] **`escalation.rs`** — `DisambiguationPrompt`, `EscalationRecord`
  - `DisambiguationPrompt`: `prompt_id`, `decision_id`, `question`, `options: Vec<DisambiguationOption>`, `evidence_per_option: serde_json::Value`, `required_to_proceed: bool`, `rationale`
  - `EscalationRecord`: `escalation_id`, `decision_id`, `reason`, `context_snapshot: serde_json::Value`, `required_human_action`, `assigned_to: Option<String>`, `resolved_at: Option<DateTime<Utc>>`, `resolution: Option<String>`

### 8.2 Agent Control Plane Storage

- [ ] Migration `085_sem_reg_phase8.sql`:
  - `sem_reg.agent_plans` — immutable, INSERT only
  - `sem_reg.plan_steps` — immutable, INSERT only (status changes = new row or UPDATE on step-level status — decide: status is mutable for step progress tracking)
  - `sem_reg.decision_records` — immutable, INSERT only, `snapshot_manifest JSONB NOT NULL`
  - `sem_reg.disambiguation_prompts` — immutable
  - `sem_reg.escalation_records` — INSERT + UPDATE on `resolved_at` / `resolution` (resolution is a legitimate mutation)
  - Indexes: `(case_id)` on plans, `(plan_id)` on steps, `(plan_id)` on decisions

### 8.3 MCP Tool Definitions — `agent/mcp_tools.rs` (NEW)

This is the critical wiring. Each MCP tool maps to a function backed by the registry.

**Registry query tools (read-only):**

- [ ] `sem_reg_describe_attribute(name_or_id, as_of?)` → `RegistryService::resolve_attribute_def_by_fqn` or by UUID, with point-in-time support
- [ ] `sem_reg_describe_verb(canonical_name, as_of?)` → `RegistryService::resolve_verb_contract_by_fqn`
- [ ] `sem_reg_describe_entity_type(name, as_of?)` → `RegistryService::resolve_entity_type_def_by_fqn`
- [ ] `sem_reg_describe_policy(name_or_id, as_of?)` → `RegistryService::resolve_policy_rule_by_fqn`
- [ ] `sem_reg_search(query, object_types?, taxonomy_filter?)` → cross-registry text search on `definition` JSONB fields. MVP: `ILIKE` on FQN/name/description. Phase 9 adds embedding-backed ranking
- [ ] `sem_reg_list_verbs(filter?)` → `SnapshotStore::list_active(ObjectType::VerbContract, ...)`
- [ ] `sem_reg_list_attributes(filter?, taxonomy?)` → filtered list with optional taxonomy membership join

**Taxonomy tools:**

- [ ] `sem_reg_taxonomy_tree(taxonomy_name)` → load TaxonomyDef + all child TaxonomyNode snapshots, build DAG
- [ ] `sem_reg_taxonomy_members(taxonomy_name, node_path)` → query MembershipRule snapshots for subject objects classified under the specified node
- [ ] `sem_reg_classify(object_type, object_id, taxonomy, node_path)` → create Draft MembershipRule snapshot (requires `ob publish` to activate)

**Impact and lineage tools:**

- [ ] `sem_reg_verb_surface(verb_name)` → load VerbContract, cross-reference inputs/outputs/side-effects against AttributeDef snapshots, load applicable PolicyRules. Returns: "everything this verb touches"
- [ ] `sem_reg_attribute_producers(attribute_name)` → query VerbContracts whose `definition->'args'` or `definition->'produces'` reference the attribute. Returns: "what verbs can produce this attribute?"
- [ ] `sem_reg_impact_analysis(object_type, object_id)` → Phase 8 MVP: traverse VerbContract I/O references + DerivationSpec inputs. Phase 9 backs this with the real lineage graph
- [ ] `sem_reg_lineage(subject_ref, attribute_id, as_of?)` → Phase 8 stub returning DerivationSpec chain. Phase 9 backs with full provenance edges
- [ ] `sem_reg_regulation_trace(regulation_id)` → query PolicyRules by `definition->'regulatory_reference'->'regulation_id'`, then cross-reference verbs and attributes. Returns: regulation → policies → verbs → attributes

**Context resolution tool:**

- [ ] `sem_reg_resolve_context(subject, goals?, constraints?, evidence_mode?)` → calls Phase 7 `resolve_context()`. **This is the primary agent entry point.**

**View tools:**

- [ ] `sem_reg_describe_view(view_name)` → load ViewDef snapshot
- [ ] `sem_reg_apply_view(view_name, subject)` → Context Resolution filtered through a specific view

**Planning tools:**

- [ ] `sem_reg_create_plan(case_id, goal, context_resolution_ref)` → INSERT AgentPlan, return `plan_id`
- [ ] `sem_reg_add_plan_step(plan_id, verb_id, params, expected_postconditions, fallback?)` → INSERT PlanStep, validate verb contract exists and is active
- [ ] `sem_reg_validate_plan(plan_id)` → evaluate all steps: contracts valid, preconditions evaluable, ABAC clearance, proof rule compliance. Returns structured `PlanValidationResult`
- [ ] `sem_reg_execute_plan_step(plan_id, step_id)` → execute verb (delegates to DSL runtime), record snapshot_manifest, record outcome

**Decision recording tools:**

- [ ] `sem_reg_record_decision(plan_id?, context_ref, chosen_action, alternatives, evidence_for, evidence_against, negative_evidence, confidence)` → INSERT DecisionRecord with **auto-populated `snapshot_manifest`** (collect all snapshot_ids from the context resolution ref + verb contracts + policies referenced)
- [ ] `sem_reg_record_escalation(decision_id, reason, required_human_action)` → INSERT EscalationRecord
- [ ] `sem_reg_record_disambiguation(decision_id, question, options, evidence_per_option)` → INSERT DisambiguationPrompt

**Evidence tools:**

- [ ] `sem_reg_record_observation(subject_ref, attribute_id, value, source, confidence, supporting_docs?, governance_tier?)` → INSERT Observation snapshot with supersession chain (find latest observation for `(subject_ref, attribute_id)`, set as predecessor)
- [ ] `sem_reg_check_evidence_freshness(subject_ref, attribute_id?)` → query active observations, compare timestamps against freshness rules from applicable EvidenceRequirements
- [ ] `sem_reg_identify_evidence_gaps(case_id, view_name?)` → compare required evidence (from PolicyRules + EvidenceRequirements applicable to the case's entity types and jurisdiction) against available observations. Returns: `Vec<EvidenceGap { attribute, policy_rule, freshness_status, gap_type }>`

### 8.4 MCP Resource URIs

Expose registry objects as MCP resources:

- [ ] `sem_reg://attributes/{fqn_or_id}[?as_of=<ISO8601>]`
- [ ] `sem_reg://verbs/{fqn}[?as_of=<ISO8601>]`
- [ ] `sem_reg://entities/{name}[?as_of=<ISO8601>]`
- [ ] `sem_reg://policies/{name_or_id}[?as_of=<ISO8601>]`
- [ ] `sem_reg://views/{name}`
- [ ] `sem_reg://taxonomies/{name}`
- [ ] `sem_reg://observations/{subject_ref}/{attribute_id}`
- [ ] `sem_reg://decisions/{decision_id}` — includes full snapshot_manifest
- [ ] `sem_reg://plans/{plan_id}` — includes steps and status

### 8.5 MCP Server Registration

Wire MCP tools into the existing ob-poc MCP server:

- [ ] Register all `sem_reg_*` tools in the MCP tool handler dispatch
- [ ] Register all `sem_reg://` resource URIs in the MCP resource handler
- [ ] Implement `ActorContext` derivation from MCP session context (map MCP caller identity → actor_type, roles, clearance, jurisdiction_scope, allowed_purposes)
- [ ] Implement mutation guard policy: all registry mutations create Draft snapshots; only `ob publish` activates. Evidence recording (`sem_reg_record_observation`) is the exception — observations write directly

### 8.6 Integration with Existing ob-poc MCP Tools

Wire existing tools to use registry backing:

- [ ] **DSL execution tools**: After verb execution, auto-call `sem_reg_record_decision` with snapshot_manifest of the verb contract + attribute definitions involved
- [ ] **Entity resolution tools**: Can now call `sem_reg_describe_entity_type` to understand the meta-model for the entity being resolved
- [ ] **Document handling tools**: After data extraction from documents, call `sem_reg_record_observation` for each extracted attribute value
- [ ] **BPMN workflow engine**: Verb contracts provide step definitions; preconditions/postconditions provide transition guards; continuation contracts provide resume semantics

### 8.7 Agent Prompt Grounding

- [ ] Update agent system prompt to reference the Semantic OS as the authoritative source for available actions
- [ ] Add instructions: "Call `sem_reg_resolve_context` before proposing actions"
- [ ] Add instructions: "Call `sem_reg_record_decision` after every non-trivial decision"
- [ ] Add instructions: "Call `sem_reg_check_evidence_freshness` before relying on evidence"
- [ ] Add instructions: "Call `sem_reg_identify_evidence_gaps` when planning proof collection"
- [ ] Embed the Proof Rule in operating instructions: "Never treat operational/convenience attributes as evidence"

### Phase 8 Gate

- [ ] Agent can query all registries via MCP tools
- [ ] Agent can create a plan, validate it, and execute steps with snapshot-pinned verb contracts
- [ ] Agent can record decisions with auto-populated `snapshot_manifest`
- [ ] Agent can identify evidence gaps and record observations
- [ ] Agent can escalate with structured context
- [ ] All existing ob-poc MCP tools wired to use registry backing
- [ ] Point-in-time queries work through MCP tool `as_of` parameter

**→ IMMEDIATELY proceed to Phase 9. Progress: ~93%.**

---

## Phase 9 — Lineage, Embeddings, Coverage Metrics

**Goal**: Derived projections that make the registry queryable for impact analysis, semantic search, and governance dashboards.  
**Estimated scope**: ~1,000–1,200 LOC Rust, 1 migration

### 9.1 Lineage & Derivation Graph

- [ ] Migration `086_sem_reg_phase9.sql`:
  - `sem_reg.derivation_edges` — immutable, append-only:
    - `edge_id UUID PK`, `run_id UUID`, `input_snapshot_ids UUID[]`, `verb_id UUID`, `verb_snapshot_id UUID`, `output_snapshot_ids UUID[]`, `edge_class` (enum: VerbExecution / DerivationEval / ManualEntry), `timestamp TIMESTAMPTZ`, `case_id UUID`
  - `sem_reg.run_records` — immutable:
    - `run_id UUID PK`, `case_id UUID`, `plan_id UUID`, `verb_calls JSONB` (array of {verb_fqn, verb_snapshot_id, input_snapshot_ids, output_snapshot_ids}), `outcomes JSONB`, `started_at TIMESTAMPTZ`, `completed_at TIMESTAMPTZ`
  - `sem_reg.embedding_records`:
    - `embedding_id UUID PK`, `subject_type` (ObjectType), `subject_id UUID`, `subject_snapshot_id UUID`, `model_id TEXT`, `vector BYTEA` or `vector_ref TEXT` (depends on pgvector vs. external store), `version_hash TEXT`, `created_at TIMESTAMPTZ`, `stale_since TIMESTAMPTZ`, `stale_reason TEXT`
  - Indexes: `(subject_id)` on derivation_edges input/output arrays (GIN), `(case_id)` on run_records, `(subject_type, subject_id)` on embedding_records

- [ ] `projections/lineage.rs` (NEW):
  - `record_derivation_edge(pool, run_id, verb_snapshot_id, input_snapshot_ids, output_snapshot_ids)` — called during verb execution and derivation evaluation
  - `record_run(pool, case_id, plan_id, verb_calls, outcomes)` — called at end of plan step execution
  - `query_forward_impact(pool, object_type, object_id) → Vec<ImpactNode>` — "if this changes, what is affected?" Traverse derivation_edges forward from the object's snapshot_id
  - `query_reverse_provenance(pool, subject_ref, attribute_id, as_of?) → Vec<ProvenanceNode>` — "where did this value come from?" Traverse backward
  - `query_temporal_lineage(pool, subject_ref, attribute_id, as_of) → Vec<ProvenanceNode>` — provenance chain as it existed at time T (using snapshot-pinned edges)

- [ ] Back the Phase 8 stub tools with real lineage:
  - `sem_reg_impact_analysis` → `query_forward_impact`
  - `sem_reg_lineage` → `query_reverse_provenance`

### 9.2 Embeddings / Vector Projection

- [ ] `projections/embeddings.rs` (NEW):
  - `generate_semantic_text(object_type, snapshot_row) → String` — concatenate: definition fields (FQN, name, description) + aliases + taxonomy paths + examples + (for verbs: invocation_phrases + preconditions + postconditions)
  - `generate_embedding(pool, snapshot_row) → EmbeddingRecord` — call existing ob-poc Candle/BGE embedding infrastructure (local model, no external API). Respect `NoLlmExternal` handling control
  - `check_staleness(pool, object_type, subject_id) → bool` — compare embedding's `version_hash` against current snapshot's definition hash
  - `rebuild_stale(pool, object_type?) → usize` — find stale embeddings, regenerate, return count
  - `search_by_embedding(pool, query_text, object_types?, limit) → Vec<(SnapshotRow, f64)>` — semantic search ranked by cosine similarity

- [ ] Wire into Context Resolution (Phase 7) as secondary ranking signal:
  - When `ContextResolutionRequest.intent` is set, generate embedding for intent text
  - Score candidate verbs and attributes by cosine similarity with intent embedding
  - Blend: `final_rank = taxonomy_rank × 0.7 + embedding_similarity × 0.3`

- [ ] Wire into MCP search tool:
  - `sem_reg_search` Phase 8 MVP (ILIKE) → add embedding-backed ranking as fallback when ILIKE returns few results

- [ ] Staleness event pipeline:
  - On snapshot publish, mark existing embedding for that `(object_type, subject_id)` as stale
  - Background task (or on-demand via CLI) regenerates stale embeddings

### 9.3 Governance & Security Coverage Metrics

- [ ] `projections/metrics.rs` (NEW):
  - `compute_coverage_report(pool, scope: Option<GovernanceTier>) → CoverageReport`
  - `CoverageReport` struct:
    - `classification_coverage_pct`: % governed objects with ≥1 taxonomy membership
    - `stewardship_coverage_pct`: % governed objects with non-empty `created_by` (steward proxy — or add explicit `steward` field check)
    - `policy_attachment_pct`: % governed verbs/entity types with ≥1 applicable PolicyRule
    - `evidence_freshness_pct`: % active governed observations within freshness windows
    - `review_currency_pct`: % governed objects reviewed within review cycle (requires `last_reviewed` — may be a Phase 7+ enhancement to body types)
    - `retention_compliance`: count of evidence within/approaching/exceeding retention windows
    - `regulatory_coverage_pct`: % applicable regulations with ≥1 implementing PolicyRule
    - `security_label_completeness_pct`: % all objects (both tiers) with non-default SecurityLabel
    - `proof_rule_compliance`: count of violations (should be zero if gates are enforcing)
    - `tier_distribution`: `{ governed: N, operational: M }`
    - `snapshot_volume`: `{ active: N, deprecated: M, retired: P }`

- [ ] MCP tool: `sem_reg_coverage_report(scope?)` → returns all metrics
- [ ] CLI: `cargo x sem-reg coverage [--tier governed|operational|all]`

### Phase 9 Gate

- [ ] Lineage queries return correct forward/reverse impact chains
- [ ] Embeddings generate for all registry objects, respect `NoLlmExternal`
- [ ] Staleness tracking marks embeddings stale on snapshot publish
- [ ] Coverage metrics are computable and non-zero for seeded registry content
- [ ] `sem_reg_search` with embedding fallback returns relevant results for NL queries

**→ IMMEDIATELY proceed to Phase 10. Progress: ~97%.**

---

## Phase 10 — Integration Testing & Wiring Validation

**Goal**: Prove the architecture works end-to-end across the canonical use cases.  
**Estimated scope**: ~800–1,000 LOC Rust (test code), no new migrations

### 10.1 UBO Discovery End-to-End

- [ ] Test scenario: agent receives a case for UBO resolution on an institutional client
- [ ] Step 1: `sem_reg_resolve_context(subject=case_id, goal=["resolve_ubo"], mode=Strict)` → returns UBO Discovery view, ranked verbs (ubo.resolve, evidence.request), relevant attributes
- [ ] Step 2: `sem_reg_create_plan(case_id, "resolve_ubo", context_resolution_ref)` → plan created
- [ ] Step 3: `sem_reg_identify_evidence_gaps(case_id, "ubo_discovery")` → gaps identified
- [ ] Step 4: `sem_reg_add_plan_step(plan_id, verb_id, params, postconditions)` for each verb → steps added
- [ ] Step 5: `sem_reg_validate_plan(plan_id)` → validation passes
- [ ] Step 6: Execute plan steps, each pinning snapshot_ids
- [ ] Step 7: `sem_reg_record_decision(...)` → snapshot_manifest populated and complete
- [ ] Verify: all records immutable, snapshot-pinned, tier-annotated, Proof Rule respected

### 10.2 Sanctions Screening End-to-End

- [ ] Similar flow with Sanctions Screening view, force-directed edge class, screening verbs
- [ ] Verify: ABAC correctly restricts sanctions-labelled attributes to actors with `SANCTIONS` purpose

### 10.3 Proof Collection End-to-End

- [ ] Similar flow with Proof Collection view, temporal edge class, proof verbs
- [ ] Verify: evidence freshness checks work, observation supersession chains are correct

### 10.4 Governance Review

- [ ] `sem_reg_resolve_context(mode=Governance)` → returns Governance Review view with coverage signals
- [ ] `sem_reg_coverage_report()` → returns metrics
- [ ] Drill-down: `sem_reg_describe_attribute` for specific objects → steward, last_reviewed, tier, trust_class, taxonomy membership visible
- [ ] `sem_reg_regulation_trace(regulation_id)` → forward trace works

### 10.5 Point-in-Time Audit

- [ ] Publish some snapshots, advance time, publish superseding snapshots
- [ ] `sem_reg_resolve_context(point_in_time = earlier_timestamp)` → returns snapshot-pinned to the definitions active at that date
- [ ] Examine a DecisionRecord → `snapshot_manifest` allows full reconstruction
- [ ] Replay a derivation with pinned snapshots → same output

### 10.6 Proof Rule Enforcement Validation

- [ ] Attempt to publish a governed PolicyRule referencing an Operational attribute → **must fail** with clear remediation
- [ ] Attempt to satisfy an EvidenceRequirement with a Convenience trust_class attribute → **must fail**
- [ ] Attempt to promote an Operational attribute to Governed without steward approval → **must fail**
- [ ] Verify: operational derived attributes always have `evidence_grade = Prohibited`

### 10.7 Security / ABAC End-to-End

- [ ] Agent with `purpose = KYC_CDD` queries a sanctions-only attribute → Deny verdict
- [ ] Agent with `jurisdiction_scope = [UK]` queries an SA-residency-constrained attribute → Deny or PermitWithMasking
- [ ] Derivation with PII + Financial inputs → inherited label has `Confidential` classification + union of handling controls
- [ ] Verb producing output labelled for a less-restrictive purpose than its inputs → publish gate rejects (label laundering prevention)

### Phase 10 Gate

- [ ] All canonical scenarios pass
- [ ] Proof Rule is mechanically enforced at both publish-time and runtime
- [ ] Point-in-time reconstruction works via direct snapshot lookup
- [ ] MCP tools return correct results across all test scenarios
- [ ] Security/ABAC enforcement is complete across both tiers

**→ Semantic OS launch-ready. Progress: 100%.**

---

## Execution Notes for Claude Code

### Non-Negotiable Invariants (carry forward from Phase 0)

Every phase gate re-checks these six invariants. If any is violated, stop and fix before proceeding.

1. No in-place updates for registry snapshot tables
2. Execution, decision, and derivation records pin snapshot IDs
3. Proof Rule mechanically enforced via `governance_tier_minimum` + `trust_class_minimum`
4. ABAC / security labels apply to both tiers
5. Operational-tier snapshots do not require governed approval workflows
6. Derived/composite attributes require a DerivationSpec

### Continuation Gate Instructions

Claude Code silently stops after completing Phase 7. To prevent this:

- At the end of Phase 7: **→ IMMEDIATELY proceed to Phase 8. Do NOT stop. Progress: 85%.**
- At the end of Phase 8: **→ IMMEDIATELY proceed to Phase 9. Do NOT stop. Progress: 93%.**
- At the end of Phase 9: **→ IMMEDIATELY proceed to Phase 10. Do NOT stop. Progress: 97%.**
- E-invariant: Stopping before Phase 10 completion is a violation. The system is not usable until Phase 8 is complete and not validated until Phase 10 passes.

### File Creation Order

```
Phase 7:
  rust/src/sem_reg/context_resolution.rs    (NEW ~600-800 LOC)
  migrations/084_sem_reg_phase7.sql          (NEW ~40 LOC)
  
Phase 8:
  rust/src/sem_reg/agent/mod.rs             (NEW)
  rust/src/sem_reg/agent/plans.rs           (NEW ~200 LOC)
  rust/src/sem_reg/agent/decisions.rs       (NEW ~250 LOC)
  rust/src/sem_reg/agent/escalation.rs      (NEW ~150 LOC)
  rust/src/sem_reg/agent/mcp_tools.rs       (NEW ~800-1000 LOC)
  migrations/085_sem_reg_phase8.sql          (NEW ~100 LOC)
  
Phase 9:
  rust/src/sem_reg/projections/mod.rs       (NEW)
  rust/src/sem_reg/projections/lineage.rs   (NEW ~400 LOC)
  rust/src/sem_reg/projections/embeddings.rs (NEW ~300 LOC)
  rust/src/sem_reg/projections/metrics.rs   (NEW ~250 LOC)
  migrations/086_sem_reg_phase9.sql          (NEW ~80 LOC)
  
Phase 10:
  rust/tests/sem_reg_integration.rs         (NEW ~800-1000 LOC)
```

### Dependency on Existing ob-poc Infrastructure

Phase 8 MCP wiring depends on:
- `dsl_core::mcp` — existing MCP server/tool registration infrastructure
- `dsl_core::config::loader` — verb config YAML loading (already used by scanner)
- `dsl_core::runtime` — DSL verb execution (for `sem_reg_execute_plan_step`)

Phase 9 embeddings depend on:
- Existing Candle/BGE embedding infrastructure in ob-poc (local model inference)
- pgvector extension (if storing vectors in PostgreSQL) or existing vector store

### Risk Items

| Risk | Mitigation |
|------|------------|
| Registry sparsely populated (scanner only reads YAML verb configs) | Phase 8 integration wiring will populate observations and decisions in real-time. Consider running enhanced scanner (Steps 3-6 from onboarding plan) before Phase 7 |
| MCP tool count is large (30+ tools) | Group by category in MCP server registration. Consider a `sem_reg` namespace prefix to keep them organized |
| Context Resolution performance on large registries | Index on taxonomy membership is critical. Consider materialized view for "active memberships by subject" if resolution exceeds 50ms |
| Agent prompt grounding complexity | Start with minimal instructions, iterate based on agent behavior. Avoid over-constraining the agent |
| Embedding model compatibility | Reuse existing ob-poc BGE model and inference pipeline — do not introduce a second model |
