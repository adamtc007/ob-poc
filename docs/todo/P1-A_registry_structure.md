# P1-A: SemOS Registry Structure & Coverage Audit

**Review Session:** P1-A
**Date:** 2026-03-16
**Scope:** `sem_reg.snapshots` table — object type distribution, governance tier posture, scanner fidelity, and structural gaps in the live registry.

---

## 1. Executive Summary

The Semantic OS registry contains **4,875 active snapshots** across **7 of 16 defined object types**. Every single snapshot carries `governance_tier = operational` and `trust_class = convenience`. The governed tier has never been used in production. Nine object types defined in the Rust enum have zero live entries. The verb contract scanner populates schema-level metadata (fqn, domain, action, description, behavior, crud_mapping) but leaves all relational contract fields empty: preconditions, postconditions, writes_to, reads_from, and invocation_phrases are uniformly absent across all 1,004 verb_contracts.

The registry functions as a read-only catalogue of what exists, not a governance instrument capable of constraining or reasoning about system behaviour.

---

## 2. Object Type Distribution

### 2.1 Populated Types (7/16)

| Object Type | Active Snapshots | % of Total | Notes |
|---|---|---|---|
| `attribute_def` | 3,241 | 66.5% | Auto-generated from YAML verb args and entity definitions |
| `verb_contract` | 1,004 | 20.6% | Scanned from YAML, incomplete contract fields |
| `relationship_type_def` | 358 | 7.3% | Generated from schema FK relationships |
| `entity_type_def` | 262 | 5.4% | 3 source domains |
| `membership_rule` | 8 | 0.2% | Effectively empty governance |
| `policy_rule` | 1 | <0.1% | Single rule across the entire system |
| `evidence_requirement` | 1 | <0.1% | Single requirement record |

**Total: 4,875 active snapshots**

### 2.2 Phantom Types (9/16 — Zero Entries)

Nine ObjectType enum variants are defined in `sem_os_core::types::ObjectType` but have no live database records:

| Object Type | Purpose (Design Intent) | Impact of Absence |
|---|---|---|
| `TaxonomyDef` | Hierarchical classification trees | No taxonomy-based verb/attribute filtering |
| `TaxonomyNode` | Individual taxonomy nodes | Dependent on TaxonomyDef; entire subsystem dark |
| `ViewDef` | Verb surface + attribute prominence per context | `resolve_context()` returns no view candidates |
| `DocumentTypeDef` | Document classification | Document governance ungrounded |
| `RequirementProfileDef` | Governed requirement matrices | KYC/entity document flow lacks formal profiles |
| `ProofObligationDef` | Proof obligation specifications | Evidence chain verification impossible |
| `EvidenceStrategyDef` | Evidence collection strategies | Strategy selection code path never exercised |
| `ObservationDef` | Observation recording templates | No templates; observation semantics undefined |
| `DerivationSpec` | Derived/composite attribute computation | Zero derivation coverage; lineage graph empty |

The absence of TaxonomyDef and TaxonomyNode is particularly significant: the `context_resolution.rs` pipeline has a dedicated step (Step 4) for taxonomy-based verb surface filtering, which silently returns the full unfiltered verb set because no taxonomy overlap can be computed.

---

## 3. Governance Tier Posture

### 3.1 Critical Finding: Zero Governed Objects

```
governance_tier distribution across all 4,875 active snapshots:
  operational: 4,875  (100.0%)
  governed:         0  (  0.0%)

trust_class distribution:
  convenience:  4,875  (100.0%)
  decision_support:  0  (  0.0%)
  proof:             0  (  0.0%)
```

The `GovernanceTier::Governed` and `TrustClass::Proof` code paths in the publish gate framework, ABAC evaluation, and context resolution pipeline have **never been exercised in production**. The Proof Rule gate (which blocks `TrustClass::Proof` unless `governance_tier = Governed`) is structurally sound but permanently inactive.

### 3.2 Root Cause: SnapshotMeta Constructor Lock-In

