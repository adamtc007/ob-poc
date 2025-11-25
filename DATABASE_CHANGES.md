# Database Changes for KYC/UBO Idempotent Vocabulary

This document details the database schema changes required to support the idempotent KYC/UBO vocabulary implementation.

## Overview

The refactoring introduces UPSERT semantics throughout the KYC workflow, requiring specific unique constraints and new tables to support idempotent operations.

## Key Principle: Natural Keys for Idempotency

Each entity type uses a **natural key** for UPSERT operations:

| Entity Type | Natural Key | Constraint Name |
|-------------|-------------|-----------------|
| CBU | `name` | `cbus_name_key` (existing) |
| Entity Role Connection | `cbu_id, entity_id, role_id` | `entity_role_connections_natural_key` |
| Monitoring Setup | `cbu_id` | `monitoring_setup_cbu_unique` |

## Migration: 017_kyc_investigation_tables.sql

### New Tables Created

#### 1. `kyc_investigations`
Core investigation tracking table.

```sql
CREATE TABLE "ob-poc".kyc_investigations (
    investigation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_type VARCHAR(50) NOT NULL,  -- STANDARD, ENHANCED_DUE_DILIGENCE, PERIODIC_REVIEW
    status VARCHAR(50) DEFAULT 'INITIATED',
    risk_rating VARCHAR(20),
    ubo_threshold DECIMAL(5,2) DEFAULT 25.00,
    deadline DATE,
    initiated_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);
```

#### 2. `investigation_assignments`
Links investigations to assigned analysts/teams.

