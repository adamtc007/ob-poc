# Semantic OS — Gap Remediation Plan

**Purpose**: Peer review document identifying all gaps between `semantic-os-implementation-todo-v2.md` (the spec) and the current implementation (code + DB). Each gap has acceptance criteria and implementation guidance.

**Audit Date**: 2026-02-16
**Auditor**: Claude Code (Opus 4.6)
**Spec Reference**: `docs/semantic-os-implementation-todo-v2.md` (v2.0, February 2026)
**Implementation Reference**: `rust/src/sem_reg/` (35 files, ~13,200 LOC), migrations 078-086

---

## Audit Methodology

Every numbered item in the v2 spec was checked against:
1. Rust source code in `rust/src/sem_reg/`
2. SQL migrations 078-086
3. MCP tool dispatch in `rust/src/sem_reg/agent/mcp_tools.rs`
4. CLI commands in `rust/xtask/src/sem_reg.rs`
5. Integration test scenarios in `rust/tests/sem_reg_integration.rs`

Items marked COMPLETE are omitted. Only gaps requiring remediation are listed.

---

## Priority Classification

| Priority | Meaning |
|----------|---------|
| **P0** | Foundational invariant violation or semantic misalignment — must fix before any other work |
| **P1** | Spec-mandated feature missing — required for the architecture to function as designed |
| **P2** | Integration wiring absent — feature exists in isolation but is not connected to consumers |
| **P3** | Seed data / content gap — infrastructure exists but registries are empty |
| **P4** | Polish / completeness — CLI commands, resource URIs, prompt grounding |

---

## P0 — Foundational Invariant Violations

### P0-1: Evidence MCP Tools Are Semantically Misaligned

**Spec** (§8.3): Three evidence tools with specific contracts:
- `sem_reg_record_observation` — write an Observation (immutable, supersession chain) to `sem_reg.observations`
- `sem_reg_check_evidence_freshness` — check freshness of active observations for a subject/attribute pair against EvidenceRequirement freshness windows
- `sem_reg_identify_evidence_gaps` — compare required evidence (from PolicyRules) against available observations

**Current**: All three tools exist in dispatch but do the wrong thing:
- `sem_reg_record_observation` → writes to `sem_reg.derivation_edges` (lineage, not observations)
- `sem_reg_check_evidence_freshness` → checks embedding staleness in `sem_reg.embedding_records` (not domain evidence)
- `sem_reg_identify_evidence_gaps` → thin wrapper over `resolve_context()` filtering `required == true && present == false` attributes (no observation table lookup)

**Impact**: The Observation supersession chain (spec §3.2) is completely absent. No tool writes or reads domain observations. The names imply evidence semantics but the implementations are unrelated. This violates Invariant #2 (execution records must pin snapshot IDs — observations are the evidence that decisions reference).

**Remediation**:
1. Create the `sem_reg.observations` table (see P1-1 below)
2. Rewrite `handle_record_observation()` to INSERT into `sem_reg.observations` with supersession chain semantics
3. Rewrite `handle_check_freshness()` to query `sem_reg.observations` joined with `sem_reg.evidence` freshness windows
4. Rewrite `handle_identify_gaps()` to compare PolicyRule evidence requirements against available observations for a case/subject
5. Rename the current `handle_record_observation()` body to a new internal function `record_lineage_edge()` (its current behavior is correct for lineage — just misnamed)

**Acceptance Criteria**:
- `sem_reg_record_observation` creates a row in `sem_reg.observations` with `supersedes` chain
- Point-in-time observation query returns correct active observation at any timestamp
- `sem_reg_check_evidence_freshness` returns freshness status per observation vs EvidenceRequirement.freshness_window
- `sem_reg_identify_evidence_gaps` returns gaps: required evidence (from policy) with no matching observation

### P0-2: `check_verb_surface_disclosure` Gate Is a No-Op Stub

**Spec** (§6.1): "Verb read/write surface disclosure: every attribute a verb reads or writes must appear in its inputs/outputs/side_effects (ABAC must be evaluable)"