All snapshots are created via `SnapshotMeta::new_operational()`:

```rust
// rust/crates/sem_os_core/src/types.rs
pub fn new_operational(...) -> Self {
    SnapshotMeta {
        governance_tier: GovernanceTier::Operational,
        trust_class: TrustClass::Convenience,
        approved_by: Some("auto".to_string()),
        ...
    }
}
```

This is the only constructor invoked by the scanner, the seed bundle bootstrap, and the stewardship authoring pipeline. No code path creates a `GovernanceTier::Governed` snapshot. The changeset workflow in `sem_reg.changesets` exists (migrations 095, 097, 099) but the publish path through `authoring_publish()` still creates snapshots via the same operational constructor.

### 3.3 Implications for Downstream Systems

| System | Governed-Tier Behaviour | Current State |
|---|---|---|
| ABAC `evaluate_abac()` | Restricts high-clearance objects to matching actors | All objects public-accessible (all operational) |
| Publish gate: Proof Rule | Blocks Proof trust without Governed tier | Never triggers (zero Proof objects) |
| Changeset approval workflow | Requires reviewer sign-off for Governed changes | Bypassed by operational auto-approve |
| `resolve_context()` governance signals | Signals unowned/unreviewed objects | All objects flagged as operational, no escalation |
| Stewardship guardrail G04 | Proof chain validation | Unreachable (zero Proof trust class) |

---

## 4. Verb Contract Scanner Fidelity

### 4.1 Fields Populated by Scanner

The `sem_os_obpoc_adapter` scanner (`rust/crates/sem_os_obpoc_adapter/src/scanner.rs`) populates the following `VerbContractBody` fields from YAML:

| Field | Source | Status |
|---|---|---|
| `fqn` | `{domain}.{action}` | ✅ Populated |
| `domain` | YAML domain key | ✅ Populated |
| `action` | YAML action key | ✅ Populated |
| `description` | YAML `description:` | ✅ Populated |
| `behavior` | YAML `behavior:` | ✅ Populated |
| `args` | YAML `args:[]` | ✅ Populated |
| `returns` | YAML `returns:` | ✅ Populated |
| `crud_mapping` | YAML `crud.*` | ✅ Populated (for crud behavior) |
| `subject_kinds` | YAML `metadata.subject_kinds` | ✅ Populated (via scanner heuristic) |
| `phase_tags` | YAML `metadata.phase_tags` | ✅ Populated |
| `metadata` | YAML `metadata.*` | ✅ Populated |

### 4.2 Fields NOT Populated (All Zero Across 1,004 Verb Contracts)

| Field | VerbContractBody Type | Live DB Count | Root Cause |
|---|---|---|---|
| `preconditions` | `Vec<VerbPrecondition>` | **0** | Scanner does not parse `lifecycle.preconditions` YAML |
| `postconditions` | `Vec<String>` | **0** | Scanner does not parse `lifecycle.postconditions` YAML |
| `writes_to` | `Vec<String>` | **0** | DomainMetadata overlay not threaded through scanner |
| `reads_from` | `Vec<String>` | **0** | DomainMetadata overlay not threaded through scanner |
| `invocation_phrases` | `Vec<String>` | **0** | Phrases stored in `ob-poc.dsl_verbs`, not in registry |
| `produces` | `Option<VerbProducesSpec>` | Not queried | Likely partially populated from `produces:` YAML |
| `consumes` | `Vec<String>` | Not queried | Likely empty |

**Consequence:** The verb contract registry cannot answer "what does this verb write?", "what does it require?", or "how can a user invoke it?" — the three questions a governance-aware agent most needs to answer.

### 4.3 Template Verb Registration Gap

```
Template-behavior verb_contracts in registry:   0
Template verbs defined in config/verbs/*.yaml: ~54 macros
```

No macro/template verb is registered in the SemOS registry. The macro expansion system (`rust/src/dsl_v2/macros/`) and the registry operate as completely separate systems. Cross-referencing a macro's verb surface against governed policy is structurally impossible.

