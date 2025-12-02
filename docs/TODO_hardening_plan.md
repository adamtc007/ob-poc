# DSL & Database Hardening Plan

**Created**: 2025-12-02  
**Updated**: 2025-12-02  
**Status**: PLAN - Ready for implementation

---

## Summary

Seven workstreams to harden the DSL pipeline and database schema:

| # | Workstream | Effort | Priority |
|---|------------|--------|----------|
| 1 | Harden statuses | Low | HIGH |
| 2 | DSL Idempotency | High | CRITICAL |
| 3 | Snapshot-based soft delete | Medium | HIGH |
| 4 | KYC State Snapshots | Medium | HIGH |
| 5 | Encode KYC state transitions | Medium | HIGH |
| 6 | Split verb config + versioning | Medium | MEDIUM |
| 7 | ~~Event emission~~ | - | PARKED (prod only) |

---

## 1. Harden Statuses

### Current State
- **Good**: Most tables already have CHECK constraints
- **Gap**: Some tables missing constraints

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

### Files to Change
- sql/migrations/YYYYMMDD_harden_statuses.sql (new)

---

## 2. DSL Idempotency (CRITICAL)

### Problem
DSL programs must be re-runnable without side effects. Currently:
- `cbu.ensure` is idempotent (upsert by name)
- Most other verbs are NOT idempotent (insert creates duplicates)
- If execution fails mid-program, re-run creates partial duplicates

### Solution: Idempotency Keys

Every verb call gets an **idempotency key** derived from:
1. DSL instance ID (or execution context ID)
2. Statement index in program
3. Hash of verb + args

```
idempotency_key = hash(dsl_instance_id + statement_index + verb + canonical_args)
```

### Implementation

**Option A: Idempotency table (recommended)**
```sql
CREATE TABLE "ob-poc".dsl_idempotency (
    idempotency_key VARCHAR(64) PRIMARY KEY,
    execution_id UUID NOT NULL,
    statement_index INTEGER NOT NULL,
    verb VARCHAR(100) NOT NULL,
    result_id UUID,  -- returned ID from original execution
    created_at TIMESTAMPTZ DEFAULT now()
);
```

Executor flow:
1. Compute idempotency_key for statement
2. Check if key exists in dsl_idempotency
3. If exists: return cached result_id (skip execution)
4. If not: execute, store result_id with key

**Option B: Natural keys on tables**
- Add unique constraints on natural business keys
- Use upsert (ON CONFLICT DO NOTHING/UPDATE)
- Problem: not all tables have natural keys

### Verb Classification

