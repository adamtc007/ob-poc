# TODO: Domain Metadata → 100% Table Coverage

**Goal:** Every business table in `ob-poc` has an entry in `domain_metadata.yaml` with description, governance_tier, classification, and pii flag. Current: 203/304 (68%). Target: 304/304.  
**Date:** 2026-03-08  
**File:** `rust/config/sem_os_seeds/domain_metadata.yaml`

---

## What each entry needs

```yaml
  table_name:
    description: "One-line purpose"
    governance_tier: governed | operational    # governed = CDE, operational = non-CDE
    classification: internal | confidential | restricted
    pii: true | false
```

---

## Batch 1 — Compliance-critical (KYC + verification) — 14 tables

These are load-bearing for the KYC aggregate and currently invisible to SemOS.

**Domain: `kyc`** (add to existing domain, which has 19 tables)

```yaml
    case_evaluation_snapshots:
      description: "Point-in-time KYC case evaluation snapshots for audit trail"
      governance_tier: governed
      classification: confidential
      pii: false
    case_types:
      description: "Reference data: KYC case type definitions"
      governance_tier: governed
      classification: internal
      pii: false
    kyc_decisions:
      description: "Recorded KYC decisions with rationale and approver"
      governance_tier: governed
      classification: confidential
      pii: false
    kyc_service_agreements:
      description: "KYC service agreements linking cases to service commitments"
      governance_tier: governed
      classification: confidential
      pii: false
    kyc_ubo_evidence:
      description: "Case-workflow evidence model tying evidence to screenings, relationships, determination runs"
      governance_tier: governed
      classification: confidential
      pii: true
    kyc_ubo_registry:
      description: "Case-scoped UBO assertions with verification and screening status"
      governance_tier: governed
      classification: confidential
      pii: true
    outreach_requests:
      description: "Client outreach requests for missing KYC proofs"
      governance_tier: operational
      classification: internal
      pii: false
    threshold_factors:
      description: "UBO threshold factor definitions for ownership percentage computation"
      governance_tier: governed
      classification: internal
      pii: false
    threshold_requirements:
      description: "Jurisdiction/entity-type threshold requirements for UBO determination"
      governance_tier: governed
      classification: internal
      pii: false
    trust_parties:
      description: "Trust party roles (settlor, protector, beneficiary) for trust structures"
      governance_tier: governed
      classification: confidential
      pii: true
    ubo_assertion_log:
      description: "Audit log of UBO assertion changes with before/after snapshots"
      governance_tier: governed
      classification: confidential
      pii: true
    ubo_snapshot_comparisons:
      description: "Diff records between consecutive UBO determination snapshots"
      governance_tier: governed
      classification: confidential
      pii: false
    verification_challenges:
      description: "Identity verification challenges issued to entities"
      governance_tier: governed
      classification: confidential
      pii: true
    verification_escalations:
      description: "Escalated verification cases requiring manual review"
      governance_tier: governed
      classification: confidential
      pii: true
```

- [ ] 14 tables added to `kyc` domain
- [ ] `cargo test` passes

---

## Batch 2 — Evidence pipeline (attribute system) — 3 tables

Powers document extraction → typed attributes → evidence. Underpins the entire proof chain.

**Domain: `attribute`** (NEW domain — or merge into `document`)

```yaml
  attribute:
    description: "Governed attribute dictionary and typed value storage for evidence extraction"
    tables:
      attribute_registry:
        description: "Master dictionary of governed attributes (names, types, domains, validation rules)"
        governance_tier: governed
        classification: internal
        pii: false
      attribute_values_typed:
        description: "Typed attribute value storage with entity/CBU scoping"
        governance_tier: governed
        classification: confidential
        pii: true
      attribute_observations:
        description: "Observed/derived attribute facts with provenance and confidence scores"
        governance_tier: operational
        classification: confidential
        pii: true
```

- [ ] 3 tables added as new `attribute` domain (or extend `document`)

---

## Batch 3 — DSL / staged execution / REPL — 14 tables

The DSL's own operational tables. Critical for the execution pipeline.