### 4.4 Invocation Phrase Schism

Invocation phrases live in two mutually exclusive stores:
- `ob-poc.dsl_verbs.yaml_intent_patterns` / `ob-poc.verb_pattern_embeddings` — semantic search index (15,940 patterns)
- `sem_reg.snapshots` (verb_contract.invocation_phrases) — **zero entries**

The SemOS registry has no knowledge of how verbs are discovered. Conversely, the intent pipeline has no knowledge of which verbs are governed or what their preconditions are.

---

## 5. Entity Type Definition Coverage

### 5.1 Domain Distribution

| Domain | Entity Type Defs | Notes |
|---|---|---|
| `ob_poc` | 209 | Business entity types |
| `kyc` | 37 | KYC/compliance entity types |
| `sem_reg` | 16 | Registry meta-entity types |
| **Total** | **262** | |

### 5.2 Coverage Gap

The `ob-poc` schema contains ~306 tables per domain metadata coverage. With 209 entity type defs covering the `ob_poc` domain, approximately **97 tables lack a canonical entity type definition** in the registry. These tables are invisible to `resolve_context()` taxonomy filtering and AffinityGraph entity-to-table mapping.

### 5.3 Relationship Type Definitions

358 relationship_type_def entries exist — the second most populous non-attribute type. However, per the peer review remediation (D5), `edge_class` and `directionality` fields were only recently added to `RelationshipTypeDefBody`. The proportion of existing entries with populated `edge_class` is unknown and likely low.

---

## 6. Governance Object Sparsity Analysis

### 6.1 The Governance Desert

| Object Type | Count | Governs |
|---|---|---|
| `membership_rule` | 8 | Taxonomy membership (8 rules for entire system) |
| `policy_rule` | 1 | ABAC/access policy (1 rule for 262 entity types) |
| `evidence_requirement` | 1 | Evidence collection requirements |

The governance layer is structurally hollow. 1,004 verb contracts operate with one policy rule. 262 entity types have membership coverage from 8 rules. The changeset workflow, stewardship guardrails, and governance audit log exist in code and schema but govern zero meaningful objects.

### 6.2 Membership Rule Coverage

With 8 membership rules covering system with hundreds of verb and entity combinations, each rule must be governing dozens of implicit members. The actual taxonomy overlap computation in `resolve_context()` Step 4 is therefore returning either everything or nothing, not genuinely scoped verb surfaces.

---

## 7. AttributeDef Dominance

3,241 attribute_def entries represent 66.5% of the registry. This is the most complete object type:
- Derived from verb YAML args and entity schema columns
- Auto-populated by the scanner
- However: `AttributeSource` (schema, table, column) triple resolution quality varies; real source triples added in scanner improvement but may not be backfilled for older entries

**Structural concern:** AttributeDefs vastly outnumber the entity types they belong to (3,241 vs 262). Average of ~12.4 attributes per entity type, but this average is misleading — many attributes may be orphaned (no entity type def references them) or overcounted (same attribute modelled in multiple domains).

---

## 8. Snapshot Set Coherence

All 4,875 snapshots belong to the same `snapshot_set_id`. No previous snapshot_sets exist in the active view (retired sets are not in scope). This means:
- Zero point-in-time rollback options currently exist
- The `resolve_at(as_of)` API would return the current state for any timestamp prior to the initial bootstrap
- Snapshot supersession chains (version history) exist within the single set but cross-set history is absent

---

## 9. Structural Gaps — Priority Matrix