**Current**: `gates_technical.rs::check_verb_surface_disclosure()` builds a `declared_surface` HashSet but returns an empty `Vec<GateFailure>` unconditionally. The comparison logic is missing.

**Impact**: Verbs can reference undisclosed attributes, making ABAC enforcement incomplete. A verb could read PII attributes not listed in its I/O surface, bypassing access control.

**Remediation**: Implement the comparison: for each attribute referenced in verb implementation (from scanner metadata or side-effects), check it appears in `consumes` or `produces` or `side_effects`. Emit `GateFailure` for undisclosed references.

**Acceptance Criteria**:
- Verb with undisclosed attribute reference → `GateFailure` emitted
- Verb with fully disclosed surface → empty failures
- Gate integrated into `ob publish` pipeline

### P0-3: `check_type_correctness` Gate Is Largely Stub

**Spec** (§6.1): "Type correctness: verb contract I/O types match attribute dictionary type_specs"

**Current**: `gates_technical.rs::check_type_correctness()` only checks the `consumes` arm. The `entity_type_fqn` lookup arm is a no-op (`let _ = entity_type_fqn`). The `produces` check is also a no-op. Only `consumes` produces real warnings.

**Remediation**: Implement `produces` and `entity_type` arms:
- For each `produces` entry, verify the attribute FQN exists in the dictionary
- For `entity_type_fqn`, verify the entity type is registered
- Check type compatibility between verb I/O type_spec and attribute type_spec

**Acceptance Criteria**:
- Verb producing an unregistered attribute → `GateFailure`
- Verb referencing an unregistered entity type → `GateFailure`
- Type mismatch between verb I/O and attribute type_spec → `GateFailure`

### P0-4: `EvidenceMode::Normal` Does Not Check `view.includes_operational`

**Spec** (§7.3): "Normal: Governed + Proof/DecisionSupport primary. Operational only if view.includes_operational = true. Operational candidates always flagged `usable_for_proof = false`."

**Current**: `context_resolution.rs::tier_allowed()` returns `true` unconditionally for `EvidenceMode::Normal`. It does not check the view's `includes_operational` flag. All operational candidates pass regardless of view configuration.

**Remediation**: Pass the active `ViewDef` (or its `includes_operational` flag) into `tier_allowed()`. In Normal mode:
- Governed + Proof/DecisionSupport → allow
- Governed + Convenience → allow (tagged `usable_for_proof = false`)
- Operational → allow ONLY if `view.includes_operational == true`, tagged `usable_for_proof = false`
- Operational when `includes_operational == false` → deny

**Acceptance Criteria**:
- View with `includes_operational = false` in Normal mode → operational candidates filtered out
- View with `includes_operational = true` in Normal mode → operational candidates included with `usable_for_proof = false`
- Strict mode behavior unchanged (already correct)

---

## P1 — Missing Spec-Mandated Features

### P1-1: Evidence Instance Tables Absent (Spec §3.2)

**Spec**: Four instance-layer tables:
- `sem_reg.observations` — immutable INSERT-only, supersession chain via `supersedes` column
- `sem_reg.document_instances` — document lifecycle (received, validated, expired)
- `sem_reg.provenance_edges` — from_ref → to_ref with method (Human/OCR/API/Derived/Attested)
- `sem_reg.retention_policies` — retention windows tied to DocumentTypeDef

**Current**: None exist. No migration creates them. The spec's `sem_reg.observations` is distinct from `ob-poc.attribute_observations` (which exists in the master schema for a different purpose).

**Remediation**: New migration (`089_sem_reg_evidence_instances.sql` or next available number):

