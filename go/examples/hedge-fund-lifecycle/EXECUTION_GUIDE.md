# Hedge Fund Investor Lifecycle - Execution Guide

Step-by-step guide to execute the complete investor lifecycle using CLI commands and verify DSL persistence.

## Prerequisites

### 1. Database Setup

```bash
# Set connection string
export DB_CONN_STRING="postgres://localhost:5432/postgres?sslmode=disable"

# Apply hedge fund migration
psql "$DB_CONN_STRING" -f hedge-fund-investor-source/sql/migration_hedge_fund_investor.sql

# Verify DSL persistence table exists
psql "$DB_CONN_STRING" -c "SELECT COUNT(*) FROM \"hf-investor\".hf_dsl_executions;"
```

### 2. Build Application

```bash
# Build the application
cd dsl-ob-poc
go build -o dsl-poc

# Verify hedge fund commands available
./dsl-poc help | grep "hf-"
```

### 3. Prepare Test Data

You'll need UUIDs for fund, class, and series. Either:
- Use existing data from your database
- Create test data with seed scripts
- Use the UUIDs from this example (you'll need to insert test records)

## Step-by-Step Execution

### STATE 1: OPPORTUNITY → Create Investor

**Command:**
```bash
./dsl-poc hf-create-investor \
  --code="INV-2024-001" \
  --legal-name="Acme Capital Partners LP" \
  --type="CORPORATE" \
  --domicile="US" \
  --short-name="Acme Capital" \
  --contact-email="invest@acmecapital.com" \
  --contact-name="John Smith"
```

**Verify:**
```sql
-- Check investor was created
SELECT investor_id, investor_code, legal_name, status 
FROM "hf-investor".hf_investors 
WHERE investor_code = 'INV-2024-001';

-- Save the investor_id for next steps!
-- Example: a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d

-- Check DSL was persisted
SELECT dsl_text, execution_status 
FROM "hf-investor".hf_dsl_executions 
ORDER BY created_at DESC LIMIT 1;
```

**Expected Result:**
- Investor created with status: `OPPORTUNITY`
- DSL operation stored in `hf_dsl_executions`

---

### STATE 2: PRECHECKS → Record Indication

**Command:**
```bash
./dsl-poc hf-record-indication \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --fund=f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c \
  --class=c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f \
  --ticket=5000000 \
  --currency=USD
```

**Verify:**
```sql
-- Check investor status updated
SELECT investor_id, status 
FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Check indication was recorded
SELECT * FROM "hf-investor".hf_indications 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Check DSL count
SELECT COUNT(*) FROM "hf-investor".hf_dsl_executions 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `PRECHECKS`
- Indication of $5M recorded
- 2 DSL operations stored

---

### STATE 3: KYC_PENDING → Begin KYC Process

**Command:**
```bash
./dsl-poc hf-begin-kyc \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --tier=STANDARD
```

**Verify:**
```sql
-- Check KYC profile created
SELECT investor_id, kyc_tier, kyc_status 
FROM "hf-investor".hf_kyc_profiles 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `KYC_PENDING`
- KYC profile created with tier: `STANDARD`

---

### STATE 3: KYC_PENDING → Collect Documents

**Commands:**
```bash
# Certificate of Incorporation
./dsl-poc hf-collect-document \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --doc-type=CERTIFICATE_OF_INCORPORATION \
  --subject=primary_entity \
  --file-path=/docs/kyc/acme-capital/cert-of-inc.pdf

# Partnership Agreement
./dsl-poc hf-collect-document \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --doc-type=PARTNERSHIP_AGREEMENT \
  --subject=primary_entity \
  --file-path=/docs/kyc/acme-capital/partnership-agreement.pdf

# Authorized Signatories
./dsl-poc hf-collect-document \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --doc-type=AUTHORIZED_SIGNATORY_LIST \
  --subject=signatories \
  --file-path=/docs/kyc/acme-capital/auth-signatories.pdf

# Managing Partner Passport
./dsl-poc hf-collect-document \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --doc-type=PASSPORT \
  --subject=managing_partner_john_smith \
  --file-path=/docs/kyc/acme-capital/john-smith-passport.pdf
```

**Verify:**
```sql
-- Check documents collected
SELECT doc_type, subject, status 
FROM "hf-investor".hf_document_requirements 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Should show 4 documents
```

---

### STATE 3: KYC_PENDING → Screen Investor

**Command:**
```bash
./dsl-poc hf-screen-investor \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --provider=worldcheck
```

**Verify:**
```sql
-- Check screening result
SELECT screening_result, screening_provider, screening_date 
FROM "hf-investor".hf_screening_results 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d' 
ORDER BY screening_date DESC LIMIT 1;
```

**Expected Result:**
- Screening completed
- Result: `CLEAR` (no adverse findings)

---

### STATE 4: KYC_APPROVED → Approve KYC

**Command:**
```bash
./dsl-poc hf-approve-kyc \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --risk=MEDIUM \
  --refresh-due=2025-01-28 \
  --approved-by="Sarah Johnson, Head of Compliance"
```

**Verify:**
```sql
-- Check KYC approved
SELECT kyc_status, risk_rating, kyc_approval_date, next_refresh_due 
FROM "hf-investor".hf_kyc_profiles 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Check investor status
SELECT status FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `KYC_APPROVED`
- Risk rating: `MEDIUM`
- Refresh due date set

---

### STATE 4: KYC_APPROVED → Set Refresh Schedule

**Command:**
```bash
./dsl-poc hf-set-refresh-schedule \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --frequency=ANNUAL \
  --next=2025-01-28
```

---

### STATE 4: KYC_APPROVED → Enable Continuous Screening

**Command:**
```bash
./dsl-poc hf-set-continuous-screening \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --frequency=DAILY
```

---

### STATE 4: KYC_APPROVED → Capture Tax Information

**Command:**
```bash
./dsl-poc hf-capture-tax \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --fatca=NON_US_PERSON \
  --crs=ENTITY \
  --form=W8_BEN_E
```

**Verify:**
```sql
-- Check tax profile
SELECT fatca_status, crs_classification, tax_form_type 
FROM "hf-investor".hf_tax_profiles 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

---

### STATE 4: KYC_APPROVED → Set Banking Instructions

**Command:**
```bash
./dsl-poc hf-set-bank-instruction \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --currency=USD \
  --bank-name="JPMorgan Chase Bank N.A." \
  --account-name="Acme Capital Partners LP" \
  --swift=CHASUS33 \
  --account-num=1234567890
```

**Verify:**
```sql
-- Check banking instructions
SELECT currency, bank_name, account_name, swift_bic 
FROM "hf-investor".hf_bank_instructions 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

---

### STATE 5: SUB_PENDING_CASH → Submit Subscription

**Command:**
```bash
./dsl-poc hf-subscribe-request \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --fund=f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c \
  --class=c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f \
  --amount=5000000 \
  --currency=USD \
  --trade-date=2024-02-05 \
  --value-date=2024-02-10
```

**Verify:**
```sql
-- Check subscription trade created
SELECT trade_id, trade_type, subscription_amount, trade_date, value_date 
FROM "hf-investor".hf_trades 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d' 
  AND trade_type = 'SUBSCRIPTION';

-- Save the trade_id for next steps!

-- Check investor status
SELECT status FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `SUB_PENDING_CASH`
- Subscription trade created for $5M

---

### STATE 6: FUNDED_PENDING_NAV → Confirm Cash Receipt

**Command:**
```bash
./dsl-poc hf-confirm-cash \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --trade=t1r2a3-4567-8901-2345-678901234576 \
  --amount=5000000 \
  --value-date=2024-02-10 \
  --bank-currency=USD \
  --reference=ACME-SUB-20240210-001
```

**Verify:**
```sql
-- Check cash confirmed
SELECT cash_received, cash_received_date, bank_reference 
FROM "hf-investor".hf_trades 
WHERE trade_id = 't1r2a3-4567-8901-2345-678901234576';

-- Check investor status
SELECT status FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `FUNDED_PENDING_NAV`
- Cash receipt confirmed

---

### STATE 6: FUNDED_PENDING_NAV → Strike NAV

**Command:**
```bash
./dsl-poc hf-set-nav \
  --fund=f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c \
  --class=c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f \
  --nav-date=2024-02-10 \
  --nav=1250.75
```

**Verify:**
```sql
-- Check NAV was set
SELECT nav_date, nav_per_share, status 
FROM "hf-investor".hf_nav_history 
WHERE class_id = 'c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f' 
  AND nav_date = '2024-02-10';
```

---

### STATE 7-8: ISSUED → ACTIVE → Issue Units

**Command:**
```bash
./dsl-poc hf-issue-units \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --trade=t1r2a3-4567-8901-2345-678901234576 \
  --class=c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f \
  --series=s1e2r3-4567-8901-2345-678901234579 \
  --nav-per-share=1250.75 \
  --units=3997.6
```

**Verify:**
```sql
-- Check register lot created
SELECT lot_id, units, total_cost, average_cost, status 
FROM "hf-investor".hf_register_lots 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Check register event
SELECT event_type, delta_units, nav_per_share, value_date 
FROM "hf-investor".hf_register_events 
WHERE lot_id IN (
  SELECT lot_id FROM "hf-investor".hf_register_lots 
  WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
);

-- Check investor status
SELECT status FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `ACTIVE`
- Units: 3,997.60
- Cost basis: $5,000,000
- Average cost: $1,250.75/unit

---

### STATE 9: REDEEM_PENDING → Request Redemption

**Command:**
```bash
./dsl-poc hf-redeem-request \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --class=c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f \
  --percentage=100 \
  --notice-date=2024-10-31 \
  --value-date=2024-12-31
```

**Verify:**
```sql
-- Check redemption trade
SELECT trade_id, trade_type, redemption_units, notice_date, value_date 
FROM "hf-investor".hf_trades 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d' 
  AND trade_type = 'REDEMPTION';

-- Check investor status
SELECT status FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `REDEEM_PENDING`
- Redemption for 100% (3,997.60 units)

---

### STATE 9: REDEEM_PENDING → Strike Redemption NAV

**Command:**
```bash
./dsl-poc hf-set-nav \
  --fund=f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c \
  --class=c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f \
  --nav-date=2024-12-31 \
  --nav=1402.30
```

---

### STATE 10: REDEEMED → Settle Redemption

**Command:**
```bash
./dsl-poc hf-settle-redemption \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --trade=t2r2d3-4567-8901-2345-678901234582 \
  --amount=5604828.48 \
  --settle-date=2025-01-05 \
  --reference=ACME-RED-20250105-001
```

**Verify:**
```sql
-- Check register lot closed
SELECT units, status 
FROM "hf-investor".hf_register_lots 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Check redemption event
SELECT event_type, delta_units, proceeds 
FROM "hf-investor".hf_register_events 
WHERE event_type = 'REDEEM' 
  AND lot_id IN (
    SELECT lot_id FROM "hf-investor".hf_register_lots 
    WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
  );

-- Check investor status
SELECT status FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `REDEEMED`
- Units: 0.00 (fully redeemed)
- Proceeds: $5,604,828.48
- Realized gain: $604,828.48

---

### STATE 11: OFFBOARDED → Close Relationship

**Command:**
```bash
./dsl-poc hf-offboard-investor \
  --investor=a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d \
  --reason="Investor fully redeemed, relationship terminated per client request"
```

**Verify:**
```sql
-- Check final status
SELECT status, offboarded_at, offboarding_reason 
FROM "hf-investor".hf_investors 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';

-- Check complete lifecycle
SELECT to_state, transitioned_at 
FROM "hf-investor".hf_lifecycle_states 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d' 
ORDER BY transitioned_at ASC;

-- Count total DSL operations
SELECT COUNT(*) FROM "hf-investor".hf_dsl_executions 
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

**Expected Result:**
- Status changed to: `OFFBOARDED` (terminal state)
- 11 lifecycle state transitions recorded
- 21 DSL operations persisted

---

## Verify Complete DSL Persistence

### View All DSL Operations

```sql
-- Complete DSL history for the investor
SELECT
  execution_id,
  LEFT(dsl_text, 80) || '...' as operation,
  execution_status,
  execution_time_ms,
  TO_CHAR(completed_at, 'YYYY-MM-DD HH24:MI:SS') as completed_at
FROM "hf-investor".hf_dsl_executions
WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
ORDER BY created_at ASC;
```

### Extract Verb Usage

```sql
-- Count operations by verb type
WITH verb_extract AS (
  SELECT SUBSTRING(dsl_text FROM '^\(([a-z]+\.[a-z\-]+)') as verb
  FROM "hf-investor".hf_dsl_executions
  WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
)
SELECT verb, COUNT(*) as count
FROM verb_extract
GROUP BY verb
ORDER BY count DESC;
```

### View Investment Summary

```sql
-- Complete investment performance
SELECT
  i.investor_code,
  i.legal_name,
  i.status,
  rl.units as final_units,
  rl.total_cost as total_invested,
  (SELECT SUM(proceeds) FROM "hf-investor".hf_register_events 
   WHERE lot_id = rl.lot_id AND event_type = 'REDEEM') as total_redeemed,
  (SELECT SUM(proceeds) - rl.total_cost 
   FROM "hf-investor".hf_register_events 
   WHERE lot_id = rl.lot_id AND event_type = 'REDEEM') as realized_gain
FROM "hf-investor".hf_investors i
LEFT JOIN "hf-investor".hf_register_lots rl ON i.investor_id = rl.investor_id
WHERE i.investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d';
```

---

## Troubleshooting

### Issue: "Investor not found"
**Solution:** Check the investor_id is correct and exists in the database.

### Issue: "Invalid state transition"
**Solution:** Check current state and verify guard conditions are met.

### Issue: "Foreign key violation"
**Solution:** Ensure fund_id, class_id, and series_id exist in respective tables.

### Issue: DSL not persisting
**Solution:** 
1. Verify table exists: `\d "hf-investor".hf_dsl_executions`
2. Check permissions
3. Review application logs for errors

---

## Reporting Commands

### View Register

```bash
./dsl-poc hf-show-register --format=table
./dsl-poc hf-show-register --format=json
./dsl-poc hf-show-register --format=csv
```

### View KYC Dashboard

```bash
./dsl-poc hf-show-kyc-dashboard
./dsl-poc hf-show-kyc-dashboard --overdue
./dsl-poc hf-show-kyc-dashboard --risk=MEDIUM
```

---

## Summary

**Total Commands Executed:** 21
**Total States Traversed:** 11
**Total DSL Operations Persisted:** 21
**Investment Lifecycle:** Complete from opportunity to offboarding
**ROI:** 12.10% over 10-month holding period

All DSL operations are stored in `"hf-investor".hf_dsl_executions` providing:
- Complete audit trail
- Replay capability
- Regulatory compliance
- Operational history

**Next Steps:**
- Review persisted DSL in database
- Generate reports using the register views
- Explore alternative paths (partial redemptions, etc.)
- Test error handling and rollback scenarios