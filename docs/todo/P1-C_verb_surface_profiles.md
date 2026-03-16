# P1-C: Verb Surface & SemOS Profiles Audit

**Review Session:** P1-C
**Date:** 2026-03-16
**Scope:** Verb registry inventory, contract completeness, SemOS profile coverage, stale/placeholder verb families, and discrimination dimension coverage.

---

## 1. Executive Summary

The verb registry contains **1,081 verbs across 120 domains** (588 plugin, 481 CRUD, 9 graph_query, 3 durable) with **586 registered CustomOps** (99.7% plugin coverage). Basic contract fields are strong: 97% have `returns`, 96% have arguments, 100% have descriptions. However, governance-critical metadata is almost entirely absent: `harm_class` at 2.8%, `action_class` at 2.8%, `produces` at 3%, `lifecycle` at 8%, and `preconditions`/`postconditions` at 0%. The SemOS profile layer (ViewDefs, TaxonomyDefs, TaxonomyNodes) is structurally implemented but contains zero active records, causing the 12-step `resolve_context()` pipeline to degenerate to a no-op. The runtime verb surface is effectively governed by a hardcoded Rust domain-allowlist, not the registry.

**Severity Distribution:**

| Severity | Count | Description |
|----------|-------|-------------|
| P0 | 4 | Registry data gaps that nullify governance pipelines |
| P1 | 5 | Missing discrimination metadata that limits safety and intent resolution |
| P2 | 3 | Stale verb families and minor coverage gaps |
| P3 | 2 | Cosmetic and documentation issues |

---

## 2. Verb Surface Summary

### 2.1 Inventory by Domain (Top 30)

| Domain | Verbs | Behavior | File |
|--------|-------|----------|------|
| deal | 42 | 28 plugin + 14 crud | deal.yaml |
| cbu | 37 | 37 plugin | cbu.yaml |
| trading-profile | 32 | 32 plugin | trading-profile.yaml |
| capital | 30 | 30 plugin | capital.yaml |
| registry | 26 | 26 plugin | sem-reg/registry.yaml |
| service-resource | 26 | 14 plugin + 12 crud | service-resource.yaml |
| client-group | 23 | 23 plugin | client-group.yaml |
| fund | 22 | 0 plugin + 22 crud | fund.yaml |
| ownership | 22 | 22 plugin | ownership.yaml |
| document | 21 | 7 plugin + 14 crud | document.yaml |
| agent | 20 | 20 plugin | agent.yaml |
| investor | 20 | 20 plugin | registry/investor.yaml + investor-role.yaml |
| ubo | 20 | 20 plugin | ubo.yaml |
| session | 18 | 18 plugin | session.yaml |
| billing | 17 | 17 plugin | billing.yaml |
| control | 16 | 16 plugin | control.yaml |
| gleif | 16 | 16 plugin | gleif.yaml |
| verify | 16 | 16 plugin | verification/verify.yaml |
| entity | 15 | 7 plugin + 8 crud | entity.yaml |
| team | 15 | 8 plugin + 7 crud | team.yaml |
| view | 14 | 14 plugin | view.yaml |
| contract | 14 | 0 plugin + 14 crud | contract.yaml |
| schema | 14 | 14 plugin | sem-reg/schema.yaml |
| changeset | 14 | 14 plugin | sem-reg/changeset.yaml |
| discovery | 13 | 13 plugin | discovery.yaml |
| settlement-chain | 13 | 0 plugin + 13 crud | custody/settlement-chain.yaml |
| movement | 14 | 0 plugin + 14 crud | registry/movement.yaml |
| trade-gateway | 12 | 0 plugin + 12 crud | custody/trade-gateway.yaml |
| identifier | 11 | 0 plugin + 11 crud | identifier.yaml |
| tax-config | 11 | 0 plugin + 11 crud | custody/tax-config.yaml |

**Remaining 90 domains:** 1-10 verbs each. Full distribution across all 120 domains in Appendix A.

### 2.2 Behavior Distribution

| Behavior | Count | Percentage |
|----------|-------|------------|
| plugin | 588 | 54.4% |
| crud | 481 | 44.5% |
| graph_query | 9 | 0.8% |
| durable | 3 | 0.3% |
| **Total** | **1,081** | **100%** |