```sql
CREATE TABLE sem_reg.observations (
    obs_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_ref UUID NOT NULL,
    attribute_id UUID NOT NULL,       -- FK to snapshots(object_id) WHERE object_type = 'attribute_def'
    value_ref JSONB NOT NULL,
    source TEXT NOT NULL,
    confidence NUMERIC(3,2) CHECK (confidence BETWEEN 0 AND 1),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    supporting_doc_ids UUID[] DEFAULT '{}',
    governance_tier VARCHAR(20) NOT NULL DEFAULT 'operational',
    security_label JSONB,
    supersedes UUID REFERENCES sem_reg.observations(obs_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_obs_subject_attr_ts ON sem_reg.observations(subject_ref, attribute_id, timestamp DESC);

CREATE TABLE sem_reg.document_instances (
    doc_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_type_id UUID NOT NULL,        -- FK to snapshots(object_id) WHERE object_type = 'document_type_def'
    storage_ref TEXT,
    extracted_fields JSONB DEFAULT '{}',
    source_actor TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    validated_at TIMESTAMPTZ,
    expiry TIMESTAMPTZ,
    retention_until TIMESTAMPTZ,
    security_label JSONB,
    status VARCHAR(20) NOT NULL DEFAULT 'received'
);

CREATE TABLE sem_reg.provenance_edges (
    edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_ref UUID NOT NULL,
    to_ref UUID NOT NULL,
    method VARCHAR(20) NOT NULL CHECK (method IN ('Human','OCR','API','Derived','Attested')),
    verb_id UUID,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confidence NUMERIC(3,2)
);
CREATE INDEX idx_prov_from ON sem_reg.provenance_edges(from_ref);
CREATE INDEX idx_prov_to ON sem_reg.provenance_edges(to_ref);

CREATE TABLE sem_reg.retention_policies (
    retention_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_type_id UUID NOT NULL,
    retention_window_days INTEGER NOT NULL,
    jurisdiction VARCHAR(10),
    regulatory_reference JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Rust types**: Add `Observation`, `DocumentInstance`, `ProvenanceEdge`, `RetentionPolicy` structs in a new `rust/src/sem_reg/evidence_instances.rs` module.

**Acceptance Criteria**:
- All four tables exist and are queryable
- Observation supersession chain: insert obs1, insert obs2 with `supersedes = obs1.obs_id` → point-in-time query at obs1.timestamp returns obs1, at obs2.timestamp returns obs2
- Provenance edge traversal: forward (from_ref → to_refs) and reverse (to_ref → from_refs)

### P1-2: `RelationshipTypeDefBody` Is a Phantom Variant

**Spec** (§1.2): "RelationshipTypeDef per v1.1 §9.2: rel_type_id, snapshot_id, name, description, source_entity_type, target_entity_type, edge_class, directionality, cardinality, constraints, semantics, governance_tier, security_label, steward, version, snapshot meta fields"

**Current**: `ObjectType::RelationshipTypeDef` enum variant exists in `types.rs` (wire name `"relationship_type_def"`). But there is no `RelationshipTypeDefBody` struct, no module file, no CLI command, no scanner support, and no registry convenience method.

**Remediation**:
1. Create `rust/src/sem_reg/relationship_type_def.rs` with:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct RelationshipTypeDefBody {
       pub name: String,
       pub description: Option<String>,
       pub source_entity_type: String,  // FQN
       pub target_entity_type: String,  // FQN
       pub edge_class: EdgeClass,
       pub directionality: Directionality,
       pub cardinality: Cardinality,
       pub constraints: Vec<serde_json::Value>,
       pub semantics: Option<String>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum EdgeClass { Structural, Derivation, Reference, Association, Temporal }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum Directionality { Unidirectional, Bidirectional }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum Cardinality { OneToOne, OneToMany, ManyToMany }
   ```
2. Add `mod relationship_type_def;` to `mod.rs`
3. Add `publish_relationship_type_def()` / `resolve_relationship_type_def()` to `RegistryService`
4. Add `ob reg rel describe` CLI command
5. Update scanner to detect FK relationships and emit `RelationshipTypeDef` candidates (Phase 1.4 Step 4)

**Acceptance Criteria**:
- `RelationshipTypeDefBody` serializes/deserializes to JSONB in `sem_reg.snapshots`
- `RegistryService` can publish and resolve relationship types
- CLI `sem-reg rel-describe <fqn>` works

### P1-3: Five Publish Gates Missing (Spec §6.1-6.2)

**Spec** defines these gates. **Current** status:

| Gate | Spec Section | Status |
|------|-------------|--------|
| Continuation completeness (DurableStart ↔ DurableResume) | §6.1 | MISSING |
| Macro expansion integrity (ExpansionContract → registered verbs) | §6.1 | MISSING |
| Regulatory linkage (governed PolicyRules need RegulatoryReference) | §6.2 | MISSING |
| Review-cycle compliance (governed objects reviewed within cycle) | §6.2 | MISSING |
| Version consistency (breaking changes need compatibility analysis) | §6.2 | MISSING |

**Remediation**: Add five new gate functions in `gates_governance.rs` and/or `gates_technical.rs`:

1. **`check_continuation_completeness`**: For verbs with `exec_mode = DurableStart`, verify a corresponding `DurableResume` verb exists with compatible correlation keys.
2. **`check_macro_expansion_integrity`**: For verbs with expansion contracts, verify all expansion target verbs are registered with compatible I/O.
3. **`check_regulatory_linkage`**: For governed PolicyRules, verify at least one `RegulatoryReference` is present.
4. **`check_review_cycle_compliance`**: For governed objects, verify `last_reviewed` is within the review cycle window. (Report-only initially.)
5. **`check_version_consistency`**: For breaking changes (major version bump), verify predecessor compatibility analysis was performed.

**Acceptance Criteria**:
- Each gate returns structured `GateFailure` with remediation hint
- Gates 1-2 are technical (both tiers)
- Gates 3-5 are governance (governed tier only, report-only initially)
- All gates wired into the publish pipeline

### P1-4: Onboarding Pipeline Steps 2-6 Missing (Spec §1.4.2-1.4.7)

**Spec** (§1.4): Six-step onboarding pipeline. Step 1 (verb inventory scan) exists in `scanner.rs`. Steps 2-6 are absent:
- Step 2: Attribute extraction from verb surfaces
- Step 3: Schema cross-reference against PostgreSQL operational tables
- Step 4: Entity type inference from table/FK structure
- Step 5: Seed all three registries with connected subgraph
- Step 6: Identify and classify orphans (Framework / UI / Genuine / Dead)

**Current**: `scanner.rs` scans verb YAML and seeds `AttributeDef`, `EntityTypeDef`, `VerbContract` snapshots. But it does not cross-reference against the PostgreSQL schema, infer entity types from FK relationships, or classify orphans.

**Remediation**: This is the largest P1 item. Create `rust/src/sem_reg/onboarding/` module with:
- `verb_scan.rs` — refactor existing `scanner.rs` scan logic
- `attr_extract.rs` — Step 2: extract attributes from verb I/O surfaces
- `schema_xref.rs` — Step 3: query `information_schema.columns` and `information_schema.table_constraints` to cross-reference
- `entity_infer.rs` — Step 4: group columns by table → infer entity types, detect FK → infer relationships
- `seed.rs` — Step 5: produce registry snapshots from the connected subgraph
- `orphans.rs` — Step 6: classify unreferenced columns into Framework/UI/Genuine/Dead
- `report.rs` — onboarding status report (seeded counts, orphans, wiring completeness)

**CLI commands**:
- `cargo x sem-reg onboard scan` — Steps 1-4, produce report without writing
- `cargo x sem-reg onboard apply` — Step 5, seed registries
- `cargo x sem-reg onboard report` — display current status
- `cargo x sem-reg onboard verify` — verify verb I/O maps to registered AttributeDefs

**Acceptance Criteria**:
- `cargo x sem-reg onboard scan` produces a report showing verb-connected attributes, inferred entity types, and orphan classification
- `cargo x sem-reg onboard apply` seeds all three registries with connected subgraph
- `cargo x sem-reg onboard verify` reports wiring completeness >= 80% for existing verbs

---

## P2 — Integration Wiring Gaps

### P2-1: Existing Tool Execution Does Not Pin Snapshot IDs (Spec §8.5)

**Spec** (§8.5): "Existing DSL execution tools now pin snapshot_ids when executing verbs; call `sem_reg_record_decision` after decisions"

**Current**: Neither `agent/orchestrator.rs` nor `api/agent_service.rs` reference `DecisionRecord`, `record_decision`, or `snapshot_manifest`. Verb execution does not automatically pin snapshot IDs. The `DecisionRecord` type exists and the MCP tool can write it, but it is never called automatically.

