# Hedge Fund Investor Lifecycle - DSL Examples

Complete lifecycle demonstration for hedge fund investor onboarding, operations, and offboarding using Domain-Specific Language (DSL).

## Overview

This directory contains a complete end-to-end example of a hedge fund investor lifecycle, demonstrating all 11 states and 17 DSL verbs in the hedge fund investor vocabulary.

**Investor Profile:**
- **Name**: Acme Capital Partners LP
- **Type**: Corporate Investor
- **Investment**: $5,000,000 USD
- **Fund**: Global Opportunities Hedge Fund (Class A USD)
- **Timeline**: January 2024 - January 2025 (360 days)
- **Outcome**: Full redemption with 12.10% ROI

## Files in This Directory

### 1. `complete-lifecycle-example.dsl`
S-expression format DSL showing the complete lifecycle with detailed annotations:
- **Format**: Lisp-style S-expressions
- **Total Operations**: 21 DSL operations
- **Total States**: 11 lifecycle states
- **Comments**: Extensive inline documentation explaining each state transition
- **Results**: Shows expected outcomes for each operation

### 2. `lifecycle-plan.json`
JSON format for programmatic execution:
- **Format**: Structured JSON
- **Purpose**: Machine-readable execution plan
- **Fields**: Step IDs, verbs, parameters, expected states, timestamps
- **Metadata**: Investment summary, verb usage, persistence information

## Lifecycle States Demonstrated

The examples walk through all 11 states in order:

| State | DSL Operations | Purpose |
|-------|----------------|---------|
| **1. OPPORTUNITY** | `investor.start-opportunity` | Initial lead capture |
| **2. PRECHECKS** | `investor.record-indication` | Interest confirmation |
| **3. KYC_PENDING** | `kyc.begin`, `kyc.collect-doc` (×4), `kyc.screen` | KYC document collection and screening |
| **4. KYC_APPROVED** | `kyc.approve`, `kyc.refresh-schedule`, `screen.continuous`, `tax.capture`, `bank.set-instruction` | KYC approval and setup |
| **5. SUB_PENDING_CASH** | `subscribe.request` | Subscription order placed |
| **6. FUNDED_PENDING_NAV** | `cash.confirm`, `deal.nav` | Cash received, awaiting NAV |
| **7. ISSUED** | `subscribe.issue` | Units allocated |
| **8. ACTIVE** | _(holding period)_ | Active investment position |
| **9. REDEEM_PENDING** | `redeem.request`, `deal.nav` | Redemption requested |
| **10. REDEEMED** | `redeem.settle` | Cash paid out |
| **11. OFFBOARDED** | `offboard.close` | Relationship closed |

## DSL Persistence

### Database Table: `"hf-investor".hf_dsl_executions`

All DSL operations are persisted in the database for complete audit trails and replay capability.

**Table Schema:**
```sql
CREATE TABLE IF NOT EXISTS "hf-investor".hf_dsl_executions (
    execution_id UUID PRIMARY KEY,
    investor_id UUID NOT NULL,
    dsl_text TEXT NOT NULL,
    execution_status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    idempotency_key VARCHAR(255) UNIQUE,
    triggered_by VARCHAR(255),
    execution_engine VARCHAR(50) DEFAULT 'hedge-fund-dsl-v1',
    affected_entities JSONB,
    error_details TEXT,
    execution_time_ms INTEGER,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT (now() at time zone 'utc')
);
```

### Example Persisted Record

```sql
INSERT INTO "hf-investor".hf_dsl_executions (
  execution_id,
  investor_id,
  dsl_text,
  execution_status,
  idempotency_key,
  triggered_by,
  execution_engine,
  affected_entities,
  execution_time_ms,
  completed_at
) VALUES (
  '11111111-2222-3333-4444-555555555555',
  'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d',
  '(investor.start-opportunity
    :legal-name "Acme Capital Partners LP"
    :type "CORPORATE"
    :domicile "US"
    :source "Institutional Investor Conference Q1 2024")',
  'COMPLETED',
  'sha256:abc123...',
  'operations@fundadmin.com',
  'hedge-fund-dsl-v1',
  '{"investor_id": "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d", "investor_code": "INV-2024-001"}',
  45,
  '2024-01-15 10:00:00.045+00'
);
```

## Querying DSL History

### Retrieve Complete Lifecycle for an Investor

```sql
SELECT
  execution_id,
  dsl_text,
  execution_status,
  triggered_by,
  execution_time_ms,
  completed_at
FROM "hf-investor".hf_dsl_executions
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
ORDER BY created_at ASC;
```

### Count Operations by Status

```sql
SELECT
  execution_status,
  COUNT(*) as operation_count,
  AVG(execution_time_ms) as avg_execution_time
FROM "hf-investor".hf_dsl_executions
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
GROUP BY execution_status;
```

### Extract Specific Operation Types

```sql
SELECT
  execution_id,
  dsl_text,
  completed_at
FROM "hf-investor".hf_dsl_executions
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
  AND dsl_text LIKE '(kyc.%'
ORDER BY created_at ASC;
```

### Timeline of State Transitions

```sql
SELECT
  e.execution_id,
  SUBSTRING(e.dsl_text FROM '^\(([a-z]+\.[a-z-]+)') as verb,
  ls.to_state,
  e.completed_at,
  e.execution_time_ms
FROM "hf-investor".hf_dsl_executions e
JOIN "hf-investor".hf_lifecycle_states ls 
  ON ls.investor_id = e.investor_id
  AND ls.transitioned_at BETWEEN e.started_at AND e.completed_at
WHERE e.investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
ORDER BY e.created_at ASC;
```

## Using the Examples

### 1. View the Complete Lifecycle