### 2.3 Naming Convention Adherence

All 1,081 verbs follow the `domain.action` kebab-case naming convention. No violations found.

### 2.4 CustomOp Coverage

| Metric | Count |
|--------|-------|
| Plugin verbs in YAML | 588 |
| `#[register_custom_op]` in `domain_ops/` | 586 |
| **Gap** | **2 verbs** |

The 2-verb gap is confirmed by the `test_plugin_verb_coverage()` test in `rust/src/domain_ops/mod.rs`. The test asserts strict 1:1 coverage. Running this test will identify the exact missing implementations.

### 2.5 Missing Invocation Phrases (10 verbs)

These verbs cannot be discovered via semantic search:

| Verb | Domain | Added |
|------|--------|-------|
| `constellation.hydrate` | constellation | 2026-03-15 |
| `constellation.summary` | constellation | 2026-03-15 |
| `state.derive` | state | 2026-03-15 |
| `state.diagnose` | state | 2026-03-15 |
| `state.derive-all` | state | 2026-03-15 |
| `state.blocked-why` | state | 2026-03-15 |
| `state.check-consistency` | state | 2026-03-15 |
| `state.override` | state | 2026-03-15 |
| `state.revoke-override` | state | 2026-03-15 |
| `state.list-overrides` | state | 2026-03-15 |

All 10 are from the 2026-03-15 State Reducer Runtime and Constellation Hydration work. These were likely added to YAML but not yet enriched with invocation phrases.

---

## 3. Contract Completeness

### 3.1 Per-Verb Contract Fields

| Field | Populated | Percentage | Assessment |
|-------|-----------|------------|------------|
| `description` | 1,081/1,081 | 100% | Complete |
| `returns` | 1,044/1,081 | 97% | Strong |
| `args` (1+) | 1,040/1,081 | 96% | Strong |
| `invocation_phrases` | 1,071/1,081 | 99% | Strong (10 missing) |
| `phase_tags` (YAML) | 1,071/1,081 | 99% | Present in YAML but **not propagated to SemReg** |
| `crud_mapping` (CRUD verbs) | 481/481 | 100% | Complete for CRUD |
| `lifecycle` | 85/1,081 | 8% | **Critical gap** |
| `produces` | 29/1,081 | 3% | **Critical gap** |
| `consumes` | 6/1,081 | 1% | **Critical gap** |
| `preconditions` (YAML) | 0/1,081 | 0% | **Not implemented in YAML schema** |
| `postconditions` (YAML) | 0/1,081 | 0% | **Not implemented in YAML schema** |

### 3.2 Preconditions/Postconditions

**Finding [P1-SEV]:** Preconditions and postconditions are defined as fields on `VerbContractBody` but have **zero YAML support**. The scanner (`verb_config_to_contract()`) always emits `preconditions: vec![]` and `postconditions: vec![]`. The YAML config struct (`VerbConfig`) does not include these fields. The `lifecycle.precondition_checks` field exists on 50 verbs but is a different mechanism (runtime eligibility checks, not semantic contract preconditions).

### 3.3 Produces/Consumes

Only 29 verbs declare `produces` (what entity type they create) and 6 declare `consumes` (what bindings they require). These are concentrated in:

| Domain | `produces` count | `consumes` count |
|--------|-----------------|-----------------|
| cbu | 5 | 2 |
| entity | 3 | 0 |
| screening | 4 | 1 |
| kyc-case | 2 | 1 |
| service-resource | 3 | 1 |
| ownership | 4 | 0 |
| fund | 3 | 0 |
| Other | 5 | 1 |

The remaining 1,052 verbs have no declared data flow. This means the DAG analysis, expansion engine lock derivation, and runbook dependency ordering cannot determine statement ordering for 97% of verbs.

### 3.4 Lifecycle

85 verbs (8%) declare `lifecycle` constraints. These are distributed across 22 YAML files, concentrated in:
- `cbu.yaml` — CBU state transitions
- `deal.yaml` — Deal pipeline states
- `trading-profile.yaml` — Profile activation/approval
- `billing.yaml` — Billing period states
- `kyc/kyc.yaml` — Case lifecycle
- Various custody files — Settlement states