| Gap | Impact | Effort to Fix | Priority |
|---|---|---|---|
| Zero governed-tier objects | ABAC, proof chain, changeset approval all inactive | High — requires authoring workflow | P0 |
| 9 phantom object types | TaxonomyDef/Node disables verb surface scoping; ViewDef disables view selection | Medium — seed data required | P0 |
| Verb contract preconditions/postconditions empty | Agent cannot enforce prerequisites | Low — scanner extension | P1 |
| writes_to/reads_from empty | AffinityGraph cannot compute verb data footprint from registry | Low — DomainMetadata thread-through | P1 |
| Template verbs not registered | Macros are ungoverned | Medium — scanner extension needed | P1 |
| Invocation phrases not in registry | Intent pipeline and governance pipeline disconnected | Medium — dual-write or sync | P2 |
| policy_rule = 1 | Single policy for entire system | Medium — policy authoring needed | P2 |
| 97 tables without entity_type_def | Context resolution has blind spots | High — entity type authoring | P2 |
| edge_class/directionality missing on relationship_type_defs | Relationship-aware verb ranking ineffective | Low — backfill migration | P3 |

---

## 10. Recommendations

1. **Establish a governed seed bundle** containing at minimum 5 TaxonomyDefs (domain classification trees), 20 ViewDefs (one per major workflow context), and 50 PolicyRules (one per entity type cluster) — published via the changeset authoring pipeline with `governance_tier = Governed`.

2. **Extend the scanner** to populate `preconditions`, `postconditions`, `writes_to`, `reads_from` from YAML `lifecycle:` blocks and domain_metadata.yaml VerbFootprint entries. Target: all 1,004 verb_contracts have ≥1 non-empty contract field.

3. **Register template/macro verbs** in the registry with `behavior = template` and their expansion targets listed in `produces`/`consumes`. This enables governance-aware macro routing.

4. **Bootstrap minimum viable governance**: promote at least 10 verb_contracts (the most sensitive write verbs) to `governance_tier = Governed` with manual `trust_class = DecisionSupport` through the stewardship changeset workflow. This validates the approval pipeline end-to-end.

5. **Run an attribute orphan analysis**: identify AttributeDef entries with no referencing EntityTypeDef and either link or retire them. The 3,241:262 ratio suggests significant orphan population.

---

## Appendix: Query Evidence

```sql
-- Object type distribution (live query)
SELECT object_type, COUNT(*) AS cnt
FROM sem_reg.snapshots
WHERE status = 'active'
GROUP BY object_type
ORDER BY cnt DESC;
-- Results: attribute_def(3241), verb_contract(1004), relationship_type_def(358),
--          entity_type_def(262), membership_rule(8), policy_rule(1), evidence_requirement(1)

-- Governance tier (all operational)
SELECT governance_tier, trust_class, COUNT(*)
FROM sem_reg.snapshots WHERE status = 'active'
GROUP BY governance_tier, trust_class;
-- Results: operational/convenience = 4875

-- Verb contract completeness
SELECT
  COUNT(*) FILTER (WHERE jsonb_array_length(definition->'preconditions') > 0) AS has_preconditions,
  COUNT(*) FILTER (WHERE jsonb_array_length(definition->'postconditions') > 0) AS has_postconditions,
  COUNT(*) FILTER (WHERE definition->>'behavior' = 'template') AS template_behavior,
  COUNT(*) FILTER (WHERE jsonb_array_length(definition->'writes_to') > 0) AS has_writes_to,
  COUNT(*) FILTER (WHERE jsonb_array_length(definition->'reads_from') > 0) AS has_reads_from,
  COUNT(*) FILTER (WHERE jsonb_array_length(definition->'invocation_phrases') > 0) AS has_invocation_phrases
FROM sem_reg.snapshots
WHERE status = 'active' AND object_type = 'verb_contract';
-- Results: all columns = 0
```

---

## 11. Metadata-Layer Coverage Analysis

### 11.1 DomainMetadata YAML Statistics

Source: `rust/config/sem_os_seeds/domain_metadata.yaml`

```
Metadata domains:            39
Declared tables:            348
Verbs with footprint:       292
Footprint-referenced tables: 132
Phantom tables:              55
```

The domain metadata overlay covers 39 of the system's 131 distinct verb domains. Within those 39 domains, 292 verbs have explicit `reads`/`writes` footprint declarations — covering roughly 23% of the ~1,263 total verbs. The remaining 77% of verbs have no data footprint mapping, making them invisible to AffinityGraph queries like `verbs_for_table()` and `data_for_verb()`.