**Domain: `dsl`** (extend existing, which has 7 tables)

```yaml
    dsl_ob:
      description: "DSL operation buffer — in-flight command state"
      governance_tier: operational
      classification: internal
      pii: false
    dsl_session_locks:
      description: "Advisory locks for DSL session exclusivity"
      governance_tier: operational
      classification: internal
      pii: false
    dsl_snapshots:
      description: "DSL session state snapshots for replay and recovery"
      governance_tier: operational
      classification: internal
      pii: false
    dsl_verb_categories:
      description: "Verb categorization for UI grouping and discovery"
      governance_tier: operational
      classification: internal
      pii: false
    dsl_verb_sync_log:
      description: "Audit trail of verb registry sync operations"
      governance_tier: operational
      classification: internal
      pii: false
    dsl_view_state_changes:
      description: "View state transition log for UI/session debugging"
      governance_tier: operational
      classification: internal
      pii: false
    dsl_workflow_phases:
      description: "Workflow phase definitions for multi-step DSL operations"
      governance_tier: operational
      classification: internal
      pii: false
    staged_command:
      description: "In-construction staged command with DAG analysis and confirmation gate"
      governance_tier: operational
      classification: internal
      pii: false
    staged_command_candidate:
      description: "Candidate verb matches for staged command resolution"
      governance_tier: operational
      classification: internal
      pii: false
    staged_command_entity:
      description: "Resolved entities bound to staged command arguments"
      governance_tier: operational
      classification: internal
      pii: false
    staged_runbook:
      description: "In-construction runbook envelope before compilation"
      governance_tier: operational
      classification: internal
      pii: false
    repl_invocation_records:
      description: "REPL command invocation history for replay and debugging"
      governance_tier: operational
      classification: internal
      pii: false
    semantic_match_cache:
      description: "Cached semantic similarity results for verb resolution"
      governance_tier: operational
      classification: internal
      pii: false
    verb_centroids:
      description: "Centroid embeddings per verb for fast similarity lookups"
      governance_tier: operational
      classification: internal
      pii: false
```

- [ ] 14 tables added to `dsl` domain

---

## Batch 4 — Product / provisioning / onboarding — 10 tables

The operational delivery pipeline. Connects commercial products to provisioned resources.

**Domain: `product`** (extend existing, which has 6 tables)

```yaml
    onboarding_plans:
      description: "Onboarding plan definitions linking deals to delivery milestones"
      governance_tier: governed
      classification: internal
      pii: false
    onboarding_requests:
      description: "Legacy onboarding request records (see also deal_onboarding_requests)"
      governance_tier: operational
      classification: internal
      pii: false
    provisioning_requests:
      description: "Resource provisioning request lifecycle tracking"
      governance_tier: operational
      classification: internal
      pii: false
    provisioning_events:
      description: "Provisioning event audit trail (attempts, successes, failures)"
      governance_tier: operational
      classification: internal
      pii: false
    service_intents:
      description: "Service delivery intent records linking onboarding requests to required services"
      governance_tier: operational
      classification: internal
      pii: false
    resource_attribute_requirements:
      description: "Attribute requirements per resource type for provisioning validation"
      governance_tier: governed
      classification: internal
      pii: false
    resource_dependencies:
      description: "Inter-resource dependency graph for provisioning ordering"
      governance_tier: governed
      classification: internal
      pii: false
    resource_instance_attributes:
      description: "Attribute values on provisioned resource instances"
      governance_tier: operational
      classification: internal
      pii: false
    resource_instance_dependencies:
      description: "Materialized dependency links between resource instances"
      governance_tier: operational
      classification: internal
      pii: false
    requirement_acceptable_docs:
      description: "Mapping of onboarding requirements to acceptable document types"
      governance_tier: governed
      classification: internal
      pii: false
```

- [ ] 10 tables added to `product` domain

---

## Batch 5 — Ownership / dilution / reconciliation — 9 tables

UBO computation support tables.

**Domain: `ownership`** (extend existing, which has 10 tables)