**Remediation**:
1. In the unified orchestrator (`agent/orchestrator.rs`), after successful verb execution via the pipeline, look up the verb's `VerbContract` snapshot_id from the registry
2. For non-trivial decisions (verb execution, not navigation), create a `DecisionRecord` with:
   - `snapshot_manifest`: map of all contributing definition IDs → their snapshot IDs
   - `chosen_action`: the executed verb
   - `policy_verdicts`: from SemReg `resolve_context` if available
3. Feature-gate this behind `sem-reg-decision-audit` to allow gradual rollout
4. Start with recording only — no blocking on missing snapshots

**Acceptance Criteria**:
- With feature flag enabled, every non-trivial verb execution produces a `DecisionRecord` in `sem_reg.decision_records`
- `snapshot_manifest` contains at minimum the verb_contract's snapshot_id
- Existing pipeline latency not measurably impacted (async insert, best-effort)

### P2-2: MCP Resource Surface Absent (Spec §8.4)

**Spec** (§8.4): Nine `sem_reg://` resource URI patterns for MCP `resources/list` and `resources/read`.

**Current**: Zero MCP resource handlers exist. The MCP server only implements `tools/list` and `tools/call`. No `resources/list`, no `resources/read`, no `sem_reg://` URIs.

**Remediation**:
1. Add `resources/list` handler to MCP server that returns the 9 resource URI templates
2. Add `resources/read` handler that dispatches by URI prefix:
   - `sem_reg://attributes/{id}` → resolve active AttributeDef
   - `sem_reg://verbs/{name}` → resolve active VerbContract
   - `sem_reg://entities/{name}` → resolve active EntityTypeDef
   - `sem_reg://policies/{id}` → resolve active PolicyRule
   - `sem_reg://views/{name}` → resolve active ViewDef
   - `sem_reg://taxonomies/{name}` → resolve active TaxonomyDef
   - `sem_reg://observations/{subject}/{attr}` → current observation chain (requires P1-1)
   - `sem_reg://decisions/{id}` → DecisionRecord
   - `sem_reg://plans/{id}` → AgentPlan with steps
3. Support `?as_of=<timestamp>` query parameter for point-in-time resolution
4. Apply ABAC enforcement via `enforce_read()` on all resource reads

**Acceptance Criteria**:
- `resources/list` returns 9 resource templates
- `resources/read` with valid URI returns the resolved object as JSON
- `?as_of` parameter returns point-in-time resolution
- ABAC denials return redacted stubs (same pattern as tool enforcement)

### P2-3: Agent Prompt Grounding Absent (Spec §8.6)

**Spec** (§8.6): "Update agent system prompts / MCP server configuration to reference the Semantic OS as the authoritative source" with specific instructions to call `sem_reg_resolve_context` before proposing actions, `sem_reg_record_decision` after decisions, etc.

**Current**: Zero references to `sem_reg` or "Semantic Registry" in any LLM prompt template. Searched all `*.rs` files — no prompt strings contain sem_reg guidance.

**Remediation**:
1. Add a `SEMANTIC_OS_INSTRUCTIONS` constant to the agent prompt configuration (e.g., in `agent/orchestrator.rs` or a dedicated prompt module)
2. Content should include:
   - "The Semantic Registry is the authoritative source for available actions, their requirements, and their outputs"
   - "Call `sem_reg_resolve_context` before proposing actions to get registry-backed recommendations"
   - "Call `sem_reg_record_decision` after every non-trivial decision"
   - "Check `sem_reg_check_evidence_freshness` before relying on evidence"
   - "Never treat operational/convenience attributes as evidence — always verify governance_tier and trust_class"
3. Wire into the system prompt for MCP-connected LLM calls
4. Feature-gate behind `sem-reg-prompt-grounding` for gradual rollout

**Acceptance Criteria**:
- With feature flag enabled, agent system prompt includes Semantic OS instructions
- Instructions are appended (not replace) existing prompt content
- grep for `sem_reg_resolve_context` in prompt templates returns at least one match