The remaining 996 verbs have no lifecycle constraints, meaning the `SessionVerbSurface` Step 5 (lifecycle state filter) has no metadata to filter on for those verbs even if entity_state were threaded through.

---

## 4. SemOS Profile Coverage Matrix

### 4.1 Registry Object Type Inventory

| Object Type | Active Count | Assessment |
|-------------|-------------|------------|
| `attribute_def` | 3,241 | Strong |
| `entity_type_def` | 262 | Strong |
| `verb_contract` | 1,004 | Strong count, **empty governance fields** |
| `derivation_spec` | 120 | Moderate |
| `membership_rule` | 8 | **Stub only** (all null bodies) |
| `policy_rule` | 1 | **Stub only** (null conditions/verdict) |
| `taxonomy_def` | 0 | **Empty** |
| `taxonomy_node` | 0 | **Empty** |
| `view_def` | 0 | **Empty** |
| `evidence_requirement` | 0 | **Empty** |
| `document_type_def` | 0 | **Empty** |
| `observation_def` | 0 | **Empty** |

### 4.2 Profile Coverage by Entity Domain

The Domain Metadata YAML (`domain_metadata.yaml`) maps 39 domains covering 350 tables (100% governance_tier tagged). Of these, 25 domains have `verb_data_footprint` mappings and 14 do not.

**Domains WITH verb-to-table footprint (25):**

| Domain | Tables | Verb Mappings |
|--------|--------|---------------|
| sem-reg | 30 | 83 |
| kyc | 29 | 34 |
| cbu | 19 | 28 |
| client-group | 10 | 23 |
| deal | 11 | 17 |
| entity | 23 | 13 |
| trading-profile | 4 | 13 |
| contract | 5 | 10 |
| session | 6 | 10 |
| booking-principal | 11 | 8 |
| billing | 8 | 8 |
| state | 1 | 8 |
| document | 5 | 5 |
| bpmn | 4 | 4 |
| custody | 32 | 3 |
| gleif | 2 | 4 |
| screening | 7 | 4 |
| ownership | 19 | 4 |
| fund | 10 | 3 |
| team | 7 | 3 |
| agent | 13 | 2 |
| product | 16 | 2 |
| investor | 4 | 1 |
| docs-bundle | 0 | 1 |
| requirement | 0 | 1 |

**Domains WITHOUT verb-to-table footprint (14):**

| Domain | Tables | Gap |
|--------|--------|-----|
| reference | 14 | No verb footprint |
| stewardship | 9 | No verb footprint |
| client-portal | 8 | No verb footprint |
| dsl | 21 | No verb footprint |
| lifecycle | 3 | No verb footprint |
| observation | 1 | No verb footprint |
| research | 2 | No verb footprint |
| feedback | 3 | No verb footprint |
| bods | 2 | No verb footprint |
| view | 3 | No verb footprint |
| workflow | 3 | No verb footprint |
| graph | 1 | No verb footprint |
| attribute | 3 | No verb footprint |
| schema-admin | 1 | No verb footprint |

### 4.3 SemReg vs YAML Disconnect

**Key finding:** The YAML verb definitions contain richer metadata than what arrives in SemReg:

| Field | YAML Coverage | SemReg Coverage | Gap Cause |
|-------|--------------|-----------------|-----------|
| `phase_tags` | 1,071/1,081 (99%) | 0/1,004 (0%) | Scanner drops phase_tags during conversion |
| `subject_kinds` | 44/1,081 (4%) | 0/1,004 (0%) | Scanner heuristic not populating |
| `lifecycle` | 85/1,081 (8%) | Not in VerbContractBody | Field not mapped to contract |
| `harm_class` | 30/1,081 (3%) | Not in VerbContractBody | Field not mapped to contract |
| `action_class` | 30/1,081 (3%) | Not in VerbContractBody | Field not mapped to contract |

The scanner (`verb_config_to_contract()`) does extract `phase_tags` from YAML metadata, but the live database shows 0% coverage. This suggests either: (a) the scanner runs but the metadata field is empty in the YAML despite being present as a key, or (b) the bootstrap process does not re-run after YAML enrichment. Investigation of the actual YAML content confirms that `phase_tags` IS present on 99% of verb YAML files, so the scanner conversion path needs verification.