```yaml
    ownership_reconciliation_runs:
      description: "Ownership reconciliation run records with scope and results"
      governance_tier: governed
      classification: confidential
      pii: false
    ownership_reconciliation_findings:
      description: "Discrepancies found during ownership reconciliation"
      governance_tier: governed
      classification: confidential
      pii: false
    dilution_instruments:
      description: "Dilution instrument definitions (options, warrants, convertibles)"
      governance_tier: governed
      classification: confidential
      pii: false
    dilution_exercise_events:
      description: "Dilution instrument exercise events affecting ownership percentages"
      governance_tier: governed
      classification: confidential
      pii: false
    holding_control_links:
      description: "Links between holdings and control edges for ownership chain traversal"
      governance_tier: governed
      classification: confidential
      pii: false
    issuance_events:
      description: "Share/unit issuance events affecting ownership structure"
      governance_tier: governed
      classification: confidential
      pii: false
    issuer_control_config:
      description: "Issuer-level control configuration for ownership computation"
      governance_tier: governed
      classification: confidential
      pii: false
    proofs:
      description: "Ownership proof records linking assertions to evidence"
      governance_tier: governed
      classification: confidential
      pii: false
    special_rights:
      description: "Special voting/veto/appointment rights affecting control determination"
      governance_tier: governed
      classification: confidential
      pii: false
```

- [ ] 9 tables added to `ownership` domain

---

## Batch 6 — Entity extensions — 5 tables

**Domain: `entity`** (extend existing, which has 18 tables)

```yaml
    entity_regulatory_registrations:
      description: "Regulatory registration records per entity (regulator, status, reference)"
      governance_tier: governed
      classification: confidential
      pii: false
    entity_relationships_history:
      description: "Historical entity relationship snapshots for audit trail"
      governance_tier: governed
      classification: confidential
      pii: false
    entity_type_dependencies:
      description: "Inter-entity-type dependency rules for structural validation"
      governance_tier: governed
      classification: internal
      pii: false
    entity_ubos:
      description: "Denormalized entity→UBO lookup table for fast resolution"
      governance_tier: governed
      classification: confidential
      pii: true
    delegation_relationships:
      description: "Delegation of authority relationships between entities"
      governance_tier: governed
      classification: confidential
      pii: false
```

- [ ] 5 tables added to `entity` domain

---

## Batch 7 — CBU extensions — 5 tables

**Domain: `cbu`** (extend existing, which has 13 tables)

```yaml
    cbu_attr_values:
      description: "CBU-scoped attribute values for onboarding configuration"
      governance_tier: operational
      classification: internal
      pii: false
    cbu_layout_overrides:
      description: "Per-CBU UI layout overrides for visualization"
      governance_tier: operational
      classification: internal
      pii: false
    cbu_relationship_verification:
      description: "CBU relationship verification status and evidence tracking"
      governance_tier: governed
      classification: confidential
      pii: false
    cbu_ssi_agent_override:
      description: "Agent-applied SSI overrides per CBU"
      governance_tier: operational
      classification: internal
      pii: false
    cbu_unified_attr_requirements:
      description: "Unified attribute requirement matrix per CBU (product × jurisdiction)"
      governance_tier: governed
      classification: internal
      pii: false
```

- [ ] 5 tables added to `cbu` domain

---

## Batch 8 — SLA — 4 tables

**Domain: `billing`** (extend existing — SLA lives alongside billing, which has 4 tables) OR create new `sla` domain.

```yaml
    sla_templates:
      description: "SLA template definitions with metric types and thresholds"
      governance_tier: governed
      classification: internal
      pii: false
    sla_metric_types:
      description: "SLA metric type reference data (response time, availability, etc.)"
      governance_tier: governed
      classification: internal
      pii: false
    sla_measurements:
      description: "SLA measurement records — actual vs committed values"
      governance_tier: operational
      classification: internal
      pii: false
    sla_breaches:
      description: "SLA breach records with severity and remediation tracking"
      governance_tier: operational
      classification: internal
      pii: false
```

