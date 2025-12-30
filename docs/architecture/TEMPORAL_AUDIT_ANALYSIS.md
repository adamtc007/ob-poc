# Temporal/Historical Data Audit

## Executive Summary

**Current State: PARTIAL COVERAGE**

You have the foundation for temporal tracking but **critical gaps for regulatory audit**:

| Capability | Status | Gap |
|------------|--------|-----|
| Bi-temporal columns | ✅ Present | - |
| Current state views | ✅ Present | - |
| Point-in-time queries | ❌ Missing | **CRITICAL** |
| Automatic history capture | ❌ Missing | **CRITICAL** |
| UBO snapshots | ✅ Present | Manual trigger |
| Change logs | ⚠️ Partial | CBU only |
| Audit trail for edges | ❌ Missing | **HIGH** |

---

## 1. What You Have

### Bi-Temporal Columns ✅

Most key tables have both:
- **Business time**: `effective_from`, `effective_to` (when relationship was true in reality)
- **System time**: `created_at`, `updated_at` (when record was written)

| Table | Business Time | System Time |
|-------|--------------|-------------|
| `entity_relationships` | ✅ | ✅ |
| `cbu_entity_roles` | ✅ | ✅ |
| `fund_structure` | ✅ | ✅ |
| `delegation_relationships` | ✅ | ✅ |
| `attribute_observations` | ✅ | ✅ |
| `proofs` | ✅ `valid_from/until` | ✅ |

### Current State Views ✅

Views that filter for active records:

```sql
-- entity_relationships_current
WHERE effective_to IS NULL OR effective_to > CURRENT_DATE

-- cbu_ownership_graph
WHERE effective_to IS NULL OR effective_to > CURRENT_DATE
```

### Supersession Tracking ✅

Some tables track record replacement:

| Table | Supersession Columns |
|-------|---------------------|
| `ubo_registry` | `superseded_by`, `superseded_at`, `closed_at`, `replacement_ubo_id` |
| `attribute_observations` | `superseded_by`, `superseded_at` |

### UBO Snapshots ✅

Full state capture at point in time:

```sql
-- ubo_snapshots
snapshot_id, cbu_id, case_id
ubos                    -- JSONB: full UBO list
ownership_chains        -- JSONB: full chains
control_relationships   -- JSONB: full control structure
captured_at, captured_by
```

### Snapshot Comparisons ✅

Diff infrastructure exists:

```sql
-- ubo_snapshot_comparisons
baseline_snapshot_id, current_snapshot_id
added_ubos, removed_ubos, changed_ubos
ownership_changes, control_changes
```

### Change Logs (Partial)

```sql
-- cbu_change_log (CBU field changes only)
field_name, old_value, new_value, changed_at, changed_by, reason

-- workflow_audit_log (state transitions)
from_state, to_state, transitioned_at, transitioned_by
```

---

## 2. What's Missing (UPDATE: Some Functions EXIST!)

### ⚠️ UPDATE: Point-in-Time Functions Already Exist!

**Found after deeper audit** - these SQL functions exist:

| Function | Purpose | Status |
|----------|---------|--------|
| `cbu_relationships_as_of(cbu_id, date)` | Get ownership/control at date | ✅ EXISTS |
| `cbu_roles_as_of(cbu_id, date)` | Get roles at date | ✅ EXISTS |
| `ownership_as_of(...)` | Get ownership at date | ✅ EXISTS |
| `ubo_chain_as_of(...)` | Get UBO chain at date | ✅ EXISTS |
| `cbu_state_at_approval(cbu_id)` | State when KYC approved | ✅ EXISTS |

**The REAL gap**: These functions exist but:
1. No DSL verbs to invoke them
2. GraphRepository doesn't use them
3. UI has no date picker to trigger as-of queries

### Remaining Gaps

**Current state**: Views filter for `effective_to IS NULL OR > CURRENT_DATE` (current only)

