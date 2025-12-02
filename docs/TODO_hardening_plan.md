# DSL & Database Hardening Plan

**Created**: 2025-12-02
**Status**: PLAN - Ready for implementation

---

## Summary

Five workstreams to harden the DSL pipeline and database schema:

| # | Workstream | Effort | Priority |
|---|------------|--------|----------|
| 1 | Harden statuses | Low | HIGH |
| 2 | Encode KYC state transitions | Medium | HIGH |
| 3 | Split verb config + versioning | Medium | MEDIUM |
| 4 | Current view over observations | Low | LOW (mostly done) |
| 5 | Standardise event emission | Medium | HIGH |

---

## 1. Harden Statuses

### Current State
- **Good**: Most tables already have CHECK constraints (kyc.cases, kyc.doc_requests, kyc.screenings, ob-poc.attribute_observations, etc.)
- **Gap**: Some tables missing constraints or using inconsistent values

### Tables Needing CHECK Constraints

| Schema | Table | Column | Action |
|--------|-------|--------|--------|
| custody | cbu_ssi | status | Add CHECK (PENDING, ACTIVE, SUSPENDED, EXPIRED) |
| custody | entity_ssi | status | Add CHECK (same as cbu_ssi) |
| kyc | holdings | status | Add CHECK (active, closed) |
| kyc | share_classes | status | Add CHECK (active, closed, suspended) |
| ob-poc | document_catalog | status | Add CHECK (active, archived, deleted) |
| ob-poc | dsl_execution_log | status | Add CHECK (success, failed, partial) |
| ob-poc | dsl_instances | status | Add CHECK (draft, active, deprecated) |

### Implementation Steps
1. Create migration SQL file with ALTER TABLE ADD CONSTRAINT statements
2. Validate existing data does not violate new constraints
3. Run migration
4. Update CLAUDE.md with constraint documentation

### Files to Change
- sql/migrations/YYYYMMDD_harden_statuses.sql (new)
- CLAUDE.md (document constraints)

---

## 2. Encode KYC State Transitions

### Current State
- Status values defined in CHECK constraints
- **No machine-readable transition rules**
- CSG linter validates args and entity types, but not state transitions

### Proposed Solution

Add state_machines section to csg_rules.yaml:

```yaml
state_machines:
  kyc_case:
    entity: kyc.cases
    column: status
    initial: INTAKE
    terminal: [APPROVED, REJECTED, WITHDRAWN, EXPIRED]
    transitions:
      INTAKE: [DISCOVERY, WITHDRAWN]
      DISCOVERY: [ASSESSMENT, BLOCKED, WITHDRAWN]
      ASSESSMENT: [REVIEW, BLOCKED, WITHDRAWN]
      REVIEW: [APPROVED, REJECTED, BLOCKED]
      BLOCKED: [DISCOVERY, ASSESSMENT, REVIEW, WITHDRAWN]

  workstream:
    entity: kyc.entity_workstreams
    column: status
    initial: PENDING
    terminal: [COMPLETE]
    transitions:
      PENDING: [COLLECT, BLOCKED]
      COLLECT: [VERIFY, BLOCKED]
      VERIFY: [SCREEN, BLOCKED]
      SCREEN: [ASSESS, BLOCKED]
      ASSESS: [COMPLETE, ENHANCED_DD, BLOCKED]
      ENHANCED_DD: [ASSESS, BLOCKED]
      BLOCKED: [PENDING, COLLECT, VERIFY, SCREEN, ASSESS]

  resource_instance:
    entity: ob-poc.cbu_resource_instances
    column: status
    initial: PENDING
    terminal: [DECOMMISSIONED]
    transitions:
      PENDING: [PROVISIONING, DECOMMISSIONED]
      PROVISIONING: [ACTIVE, DECOMMISSIONED]
      ACTIVE: [SUSPENDED, DECOMMISSIONED]
      SUSPENDED: [ACTIVE, DECOMMISSIONED]
```

### Implementation Steps
1. Add state_machines schema to csg_rules.yaml
2. Add StateTransitionValidator to CSG linter
3. Wire linter to check set-status verbs against allowed transitions
4. Add tests for transition validation

### Files to Change
- rust/config/csg_rules.yaml - add state_machines section
- rust/src/dsl_v2/csg_linter.rs - add transition validation
- rust/src/dsl_v2/config/types.rs - add StateMachine serde types

---

## 3. Split Verb Config + DSL Schema Versioning

### Current State
- Single verbs.yaml file: **5,259 lines, 28 domains**
- No DSL schema versioning
- Hard to navigate and maintain

### Proposed Structure

```
rust/config/
├── verbs/
│   ├── _meta.yaml           # version, common definitions
│   ├── cbu.yaml             # CBU domain
│   ├── entity.yaml          # Entity domain
│   ├── document.yaml        # Document domain
│   ├── kyc/
│   │   ├── case.yaml        # kyc-case domain
│   │   ├── workstream.yaml  # entity-workstream domain
│   │   └── screening.yaml   # screening domain
│   ├── custody/
│   │   ├── universe.yaml    # cbu-custody domain
│   │   └── ssi.yaml
│   └── observation/
│       ├── allegation.yaml
│       ├── observation.yaml
│       └── discrepancy.yaml
└── csg_rules.yaml           # unchanged
```