- [ ] 4 tables added to `billing` domain

---

## Batch 9 — Custody extensions — 5 tables

**Domain: `custody`** (extend existing, which has 27 tables)

```yaml
    ca_event_types:
      description: "Corporate action event type reference data"
      governance_tier: governed
      classification: internal
      pii: false
    isda_product_coverage:
      description: "ISDA product coverage mapping per agreement"
      governance_tier: governed
      classification: confidential
      pii: false
    isda_product_taxonomy:
      description: "ISDA product classification taxonomy"
      governance_tier: governed
      classification: internal
      pii: false
    settlement_types:
      description: "Settlement type reference data (DVP, FOP, etc.)"
      governance_tier: governed
      classification: internal
      pii: false
    ssi_types:
      description: "SSI type reference data"
      governance_tier: governed
      classification: internal
      pii: false
```

- [ ] 5 tables added to `custody` domain

---

## Batch 10 — Remaining tables — 27 tables across multiple domains

**Domain: `client-portal`** (extend, has 6) — add 3:
- `client_allegations`, `client_portal_sessions`, `client_types`

**Domain: `view`** (NEW domain) — add 3:
- `layout_cache`, `layout_config`, `view_modes`

**Domain: `lifecycle`** (NEW domain) — add 3:
- `lifecycles`, `lifecycle_resource_capabilities`, `lifecycle_resource_types`

**Domain: `workflow`** (NEW domain) — add 3:
- `workflow_definitions`, `workflow_instances`, `workflow_audit_log`

**Domain: `screening`** (extend, has 4) — add 3:
- `detected_patterns`, `risk_bands`, `risk_ratings`

**Domain: `bods`** (NEW domain) — add 2:
- `bods_entity_types`, `bods_interest_types`

**Domain: `fund`** (extend, has 8) — add 2:
- `share_class_supply`, `instrument_lifecycles`

**Domain: `observation`** (NEW domain) — add 1:
- `observation_discrepancies`

**Domain: `research`** (NEW domain) — add 2:
- `research_anomalies`, `research_corrections`

**Domain: `graph`** (NEW domain) — add 1:
- `graph_import_runs`

**Domain: `reference`** (extend, has 10) — add 5:
- `crud_operations`, `dictionary`, `edge_types`, `node_types`, `srdef_discovery_reasons`

**Domain: `document`** (extend, has 4) — add 1:
- `sheet_execution_audit`

**Domain: `agent`** (extend, has 10) — add 1:
- `intent_feedback_analysis`

**Domain: `session`** (extend, has 4) — add 1:
- `log` (rename to `event_log` after schema consolidation)

**Domain: `schema-admin`** (NEW domain — or reference) — add 1:
- `schema_consolidation_table_map`

- [ ] 27 tables distributed across domains
- [ ] All new domains have description field

---

## Verification checklist

- [ ] Run count: `SELECT COUNT(DISTINCT table_name) FROM information_schema.tables WHERE table_schema = 'ob-poc'` — must equal number of entries in domain_metadata tables sections
- [ ] No table appears in more than one domain
- [ ] Every domain has a `description` field
- [ ] Every table has all four fields: `description`, `governance_tier`, `classification`, `pii`
- [ ] `cargo test` passes
- [ ] SemOS seed loader reports 0 unresolved table references

---

## Summary

| Batch | Domain(s) | Tables | Priority |
|-------|-----------|--------|----------|
| 1 | kyc | 14 | P0 — compliance |
| 2 | attribute (new) | 3 | P0 — evidence pipeline |
| 3 | dsl | 14 | P1 — execution pipeline |
| 4 | product | 10 | P1 — delivery pipeline |
| 5 | ownership | 9 | P1 — UBO computation |
| 6 | entity | 5 | P1 — entity model |
| 7 | cbu | 5 | P2 — CBU extensions |
| 8 | billing (SLA) | 4 | P2 — SLA |
| 9 | custody | 5 | P2 — custody extensions |
| 10 | mixed (11 domains) | 27 | P2 — tail coverage |
| **Total** | | **101** | |