**Need**:
```sql
-- Should be able to query:
SELECT * FROM ownership_as_of('2024-06-15'::date, cbu_id);

-- Or parameterized views:
SELECT * FROM entity_relationships_at_date(cbu_id, '2024-06-15');
```

### ❌ CRITICAL: Automatic History Capture

**Problem**: When `entity_relationships.effective_to` is set (ending a relationship), the old percentage/details are lost if someone also updates the record.

**Need**: Either:
1. **History table** with trigger: Every UPDATE writes old row to `entity_relationships_history`
2. **Immutable + new row**: End old relationship (set effective_to), CREATE new row for changes
3. **Temporal table extension**: Use PostgreSQL temporal tables or `periods` extension

### ❌ HIGH: Edge Audit Trail

**Problem**: No audit log for relationship changes (ownership %, control changes)

`cbu_change_log` only tracks CBU-level fields, not edges.

**Need**:
```sql
-- relationship_audit_log
relationship_id, change_type, 
old_percentage, new_percentage,
old_control_type, new_control_type,
changed_at, changed_by, reason, evidence_doc_id
```

### ❌ MEDIUM: Graph Repository Temporal Support

**Problem**: `GraphRepository` has no date parameter - always loads current state.

```rust
// Current - no date parameter
pub async fn load_ownership_edges(&self, cbu_id: Uuid) -> Result<Vec<OwnershipEdge>>

// Need
pub async fn load_ownership_edges_as_of(
    &self, 
    cbu_id: Uuid, 
    as_of_date: NaiveDate
) -> Result<Vec<OwnershipEdge>>
```

### ⚠️ LOW: UBO Snapshots Are Manual

Snapshots require explicit capture - no automatic triggers.

**Consider**: Auto-snapshot on case status change (SUBMITTED, APPROVED)

---

## 3. Regulatory Requirements

KYC regulations require ability to:

| Requirement | Current Support |
|-------------|-----------------|
| Show ownership at onboarding date | ❌ No point-in-time |
| Show what changed between reviews | ⚠️ Manual via snapshots |
| Audit trail of who changed what | ⚠️ Partial (CBU only) |
| Prove due diligence at decision time | ✅ Snapshots + evidence |
| Reconstruct historical UBO | ❌ No versioned edges |

### FATF/AML Requirements

- **Requirement**: Maintain records for 5+ years after relationship ends
- **Current**: effective_to captures end date, but no versioning of changes
- **Gap**: Cannot show "ownership was 30% from 2020-2024, then changed to 25%"

### MiFID II / Dodd-Frank

- **Requirement**: Full audit trail of beneficial ownership determinations
- **Current**: ubo_registry has supersession, but edges don't
- **Gap**: Cannot prove why UBO changed without edge history

---

## 4. Recommended Fixes

### P0: Add Point-in-Time Query Function

```sql
CREATE OR REPLACE FUNCTION "ob-poc".entity_relationships_as_of(
    p_cbu_id UUID,
    p_as_of DATE
)
RETURNS TABLE (
    relationship_id UUID,
    from_entity_id UUID,
    to_entity_id UUID,
    relationship_type VARCHAR,
    percentage NUMERIC,
    ownership_type VARCHAR,
    control_type VARCHAR,
    trust_role VARCHAR,
    effective_from DATE,
    effective_to DATE
) AS $$
BEGIN
    RETURN QUERY
    SELECT r.relationship_id, r.from_entity_id, r.to_entity_id,
           r.relationship_type, r.percentage, r.ownership_type,
           r.control_type, r.trust_role, r.effective_from, r.effective_to
    FROM "ob-poc".entity_relationships r
    JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id IN (r.from_entity_id, r.to_entity_id)
    WHERE cer.cbu_id = p_cbu_id
      AND (r.effective_from IS NULL OR r.effective_from <= p_as_of)
      AND (r.effective_to IS NULL OR r.effective_to > p_as_of);
END;
$$ LANGUAGE plpgsql;
```

### P0: Add Relationship History Table + Trigger

