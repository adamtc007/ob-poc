# ob-poc Semantic OS — Implementation Plan

**Multi-Phase TODO for Claude Code**

Version: 2.0 — February 2026
Reference: `semantic-os-v1.1.md` (the architecture vision this plan implements — ensure this exact filename is accessible to Claude Code)
Author: Adam / Lead Solution Architect

---

## Foundational Invariants (NON-NEGOTIABLE)

These six invariants must hold across every phase. If any invariant is violated during implementation, stop and fix before proceeding. Every phase gate checks these.

1. **No in-place updates for registry snapshot tables.** Every change produces a new immutable snapshot. There is no UPDATE path for registry rows — only INSERT of new snapshots and UPDATE of the `effective_until` pointer on the predecessor. This invariant applies to **registry snapshot tables** (attribute_defs, verb_contracts, entity_type_defs, policy_rules, etc.). Operational instance tables (document_instances, observations, cases, entity instances) may update per their own lifecycle rules, but any semantic definition they reference is snapshot-pinned — the reference is to a specific `snapshot_id`, not to a mutable record.

2. **Execution, decision, and derivation records must pin snapshot IDs.** Every verb execution record, every decision record, every derivation evaluation record carries the `snapshot_id` of every definition, contract, and policy that participated. The `snapshot_manifest` on DecisionRecord is mandatory, not optional.

3. **The Proof Rule is mechanically enforced via `governance_tier_minimum` + `trust_class_minimum` on evidence requirements and `predicate_trust_minimum` on policy predicates.** No interpretive enforcement. The gate checks tier and trust_class of every referenced attribute against these minimums and rejects on mismatch.

4. **ABAC / security labels apply to both tiers.** `governance_tier` affects governance workflow rigour (stewardship, classification, review cycles). It does NOT affect security posture. An operational PII field is masked, residency-constrained, and export-controlled identically to a governed PII field.

5. **Operational-tier snapshots must not require governed approval workflows.** Operational snapshots use auto-approve semantics (`approved_by = "auto"`, still recorded). Do not wire approval gates that block operational iteration.

6. **Derived/composite attributes require a DerivationSpec.** No ad-hoc derived values without a registered, snapshotted, lineage-tracked derivation recipe. Security inheritance is computed from inputs. `evidence_grade = Prohibited` for operational derivations.

---

## Phase 0 — Snapshot Infrastructure & Core Types

**Goal**: establish the immutable snapshot data model and core Rust types that every subsequent phase builds on. Nothing else ships until this is solid.

### 0.1 Snapshot base types

Define the core snapshot trait and types that all registry objects implement:

- `SnapshotMeta` struct: `snapshot_id` (UUID), `object_id` (UUID), `version` (semver major.minor), `status` (Draft/Active/Deprecated/Retired), `effective_from` (timestamptz), `effective_until` (Option<timestamptz>), `predecessor_snapshot_id` (Option<UUID>), `change_type` (Created/NonBreaking/Breaking/Promotion/Deprecation/Retirement), `change_rationale` (text), `changed_by` (text), `approved_by` (text — "auto" for operational tier)
- `GovernanceTier` enum: `Governed | Operational`
- `TrustClass` enum: `Proof | DecisionSupport | Convenience`
- `SnapshotStatus` enum: `Draft | Active | Deprecated | Retired`
- `ChangeType` enum as above
- Trait `Snapshottable` with methods for creating new snapshots, resolving current active, resolving at point-in-time
- Validation: `Operational` tier cannot have `TrustClass::Proof`

### 0.2 Snapshot database infrastructure

PostgreSQL tables and queries:

- Design the snapshot storage pattern that will be used by ALL registry tables. Each registry table follows the same pattern: composite primary key on `(snapshot_id)`, indexed on `(object_id, effective_from, effective_until)` for point-in-time resolution.
- Create a shared function/query pattern: `resolve_active(object_id, as_of timestamptz) -> snapshot row` — the fundamental query primitive.
- Create a shared function: `publish_snapshot(object_id, new_snapshot) -> snapshot_id` — inserts new row, sets `effective_until` on predecessor.
- Index strategy: B-tree on `(object_id, effective_from DESC)` with partial index on `effective_until IS NULL` for "current active" fast path.

### 0.3 Security label types

- `SecurityLabel` struct per v1.1 §5.2: confidentiality, data_category, jurisdiction_tags, regional_handling, purpose_tags (with "allowed purposes" semantics per v1.1), residency_class, handling_requirements, audit_level
- `ActorContext` struct per v1.1 §5.3
- `AccessDecision` struct per v1.1 §5.3
- `SecurityLabel` stored as JSONB column on snapshot rows (all registry tables carry this)

### 0.4 Wire up to existing ob-poc crate structure

- Determine where the semantic registry module lives relative to existing crate layout
- The registry module must be importable by: DSL parser/compiler, publish pipeline, CLI, agent/MCP layer, and any future rendering consumer
- Create `sem_reg` module (or equivalent) with sub-modules: `snapshot`, `types`, `security`, `governance`

**Phase gate**: snapshot infrastructure compiles, `resolve_active` and `publish_snapshot` work against a test table, security label types serialise/deserialise correctly. → IMMEDIATELY proceed to Phase 1.

---

## Phase 1 — Core Registries: Attributes, Entities, Verbs

**Goal**: the three primary registries exist as snapshotted, queryable objects. No gates yet, no Context Resolution yet — just the data layer.

### 1.1 Attribute Dictionary

PostgreSQL table `sem_reg.attribute_defs` following snapshot pattern:

- All fields from v1.1 §9.1: attribute_id, snapshot_id, name, display_name, description, kind (Primitive/Captured/ExternalSourced/Derived/Composite), type_spec (JSONB), constraints (JSONB array), governance_tier, trust_class, security_label (JSONB), jurisdiction_tags, aliases (JSONB array), examples, steward, last_reviewed, status, version, effective_from, effective_until, predecessor_snapshot_id, change_type, change_rationale, changed_by, approved_by

- `AttributeConstraint` sub-type: kind, expression, severity, description, data_quality_dimension
- `AttributeAlias` sub-type: text, locale, domain, weight

Rust types: `AttributeDef`, `AttributeConstraint`, `AttributeAlias`

CLI commands:
- `ob reg attr describe <name_or_id>` — resolve current active snapshot, display full definition
- `ob reg attr list [--filter ...]` — list current active attributes
- `ob reg attr history <name_or_id>` — show all snapshots for this attribute identity

### 1.2 Entity & Relationship Model

PostgreSQL tables `sem_reg.entity_type_defs`, `sem_reg.relationship_type_defs` following snapshot pattern.