### P2-4: Embedding Ranking Not Wired Into Context Resolution (Spec §9.2)

**Spec** (§9.2): "Wire into Context Resolution (Phase 7) as a secondary ranking signal alongside taxonomy-driven filtering"

**Current**: The `intent` field on `ContextResolutionRequest` is accepted but silently ignored. No embedding-based ranking is applied. `resolve_context()` uses taxonomy overlap and prominence weights only.

**Remediation**:
1. When `req.intent` is `Some(text)`:
   - Embed the intent text using the local Candle/BGE embedder
   - For each candidate verb/attribute, compute cosine similarity against stored embeddings in `sem_reg.embedding_records`
   - Blend embedding similarity as a secondary ranking signal (e.g., 0.2 weight) alongside taxonomy-based ranking (0.8 weight)
2. When embeddings are stale or missing, fall back to taxonomy-only ranking (graceful degradation)
3. Track `NoLLMExternal` handling: attributes with this constraint use internal model only

**Acceptance Criteria**:
- `resolve_context` with `intent = Some("check ownership")` returns different ranking than `intent = None`
- Missing/stale embeddings → falls back to taxonomy-only (no error)
- `NoLLMExternal` attributes excluded from external model inference

---

## P3 — Registry Content Gaps

### P3-1: Zero Taxonomy Seed Data

**Spec** (§2.1): Seed four canonical taxonomies: KYC Review Navigation, Regulatory Domain, Data Sensitivity, Execution Semantics.

**Current**: No taxonomy snapshots exist in `sem_reg.snapshots`. The `TaxonomyDefBody` and `TaxonomyNodeBody` types exist. The scanner does not seed taxonomies.

**Remediation**: Create a seeding script or extend the scanner to seed the four canonical taxonomies with their node hierarchies. Can be done as a YAML-driven seed file loaded by `cargo x sem-reg seed-taxonomies`.

**Acceptance Criteria**:
- 4 taxonomy snapshots with status=Active in `sem_reg.snapshots`
- Each taxonomy has its node hierarchy as TaxonomyNode snapshots
- `cargo x sem-reg taxonomy-tree <name>` displays the tree

### P3-2: Zero View Definition Seed Data

**Spec** (§2.2): Seed six canonical views: UBO Discovery, Sanctions Screening, Proof Collection, Case Overview, Governance Review, Operational Setup.

**Current**: No view snapshots exist. `ViewDefBody` type exists.

**Remediation**: Create seed data for the six views with their taxonomy slices, verb surfaces, and attribute prominence configurations.

**Acceptance Criteria**:
- 6 view snapshots with status=Active
- Each view has `verb_surface`, `attribute_prominence`, and `taxonomy_slices` populated
- `cargo x sem-reg view-describe <name>` works

### P3-3: Zero Policy Rule Seed Data

**Spec** (§3.1): "Scan Rust code for hardcoded business rules... Extract into PolicyRule snapshots"

**Current**: No policy snapshots exist. `PolicyRuleBody` type exists.

**Remediation**: Extract initial policy rules from existing compliance logic:
- Jurisdiction-dependent rules from verb preconditions
- Risk-tier-dependent rules from gateway conditions
- Document presence requirements from KYC workflow guards
- Seed as Operational/Convenience initially, flag for governance review

**Acceptance Criteria**:
- At least 5-10 policy snapshots seeded from extracted business rules
- Each has `predicate` expression, `enforcement` level, and `evidence_requirements`

### P3-4: Zero Derivation Spec Seed Data

**Spec** (§5.4): "Scan existing ob-poc for computed/derived fields... Extract into DerivationSpec"

**Current**: No derivation snapshots exist. `DerivationSpecBody` type exists.

**Remediation**: Identify existing computed fields (risk scores, completeness percentages, derived statuses) and create `DerivationSpec` snapshots with `function_ref` expressions pointing to existing Rust functions.

**Acceptance Criteria**:
- At least 3-5 derivation specs seeded from existing computed fields
- Each has `input` attribute references and `function_ref` expression
- Lineage graph shows input → derivation → output edges