```sql
CREATE TABLE "ob-poc".entity_relationships_history (
    history_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    relationship_id UUID NOT NULL,
    from_entity_id UUID NOT NULL,
    to_entity_id UUID NOT NULL,
    relationship_type VARCHAR NOT NULL,
    percentage NUMERIC,
    ownership_type VARCHAR,
    control_type VARCHAR,
    trust_role VARCHAR,
    effective_from DATE,
    effective_to DATE,
    source VARCHAR,
    -- History metadata
    valid_from TIMESTAMPTZ NOT NULL,
    valid_to TIMESTAMPTZ NOT NULL,
    changed_by VARCHAR,
    change_reason TEXT,
    superseded_by UUID
);

CREATE OR REPLACE FUNCTION "ob-poc".track_relationship_history()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        INSERT INTO "ob-poc".entity_relationships_history (
            relationship_id, from_entity_id, to_entity_id,
            relationship_type, percentage, ownership_type, control_type, trust_role,
            effective_from, effective_to, source,
            valid_from, valid_to, changed_by
        ) VALUES (
            OLD.relationship_id, OLD.from_entity_id, OLD.to_entity_id,
            OLD.relationship_type, OLD.percentage, OLD.ownership_type, 
            OLD.control_type, OLD.trust_role,
            OLD.effective_from, OLD.effective_to, OLD.source,
            OLD.updated_at, NOW(), current_user
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER entity_relationships_history_trigger
    BEFORE UPDATE ON "ob-poc".entity_relationships
    FOR EACH ROW EXECUTE FUNCTION "ob-poc".track_relationship_history();
```

### P1: Add Verbs for Temporal Operations

```yaml
# verbs/temporal.yaml
domains:
  temporal:
    verbs:
      ownership-as-of:
        description: Get ownership structure at a specific date
        behavior: plugin
        custom_handler: temporal_ownership_as_of
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: as-of-date
            type: date
            required: true
        returns:
          type: json

      compare-structures:
        description: Compare CBU structure between two dates
        behavior: plugin
        custom_handler: temporal_compare_structures
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: from-date
            type: date
            required: true
          - name: to-date
            type: date
            required: true
        returns:
          type: json
```

### P1: Update GraphRepository

```rust
impl GraphRepository {
    /// Load ownership edges as of a specific date
    pub async fn load_ownership_edges_as_of(
        &self,
        cbu_id: Uuid,
        as_of_date: NaiveDate,
    ) -> Result<Vec<OwnershipEdge>> {
        let rows = sqlx::query_as::<_, OwnershipEdgeRow>(
            r#"
            SELECT r.* FROM "ob-poc".entity_relationships r
            JOIN "ob-poc".cbu_entity_roles cer 
              ON cer.entity_id IN (r.from_entity_id, r.to_entity_id)
            WHERE cer.cbu_id = $1
              AND r.relationship_type = 'ownership'
              AND (r.effective_from IS NULL OR r.effective_from <= $2)
              AND (r.effective_to IS NULL OR r.effective_to > $2)
            "#,
        )
        .bind(cbu_id)
        .bind(as_of_date)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
```

### P2: Auto-Snapshot on KYC Events

```sql
CREATE OR REPLACE FUNCTION "ob-poc".auto_snapshot_on_kyc_decision()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status IN ('APPROVED', 'REJECTED', 'ESCALATED') 
       AND OLD.status != NEW.status THEN
        -- Capture UBO snapshot
        INSERT INTO "ob-poc".ubo_snapshots (
            cbu_id, case_id, snapshot_type, snapshot_reason,
            ubos, ownership_chains, captured_at
        )
        SELECT 
            NEW.cbu_id, NEW.case_id, 'AUTO', 'KYC decision: ' || NEW.status,
            -- Build JSONB from current state
            (SELECT jsonb_agg(...) FROM "ob-poc".ubo_registry WHERE cbu_id = NEW.cbu_id),
            (SELECT jsonb_agg(...) FROM "ob-poc".entity_relationships_current ...),
            NOW();
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

---

## 5. Animation/Timeline View Support

For the "animation" view you mentioned, you need:

### Data Model
```sql
-- Timeline events view
CREATE VIEW "ob-poc".v_cbu_timeline AS
SELECT cbu_id, 
       'RELATIONSHIP_ADDED' as event_type,
       from_entity_id, to_entity_id,
       effective_from as event_date,
       jsonb_build_object('type', relationship_type, 'percentage', percentage) as details