---

## 5. Blocked / Stale Verb Families

### 5.1 Pure-CRUD Domains (No Plugin Logic)

These 26 domains contain ONLY crud-behavior verbs with no custom Rust handlers. They rely entirely on the generic CRUD executor:

| Domain | Verbs | File | Risk |
|--------|-------|------|------|
| fund | 22 | fund.yaml | Low — well-established domain |
| sla | 16 | sla.yaml | Low |
| contract | 14 | contract.yaml | Low |
| movement | 14 | registry/movement.yaml | Low |
| settlement-chain | 13 | custody/settlement-chain.yaml | Low |
| trade-gateway | 12 | custody/trade-gateway.yaml | Low |
| tax-config | 11 | custody/tax-config.yaml | Low |
| identifier | 11 | identifier.yaml | Low |
| holding | 10 | registry/holding.yaml | Low |
| cash-sweep | 9 | cash-sweep.yaml | Medium — complex financial logic as CRUD |
| corporate-action | 9 | custody/corporate-action.yaml | Medium — complex event-driven logic as CRUD |
| instruction-profile | 7 | custody/instruction-profile.yaml | Low |
| user | 7 | team.yaml | Low |
| share-class | 6 | registry/share-class.yaml | Low |
| isda | 6 | custody/isda.yaml | Medium — legal contract as CRUD |
| allegation | 6 | observation/allegation.yaml | Low |
| admin.regulators | 5 | admin/regulators.yaml | Low |
| admin.role-types | 5 | admin/role-types.yaml | Low |
| delegation | 4 | delegation.yaml | Low |
| discrepancy | 4 | observation/discrepancy.yaml | Low |
| kyc-agreement | 4 | kyc-agreement.yaml | Low |
| role | 4 | refdata/role.yaml | Low |
| delivery | 3 | delivery.yaml | Low |
| entity-settlement | 3 | custody/entity-settlement.yaml | Low |
| instrument-class | 3 | reference/instrument-class.yaml | Low |
| service | 3 | service.yaml | Low |

**Assessment:** Most pure-CRUD domains are appropriate for table-level operations. `cash-sweep`, `corporate-action`, and `isda` may warrant plugin behavior if business logic complexity grows.

### 5.2 Stub/Placeholder Verbs

| Verb | Description Contains | Status |
|------|---------------------|--------|
| `schema.generate-discovery-map` | "Phase 2 stub" | Stub — discovery map generation not yet implemented |
| `registry.discover-dsl` | "Phase 2 stub" | Stub — discovery DSL pipeline not yet implemented |

### 5.3 Recently Added Domains (Potentially Incomplete)

These domains were added in the 2026-03-15 work and are missing invocation phrases:

| Domain | Verbs | Missing Phrases | Status |
|--------|-------|-----------------|--------|
| `constellation` | 2 | 2/2 (100%) | New — needs phrase enrichment |
| `state` | 8 | 8/8 (100%) | New — needs phrase enrichment |

### 5.4 Domains with Minimal CRUD Pattern Only

| Domain | Actions | Assessment |
|--------|---------|------------|
| `booking-location` | create, list, update | Minimal — may need delete, get |
| `legal-entity` | create, list, update | Minimal — may need delete, get |

---

## 6. Discrimination Dimension Coverage

### 6.1 Metadata Dimensions

| Dimension | Populated | Percentage | Assessment |
|-----------|-----------|------------|------------|
| `tier` (metadata) | 1,081/1,081 | 100% | Complete |
| `source_of_truth` (metadata) | 1,081/1,081 | 100% | Complete |
| `phase_tags` (metadata) | 1,071/1,081 | 99% | Strong in YAML, **zero in SemReg** |
| `harm_class` (metadata) | 30/1,081 | 2.8% | **Critical gap** |
| `action_class` (metadata) | 30/1,081 | 2.8% | **Critical gap** |
| `subject_kinds` (metadata) | 44/1,081 | 4.1% | **Critical gap** |
| `lifecycle` (config) | 85/1,081 | 7.9% | Moderate gap |
| `lifecycle.precondition_checks` | ~50/1,081 | 4.6% | Gap |
| `produces` (config) | 29/1,081 | 2.7% | **Critical gap** |
| `consumes` (config) | 6/1,081 | 0.6% | **Critical gap** |