### 11.2 Phantom Table References (55 tables)

55 tables are referenced in `verb_data_footprint` entries but are **not declared** in any domain's `tables:` section. These create dangling references in the AffinityGraph builder (Pass 1) — the graph knows a verb touches a table, but the table has no governance metadata (tier, classification, PII flag).

**High-traffic phantoms** (referenced by 5+ verbs):

| Phantom Table | Referencing Verb Count | Example Verbs |
|---|---|---|
| `entities` | 34 | `client-group.entity-add`, `cbu.create-from-client-group`, `gleif.import-tree`, `screening.*` |
| `cbus` | 14 | `session.load-galaxy`, `contract.subscribe`, `kyc-case.create`, `billing.add-account-target` |
| `cbu_entity_roles` | 9 | `ownership.refresh`, `gleif.import-managed-funds`, `state.derive`, `state.diagnose` |
| `cases` | 8 | `cbu.decide`, `state.derive`, `state.diagnose`, `state.override` |
| `entity_workstreams` | 7 | `screening.run`, `state.derive`, `state.diagnose`, `state.override` |
| `screenings` | 7 | `screening.run`, `state.derive`, `state.diagnose`, `state.override` |
| `client_group` | 6 | `cbu.create-from-client-group`, `deal.create`, `session.load-galaxy` |
| `case_events` | 3 | `screening.run`, `state.override`, `state.revoke-override` |
| `deals` | 3 | `billing.create-profile`, `billing.revenue-summary`, `kyc-case.create` |

The `entities` and `cbus` tables are the two most referenced tables in the entire system yet neither is declared in any domain's `tables:` metadata. This means the AffinityGraph can build forward edges (verb→table) but has no entity-to-table bimap entry (Pass 2) and no governance annotation for these tables.

**Cross-schema phantoms:** `kyc.cases`, `kyc.ownership_snapshots`, `sem_reg.basis_claims`, `sem_reg.basis_records`, `sem_reg.conflict_records`, `sem_reg.events`, `sem_reg.focus_states`, `sem_reg.templates`, `sem_reg.viewport_manifests` — these reference tables in schemas outside `"ob-poc"` and will fail the default schema resolution (`Tables without a schema prefix default to "ob-poc"`).

### 11.3 Domain Namespace Mismatch

| Direction | Count | Significance |
|---|---|---|
| Verb YAML domains with no metadata entry | 92 | These verbs have zero data footprint — invisible to AffinityGraph |
| Metadata domains with no verb YAML | 11 | Table governance exists but no verbs reference it |

**Metadata-only domains** (11): `client-portal`, `custody`, `dsl`, `feedback`, `lifecycle`, `reference`, `research`, `schema-admin`, `sem-reg`, `stewardship`, `workflow`

These domains have table declarations and governance metadata but no matching verb YAML domain. The `custody` domain is particularly notable with 32 declared tables — the second-largest metadata domain — yet no verb coverage at all. The `sem-reg` domain has 30 tables and 83 footprint entries, but its verb YAML lives under different domain names (`registry`, `changeset`, `governance`, `audit`, `focus`, etc.).

---

## 12. Domain Coverage Summary Table

**Severity classification:**
- **CLEAN** (7): Footprint coverage >= 80% of verb count
- **MINOR** (18): Footprint coverage 40-79%, or metadata-only domain
- **FLAG** (106): Footprint coverage < 40%, or no metadata at all