FROM "ob-poc".entity_relationships
UNION ALL
SELECT cbu_id,
       'RELATIONSHIP_ENDED',
       from_entity_id, to_entity_id,
       effective_to,
       jsonb_build_object('type', relationship_type) as details
FROM "ob-poc".entity_relationships
WHERE effective_to IS NOT NULL
UNION ALL
SELECT cer.cbu_id,
       'ROLE_ASSIGNED',
       entity_id, NULL,
       effective_from,
       jsonb_build_object('role', r.name) as details
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".roles r ON r.role_id = cer.role_id
-- ... more event sources
ORDER BY cbu_id, event_date;
```

### API/Verb Support
```yaml
timeline:
  list-events:
    description: Get timeline of changes for CBU
    args:
      - name: cbu-id
        type: uuid
      - name: from-date
        type: date
      - name: to-date
        type: date
      - name: event-types
        type: string[]
        required: false
```

---

## 6. Summary

| Fix | Priority | Effort | Impact |
|-----|----------|--------|--------|
| Point-in-time query function | **P0** | 2 hr | Enables "show me structure on date X" |
| Relationship history table + trigger | **P0** | 3 hr | Audit trail for edge changes |
| Temporal verbs | **P1** | 4 hr | Agent can query historical state |
| GraphRepository as_of_date | **P1** | 2 hr | Graph visualization at any date |
| Timeline events view | **P2** | 2 hr | Enables animation view |
| Auto-snapshot on KYC | **P2** | 1 hr | Ensures snapshots exist |

**Bottom line**: You have the columns and snapshot infrastructure. You're missing:
1. Query capability for historical data
2. Automatic audit trail for edge changes
3. Verb support for temporal operations


---

## 7. CORRECTION: Existing Temporal Functions Discovered

After deeper investigation for the attribute/document audit, I found **several temporal functions already exist**:

| Function | Purpose | Status |
|----------|---------|--------|
| `cbu_relationships_as_of(cbu_id, date)` | Get ownership/control at date | ✅ EXISTS |
| `cbu_roles_as_of(cbu_id, date)` | Get roles at date | ✅ EXISTS |
| `ownership_as_of(...)` | Get ownership at date | ✅ EXISTS |
| `ubo_chain_as_of(...)` | Get UBO chain at date | ✅ EXISTS |
| `cbu_state_at_approval(cbu_id)` | State when KYC approved | ✅ EXISTS |
| `entity_relationships_history_trigger` | History capture trigger | ✅ EXISTS |
| `cbu_entity_roles_history_trigger` | Role history trigger | ✅ EXISTS |

### Updated Priority

| Fix | Priority | Effort | Impact | Notes |
|-----|----------|--------|--------|-------|
| ~~Point-in-time function~~ | ~~P0~~ | - | - | **ALREADY EXISTS** |
| Wire temporal verbs to existing functions | **P0** | 1 hr | Agent access to temporal data | Just YAML config |
| Update GraphRepository to use functions | **P1** | 1 hr | UI date picker support | Simple delegation |
| Timeline events view | **P2** | 2 hr | Animation view | New view |
| ~~History trigger~~ | ~~P0~~ | - | - | **ALREADY EXISTS** |

**Revised bottom line**: The temporal SQL infrastructure is **better than assessed**. The real gap is:
1. ✅ SQL functions exist - ❌ but no DSL verbs to invoke them
2. ✅ History triggers exist - ❌ but not verified they're attached
3. ❌ GraphRepository doesn't use the as_of functions
4. ❌ UI has no date picker integration

This is a **wiring problem**, not a **capability gap**.