### P3-5: Membership Wiring From Onboarding Data (Spec §2.3)

**Spec** (§2.3): "Classify the verb-first seeded objects using the onboarding report as input" — auto-assign taxonomy memberships based on attribute patterns, verb exec_mode, entity types.

**Current**: No memberships exist. The `MembershipRuleBody` type exists.

**Remediation**: After P3-1 (taxonomy seeds) and P1-4 (onboarding pipeline), run auto-classification:
- Data Sensitivity: PII patterns → PII node; financial columns → Financial node
- Execution Semantics: verb exec_mode → Sync/Research/Durable/Macro nodes
- KYC Review Navigation: entity types → Ownership & Control / Evidence & Proofs / etc.

**Acceptance Criteria**:
- Seeded attributes/verbs have at least one taxonomy membership each
- `sem_reg_taxonomy_members` returns non-empty results for canonical taxonomy nodes

---

## P4 — Polish and Completeness

### P4-1: Missing CLI Commands

**Spec lists these CLI commands. Current status:**

| Command | Status |
|---------|--------|
| `sem-reg taxonomy-tree <name>` | MISSING |
| `sem-reg taxonomy-members <node>` | MISSING |
| `sem-reg onboard scan` | MISSING (covered by P1-4) |
| `sem-reg onboard apply` | MISSING (covered by P1-4) |
| `sem-reg onboard report` | MISSING (covered by P1-4) |
| `sem-reg onboard verify` | MISSING (covered by P1-4) |
| `sem-reg rel-describe <fqn>` | MISSING (covered by P1-2) |
| `sem-reg publish` | MISSING |
| `sem-reg classify` | MISSING |

**Remediation**: Add to `rust/xtask/src/sem_reg.rs`. The `publish` command should run all gates against a draft snapshot and promote to Active on success.

**Acceptance Criteria**: Each command executes without error and produces meaningful output.

### P4-2: `ob publish` / `sem-reg publish` Command Does Not Exist (Spec §6)

**Spec** (§6): "Wire the Semantic OS registries into the existing `ob publish` pipeline as enforcement gates."

**Current**: No `ob publish` or `cargo x sem-reg publish` command exists. The `SnapshotStore::publish_snapshot()` method exists for internal use, but there is no user-facing CLI for gate-checked publishing.

**Remediation**: Create `cargo x sem-reg publish <object_type> <fqn>` that:
1. Loads the draft snapshot
2. Runs all applicable gates (technical + governance based on tier)
3. On success: promotes to Active
4. On failure: prints structured gate failures with remediation hints

**Acceptance Criteria**:
- `cargo x sem-reg publish attribute_def foo.bar` runs gates and promotes on pass
- Gate failure produces structured output with remediation hints
- Governed-tier objects fail without steward/approval

### P4-3: Security Label Backfill Needs Seed Templates (Spec §4.4)

**Spec** (§4.4): "Define label templates for common patterns: `standard_pii_uk`, `standard_pii_eu`, `standard_financial_global`, `sanctions_restricted`, `operational_internal`"

**Current**: The `backfill-labels` CLI command exists (publishes successor snapshots, correctly immutable). But no template system exists — labels are applied with a default template only.

**Remediation**: Define 5+ label templates as constants or YAML configuration. The backfill command should apply templates based on attribute patterns (PII columns → `standard_pii_*`, sanctions → `sanctions_restricted`, etc.).

**Acceptance Criteria**:
- At least 5 label templates defined
- `backfill-labels` applies templates based on attribute classification
- Cross-validation against Data Sensitivity taxonomy membership

---

## Implementation Order

The dependency chain dictates this order:

```
P0-2, P0-3 (stub gates)                     ← Fix immediately, no dependencies
    ↓
P0-4 (tier_allowed fix)                      ← Requires view seed data ideally, but can fix logic first
    ↓
P1-1 (evidence instance tables)              ← Schema + Rust types
    ↓
P0-1 (evidence tool realignment)             ← Requires P1-1 tables
    ↓
P1-2 (RelationshipTypeDefBody)               ← Independent
    ↓
P1-3 (5 missing gates)                       ← Independent, but useful after P1-2
    ↓
P3-1, P3-2 (taxonomy + view seeds)           ← Content, independent of code changes
    ↓
P3-3, P3-4 (policy + derivation seeds)       ← Content, independent
    ↓
P3-5 (membership wiring)                     ← Requires P3-1
    ↓
P1-4 (onboarding pipeline Steps 2-6)         ← Largest item, can parallel with P3
    ↓
P2-1 (execution snapshot pinning)            ← Integration, requires registry content
    ↓
P2-2 (MCP resource surface)                  ← Integration
    ↓
P2-3 (agent prompt grounding)                ← Integration
    ↓
P2-4 (embedding ranking in resolution)       ← Requires embeddings populated
    ↓
P4-1, P4-2, P4-3 (CLI + publish + labels)   ← Polish
```

**Estimated scope**: ~15 remediation items across 4 priority bands. P0 items are surgical fixes (1-2 days each). P1 items are feature work (P1-1: 2 days, P1-2: 1 day, P1-3: 2 days, P1-4: 5-8 days). P2 items are integration wiring (1-2 days each). P3 items are content seeding (1-2 days each). P4 items are polish (1 day each).

---

## Verification Commands

After remediation, these commands should all pass:

```bash
# Unit tests
cargo test --features vnext-repl -- sem_reg

# Gate tests
cargo test --features vnext-repl -- gates_technical::tests
cargo test --features vnext-repl -- gates_governance::tests

# Static invariant checks
cargo test --features vnext-repl -- test_no_unwrap_in_runbook  # existing

# Integration tests (requires DATABASE_URL)
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test sem_reg_integration -- --ignored --nocapture

# CLI verification
cargo x sem-reg stats
cargo x sem-reg validate --enforce
cargo x sem-reg taxonomy-tree "KYC Review Navigation"
cargo x sem-reg coverage --tier all

# Onboarding verification (requires P1-4)
cargo x sem-reg onboard scan
cargo x sem-reg onboard verify
```

---

## Appendix: Current vs Spec Module Structure

| Spec Module | Current File | Status |
|---|---|---|
| `snapshot.rs` | `store.rs` | Present (different name) |
| `types.rs` | `types.rs` | Present |
| `security.rs` | `security.rs` + `abac.rs` | Present (split into two) |
| `governance.rs` | `gates.rs` + `gates_governance.rs` + `gates_technical.rs` | Present (split into three) |
| `registries/attributes.rs` | `attribute_def.rs` | Present (flat, not nested) |
| `registries/entities.rs` | `entity_type_def.rs` | Present (flat) |
| `registries/verbs.rs` | `verb_contract.rs` | Present (flat) |
| `registries/taxonomies.rs` | `taxonomy_def.rs` + `membership.rs` | Present (flat) |
| `registries/views.rs` | `view_def.rs` | Present (flat) |
| `registries/policies.rs` | `policy_rule.rs` | Present (flat) |
| `registries/evidence.rs` | `evidence.rs` + `observation_def.rs` + `document_type_def.rs` | Present (flat, split) |
| `registries/derivations.rs` | `derivation.rs` + `derivation_spec.rs` | Present (flat, split) |
| `context_resolution.rs` | `context_resolution.rs` | Present |
| `gates.rs` | `gates.rs` + `gates_governance.rs` + `gates_technical.rs` | Present (split) |
| `onboarding/` (6 files) | `scanner.rs` (Step 1 only) | **PARTIAL — Steps 2-6 missing** |
| `agent/` | `agent/` (5 files) | Present |
| `projections/` | `projections/` (4 files) | Present |
| `cli.rs` | `xtask/src/sem_reg.rs` | Present (different location) |
| — | `enforce.rs` | Extra (ABAC tool enforcement, hardening patch) |
| — | `ids.rs` | Extra (deterministic UUID v5, hardening patch) |
| — | `registry.rs` | Extra (typed publish/resolve facade) |

The flat module layout is an acceptable divergence from the spec's nested `registries/` structure. The split of security/gates/evidence into multiple files is a positive divergence (better separation of concerns).