| Domain | Verb Count | Metadata Tables | Footprint Verbs | Coverage % | Severity |
|---|---|---|---|---|---|
| `admin.regulators` | 5 | 0 | 0 | 0.0% | FLAG |
| `admin.regulatory-tiers` | 2 | 0 | 0 | 0.0% | FLAG |
| `admin.role-types` | 5 | 0 | 0 | 0.0% | FLAG |
| `agent` | 20 | 13 | 2 | 10.0% | FLAG |
| `allegation` | 6 | 0 | 0 | 0.0% | FLAG |
| `attribute` | 11 | 3 | 0 | 0.0% | FLAG |
| `attributes` | 4 | 0 | 0 | 0.0% | FLAG |
| `audit` | 8 | 0 | 0 | 0.0% | FLAG |
| `batch` | 7 | 0 | 0 | 0.0% | FLAG |
| `billing` | 17 | 8 | 8 | 47.1% | MINOR |
| `board` | 9 | 0 | 0 | 0.0% | FLAG |
| `bods` | 9 | 2 | 0 | 0.0% | FLAG |
| `booking-location` | 3 | 0 | 0 | 0.0% | FLAG |
| `booking-principal` | 9 | 11 | 8 | 88.9% | CLEAN |
| `bpmn` | 5 | 4 | 4 | 80.0% | CLEAN |
| `capital` | 30 | 0 | 0 | 0.0% | FLAG |
| `case-event` | 2 | 0 | 0 | 0.0% | FLAG |
| `cash-sweep` | 9 | 0 | 0 | 0.0% | FLAG |
| `cbu` | 37 | 19 | 28 | 75.7% | MINOR |
| `cbu-custody` | 8 | 0 | 0 | 0.0% | FLAG |
| `changeset` | 14 | 0 | 0 | 0.0% | FLAG |
| `client-group` | 23 | 10 | 23 | 100.0% | CLEAN |
| `client-portal` | 0 | 8 | 0 | — | MINOR |
| `client-principal-relationship` | 4 | 0 | 0 | 0.0% | FLAG |
| `constellation` | 2 | 0 | 0 | 0.0% | FLAG |
| `contract` | 14 | 5 | 10 | 71.4% | MINOR |
| `contract-pack` | 2 | 0 | 0 | 0.0% | FLAG |
| `control` | 16 | 0 | 0 | 0.0% | FLAG |
| `corporate-action` | 9 | 0 | 0 | 0.0% | FLAG |
| `coverage` | 1 | 0 | 0 | 0.0% | FLAG |
| `custody` | 0 | 32 | 3 | — | MINOR |
| `deal` | 42 | 11 | 17 | 40.5% | MINOR |
| `delegation` | 4 | 0 | 0 | 0.0% | FLAG |
| `delivery` | 3 | 0 | 0 | 0.0% | FLAG |
| `discovery` | 12 | 0 | 0 | 0.0% | FLAG |
| `discrepancy` | 4 | 0 | 0 | 0.0% | FLAG |
| `docs-bundle` | 3 | 0 | 1 | 33.3% | FLAG |
| `document` | 21 | 5 | 5 | 23.8% | FLAG |
| `dsl` | 0 | 21 | 0 | — | MINOR |
| `economic-exposure` | 2 | 0 | 0 | 0.0% | FLAG |
| `edge` | 1 | 0 | 0 | 0.0% | FLAG |
| `entity` | 15 | 23 | 13 | 86.7% | CLEAN |
| `entity-settlement` | 3 | 0 | 0 | 0.0% | FLAG |
| `entity-workstream` | 9 | 0 | 0 | 0.0% | FLAG |
| `evidence` | 5 | 0 | 0 | 0.0% | FLAG |
| `feedback` | 0 | 3 | 0 | — | MINOR |
| `focus` | 6 | 0 | 0 | 0.0% | FLAG |
| `fund` | 22 | 10 | 3 | 13.6% | FLAG |
| `gleif` | 16 | 2 | 4 | 25.0% | FLAG |
| `governance` | 9 | 0 | 0 | 0.0% | FLAG |
| `graph` | 10 | 1 | 0 | 0.0% | FLAG |
| `holding` | 10 | 0 | 0 | 0.0% | FLAG |
| `identifier` | 11 | 0 | 0 | 0.0% | FLAG |
| `instruction-profile` | 7 | 0 | 0 | 0.0% | FLAG |
| `instrument-class` | 3 | 0 | 0 | 0.0% | FLAG |
| `investment-manager` | 7 | 0 | 0 | 0.0% | FLAG |
| `investor` | 20 | 4 | 1 | 5.0% | FLAG |
| `investor-role` | 10 | 0 | 0 | 0.0% | FLAG |
| `isda` | 6 | 0 | 0 | 0.0% | FLAG |
| `issuer-control-config` | 2 | 0 | 0 | 0.0% | FLAG |
| `kyc` | 1 | 29 | 34 | 100.0%+ | CLEAN |
| `kyc-agreement` | 4 | 0 | 0 | 0.0% | FLAG |
| `kyc-case` | 10 | 0 | 0 | 0.0% | FLAG |
| `legal-entity` | 3 | 0 | 0 | 0.0% | FLAG |
| `lifecycle` | 0 | 3 | 0 | — | MINOR |
| `maintenance` | 7 | 0 | 0 | 0.0% | FLAG |
| `manco` | 10 | 0 | 0 | 0.0% | FLAG |
| `matrix-overlay` | 9 | 0 | 0 | 0.0% | FLAG |
| `movement` | 14 | 0 | 0 | 0.0% | FLAG |
| `observation` | 8 | 1 | 0 | 0.0% | FLAG |
| `onboarding` | 1 | 0 | 0 | 0.0% | FLAG |
| `ownership` | 22 | 19 | 4 | 18.2% | FLAG |
| `pack` | 2 | 0 | 0 | 0.0% | FLAG |
| `partnership` | 7 | 0 | 0 | 0.0% | FLAG |
| `pipeline` | 1 | 0 | 0 | 0.0% | FLAG |
| `pricing-config` | 14 | 0 | 0 | 0.0% | FLAG |
| `product` | 2 | 16 | 2 | 100.0% | CLEAN |
| `provisioning` | 2 | 0 | 0 | 0.0% | FLAG |
| `readiness` | 2 | 0 | 0 | 0.0% | FLAG |
| `red-flag` | 8 | 0 | 0 | 0.0% | FLAG |
| `refdata` | 9 | 0 | 0 | 0.0% | FLAG |
| `reference` | 0 | 14 | 0 | — | MINOR |
| `registry` | 26 | 0 | 0 | 0.0% | FLAG |
| `regulatory.registration` | 4 | 0 | 0 | 0.0% | FLAG |
| `regulatory.status` | 1 | 0 | 0 | 0.0% | FLAG |
| `request` | 9 | 0 | 0 | 0.0% | FLAG |
| `requirement` | 10 | 0 | 1 | 10.0% | FLAG |
| `research` | 0 | 2 | 0 | — | MINOR |
| `research.companies-house` | 5 | 0 | 0 | 0.0% | FLAG |
| `research.generic` | 1 | 0 | 0 | 0.0% | FLAG |
| `research.import-run` | 3 | 0 | 0 | 0.0% | FLAG |
| `research.outreach` | 9 | 0 | 0 | 0.0% | FLAG |
| `research.sec-edgar` | 5 | 0 | 0 | 0.0% | FLAG |
| `research.sources` | 5 | 0 | 0 | 0.0% | FLAG |
| `research.workflow` | 10 | 0 | 0 | 0.0% | FLAG |
| `role` | 4 | 0 | 0 | 0.0% | FLAG |
| `rule` | 3 | 0 | 0 | 0.0% | FLAG |
| `rule-field` | 2 | 0 | 0 | 0.0% | FLAG |
| `ruleset` | 3 | 0 | 0 | 0.0% | FLAG |
| `schema` | 13 | 0 | 0 | 0.0% | FLAG |
| `schema-admin` | 0 | 1 | 0 | — | MINOR |
| `screening` | 8 | 7 | 4 | 50.0% | MINOR |
| `security-type` | 2 | 0 | 0 | 0.0% | FLAG |
| `sem-reg` | 0 | 30 | 83 | — | MINOR |
| `semantic` | 6 | 0 | 0 | 0.0% | FLAG |
| `service` | 3 | 0 | 0 | 0.0% | FLAG |
| `service-availability` | 3 | 0 | 0 | 0.0% | FLAG |
| `service-intent` | 3 | 0 | 0 | 0.0% | FLAG |
| `service-resource` | 26 | 0 | 0 | 0.0% | FLAG |
| `session` | 18 | 6 | 10 | 55.6% | MINOR |
| `settlement-chain` | 13 | 0 | 0 | 0.0% | FLAG |
| `share-class` | 10 | 0 | 0 | 0.0% | FLAG |
| `skeleton` | 1 | 0 | 0 | 0.0% | FLAG |
| `sla` | 13 | 0 | 0 | 0.0% | FLAG |
| `state` | 8 | 1 | 8 | 100.0% | CLEAN |
| `stewardship` | 0 | 9 | 0 | — | MINOR |
| `subcustodian` | 3 | 0 | 0 | 0.0% | FLAG |
| `tax-config` | 11 | 0 | 0 | 0.0% | FLAG |
| `team` | 15 | 7 | 3 | 20.0% | FLAG |
| `template` | 2 | 0 | 0 | 0.0% | FLAG |
| `temporal` | 8 | 0 | 0 | 0.0% | FLAG |
| `tollgate` | 10 | 0 | 0 | 0.0% | FLAG |
| `trade-gateway` | 12 | 0 | 0 | 0.0% | FLAG |
| `trading-profile` | 32 | 4 | 13 | 40.6% | MINOR |
| `trust` | 8 | 0 | 0 | 0.0% | FLAG |
| `ubo` | 20 | 0 | 0 | 0.0% | FLAG |
| `ubo.registry` | 6 | 0 | 0 | 0.0% | FLAG |
| `user` | 7 | 0 | 0 | 0.0% | FLAG |
| `verify` | 16 | 0 | 0 | 0.0% | FLAG |
| `view` | 14 | 3 | 0 | 0.0% | FLAG |
| `workflow` | 0 | 3 | 0 | — | MINOR |