- EntityTypeDef per v1.1 §9.2: entity_type_id, snapshot_id, name, description, attributes (JSONB array of {attribute_id, role, required}), identity_keys, lifecycle_states, governance_tier, security_label, steward, status, version, snapshot meta fields
- RelationshipTypeDef per v1.1 §9.2: rel_type_id, snapshot_id, name, description, source_entity_type, target_entity_type, edge_class (Structural/Derivation/Reference/Association/Temporal), directionality, cardinality, constraints, semantics, governance_tier, security_label, steward, version, snapshot meta fields
- `EdgeClass` enum: `Structural | Derivation | Reference | Association | Temporal`

CLI: `ob reg entity describe`, `ob reg rel describe`

### 1.3 Verb Dictionary

PostgreSQL table `sem_reg.verb_contracts` following snapshot pattern.

All fields from v1.1 §9.3:
- VerbContract: verb_id, snapshot_id, canonical_name, display_name, description, exec_mode, inputs (JSONB), outputs (JSONB), side_effects (JSONB), preconditions (JSONB), postconditions (JSONB), continuation (JSONB nullable), expansion (JSONB nullable), governance_tier, trust_class, security_label, steward, status, version, snapshot meta fields
- `ExecMode` enum: `Sync | Research | DurableStart | DurableResume`
- Sub-types: `VerbIO`, `SideEffect`, `Precondition`, `Postcondition`, `ContinuationContract`, `ExpansionContract` — all as Rust types serialised to JSONB

CLI: `ob reg verb describe <canonical_name>`, `ob reg verb list`, `ob reg verb history`

### 1.4 Targeted Dictionary Onboarding (verb-first, schema-correlated)

The registries are useless empty, but bulk-importing every column from 92+ tables produces noise. The onboarding strategy is: **start from verbs, trace to the attributes they touch, cross-reference against schema, infer entity structure from the connected subgraph.** This produces a meaningful, connected seed — not a dump of 2000 columns where 1400 are irrelevant to the agent.

#### 1.4.1 Step 1: Verb inventory

Scan the existing ob-poc codebase for all registered verb definitions:

- DSL verb definitions (parsed by nom combinators) → canonical_name, inputs, outputs, execution mode
- Macro verbs / expansion definitions → expansion targets, I/O mappings
- Durable verbs / continuations → correlation keys, resume signals
- Research verbs → query patterns, expected return types
- BPMN task definitions that map to verbs → preconditions implied by workflow transitions

Produce: a flat inventory of `(verb_canonical_name, declared_inputs[], declared_outputs[], exec_mode, expansion?, continuation?)` — the verb surface of the existing platform.

#### 1.4.2 Step 2: Attribute extraction from verb surfaces

For each verb in the inventory, extract the attributes it reads and writes:

- `declared_inputs[]` → these are attributes the verb consumes. Each becomes an AttributeDef candidate.
- `declared_outputs[]` → these are attributes the verb produces. Each becomes an AttributeDef candidate.
- Side-effects (writes to entity state, produces events) → these are implicit attributes. Extract from the verb implementation if the declaration is incomplete.

Produce: a set of `(attribute_name, producing_verbs[], consuming_verbs[], inferred_type?)` — the **verb-connected attribute universe**. These are the attributes that matter because something in the platform reads or writes them.

#### 1.4.3 Step 3: Schema cross-reference

For each verb-connected attribute, look it up in the PostgreSQL operational schema:

- Match attribute names against column names in the 92+ tables (fuzzy match on name + table context)
- Extract SQL type → map to `type_spec` (varchar → String, numeric(p,s) → Decimal, boolean → Bool, jsonb → Structured, etc.)
- Extract DDL constraints → map to `AttributeConstraint` (NOT NULL → required, CHECK → validation, FK → relationship edge candidate)
- Extract table context → infer entity type membership (columns in `client_entity` table → belong to ClientEntity type; columns in `ubo_declaration` → belong to UBODeclaration type)

Produce: enriched attribute records with `(attribute_name, type_spec, constraints[], inferred_entity_type, producing_verbs[], consuming_verbs[])`.

#### 1.4.4 Step 4: Entity type inference

Group the enriched attributes by their inferred entity type (from table membership):

- Tables that share an entity context → entity type candidates
- FK relationships between tables → RelationshipTypeDef candidates
  - FK to a "parent" table → Structural edge (ownership/hierarchy)
  - FK to a document/evidence table → Reference edge
  - FK to a lookup/taxonomy table → Association edge
- Entity lifecycle states inferred from status/state columns and their CHECK constraints

Produce: `EntityTypeDef` candidates with their attribute memberships and `RelationshipTypeDef` candidates with edge classes.

#### 1.4.5 Step 5: Seed the registries

With the connected subgraph assembled, seed all three registries:

- **Verb-connected attributes** → `AttributeDef` snapshots. `kind` = Primitive (if raw column) or Captured (if extracted from document) or ExternalSourced (if from API/feed). Set `governance_tier = Operational`, `trust_class = Convenience` for all initial seeds — promotion to Governed happens through proper governance workflow later.
- **Verb definitions** → `VerbContract` snapshots. Wire I/O to the seeded AttributeDef IDs. Set `governance_tier = Operational`, `trust_class = Convenience` initially.
- **Entity types** → `EntityTypeDef` snapshots with attribute memberships.
- **Relationships** → `RelationshipTypeDef` snapshots with edge classes.

#### 1.4.6 Step 6: Identify and flag orphans

After verb-first seeding, scan the schema for columns that NO verb touches:

- Columns in operational tables not referenced by any verb input, output, or side-effect
- Classify into four categories (not three — the distinction matters):
  - **Framework columns** (created_at, updated_by, version, audit_id, etc.) → do NOT seed as AttributeDefs. These are infrastructure, not domain semantics. Skip silently.
  - **UI / reporting / export convenience** (display_name composites, denormalised aggregates, cache columns, report-only fields) → seed ONLY if they carry genuine domain meaning. Tag as `origin = projection`, `governance_tier = Operational`, `verb_orphan = true`. Do NOT bulk-create — if in doubt, skip. These can always be added later; removing 900 unwanted attributes is painful.
  - **Genuine operational fields** (servicing setup, routing, operational status) → seed as `Operational / Convenience` with `verb_orphan = true` for governance review.
  - **Dead schema** (columns that exist but nothing reads, writes, displays, or exports) → flag for schema cleanup review, do NOT seed.

Produce: an onboarding report showing:
- N attributes seeded (verb-connected)
- M attributes flagged as operational orphans
- K columns flagged as dead schema
- Entity types and relationships inferred
- Verb contracts seeded with I/O wiring status (complete / partial / missing)

#### 1.4.7 CLI support for onboarding

- `ob reg onboard scan` — execute Steps 1–4, produce the onboarding report without writing anything
- `ob reg onboard apply` — execute Step 5, seed the registries
- `ob reg onboard report` — display the current onboarding status (seeded counts, orphans, dead schema, wiring completeness)
- `ob reg onboard verify` — after seeding, verify that every verb's declared I/O maps to a registered AttributeDef

**Phase gate**: all three registries have tables, Rust types, CLI describe/list/history commands work, verb-first seed data is loaded, onboarding report shows wiring completeness ≥ 80% for existing verbs. → IMMEDIATELY proceed to Phase 2. Progress: ~25%.