```sql
CREATE TABLE "ob-poc".investigation_assignments (
    assignment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID NOT NULL REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    assigned_to VARCHAR(255) NOT NULL,
    assigned_role VARCHAR(100),
    assigned_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### 3. `document_requests`
Tracks document collection requests during KYC.

```sql
CREATE TABLE "ob-poc".document_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID NOT NULL REFERENCES "ob-poc".kyc_investigations(investigation_id),
    document_type VARCHAR(100) NOT NULL,
    requested_from_entity_type VARCHAR(50),
    requested_from_entity_id UUID,
    status VARCHAR(50) DEFAULT 'PENDING',
    requested_at TIMESTAMPTZ DEFAULT NOW(),
    received_at TIMESTAMPTZ,
    doc_id UUID,  -- Reference to document_catalog (no FK due to schema constraints)
    notes TEXT
);
```

#### 4. `document_verifications`
Records document verification results.

```sql
CREATE TABLE "ob-poc".document_verifications (
    verification_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    doc_id UUID NOT NULL,
    verification_method VARCHAR(100) NOT NULL,
    verification_status VARCHAR(50) DEFAULT 'PENDING',
    verified_by VARCHAR(255),
    verified_at TIMESTAMPTZ,
    confidence_score DECIMAL(5,4),
    issues_found JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

#### 5. `screenings`
PEP, Sanctions, and Adverse Media screening records.

```sql
CREATE TABLE "ob-poc".screenings (
    screening_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    entity_id UUID,
    screening_type VARCHAR(50) NOT NULL,  -- PEP, SANCTIONS, ADVERSE_MEDIA
    provider VARCHAR(100),
    status VARCHAR(50) DEFAULT 'PENDING',
    result JSONB,
    match_score DECIMAL(5,4),
    screened_at TIMESTAMPTZ DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolved_by VARCHAR(255),
    resolution_notes TEXT
);
```

#### 6. `risk_assessments`
Entity and CBU risk assessment records.

```sql
CREATE TABLE "ob-poc".risk_assessments (
    assessment_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID,
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    assessment_type VARCHAR(50) NOT NULL,  -- ENTITY, CBU
    rating VARCHAR(20),  -- LOW, MEDIUM, HIGH, VERY_HIGH
    factors JSONB,
    methodology VARCHAR(50),
    rationale TEXT,
    assessed_by VARCHAR(255),
    assessed_at TIMESTAMPTZ DEFAULT NOW()
);

-- CHECK: At least one of cbu_id or entity_id must be provided
ALTER TABLE "ob-poc".risk_assessments 
ADD CONSTRAINT risk_assessments_check 
CHECK (cbu_id IS NOT NULL OR entity_id IS NOT NULL);
```

#### 7. `risk_flags`
Individual risk flags/concerns.

```sql
CREATE TABLE "ob-poc".risk_flags (
    flag_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID,
    assessment_id UUID REFERENCES "ob-poc".risk_assessments(assessment_id) ON DELETE CASCADE,
    flag_type VARCHAR(50) NOT NULL,  -- RED_FLAG, AMBER_FLAG, INFO
    description TEXT,
    status VARCHAR(50) DEFAULT 'OPEN',  -- OPEN, RESOLVED, ACCEPTED
    raised_at TIMESTAMPTZ DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolved_by VARCHAR(255)
);
```

#### 8. `kyc_decisions`
Final KYC decision records.

```sql
CREATE TABLE "ob-poc".kyc_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    investigation_id UUID REFERENCES "ob-poc".kyc_investigations(investigation_id) ON DELETE CASCADE,
    decision VARCHAR(50) NOT NULL,  -- APPROVED, REJECTED, CONDITIONAL_ACCEPTANCE, ESCALATED
    decision_authority VARCHAR(100),
    rationale TEXT,
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    decided_by VARCHAR(255),
    valid_until DATE,
    review_required BOOLEAN DEFAULT false
);
```

#### 9. `decision_conditions`
Conditions attached to conditional approvals.

```sql
CREATE TABLE "ob-poc".decision_conditions (
    condition_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    decision_id UUID NOT NULL REFERENCES "ob-poc".kyc_decisions(decision_id) ON DELETE CASCADE,
    condition_type VARCHAR(100) NOT NULL,
    description TEXT,
    status VARCHAR(50) DEFAULT 'PENDING',  -- PENDING, SATISFIED, WAIVED
    frequency VARCHAR(50),  -- For ongoing conditions: MONTHLY, QUARTERLY, ANNUALLY
    due_date DATE,
    satisfied_at TIMESTAMPTZ,
    satisfied_by VARCHAR(255)
);
```

#### 10. `monitoring_setup`
Ongoing monitoring configuration per CBU.

```sql
CREATE TABLE "ob-poc".monitoring_setup (
    monitoring_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    monitoring_level VARCHAR(50) NOT NULL,  -- STANDARD, ENHANCED, INTENSIVE
    triggers JSONB,
    thresholds JSONB,
    setup_at TIMESTAMPTZ DEFAULT NOW(),
    setup_by VARCHAR(255)
);

-- UNIQUE: One monitoring setup per CBU (for idempotent UPSERT)
ALTER TABLE "ob-poc".monitoring_setup 
ADD CONSTRAINT monitoring_setup_cbu_unique UNIQUE (cbu_id);
```

#### 11. `monitoring_events`
Individual monitoring events/alerts.

```sql
CREATE TABLE "ob-poc".monitoring_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    monitoring_id UUID REFERENCES "ob-poc".monitoring_setup(monitoring_id),
    event_type VARCHAR(100) NOT NULL,
    event_data JSONB,
    severity VARCHAR(20),  -- INFO, WARNING, ALERT, CRITICAL
    occurred_at TIMESTAMPTZ DEFAULT NOW(),
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by VARCHAR(255)
);
```

#### 12. `scheduled_reviews`
Scheduled periodic KYC reviews.

```sql
CREATE TABLE "ob-poc".scheduled_reviews (
    review_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    review_type VARCHAR(50) NOT NULL,  -- ANNUAL_KYC_REFRESH, TRIGGER_EVENT, AD_HOC
    due_date DATE NOT NULL,
    status VARCHAR(50) DEFAULT 'SCHEDULED',  -- SCHEDULED, IN_PROGRESS, COMPLETED, OVERDUE
    completed_at TIMESTAMPTZ,
    completed_by VARCHAR(255),
    outcome TEXT
);
```

#### 13. `ubo_registry` (if not exists)
Ultimate Beneficial Owner registry.

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".ubo_registry (
    ubo_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    entity_id UUID,
    ownership_percentage DECIMAL(5,2),
    control_type VARCHAR(100),
    verification_status VARCHAR(50) DEFAULT 'UNVERIFIED',
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Unique Constraints Added (Post-Migration)

These constraints were added manually after the migration to support UPSERT operations:

```sql
-- For entity role connections idempotency
ALTER TABLE "ob-poc".entity_role_connections 
ADD CONSTRAINT entity_role_connections_natural_key 
UNIQUE (cbu_id, entity_id, role_id);

-- For monitoring setup idempotency (one per CBU)
ALTER TABLE "ob-poc".monitoring_setup 
ADD CONSTRAINT monitoring_setup_cbu_unique 
UNIQUE (cbu_id);
```

## UPSERT Implementation Notes

### CBU UPSERT

The `cbu.ensure` word uses `ON CONFLICT (name)` because the existing `cbus_name_key` constraint enforces unique names:

```sql
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, nature_purpose, description, client_type, created_at, updated_at)
VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW(), NOW())
ON CONFLICT (name)
DO UPDATE SET
    jurisdiction = COALESCE(EXCLUDED.jurisdiction, cbus.jurisdiction),
    nature_purpose = COALESCE(EXCLUDED.nature_purpose, cbus.nature_purpose),
    description = COALESCE(EXCLUDED.description, cbus.description),
    client_type = COALESCE(EXCLUDED.client_type, cbus.client_type),
    updated_at = NOW()
RETURNING cbu_id
```

### Known Constraint Issues

1. **`risk_assessments_check`**: Requires either `cbu_id` OR `entity_id` to be NOT NULL. DSL words that create risk assessments must ensure they pass the appropriate ID from the RuntimeEnv context.

2. **Document tables**: `document_requests` and `document_verifications` do not have FK constraints to `document_catalog` due to the catalog's unique constraint structure. References are stored but not enforced at DB level.

## Index Summary

All new tables include appropriate indexes for:
- Primary keys (automatic)
- Foreign key columns (`cbu_id`, `entity_id`, `investigation_id`)
- Status columns for filtering
- Date columns for temporal queries

## Migration Execution

```bash
# Apply the migration
psql "postgresql:///data_designer?user=adamtc007" -f sql/migrations/017_kyc_investigation_tables.sql

# Add post-migration unique constraints
psql "postgresql:///data_designer?user=adamtc007" <<EOF
ALTER TABLE "ob-poc".entity_role_connections 
ADD CONSTRAINT entity_role_connections_natural_key 
UNIQUE (cbu_id, entity_id, role_id);

ALTER TABLE "ob-poc".monitoring_setup 
ADD CONSTRAINT monitoring_setup_cbu_unique 
UNIQUE (cbu_id);
EOF
```

## Testing Idempotency

The integration test `rust/tests/kyc_session_idempotency.rs` validates:

1. **`test_cbu_ensure_idempotency`**: Running `cbu.ensure` twice with the same name produces exactly 1 row, with updated fields.

2. **`test_kyc_session_idempotent_execution`**: Running a complete KYC session twice produces identical database state (pending: requires RuntimeEnv context propagation fix).

---

**Last Updated**: 2025-11-25