### 6.2 harm_class Coverage

Only 2 YAML files use `harm_class`: `discovery.yaml` and `agent.yaml`. The `HarmClass` enum has 4 variants:
- `ReadOnly` — default (implicit on all 1,051 untagged verbs)
- `Reversible`
- `Irreversible`
- `Destructive`

Without explicit `harm_class` tagging, the system cannot distinguish between a read-only discovery verb and a destructive delete verb at the metadata level. The intent pipeline's safety-first policy (`ExpectedOutcome::MatchedOrAmbiguous`) compensates at the test level, but runtime consumers have no signal.

### 6.3 action_class Coverage

Only 2 YAML files use `action_class`: `discovery.yaml` and `agent.yaml`. The `ActionClass` enum has 15 variants:
`List`, `Read`, `Search`, `Describe`, `Create`, `Update`, `Delete`, `Assign`, `Remove`, `Import`, `Compute`, `Review`, `Approve`, `Reject`, `Execute`

Without explicit tagging, the system cannot classify verbs by action type for UI grouping, safety gating, or governance policy.

### 6.4 subject_kinds Coverage

24 YAML files declare `subject_kinds` (out of 116 total). These are concentrated in sem-reg verbs and a few core domains. The scanner has a `domain_to_subject_kind()` heuristic but it only covers a subset of domains. For the remaining 96% of verbs, entity-kind constrained verb selection is disabled.

---

## 7. The 8-Step SessionVerbSurface Pipeline

The `compute_session_verb_surface()` function in `rust/src/agent/verb_surface.rs` runs at Stage 2.5 in every orchestrator turn.

### 7.1 Pipeline Steps and Data Status

| Step | Description | Data Available? | Effective? |
|------|-------------|----------------|------------|
| 1 | Base set from `RuntimeVerbRegistry` | ~642 verbs | Yes |
| 2 | AgentMode filter (Research vs Governed) | Mode set in session | Yes |
| 3 | Workflow phase filter (`stage_focus` -> domain allowlists) | Hardcoded in Rust | Yes -- but hardcoded |
| 4 | SemReg CCIR (`ContextEnvelope.allowed_verbs`) | Present but data-free | Partially |
| 5 | Lifecycle state filter | `entity_state: None` always | Never executes |
| 6 | Actor gating | Extension point | Passthrough |
| 7 | FailPolicy (`FailClosed` safe-harbor ~30 verbs) | Hardcoded | Yes when triggered |
| 8 | Rank & fingerprint | Deterministic | Yes |

**Effective filtering:** Only Steps 1-3 and 8 materially affect the verb surface. Step 4 returns full unfiltered results because no ViewDefs, TaxonomyDefs, or phase_tags exist in SemReg.

### 7.2 Governance Inversion

The intended architecture:
```
SemReg registry (ViewDefs, phase_tags, TaxonomyNodes)
  -> resolve_context() 12-step pipeline
  -> ContextEnvelope.allowed_verbs
  -> SessionVerbSurface Step 4
  -> Runtime verb surface
```

The actual runtime:
```
Hardcoded Rust match expression (workflow_allowed_domains)
  -> SessionVerbSurface Step 3
  -> Runtime verb surface

SemReg resolve_context() -> full unfiltered verb set (no data)
  -> ContextEnvelope = {all 1,004 verbs}
  -> Step 4 = no additional filtering
```

---

## 8. SemReg Registry Data Gaps

### 8.1 Empty Governance Object Types

| Object Type | Active Count | Impact |
|-------------|-------------|--------|
| `view_def` | 0 | Steps 3-4 of resolve_context() return no candidates |
| `taxonomy_def` | 0 | Taxonomy-based verb surface filtering always returns full set |
| `taxonomy_node` | 0 | Same as above |
| `evidence_requirement` | 0 | Evidence freshness checks cannot function |
| `document_type_def` | 0 | Document governance integration not materialized |
| `observation_def` | 0 | Observation recording templates missing |

### 8.2 Verb Contract Governance Fields (All Empty in SemReg)