```bash
# Read the annotated DSL
cat complete-lifecycle-example.dsl

# Read the JSON plan
cat lifecycle-plan.json | jq .
```

### 2. Execute Individual Operations

```bash
# Create opportunity (State 1)
./dsl-poc hf-create-investor \
  --code="INV-2024-001" \
  --legal-name="Acme Capital Partners LP" \
  --type="CORPORATE" \
  --domicile="US"

# Record indication (State 2)
./dsl-poc hf-record-indication \
  --investor=<uuid> \
  --fund=<uuid> \
  --class=<uuid> \
  --ticket=5000000 \
  --currency="USD"

# Begin KYC (State 3)
./dsl-poc hf-begin-kyc \
  --investor=<uuid> \
  --tier="STANDARD"
```

### 3. Verify DSL Persistence

After executing operations, verify they're stored:

```bash
# Connect to database
psql "$DB_CONN_STRING"

# Check stored DSL
SELECT COUNT(*) FROM "hf-investor".hf_dsl_executions;

# View latest operations
SELECT 
  LEFT(dsl_text, 50) as operation,
  execution_status,
  completed_at
FROM "hf-investor".hf_dsl_executions
ORDER BY created_at DESC
LIMIT 10;
```

## Investment Summary

The complete lifecycle demonstrates:

| Metric | Value |
|--------|-------|
| **Initial Investment** | $5,000,000.00 |
| **Subscription NAV** | $1,250.75/share |
| **Units Allocated** | 3,997.60 |
| **Holding Period** | 10 months (Feb - Dec 2024) |
| **Redemption NAV** | $1,402.30/share |
| **Redemption Proceeds** | $5,604,828.48 |
| **Realized Gain** | $604,828.48 |
| **ROI** | 12.10% |
| **Total Operations** | 21 DSL operations |
| **Total States** | 11 lifecycle states |
| **Verbs Demonstrated** | 17 of 17 (100% coverage) |

## DSL Verb Coverage

All 17 verbs in the hedge fund investor vocabulary are demonstrated:

- ✅ `investor.start-opportunity`
- ✅ `investor.record-indication`
- ✅ `kyc.begin`
- ✅ `kyc.collect-doc`
- ✅ `kyc.screen`
- ✅ `kyc.approve`
- ✅ `kyc.refresh-schedule`
- ✅ `tax.capture`
- ✅ `bank.set-instruction`
- ✅ `subscribe.request`
- ✅ `cash.confirm`
- ✅ `deal.nav`
- ✅ `subscribe.issue`
- ✅ `screen.continuous`
- ✅ `redeem.request`
- ✅ `redeem.settle`
- ✅ `offboard.close`

## State Machine Validation

The example demonstrates successful state transitions with all guard conditions met:

### OPPORTUNITY → PRECHECKS
- ✅ Guard: `indication_recorded = true`

### PRECHECKS → KYC_PENDING
- ✅ Guard: `initial_documents_submitted = true`

### KYC_PENDING → KYC_APPROVED
- ✅ Guard: `documents_verified = true`
- ✅ Guard: `screening_passed = true`
- ✅ Guard: `risk_rating_assigned = true`

### KYC_APPROVED → SUB_PENDING_CASH
- ✅ Guard: `valid_subscription_order = true`
- ✅ Guard: `minimum_investment_met = true`
- ✅ Guard: `banking_instructions_set = true`

### SUB_PENDING_CASH → FUNDED_PENDING_NAV
- ✅ Guard: `settlement_funds_received = true`

### FUNDED_PENDING_NAV → ISSUED → ACTIVE
- ✅ Guard: `nav_struck = true`
- ✅ Guard: `units_allocated = true`

### ACTIVE → REDEEM_PENDING
- ✅ Guard: `valid_redemption_notice = true`
- ✅ Guard: `notice_period_satisfied = true`

### REDEEM_PENDING → REDEEMED
- ✅ Guard: `units_redeemed = true`
- ✅ Guard: `cash_payment_made = true`

### REDEEMED → OFFBOARDED
- ✅ Guard: `final_docs_complete = true`

## Related Resources

- **SQL Migration**: `hedge-fund-investor-source/sql/migration_hedge_fund_investor.sql`
- **DSL Vocabulary**: `hedge-fund-investor-source/hf-investor/dsl/hedge_fund_dsl.go`
- **State Machine**: `hedge-fund-investor-source/hf-investor/state/state_machine.go`
- **CLI Commands**: `hedge-fund-investor-source/shared-cli/hf_*.go`
- **Main Documentation**: `HEDGE_FUND_INVESTOR.md`

## Validation

To validate the examples are complete and accurate:

```bash
# 1. Verify table exists
psql "$DB_CONN_STRING" -c "
  SELECT table_name 
  FROM information_schema.tables 
  WHERE table_schema = 'hf-investor' 
    AND table_name = 'hf_dsl_executions';
"

# 2. Check table structure
psql "$DB_CONN_STRING" -c "
  \d \"hf-investor\".hf_dsl_executions
"

# 3. Count existing DSL executions
psql "$DB_CONN_STRING" -c "
  SELECT COUNT(*) FROM \"hf-investor\".hf_dsl_executions;
"
```

## Notes

- **Idempotency**: Each DSL operation includes an idempotency key for safe retry
- **Audit Trail**: Complete history preserved in `hf_dsl_executions` table
- **Replay Capability**: DSL can be replayed from any point in the lifecycle
- **Regulatory Compliance**: Immutable audit log meets regulatory requirements
- **Performance**: Indexed on investor_id, status, and created_at for fast queries

---

**Created**: December 2024  
**Version**: 1.0.0  
**Related**: HEDGE_FUND_INVESTOR.md