| Verb Pattern | Idempotent? | Strategy |
|--------------|-------------|----------|
| *.ensure | Yes | Upsert by natural key |
| *.create | No → Yes | Idempotency table |
| *.update | No → Yes | Idempotency table |
| *.set-* | No → Yes | Idempotency table |
| *.delete | Yes | Already idempotent |
| *.read/* .list/* | Yes | Read-only |

### Files to Change
- sql/migrations/YYYYMMDD_idempotency.sql (new table)
- rust/src/dsl_v2/executor.rs - idempotency check before execute
- rust/src/dsl_v2/generic_executor.rs - record idempotency after execute

---

## 3. Snapshot-Based Soft Delete

### Principle
**Never update, never delete. Insert new snapshots.**

This is already the pattern in `attribute_observations`:
- New observation supersedes old one
- Old row gets `superseded_by` FK and `superseded_at` timestamp
- `status = 'ACTIVE'` vs `status = 'SUPERSEDED'`

### Apply to Other Tables

Tables that should use snapshot pattern:

| Table | Current | Change |
|-------|---------|--------|
| kyc.cases | status column | Add case_snapshots table |
| kyc.entity_workstreams | status column | Add workstream_snapshots table |
| ob-poc.entity_kyc_status | single row per entity/cbu | Already snapshot-like, add version |
| ob-poc.cbu_resource_instances | status column | Add instance_snapshots table |

### Snapshot Table Pattern

```sql
CREATE TABLE kyc.case_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES kyc.cases(case_id),
    
    -- Snapshot of state at this moment
    status VARCHAR(30) NOT NULL,
    escalation_level VARCHAR(30) NOT NULL,
    risk_rating VARCHAR(20),
    assigned_analyst_id UUID,
    assigned_reviewer_id UUID,
    
    -- Snapshot metadata
    snapshot_reason VARCHAR(100),  -- 'STATUS_CHANGE', 'ESCALATION', 'ASSIGNMENT'
    triggered_by_verb VARCHAR(100),  -- 'kyc-case.set-status'
    
    -- Versioning
    version INTEGER NOT NULL,
    is_current BOOLEAN DEFAULT true,
    
    created_at TIMESTAMPTZ DEFAULT now(),
    created_by TEXT
);

-- Only one current snapshot per case
CREATE UNIQUE INDEX idx_case_current_snapshot 
ON kyc.case_snapshots(case_id) WHERE is_current = true;
```

### Current View

```sql
CREATE VIEW kyc.v_case_current AS
SELECT c.case_id, c.cbu_id, c.opened_at, c.closed_at, c.case_type,
       s.status, s.escalation_level, s.risk_rating,
       s.assigned_analyst_id, s.assigned_reviewer_id,
       s.version, s.created_at as state_changed_at
FROM kyc.cases c
JOIN kyc.case_snapshots s ON s.case_id = c.case_id AND s.is_current = true;
```

### On State Change

When `kyc-case.set-status` is called:
1. Mark current snapshot as `is_current = false`
2. Insert new snapshot with `is_current = true`, `version = old_version + 1`
3. New snapshot captures full state, not just changed field

### Files to Change
- sql/migrations/YYYYMMDD_snapshot_tables.sql (new)
- rust/config/verbs.yaml - modify set-status verbs to use snapshot pattern
- rust/src/dsl_v2/custom_ops/mod.rs - add snapshot operation handlers

---

## 4. KYC State Snapshots on Transition

### Pattern: Snapshot Function

Instead of updating status directly, call a function that:
1. Validates transition is allowed
2. Creates snapshot with old state
3. Updates to new state
4. Returns new snapshot_id

```sql
CREATE FUNCTION kyc.transition_case_status(
    p_case_id UUID,
    p_new_status VARCHAR(30),
    p_reason TEXT DEFAULT NULL,
    p_actor TEXT DEFAULT 'SYSTEM'
) RETURNS UUID AS $$
DECLARE
    v_current_status VARCHAR(30);
    v_current_version INTEGER;
    v_snapshot_id UUID;
BEGIN
    -- Get current state
    SELECT status, version INTO v_current_status, v_current_version
    FROM kyc.case_snapshots
    WHERE case_id = p_case_id AND is_current = true;
    
    -- Validate transition (could query csg_rules or hardcode)
    IF NOT kyc.is_valid_transition('case', v_current_status, p_new_status) THEN
        RAISE EXCEPTION 'Invalid transition: % -> %', v_current_status, p_new_status;
    END IF;
    
    -- Mark old snapshot as not current
    UPDATE kyc.case_snapshots
    SET is_current = false
    WHERE case_id = p_case_id AND is_current = true;
    
    -- Insert new snapshot
    INSERT INTO kyc.case_snapshots (
        case_id, status, escalation_level, risk_rating,
        assigned_analyst_id, assigned_reviewer_id,
        snapshot_reason, version, is_current, created_by
    )
    SELECT 
        case_id, p_new_status, escalation_level, risk_rating,
        assigned_analyst_id, assigned_reviewer_id,
        COALESCE(p_reason, 'STATUS_CHANGE'),
        v_current_version + 1, true, p_actor
    FROM kyc.case_snapshots
    WHERE case_id = p_case_id AND version = v_current_version
    RETURNING snapshot_id INTO v_snapshot_id;
    
    RETURN v_snapshot_id;
END;
$$ LANGUAGE plpgsql;
```

### Transition Validation Function

```sql
CREATE FUNCTION kyc.is_valid_transition(
    p_machine VARCHAR(50),
    p_from_status VARCHAR(30),
    p_to_status VARCHAR(30)
) RETURNS BOOLEAN AS $$
BEGIN
    -- Could query a transitions table or use CASE statement
    CASE p_machine
        WHEN 'case' THEN
            RETURN CASE p_from_status
                WHEN 'INTAKE' THEN p_to_status IN ('DISCOVERY', 'WITHDRAWN')
                WHEN 'DISCOVERY' THEN p_to_status IN ('ASSESSMENT', 'BLOCKED', 'WITHDRAWN')
                WHEN 'ASSESSMENT' THEN p_to_status IN ('REVIEW', 'BLOCKED', 'WITHDRAWN')
                WHEN 'REVIEW' THEN p_to_status IN ('APPROVED', 'REJECTED', 'BLOCKED')
                WHEN 'BLOCKED' THEN p_to_status IN ('DISCOVERY', 'ASSESSMENT', 'REVIEW', 'WITHDRAWN')
                ELSE false
            END;
        WHEN 'workstream' THEN
            -- Similar for workstream
            RETURN true;  -- Placeholder
        ELSE
            RETURN false;
    END CASE;
END;
$$ LANGUAGE plpgsql;
```

### Wire to DSL Verbs

`kyc-case.set-status` verb becomes:
```yaml
set-status:
  description: "Transition case to new status (creates snapshot)"
  behavior: plugin
  handler: kyc_case_transition
  args:
    - name: case-id
      type: uuid
      required: true
    - name: status
      type: string
      required: true
    - name: reason
      type: string
      required: false
  returns:
    type: uuid
    name: snapshot_id
```

Custom op calls `kyc.transition_case_status()` function.

### Files to Change
- sql/migrations/YYYYMMDD_snapshot_functions.sql (new)
- rust/config/verbs.yaml - change set-status to plugin
- rust/src/dsl_v2/custom_ops/mod.rs - add KycCaseTransitionOp

---

## 5. Encode State Transitions in csg_rules.yaml

### Purpose
Machine-readable transitions for:
1. Documentation
2. Linter validation (catch invalid transitions at lint time)
3. UI state machine visualization

### Schema

```yaml
state_machines:
  kyc_case:
    table: kyc.cases
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
    table: kyc.entity_workstreams
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
    table: ob-poc.cbu_resource_instances
    column: status
    initial: PENDING
    terminal: [DECOMMISSIONED]
    transitions:
      PENDING: [PROVISIONING, DECOMMISSIONED]
      PROVISIONING: [ACTIVE, DECOMMISSIONED]
      ACTIVE: [SUSPENDED, DECOMMISSIONED]
      SUSPENDED: [ACTIVE, DECOMMISSIONED]
```

### Linter Integration

CSG linter checks `set-status` verbs:
1. Load state machine config
2. If verb is `*.set-status` and we can resolve current status from context
3. Warn if transition looks invalid

Note: Full validation happens at DB level via function. Linter is advisory.

### Files to Change
- rust/config/csg_rules.yaml - add state_machines section
- rust/src/dsl_v2/config/types.rs - add StateMachine serde types
- rust/src/dsl_v2/csg_linter.rs - optional transition warnings

---

## 6. Split Verb Config

### Current State
- Single verbs.yaml: **5,259 lines, 28 domains**
- Hard to navigate and maintain

### Proposed Structure

```
rust/config/
├── verbs/
│   ├── _meta.yaml           # version, common definitions
│   ├── cbu.yaml
│   ├── entity.yaml
│   ├── document.yaml
│   ├── kyc/
│   │   ├── case.yaml
│   │   ├── workstream.yaml
│   │   └── screening.yaml
│   ├── custody/
│   │   ├── universe.yaml
│   │   └── ssi.yaml
│   └── observation/
│       ├── allegation.yaml
│       ├── observation.yaml
│       └── discrepancy.yaml
└── csg_rules.yaml
```

### Files to Change
- rust/config/verbs/*.yaml (new, split from verbs.yaml)
- rust/src/dsl_v2/config/loader.rs - multi-file loading

---

## 7. Event Emission (PARKED)

Parked for POC. Required for production audit trail.

---

## Implementation Order

```
1. Idempotency (CRITICAL)     ─────────────────────┐
                                                    │
2. Snapshot tables            ──────┐              │
                                    ├─► 4. State   │
3. Harden statuses            ──────┘    snapshots │
                                              │     │
5. State transitions (csg)    ◄───────────────┘    │
                                                    │
6. Split verb config          ◄────────────────────┘
                              (can be done anytime)
```

### Estimated Effort

| Workstream | Effort |
|------------|--------|
| 1. Idempotency | 6-8 hours |
| 2. Snapshot tables | 4-6 hours |
| 3. Harden statuses | 1-2 hours |
| 4. State snapshots + functions | 4-6 hours |
| 5. State transitions (csg) | 2-3 hours |
| 6. Split verb config | 2-4 hours |
| **Total** | **19-29 hours** |