| Field | SemReg State | Cause |
|-------|-------------|-------|
| `phase_tags` | All empty | Scanner may not propagate YAML metadata to contract body |
| `subject_kinds` | All empty | Scanner heuristic not reaching contract body |
| `preconditions` | All empty | Not in YAML schema; scanner always emits empty vec |
| `postconditions` | All empty | Not in YAML schema; scanner always emits empty vec |
| `requires_subject` | All true | Scanner default; no YAML override mechanism |
| `produces_focus` | All false | Scanner default; no YAML override mechanism |

### 8.3 sem_reg_pub Schema

The `sem_reg_pub` schema contains zero views. Documented views (`active_requirement_profiles`, `active_proof_obligations`, `active_evidence_strategies`) have not been created.

---

## 9. Findings (Severity-Tagged)

### P0 — Registry Governance Pipeline Nullified

| ID | Finding | Impact |
|----|---------|--------|
| P0-1 | Zero ViewDefs in SemReg | resolve_context() Steps 3-4 produce no candidates; workflow-specific verb scoping impossible through registry |
| P0-2 | Zero TaxonomyDefs/TaxonomyNodes | Taxonomy-based verb filtering is always a no-op; membership rules cannot be evaluated |
| P0-3 | `phase_tags` empty on all 1,004 verb contracts in SemReg | Goals-based filtering (`filter_and_rank_verbs()`) returns full verb set for every request |
| P0-4 | `subject_kinds` empty on all 1,004 verb contracts | Entity-kind constrained verb selection disabled at registry level |

### P1 — Missing Discrimination Metadata

| ID | Finding | Impact |
|----|---------|--------|
| P1-1 | `harm_class` on 2.8% of verbs (30/1,081) | Cannot distinguish read-only from destructive verbs at metadata level; safety gating relies on test harness, not runtime signal |
| P1-2 | `action_class` on 2.8% of verbs (30/1,081) | Cannot classify verbs by action type for UI grouping or governance policy |
| P1-3 | `produces`/`consumes` on 3%/1% of verbs | DAG analysis, expansion lock derivation, and runbook dependency ordering blind for 97% of verbs |
| P1-4 | `preconditions`/`postconditions` at 0% | Verb contract semantic surface incomplete; not implemented in YAML schema at all |
| P1-5 | Lifecycle state filter disabled (`entity_state: None`) | Verb eligibility does not change with entity state at runtime |

### P2 — Stale/Incomplete Verb Families

| ID | Finding | Impact |
|----|---------|--------|
| P2-1 | 10 verbs missing `invocation_phrases` (constellation.*, state.*) | Cannot be discovered via semantic search; invisible to intent pipeline |
| P2-2 | 2 stub verbs (`schema.generate-discovery-map`, `registry.discover-dsl`) | Described as "Phase 2 stub" in descriptions |
| P2-3 | 14 domain metadata domains without `verb_data_footprint` | AffinityGraph verb-to-table mapping incomplete for reference, stewardship, lifecycle, observation, research, workflow domains |

### P3 — Minor

| ID | Finding | Impact |
|----|---------|--------|
| P3-1 | `sem_reg_pub` schema has zero views | Document governance views not materialized |
| P3-2 | 2-verb CustomOp gap (586 vs 588) | Minor; `test_plugin_verb_coverage()` test will catch specific verbs |

---

## 10. Recommendations

### Immediate (P0)

1. **Fix scanner `phase_tags` propagation:** Verify that `verb_config_to_contract()` in `scanner.rs` correctly reads `metadata.phase_tags` from VerbConfig and writes it to `VerbContractBody.phase_tags`. The YAML has 99% coverage; the SemReg database has 0%. Re-run bootstrap after fix.

2. **Seed 5 ViewDefs:** Create active ViewDefs for onboarding, kyc, data-management, stewardship, and navigation via the governed authoring pipeline. Each defines a `verb_surface` scoped by `phase_tags`. This converts the hardcoded `workflow_allowed_domains()` from code to data.

3. **Publish canonical taxonomy seeds:** The 4 KYC-canonical taxonomy seeds (subject-category, risk-tier, document-class, jurisdiction-regime) exist in Rust code (`seeds/taxonomy_seeds.rs`) but are not published as active snapshots.