### DSL Schema Versioning

Add to dsl_instances table:
```sql
ALTER TABLE "ob-poc".dsl_instances 
ADD COLUMN dsl_schema_version VARCHAR(20) DEFAULT '1.0';
```

### Implementation Steps
1. Create directory structure
2. Split verbs.yaml into domain files (scripted)
3. Update ConfigLoader to merge multiple files
4. Add schema version to _meta.yaml
5. Add dsl_schema_version column to dsl_instances
6. Update executor to stamp version on new instances

### Files to Change
- rust/config/verbs/*.yaml (new, split from verbs.yaml)
- rust/src/dsl_v2/config/loader.rs - multi-file loading
- sql/migrations/YYYYMMDD_dsl_schema_version.sql (new)

---

## 4. Current View Over Observations

### Current State
**Already implemented**: v_attribute_current view exists

```sql
SELECT DISTINCT ON (entity_id, attribute_id) ...
FROM "ob-poc".attribute_observations
WHERE status = 'ACTIVE'
ORDER BY entity_id, attribute_id, 
         is_authoritative DESC, 
         confidence DESC, 
         observed_at DESC;
```

### Gaps
- View does not include CBU context (some use cases need per-CBU values)
- No view joining allegations to their verifying observations

### Proposed Addition

**v_entity_attribute_summary** - Current values with allegation status:
```sql
CREATE VIEW "ob-poc".v_entity_attribute_summary AS
SELECT 
    e.entity_id,
    e.name as entity_name,
    ar.name as attribute_name,
    vac.value_text, vac.value_number, vac.value_date,
    vac.source_type,
    vac.confidence,
    ca.verification_status as allegation_status
FROM "ob-poc".entities e
CROSS JOIN "ob-poc".attribute_registry ar
LEFT JOIN "ob-poc".v_attribute_current vac 
    ON vac.entity_id = e.entity_id AND vac.attribute_id = ar.uuid
LEFT JOIN "ob-poc".client_allegations ca
    ON ca.entity_id = e.entity_id AND ca.attribute_id = ar.uuid;
```

### Implementation Steps
1. Create v_entity_attribute_summary view
2. Add DSL verb observation.get-summary to query it

### Files to Change
- sql/migrations/YYYYMMDD_observation_views.sql (new)
- rust/config/verbs.yaml - add get-summary verb

---

## 5. Standardise Event Emission

### Current State
- case-event.log verb exists for manual logging
- **No automatic event emission** from mutating verbs
- Event emission is opt-in, not automatic

### Proposed Solution

Add emits_event configuration to verb definitions:

```yaml
kyc-case:
  verbs:
    set-status:
      description: "Update case status"
      behavior: crud
      crud:
        operation: update
        table: cases
        schema: kyc
      emits_event:
        table: case_events
        schema: kyc
        event_type: "CASE_STATUS_CHANGED"
        include_args: [case-id, status]
        capture_old_value: status
```

### Executor Changes

In GenericCrudExecutor, after successful CRUD operation:
1. Check if verb has emits_event config
2. If yes, insert row into event table
3. Include: case_id, event_type, event_data (old + new values), occurred_at

### Implementation Steps
1. Add emits_event schema to verb config types
2. Update GenericCrudExecutor to check for and emit events
3. Add emits_event config to key verbs:
   - kyc-case.set-status
   - entity-workstream.set-status
   - allegation.verify, allegation.contradict
   - discrepancy.resolve
   - red-flag.raise, red-flag.resolve
4. Add tests for event emission

### Files to Change
- rust/src/dsl_v2/config/types.rs - add EmitsEvent struct
- rust/src/dsl_v2/generic_executor.rs - emit events after CRUD
- rust/config/verbs.yaml - add emits_event to key verbs

---

## Implementation Order

Recommended sequence:

1. **Harden statuses** (1-2 hours)
   - Quick win, database-only, no Rust changes

2. **Event emission** (4-6 hours)
   - High value for audit trail
   - Needed before KYC transitions (transitions should emit events)

3. **KYC state transitions** (4-6 hours)
   - Depends on event emission
   - Critical for KYC workflow integrity

4. **Split verb config** (2-4 hours)
   - Maintenance improvement
   - Can be done independently

5. **Observation views** (1-2 hours)
   - Low priority, mostly done
   - Nice to have

---

## Total Estimated Effort

| Workstream | Effort |
|------------|--------|
| Harden statuses | 1-2 hours |
| Event emission | 4-6 hours |
| KYC transitions | 4-6 hours |
| Split verb config | 2-4 hours |
| Observation views | 1-2 hours |
| **Total** | **12-20 hours** |