**Summary:** 7 CLEAN, 18 MINOR, 106 FLAG across 131 distinct domains.

---

## 13. Metadata-Layer Recommendations

6. **Resolve the 55 phantom table references.** Every table referenced in `verb_data_footprint` should be declared in the corresponding domain's `tables:` section with governance tier, classification, and PII flags. The `entities` and `cbus` tables are the highest priority — they are the two most referenced tables in the system yet have no governance metadata. This requires either adding them to an existing domain (likely `entity` and `cbu` respectively) or creating a shared `core` metadata domain.

7. **Backfill footprint mappings for the top FLAG domains.** The 106 FLAG domains represent ~970 verbs with zero data footprint. Prioritize by verb count: `capital` (30), `deal` (42 — partially covered), `cbu` (37 — partially covered), `service-resource` (26), `registry` (26), `fund` (22), `ownership` (22), `document` (21), `investor` (20), `ubo` (20). These 10 domains account for ~266 verbs. Adding `reads`/`writes` entries for these would raise overall footprint coverage from 23% to ~44%.

8. **Reconcile the domain namespace mismatch.** The 11 metadata-only domains and 92 verb-YAML-only domains indicate a naming divergence between `domain_metadata.yaml` and `config/verbs/*.yaml`. The most impactful mismatch is `sem-reg` (metadata) vs `registry`/`changeset`/`governance`/`audit`/`focus` (verb YAML) — 83 footprint entries exist but cannot be joined to any verb YAML domain. Either rename the metadata domain keys to match verb YAML, or add alias resolution in the adapter.

9. **Thread `verb_data_footprint` into the scanner.** The scanner (`sem_os_obpoc_adapter/src/scanner.rs`) does not currently consume `domain_metadata.yaml` footprint data when building `VerbContractBody`. Adding a post-scan enrichment step that populates `writes_to` and `reads_from` from the YAML footprint would make the 292 already-mapped verbs queryable via the registry (addressing the zero-count finding in Section 4.2).