4. **Extend scanner `subject_kinds` heuristic:** The existing `domain_to_subject_kind()` covers a subset. Extend to all domains using CRUD table names and `produces.entity_type` as signals.

### Short-Term (P1)

5. **Batch-tag `harm_class`:** Apply default `harm_class` across all 1,081 verbs using action-name heuristics: `delete*`/`remove*`/`cascade*` -> Destructive, `create*`/`update*`/`assign*` -> Reversible, `list*`/`get*`/`read*`/`search*` -> ReadOnly.

6. **Batch-tag `action_class`:** Same heuristic approach: verb action name maps to ActionClass variant.

7. **Add invocation phrases to constellation.* and state.* verbs:** 10 verbs added 2026-03-15 need phrase enrichment + `populate_embeddings`.

8. **Thread entity_state into SessionVerbSurface:** The lifecycle infrastructure exists. The orchestrator needs to resolve entity state from session scope and pass to `VerbSurfaceContext`.

### Medium-Term (P2)

9. **Add `preconditions`/`postconditions` to YAML schema:** Extend `VerbConfig` struct with `preconditions` and `postconditions` fields. Update scanner to propagate to `VerbContractBody`.

10. **Complete verb_data_footprint for 14 ungapped domains:** Especially `stewardship`, `reference`, `research`, `workflow`.

11. **Create sem_reg_pub views:** `active_requirement_profiles`, `active_proof_obligations`, `active_evidence_strategies`.

---

## Appendix A: Full Domain Distribution

```
deal:42  cbu:37  trading-profile:32  capital:30  registry:26
service-resource:26  client-group:23  fund:22  ownership:22
document:21  agent:20  investor:20  ubo:20  session:18  billing:17
control:16  gleif:16  verify:16  entity:15  team:15  view:14
contract:14  schema:14  changeset:14  discovery:13  settlement-chain:13
movement:14  trade-gateway:12  identifier:11  tax-config:11  holding:10
research.sources:10  sla:10  cash-sweep:9  governance:9  graph:9
corporate-action:9  screening:8  red-flag:8  state:8  request:8
maintenance:7  instruction-profile:7  user:7  temporal:8  batch:8
semantic:6  audit:8  share-class:6  focus:6  allegation:6  isda:6
service-pipeline:6  bods:6  booking-principal:8  skeleton:1
cbu-specialist-roles:6  admin.regulators:5  admin.role-types:5
refdata:10  product:3  pack:3  template:2  bpmn:5  regulatory:3
onboarding:3  service:3  legal-entity:3  booking-location:3
client-principal-relationship:4  kyc-agreement:4  delegation:4
pricing-config:5  docs-bundle:3  requirement:3  manco-group:4
discrepancy:4  role:4  entity-settlement:3  delivery:3
instrument-class:3  service-availability:3  client:3  constellation:2
investment-manager:1  edge:1  matrix-overlay:3  ruleset:3  rule:3
rule-field:2  contract-pack:3  kyc-case:4  entity-workstream:2
evidence:5  coverage:1  economic-exposure:2  investor-role:6
research.*:13  kyc.*:28  observation:3  tollgate:5
fund-vehicle:2  subcustodian:1  security-type:1
service-intent:3  readiness:2  provisioning:2  pipeline:1
bulk-load:1  core:4
```

## Appendix B: Query Evidence

```sql
-- ViewDef, TaxonomyDef, TaxonomyNode -- all return 0 rows
SELECT object_type, COUNT(*) FROM sem_reg.snapshots
WHERE status = 'active'
  AND object_type IN ('view_def', 'taxonomy_def', 'taxonomy_node')
GROUP BY object_type;
-- Results: (empty)

-- phase_tags -- all verb contracts return empty array
SELECT COUNT(*) FILTER (
  WHERE jsonb_array_length(definition->'phase_tags') > 0
) AS has_phase_tags FROM sem_reg.snapshots
WHERE status = 'active' AND object_type = 'verb_contract';
-- Results: 0

-- subject_kinds -- all verb contracts return empty array
SELECT COUNT(*) FILTER (
  WHERE jsonb_array_length(definition->'subject_kinds') > 0
) AS has_subject_kinds FROM sem_reg.snapshots
WHERE status = 'active' AND object_type = 'verb_contract';
-- Results: 0
```