---

## Phase 2 — Taxonomy, Membership, View Definitions

**Goal**: classification infrastructure and context projection definitions.

### 2.1 Taxonomy Registry

Tables: `sem_reg.taxonomies`, `sem_reg.taxonomy_nodes`, `sem_reg.memberships`

- Taxonomy per v1.1 §9.4 (snapshot pattern)
- TaxonomyNode per v1.1 §9.4 (snapshot pattern, DAG support via parents[] array)
- Membership: subject_type (enum: Attribute/VerbContract/EntityType/RelationshipType/DocumentType/PolicyRule/ExternalSource), subject_id, taxonomy_id, node_id, weight, layout_role (nullable enum: Group/Container/Swimlane/Hidden), notes
- `LayoutRole` enum

Seed the four canonical taxonomies from v1.1 §9.4:
- KYC Review Navigation (DAG structure)
- Regulatory Domain
- Data Sensitivity
- Execution Semantics

CLI: `ob reg tax describe <taxonomy>`, `ob reg tax tree <taxonomy>`, `ob reg tax members <node_path>`

### 2.2 View Definitions

Table: `sem_reg.view_defs` (snapshot pattern)

- ViewDef per v1.1 §9.5: view_id, snapshot_id, name, description, taxonomy_slices (JSONB array of node references), primary_edge_class, layout_strategy_hint (enum: Hierarchical/ForceDirected/Swimlane/Tabular/Timeline), verb_surface (JSONB array of VerbSurfaceEntry), attribute_prominence (JSONB array), filters (JSONB array), includes_operational (boolean — controls operational visibility in Strict/Normal modes), view_security_profile, governance_tier, steward, status, version, snapshot meta fields

Seed the six canonical views from v1.1 §9.5:
- UBO Discovery, Sanctions Screening, Proof Collection, Case Overview, Governance Review, Operational Setup

CLI: `ob reg view describe <name>`, `ob reg view list`

### 2.3 Membership wiring from onboarding data

Classify the verb-first seeded objects using the onboarding report as input:

- **Data Sensitivity**: auto-assign from the onboarding scan — attributes whose schema columns had PII markers (name, address, SSN patterns, date_of_birth, etc.) → classify under PII node. Financial columns → Financial node. Use the security label backfill (Phase 4.4) to cross-validate.
- **Execution Semantics**: auto-assign from verb exec_mode extracted during onboarding Step 1 — Sync verbs, Research verbs, Durable verbs, Macro verbs each get taxonomy membership.
- **KYC Review Navigation**: manual/semi-automated — use the entity type inference from onboarding Step 4 to propose classifications. Entity types related to ownership → Ownership & Control. Document-related entity types → Evidence & Proofs. Screening-related verbs → Sanctions / Adverse Media. Generate a proposed classification report for steward review.
- **Regulatory Domain**: semi-automated — verbs and attributes that participate in CDD/EDD/sanctions workflows (identifiable from verb names, preconditions, and policy references found during onboarding) → propose regulatory domain membership.

CLI: `ob reg onboard classify --taxonomy <name> --mode auto|propose` — auto-assign where confidence is high, generate proposals for steward review where it's not.

**Phase gate**: taxonomies seeded with nodes, memberships link existing attributes/verbs to taxonomy nodes, view definitions seeded, CLI commands work. → IMMEDIATELY proceed to Phase 3. Progress: ~35%.

---

## Phase 3 — Policy, Evidence, Observations

**Goal**: the enforcement and provenance registries.

### 3.1 Policy & Controls Registry

Table: `sem_reg.policy_rules` (snapshot pattern)

- PolicyRule per v1.1 §9.6: policy_id, snapshot_id, name, description, scope, predicate (JSONB expression tree), predicate_trust_minimum (enum: DecisionSupport/Proof), enforcement (Hard/Soft), remediation_hint, evidence_requirements (JSONB array), jurisdiction_tags, security_implications, regulatory_reference (JSONB), effective_date, review_date, steward, version, snapshot meta fields
- EvidenceRequirement sub-type: required_doc_type, required_attribute, acceptable_sources, freshness_window, confidence_threshold, governance_tier_minimum (always Governed), trust_class_minimum (default Proof)
- RegulatoryReference sub-type: regulation_id, regulation_name, section, jurisdiction, effective_date, sunset_date, uri

Seed initial policy rules by extracting from existing ob-poc compliance logic:

- Scan Rust code for hardcoded business rules (if/match guards on jurisdiction, entity status, document presence, risk tier) → extract into PolicyRule snapshots with predicate expressions
- Scan BPMN workflow definitions for gateway conditions → extract as PolicyRules scoped to the gateway's verb context
- Scan DSL verb preconditions that are actually policy-flavoured (jurisdiction-dependent, risk-dependent) → migrate from VerbContract.preconditions to PolicyRule.predicate with verb scope
- Cross-reference against the verb-first onboarding report: verbs that had "unexplained" preconditions (not intrinsic to the verb, but context-dependent) are strong candidates for policy extraction
- For each extracted policy: attempt to identify the regulatory basis (CDD/EDD/sanctions/FATCA requirement) and create a preliminary RegulatoryReference — flag as "needs governance review" where the regulatory link is uncertain

CLI: `ob reg policy describe`, `ob reg policy trace-regulation <regulation_id>` (forward trace), `ob reg policy explain <policy_id>` (reverse trace)

### 3.2 Source & Evidence Registry

Tables: `sem_reg.document_type_defs` (snapshot pattern), `sem_reg.document_instances`, `sem_reg.observations`, `sem_reg.provenance_edges`, `sem_reg.retention_policies`

- All types per v1.1 §9.7
- DocumentInstance: doc_id, doc_type_id, storage_ref, extracted_fields (JSONB), source_actor, received_at, validated_at, expiry, retention_until, security_label, status
- Observation: obs_id, subject_ref, attribute_id, value_ref, source, confidence, timestamp, supporting_doc_ids, governance_tier, security_label, supersedes (nullable obs_id — chain of supersession)
- ProvenanceEdge: from_ref, to_ref, method (Human/OCR/API/Derived/Attested), verb_id, timestamp, confidence

**Observation supersession**: INSERT only, never UPDATE. New observation references predecessor via `supersedes`. Point-in-time observation query semantics: **active at time T** = the newest observation for `(subject_ref, attribute_id)` where `timestamp <= T` AND no other observation for the same `(subject_ref, attribute_id)` with `timestamp <= T` supersedes it. In SQL terms: the observation with `MAX(timestamp)` where `timestamp <= T` for the `(subject_ref, attribute_id)` pair, verified that no later observation in the chain has `supersedes` pointing to it with `timestamp <= T`. Index strategy: `(subject_ref, attribute_id, timestamp DESC)` for efficient point-in-time lookups.

### 3.3 Retention policy wiring

- RetentionPolicy attached to DocumentTypeDef
- Query: "which evidence objects are approaching/exceeding retention windows?"
- Rule: snapshots referenced by active decision records or open cases cannot be archived regardless of retention status

**Phase gate**: policy rules, evidence types, and observations are queryable. Supersession chains work. Regulatory trace queries work. → IMMEDIATELY proceed to Phase 4. Progress: ~50%.

---

## Phase 4 — Security Framework (ABAC)

**Goal**: security labels on all registry objects, ABAC evaluation, and security inheritance.

### 4.1 ABAC evaluation engine

- Function: `evaluate_access(actor: ActorContext, target_label: SecurityLabel, purpose: Purpose) -> AccessDecision`
- Implements intersection logic per v1.1 §5.3: actor jurisdiction ∩ target jurisdiction, actor purposes ∩ target purpose_tags, clearance vs. confidentiality, etc.
- Returns structured AccessDecision with verdict, masking_plan, export_controls, residency_constraints, required_controls
- AccessDecision references the PolicyRule snapshot_ids that contributed to the verdict (snapshot-pinned)

### 4.2 Security inheritance on derivations

- Function: `compute_inherited_label(inputs: &[SecurityLabel], override: Option<DeclaredOverride>) -> SecurityLabel`
- Default: most restrictive confidentiality, most restrictive residency, union of handling_requirements. Purpose tags: if all inputs specify non-empty purpose sets, intersect them; if any input has empty `purpose_tags` (meaning "no restriction"), treat it as a neutral element (the result is the other inputs' intersection). This prevents empty-set deadlocks where an unrestricted input accidentally denies all downstream purposes.
- Override path: requires steward approval flag, logs rationale, less-restrictive overrides flagged as exceptional

### 4.3 Security inheritance through verb side-effects

- At publish time: validate that verb side-effects' declared security implications are compatible with the verb's execution security context
- Prevent label laundering: a verb running under purpose P cannot produce output labelled for a different (less restrictive) purpose without explicit policy authorisation

### 4.4 Backfill security labels on existing snapshots

- Assign SecurityLabels to all seeded registry objects from Phases 1–3 using onboarding metadata as input:
  - Attributes flagged as PII during onboarding scan (name patterns, taxonomy membership from Phase 2.3) → `data_category = PII`, `handling_requirements = [MaskByDefault]`
  - Attributes in sanctions/screening verbs → `data_category = Sanctions`, `purpose_tags = [SANCTIONS]`
  - Attributes with jurisdiction context (extracted from verb preconditions or schema table naming) → populate `jurisdiction_tags` and `residency_class`
  - Financial data attributes → `data_category = Financial`
- Define label templates for common patterns: `standard_pii_uk`, `standard_pii_eu`, `standard_financial_global`, `sanctions_restricted`, `operational_internal` — apply templates rather than hand-crafting
- Cross-validate against Phase 2.3 Data Sensitivity taxonomy membership (they should agree; discrepancies indicate either a classification error or a label error)
- **Default label rule**: when no classification evidence exists for an attribute, apply the deterministic default: `confidentiality = Internal, data_category = None, purpose_tags = [] (no restriction), residency_class = Global, handling_requirements = [], audit_level = Standard`. Governance tier is unaffected by the default label. This prevents random or inconsistent defaults and ensures 100% coverage is achievable without manual review of every object.
- Ensure 100% security label coverage before proceeding — every snapshot must have a SecurityLabel, even if it's the default template

**Phase gate**: ABAC evaluation works, security labels present on all existing snapshots, inheritance computation works for derivations and verb side-effects. → IMMEDIATELY proceed to Phase 5. Progress: ~60%.

---

## Phase 5 — Derived & Composite Attributes

**Goal**: first-class derivation recipes with security inheritance, evidence-grade rules, and lineage.

### 5.1 DerivationSpec

Table: `sem_reg.derivation_specs` (snapshot pattern)

- All fields from v1.1 §9.8: derivation_id, snapshot_id, output_attribute_id, inputs (JSONB array of {attribute_id, role, required}), expression (JSONB — see MVP note below), null_semantics, freshness_rule, security_inheritance (Strict/DeclaredOverride), residency_inheritance (always Strict), evidence_grade (Prohibited/AllowedWithConstraints), tests (JSONB array), steward, status, version, snapshot meta fields

**MVP implementation**: the `expression` field supports three forms (`expression_ast | query_plan | function_ref`) but **start with `function_ref` only** — a Rust function pointer registry or enum dispatch that maps a derivation_id to a compiled Rust function. This is sufficient for all existing derivations (which are already Rust code) and avoids prematurely building an AST interpreter. Store `function_ref` as `{ "kind": "function_ref", "ref": "compute_risk_score" }` in JSONB. `expression_ast` support can be added in a later phase when the designer capability requires it.

### 5.2 Derivation evaluation

- Function: `evaluate_derivation(spec: &DerivationSpec, inputs: &[ResolvedInput]) -> DerivedValue`
- Pins snapshot IDs of DerivationSpec and all input AttributeDefs in the evaluation record
- Deterministic: same input snapshots + same input values → same output, always
- Computes inherited SecurityLabel from inputs using Phase 4 inheritance logic

### 5.3 Derivation gates

- Publish-time: cycle detection in derivation dependency graph, type compatibility, security inheritance validation
- Operational derivations: `evidence_grade = Prohibited` (enforced — cannot be overridden)
- Governed derivations: `evidence_grade = AllowedWithConstraints` only when policy-linked and tests pass

### 5.4 Convert existing ad-hoc derivations

Scan existing ob-poc for computed/derived fields using the onboarding data as a starting point:

- **Verb side-effect outputs that compute from inputs**: the onboarding report (Phase 1.4) identifies verbs whose outputs don't map to raw schema columns — these are derivation candidates. Extract the computation logic from the verb implementation into a DerivationSpec.
- **SQL views and computed columns**: query `information_schema.views` and scan for computed column expressions in the operational schema. Each view/expression → a candidate DerivationSpec. For MVP, wrap the SQL logic in a Rust function and register as `function_ref`; the original SQL expression can be stored in the DerivationSpec `notes` field for documentation and future AST migration.
- **Application-layer computations**: scan Rust code for functions that combine multiple attribute values to produce a result (risk scores, completeness percentages, derived statuses, aggregated flags). These are the highest-risk derivations because they currently have zero lineage visibility.
- **Attributes flagged `verb_orphan = true` in onboarding**: some of these may be derived values maintained by triggers or application logic rather than by verbs. Cross-reference against SQL triggers and Rust computation code.

For each identified derivation:
- Create an `AttributeDef` snapshot with `kind = Derived` or `kind = Composite`
- Create a `DerivationSpec` snapshot capturing: input attribute_ids (linked to existing seeded AttributeDefs), computation expression/function, null semantics, and at least one test case
- Set `governance_tier = Operational`, `evidence_grade = Prohibited` for initial seeds
- Wire the derivation into the lineage graph (input attributes → DerivationSpec → output attribute)
- Flag derivations that feed existing compliance-flavoured logic for priority promotion review

**Phase gate**: derivation specs work, evaluation is deterministic and snapshot-pinned, security inheritance computes correctly, existing derivations migrated. → IMMEDIATELY proceed to Phase 6. Progress: ~68%.

---

## Phase 6 — Publish Gates (`ob publish` Integration)

**Goal**: wire the Semantic OS registries into the existing `ob publish` pipeline as enforcement gates.

**Rollout strategy**: gates ship in **report-only mode first** (emit warnings, do not block publish) for the operational tier, then switch to **hard-fail for the governed tier** once Context Resolution (Phase 7) and MCP tools (Phase 8) are usable and the registries have meaningful content. This prevents the "we built 40 gates and nobody can use the system" effect. The non-negotiable invariants (Proof Rule, security label presence, snapshot integrity) go hard-fail immediately for both tiers; the governance-specific gates (taxonomy, stewardship, policy attachment) start report-only and are promoted to hard-fail once governed-tier content exists.

### 6.1 Technical gates (both tiers)

Wire into `ob publish` pipeline:

- Type correctness: verb contract I/O types match attribute dictionary type_specs
- Dependency correctness: no cycles in derivation chains, no unknown attribute references in verb contracts
- Security label presence: all objects have SecurityLabel
- Residency declarations present where jurisdiction_tags include residency-sensitive jurisdictions
- Verb read/write surface disclosure: every attribute a verb reads or writes must appear in its inputs/outputs/side_effects (ABAC must be evaluable)
- Orphan detection: governed attributes referenced by zero verbs → fail; operational → warn
- Macro expansion integrity: every ExpansionContract resolves to registered DSL verbs with compatible I/O
- Continuation completeness: every DurableStart verb has a corresponding DurableResume with compatible correlation keys
- Snapshot integrity: new snapshots correctly reference predecessors

### 6.2 Governance gates (governed tier only)

- Taxonomy membership: governed attributes, verbs, and entity types must have ≥1 taxonomy membership
- Stewardship: governed objects must have `steward` set
- Policy attachment: governed verbs operating in regulated contexts must have applicable PolicyRules
- Regulatory linkage: governed PolicyRules must carry at least one RegulatoryReference
- Review-cycle compliance: governed objects must have `last_reviewed` within their review cycle
- Evidence-grade: governed derived attributes with `evidence_grade = AllowedWithConstraints` must have passing tests and policy linkage
- Version consistency: breaking changes (major version bump) require compatibility analysis

### 6.3 Proof Rule gate

Mechanically enforce at publish time:
- For every PolicyRule with `enforcement = Hard`: verify that all attributes referenced in `predicate` have `governance_tier = Governed` and `trust_class >= predicate_trust_minimum`
- For every EvidenceRequirement: verify that `required_attribute` (if specified) has `governance_tier >= governance_tier_minimum` and `trust_class >= trust_class_minimum`
- Reject publish on any mismatch with a clear remediation message

### 6.4 Gate failure output format

All gate failures produce structured output consumable by both CLI (human-readable) and agent (machine-readable JSONB):
- `gate_type`, `severity` (Error/Warning), `object_ref`, `snapshot_id`, `message`, `remediation_hint`, `regulatory_reference` (if applicable)

**Phase gate**: `ob publish` enforces all gates. A publish with violations is rejected with structured remediation output. → IMMEDIATELY proceed to Phase 7. Progress: ~78%.

---

## Phase 7 — Context Resolution API

**Goal**: the single query contract that serves agent, UI, CLI, workflow engine, and governance.

### 7.1 Request/Response types

Implement `ContextResolutionRequest` and `ContextResolutionResponse` per v1.1 §8.1 and §8.2:

- Request: subject, intent (optional NL), current_state_snapshot, actor (ActorContext), goals, constraints, evidence_mode (Strict/Normal/Exploratory/Governance), point_in_time (optional — defaults to now)
- Response: as_of_time, resolved_at, applicable_view_definitions (ranked, snapshot-pinned), candidate_verbs (ranked with snapshot_ids, tier, trust, usable_for_proof), candidate_attributes (ranked with snapshot_ids, tier, trust), required_preconditions with remediation, disambiguation_questions, evidence (positive + negative), policy_verdicts (snapshot-pinned), security_handling (AccessDecision), governance_signals

### 7.2 Resolution logic

Phase 7 implementation — deterministic resolution (no embeddings yet):

1. Resolve `point_in_time` → snapshot epoch
2. Resolve subject → entity type, case state, applicable jurisdiction
3. Select applicable ViewDefs by taxonomy overlap with subject context
4. For top ViewDef: extract verb_surface, attribute_prominence, taxonomy_slices
5. Filter verbs by: taxonomy membership in view slices + precondition evaluability + ABAC (actor can execute) + tier/trust (respect evidence_mode — Strict/Normal exclude Operational unless view.includes_operational)
6. Filter attributes by: taxonomy membership in view slices + ABAC (actor can read) + tier/trust
7. Rank verbs and attributes by view prominence weights
8. Evaluate preconditions for top candidate verbs against current state
9. Evaluate policy rules for top candidates — produce verdicts with snapshot references
10. Compute security handling (ABAC decision for the response as a whole)
11. Generate governance signals: scan for stewardship gaps, classification gaps, stale evidence, approaching retention
12. Compute confidence score for the resolution. **Deterministic heuristic** (no ML, no embeddings — those come in Phase 9):
    - `view_match_score`: how well the top ViewDef's taxonomy slices overlap with the subject's entity type and state (1.0 = exact match, 0.0 = no overlap)
    - `precondition_satisfiable_pct`: percentage of top candidate verbs whose preconditions are currently satisfiable
    - `required_inputs_present_pct`: percentage of required attributes (from top view's attribute_prominence) that have active observations
    - `abac_permit_pct`: percentage of candidates where ABAC verdict = Permit (not Deny/Escalate)
    - `confidence = weighted_mean(view_match × 0.3, precondition_pct × 0.25, inputs_present_pct × 0.3, abac_pct × 0.15)`
    - If `confidence < 0.5` or multiple views score within 0.1 of each other: generate disambiguation prompts

### 7.3 Trust-aware filtering

Per v1.1 §8.3:
- Strict/Normal: Governed + Proof/DecisionSupport primary. Operational only if view.includes_operational = true. Operational candidates always flagged `usable_for_proof = false`.
- Exploratory: all tiers, all trust classes, with annotations.
- Governance: focus on coverage metrics, not verb/attribute recommendation.

### 7.4 Expose as internal API

- Rust function: `resolve_context(req: ContextResolutionRequest) -> ContextResolutionResponse`
- CLI: `ob ctx resolve --subject <id> --actor <role> --mode <strict|normal|exploratory|governance> [--as-of <timestamp>]`
- Output: human-readable summary + JSON for machine consumption

**Phase gate**: Context Resolution returns correct, tier-aware, snapshot-pinned responses for test cases covering UBO Discovery, Sanctions Screening, and Proof Collection views. Point-in-time resolution works. → IMMEDIATELY proceed to Phase 8. Progress: ~85%.

---

## Phase 8 — Agent Control Plane + MCP Tool Integration

**Goal**: the agent can plan, decide, execute, escalate, and record — all through MCP tools backed by the Semantic OS. **This is the phase that makes the entire architecture usable. Without it, everything prior is infrastructure without a consumer.**

### 8.1 Agent control plane types

Per v1.1 §10:

- `AgentPlan`: plan_id, case_id, goal, context_resolution_ref (snapshot-pinned response), steps, assumptions, risk_flags, security_clearance (pre-computed AccessDecision for plan scope), created_at, approved_by, status
- `PlanStep`: step_id, verb_id, verb_snapshot_id (pinned to exact contract version), params, expected_postconditions, fallback_steps, depends_on_steps, governance_tier_of_outputs
- `DecisionRecord`: decision_id, plan_id, context_ref, chosen_action, alternatives_considered, evidence_for, evidence_against, negative_evidence, policy_verdicts (snapshot-pinned), security_handling, tier_trust_annotations, **snapshot_manifest** (complete map of object_id → snapshot_id for every contributing definition), confidence, escalation_flag, timestamps
- `DisambiguationPrompt`: prompt_id, decision_id, question, options, evidence_per_option, required_to_proceed, rationale
- `EscalationRecord`: escalation_id, decision_id, reason, context_snapshot, required_human_action, assigned_to, resolved_at, resolution

Tables: `sem_reg.agent_plans`, `sem_reg.plan_steps`, `sem_reg.decision_records`, `sem_reg.disambiguation_prompts`, `sem_reg.escalation_records`

All records are immutable (INSERT only).

### 8.2 MCP tools — registry query surface

Expose the Semantic OS to the agent via MCP tools. These are the agent's "syscalls":

**MUTATION GUARD POLICY**: MCP tools that **read** the registry are available to all actor types. MCP tools that **mutate** the registry (create snapshots, classify, record observations with governance_tier=Governed) are guarded:
- All registry mutations create **Draft snapshots** that only become Active on `ob publish`. The agent cannot unilaterally change active semantic definitions at runtime.
- Mutations that affect governed-tier objects require `actor_type = GovernanceReviewer | System` in the ActorContext.
- Operational-tier mutations may be performed by any authorised actor but still go through the draft → publish flow.
- Evidence recording (`sem_reg_record_observation`) is the exception — observations are operational data, not registry definitions, and can be written directly. But observations with `governance_tier = Governed` require steward-level actor authority.
This prevents accidental "semantic drift" where the agent silently redefines the platform's vocabulary at runtime.

**Registry exploration tools (read-only, all actors):**
- `sem_reg_describe_attribute(name_or_id, as_of?)` → full AttributeDef snapshot
- `sem_reg_describe_verb(canonical_name, as_of?)` → full VerbContract snapshot
- `sem_reg_describe_entity_type(name, as_of?)` → full EntityTypeDef snapshot
- `sem_reg_describe_policy(name_or_id, as_of?)` → full PolicyRule snapshot with regulatory reference
- `sem_reg_search(query, object_types?, taxonomy_filter?)` → ranked results across registries
- `sem_reg_list_verbs(filter?)` → list active verb contracts
- `sem_reg_list_attributes(filter?, taxonomy?)` → list active attribute definitions

**Taxonomy tools:**
- `sem_reg_taxonomy_tree(taxonomy_name)` → full DAG structure
- `sem_reg_taxonomy_members(taxonomy_name, node_path)` → all objects classified under this node
- `sem_reg_classify(object_type, object_id, taxonomy, node_path)` → create draft membership (requires publish to activate; governed objects require GovernanceReviewer actor)

**Impact and lineage tools:**
- `sem_reg_verb_surface(verb_name)` → "show me everything this verb touches" (inputs, outputs, side-effects, preconditions, postconditions, applicable policies)
- `sem_reg_attribute_producers(attribute_name)` → "what verbs can produce this attribute?"
- `sem_reg_impact_analysis(object_type, object_id)` → "if this changes, what is affected?"
- `sem_reg_lineage(subject_ref, attribute_id, as_of?)` → provenance chain for a specific data point
- `sem_reg_regulation_trace(regulation_id)` → forward trace: regulation → policies → verbs → attributes

**Context resolution tool (the primary agent entry point):**
- `sem_reg_resolve_context(subject, goals?, constraints?, evidence_mode?)` → full ContextResolutionResponse

**View tools:**
- `sem_reg_describe_view(view_name)` → full ViewDef snapshot
- `sem_reg_apply_view(view_name, subject)` → Context Resolution filtered through specific view

### 8.3 MCP tools — agent action surface

**Planning tools:**
- `sem_reg_create_plan(case_id, goal, context_resolution_ref)` → creates AgentPlan, returns plan_id
- `sem_reg_add_plan_step(plan_id, verb_id, params, expected_postconditions, fallback?)` → adds PlanStep, validates verb contract exists, checks preconditions evaluable
- `sem_reg_validate_plan(plan_id)` → checks all steps against current registry state: contracts valid, preconditions evaluable, security clearances, proof rule compliance, policy verdicts
- `sem_reg_execute_plan_step(plan_id, step_id)` → executes verb, records snapshot_manifest, records outcome

**Decision recording tools:**
- `sem_reg_record_decision(plan_id?, context_ref, chosen_action, alternatives, evidence_for, evidence_against, negative_evidence, confidence)` → creates DecisionRecord with auto-populated snapshot_manifest and policy_verdicts
- `sem_reg_record_escalation(decision_id, reason, required_human_action)` → creates EscalationRecord
- `sem_reg_record_disambiguation(decision_id, question, options, evidence_per_option)` → creates DisambiguationPrompt

**Evidence tools:**
- `sem_reg_record_observation(subject_ref, attribute_id, value, source, confidence, supporting_docs?, governance_tier?)` → creates Observation (with supersession chain)
- `sem_reg_check_evidence_freshness(subject_ref, attribute_id?)` → returns freshness status for all active observations
- `sem_reg_identify_evidence_gaps(case_id, view_name?)` → compares required evidence (from applicable policies) against available observations

### 8.4 MCP resource surface

Expose registry objects as MCP resources that can be read by the agent or attached to context:

- `sem_reg://attributes/{name_or_id}` → attribute definition
- `sem_reg://verbs/{canonical_name}` → verb contract
- `sem_reg://entities/{name}` → entity type definition
- `sem_reg://policies/{name_or_id}` → policy rule with regulatory reference
- `sem_reg://views/{name}` → view definition
- `sem_reg://taxonomies/{name}` → taxonomy structure
- `sem_reg://observations/{subject_ref}/{attribute_id}` → current observation chain
- `sem_reg://decisions/{decision_id}` → decision record with full snapshot manifest
- `sem_reg://plans/{plan_id}` → plan with steps and status

Resources support `?as_of=<timestamp>` for point-in-time resolution.

### 8.5 MCP document/resource wiring to existing ob-poc

**Critical integration**: the Semantic OS MCP tools must coexist with and complement existing ob-poc MCP tools. Wire up:

- Existing case management tools → can now call `sem_reg_resolve_context` to get registry-backed recommendations
- Existing entity resolution tools → can now call `sem_reg_describe_entity_type` to understand the meta-model
- Existing DSL execution tools → now pin snapshot_ids when executing verbs; call `sem_reg_record_decision` after decisions
- Existing document handling → now calls `sem_reg_record_observation` when extracting data from documents
- Existing BPMN workflow engine → verb contracts provide the step definitions; preconditions/postconditions provide the transition guards; continuation contracts provide resume semantics

### 8.6 Agent prompt grounding

Update agent system prompts / MCP server configuration to:

- Reference the Semantic OS as the authoritative source for "what actions are available, what do they require, and what do they produce"
- Include instructions to call `sem_reg_resolve_context` before proposing actions
- Include instructions to call `sem_reg_record_decision` after every non-trivial decision
- Include instructions to check `sem_reg_check_evidence_freshness` before relying on evidence
- Include instructions to call `sem_reg_identify_evidence_gaps` when planning proof collection
- Include the Proof Rule in the agent's operating instructions: "never treat operational/convenience attributes as evidence; always verify governance_tier and trust_class"

**Phase gate**: agent can query all registries via MCP tools, plan multi-step actions with snapshot-pinned contracts, record decisions with snapshot manifests, identify evidence gaps, and escalate with context. All existing ob-poc MCP tools are wired to use registry backing. → IMMEDIATELY proceed to Phase 9. Progress: ~93%.

---

## Phase 9 — Lineage, Embeddings, Coverage Metrics

**Goal**: derived projections that make the registry queryable for impact analysis, semantic search, and governance dashboards.

### 9.1 Lineage & derivation graph

Table: `sem_reg.derivation_edges` (immutable, append-only)

- Per v1.1 §11.1: input_refs, verb_id, output_refs, edge_class, timestamp, run_id
- RunRecord: run_id, case_id, plan_id, verb_calls (with snapshot_ids), outcomes, timestamps
- Include derivation chains from DerivationSpec evaluations (Phase 5)

Queries:
- Forward impact: "if attribute A changes, what downstream derivations and assertions are affected?"
- Reverse provenance: "where did this value come from?"
- Temporal: "show me the derivation chain as it existed at time T" (using snapshot-pinned edges)

MCP tool: `sem_reg_impact_analysis` and `sem_reg_lineage` from Phase 8 now backed by real data

### 9.2 Embeddings / vector projection

- Generate semantic text for attributes (definitions + aliases + taxonomy paths + examples) and verbs (purpose + parameters + taxonomy paths + typical prompts + side-effects + preconditions + postconditions)
- Embed using local model (Candle / BGE — respecting existing ob-poc embedding infrastructure)
- Store: `sem_reg.embedding_records` — subject_ref, model_id, vector_ref, version_hash, created_at, stale_since, stale_reason
- Staleness tracking: mark embeddings stale when source snapshot changes; rebuild pipeline (event-driven from snapshot publishes)
- **NoLLMExternal enforcement**: attributes with `handling_requirements` containing `NoLLMExternal` are excluded from external model inference; use internal model only, tracked on EmbeddingRecord
- Wire into Context Resolution (Phase 7) as a secondary ranking signal alongside taxonomy-driven filtering

### 9.3 Governance & security coverage metrics

Per v1.1 §11.3 — computable from registry state:

- Classification coverage: % governed objects with taxonomy membership
- Stewardship coverage: % governed objects with active stewards
- Policy attachment: % governed verbs/entities with applicable PolicyRules
- Evidence freshness: % active governed observations within freshness windows
- Review currency: % governed objects reviewed within cycle
- Retention compliance: evidence within/approaching/exceeding windows
- Regulatory coverage: % applicable regulations with implementing PolicyRules
- Security label completeness: % all objects (both tiers) with labels
- Proof Rule compliance: zero operational attributes in governed evidence requirements
- Tier distribution: governed vs. operational counts
- Snapshot volume: active/deprecated/retired/archived counts

MCP tool: `sem_reg_coverage_report(scope?)` → returns all metrics
CLI: `ob reg coverage [--tier governed|operational|all]`

**Phase gate**: lineage queries work end-to-end, embeddings generate and track staleness, coverage metrics are computable and accurate. → IMMEDIATELY proceed to Phase 10. Progress: ~97%.

---

## Phase 10 — Integration Testing & Wiring Validation

**Goal**: prove the architecture works end-to-end across the three canonical use cases from v1.1.

### 10.1 UBO Discovery end-to-end

Test scenario: agent receives a case for UBO resolution on an institutional client.

1. Agent calls `sem_reg_resolve_context` with subject=case, goal=resolve_ubo, mode=Strict
2. Response returns UBO Discovery view, ranked verbs (ubo.resolve, evidence.request), relevant attributes, policy verdicts
3. Agent creates plan with `sem_reg_create_plan`
4. Agent identifies evidence gaps with `sem_reg_identify_evidence_gaps`
5. Agent executes plan steps, each pinning snapshot_ids
6. Agent records decision with `sem_reg_record_decision` — snapshot_manifest populated
7. If disambiguation needed: agent calls `sem_reg_record_disambiguation`
8. If escalation needed: agent calls `sem_reg_record_escalation`
9. Verify: all records are immutable, snapshot-pinned, tier-annotated, Proof Rule respected

### 10.2 Sanctions Screening end-to-end

Similar flow with Sanctions Screening view, force-directed edge class, screening verbs.

### 10.3 Proof Collection end-to-end

Similar flow with Proof Collection view, temporal edge class, proof verbs, evidence freshness checks.

### 10.4 Governance review

1. Governance reviewer calls `sem_reg_resolve_context` with mode=Governance
2. Response returns Governance Review view with coverage signals
3. Reviewer calls `sem_reg_coverage_report`
4. Reviewer can drill into specific attributes: `sem_reg_describe_attribute` → see steward, last_reviewed, tier, trust_class, taxonomy membership
5. Reviewer can trace regulations: `sem_reg_regulation_trace`

### 10.5 Point-in-time audit

1. Auditor calls `sem_reg_resolve_context` with `point_in_time = 6 months ago`
2. Verify: response is snapshot-pinned to the definitions that were active at that date
3. Auditor examines a DecisionRecord: verify snapshot_manifest allows full reconstruction
4. Auditor replays a derivation: same input snapshots → same output

### 10.6 Proof Rule enforcement validation

- Attempt to publish a governed PolicyRule whose predicate references an Operational attribute → must fail with clear remediation
- Attempt to satisfy an EvidenceRequirement with a Convenience trust_class attribute → must fail
- Attempt to promote an Operational attribute to Governed without steward approval → must fail
- Verify: operational derived attributes are always `evidence_grade = Prohibited`

**Phase gate**: all canonical scenarios pass. Proof Rule is mechanically enforced. Point-in-time works. MCP tools return correct results. This is launch-ready for the Semantic OS. Progress: 100%.

---

## Phase Dependency Summary

```
Phase 0: Snapshot infra + types
    ↓
Phase 1: Attributes + Entities + Verbs (core registries)
    ↓
Phase 2: Taxonomies + View Definitions
    ↓
Phase 3: Policies + Evidence + Observations
    ↓
Phase 4: Security framework (ABAC)
    ↓
Phase 5: Derived attributes + DerivationSpec
    ↓
Phase 6: Publish gates (ob publish integration)
    ↓
Phase 7: Context Resolution API
    ↓
Phase 8: Agent control plane + MCP tools ← makes it all usable
    ↓
Phase 9: Lineage + embeddings + coverage metrics
    ↓
Phase 10: Integration testing + wiring validation
```

Each phase gate MUST pass before the next phase begins. Each gate checks the six foundational invariants.

---

## Appendix: Files and Modules (Expected Structure)

```
src/sem_reg/
├── mod.rs                  // module root
├── snapshot.rs             // SnapshotMeta, Snapshottable trait, resolve/publish
├── types.rs                // GovernanceTier, TrustClass, ExecMode, EdgeClass, etc.
├── security.rs             // SecurityLabel, ActorContext, AccessDecision, ABAC eval
├── governance.rs           // governance framework helpers, coverage metrics
├── registries/
│   ├── mod.rs
│   ├── attributes.rs       // AttributeDef, AttributeConstraint, AttributeAlias
│   ├── entities.rs         // EntityTypeDef, RelationshipTypeDef, InstanceRef
│   ├── verbs.rs            // VerbContract, VerbIO, SideEffect, Pre/Postcondition, etc.
│   ├── taxonomies.rs       // Taxonomy, TaxonomyNode, Membership
│   ├── views.rs            // ViewDef, ViewFilter, VerbSurfaceEntry
│   ├── policies.rs         // PolicyRule, EvidenceRequirement, RegulatoryReference
│   ├── evidence.rs         // DocumentTypeDef, DocumentInstance, Observation, Provenance
│   └── derivations.rs      // DerivationSpec, derivation evaluation
├── context_resolution.rs   // ContextResolutionRequest/Response, resolution logic
├── gates.rs                // publish-time gates, runtime gates, Proof Rule checks
├── onboarding/
│   ├── mod.rs              // orchestrates the verb-first onboarding pipeline
│   ├── verb_scan.rs        // Step 1: inventory verbs from DSL defs, macros, BPMN
│   ├── attr_extract.rs     // Step 2: extract attributes from verb I/O surfaces
│   ├── schema_xref.rs      // Step 3: cross-reference against PostgreSQL schema
│   ├── entity_infer.rs     // Step 4: infer entity types from table/FK structure
│   ├── seed.rs             // Step 5: produce and apply registry snapshots
│   ├── orphans.rs          // Step 6: identify and classify schema orphans
│   └── report.rs           // onboarding status reports and verification
├── agent/
│   ├── mod.rs
│   ├── plans.rs            // AgentPlan, PlanStep
│   ├── decisions.rs        // DecisionRecord, snapshot_manifest
│   ├── escalation.rs       // DisambiguationPrompt, EscalationRecord
│   └── mcp_tools.rs        // MCP tool definitions for agent
├── projections/
│   ├── lineage.rs          // DerivationEdge, RunRecord, impact analysis
│   ├── embeddings.rs       // SemanticText, EmbeddingRecord, staleness
│   └── metrics.rs          // coverage metric computations
└── cli.rs                  // ob reg * CLI commands

migrations/
├── sem_reg_001_snapshot_infra.sql
├── sem_reg_002_attributes.sql
├── sem_reg_003_entities.sql
├── sem_reg_004_verbs.sql
├── sem_reg_005_taxonomies.sql
├── sem_reg_006_views.sql
├── sem_reg_007_policies.sql
├── sem_reg_008_evidence.sql
├── sem_reg_009_derivations.sql
├── sem_reg_010_agent_control.sql
├── sem_reg_011_lineage.sql
└── sem_reg_012_embeddings.sql
```

---

## Appendix: MCP Tool Summary

| Tool | Category | Phase |
|------|----------|-------|
| `sem_reg_describe_attribute` | Registry query | 8 |
| `sem_reg_describe_verb` | Registry query | 8 |
| `sem_reg_describe_entity_type` | Registry query | 8 |
| `sem_reg_describe_policy` | Registry query | 8 |
| `sem_reg_search` | Registry query | 8 |
| `sem_reg_list_verbs` | Registry query | 8 |
| `sem_reg_list_attributes` | Registry query | 8 |
| `sem_reg_taxonomy_tree` | Taxonomy | 8 |
| `sem_reg_taxonomy_members` | Taxonomy | 8 |
| `sem_reg_classify` | Taxonomy | 8 |
| `sem_reg_verb_surface` | Impact/lineage | 8 |
| `sem_reg_attribute_producers` | Impact/lineage | 8 |
| `sem_reg_impact_analysis` | Impact/lineage | 8 (backed by 9) |
| `sem_reg_lineage` | Impact/lineage | 8 (backed by 9) |
| `sem_reg_regulation_trace` | Regulatory | 8 |
| `sem_reg_resolve_context` | Context resolution | 8 (backed by 7) |
| `sem_reg_describe_view` | Views | 8 |
| `sem_reg_apply_view` | Views | 8 |
| `sem_reg_create_plan` | Agent planning | 8 |
| `sem_reg_add_plan_step` | Agent planning | 8 |
| `sem_reg_validate_plan` | Agent planning | 8 |
| `sem_reg_execute_plan_step` | Agent execution | 8 |
| `sem_reg_record_decision` | Agent decisions | 8 |
| `sem_reg_record_escalation` | Agent escalation | 8 |
| `sem_reg_record_disambiguation` | Agent disambiguation | 8 |
| `sem_reg_record_observation` | Evidence | 8 |
| `sem_reg_check_evidence_freshness` | Evidence | 8 |
| `sem_reg_identify_evidence_gaps` | Evidence | 8 |
| `sem_reg_coverage_report` | Governance metrics | 9 |

| Resource URI | Phase |
|---|---|
| `sem_reg://attributes/{id}` | 8 |
| `sem_reg://verbs/{name}` | 8 |
| `sem_reg://entities/{name}` | 8 |
| `sem_reg://policies/{id}` | 8 |
| `sem_reg://views/{name}` | 8 |
| `sem_reg://taxonomies/{name}` | 8 |
| `sem_reg://observations/{ref}/{attr}` | 8 |
| `sem_reg://decisions/{id}` | 8 |
| `sem_reg://plans/{id}` | 8 |
