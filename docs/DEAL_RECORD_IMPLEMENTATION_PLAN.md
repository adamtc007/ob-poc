# Deal Record & Fee Billing - Implementation Plan

**Status:** Ready for Implementation  
**Created:** 2026-02-05  
**Source:** `docs/TODO_DEAL_RECORD_1.md`

---

## Overview

Implement the Deal Record domain - the commercial origination hub that sits upstream of contracting, onboarding, and servicing. Includes the Fee Billing closed-loop that connects negotiated rate cards through to billable activity on CBU resource instances.

**Closed Loop:** Deal → Contract → Rate Card → Fee Billing Profile → Account Targets (cbu_resource_instances) → Activity → Fee Calculation → Invoice → Client Entity

---

## FK Validation Results

Pre-flight FK validation against live schema:

| Planned FK Target | Actual Table | Actual PK Column | Action |
|-------------------|--------------|------------------|--------|
| `client_groups(group_id)` | `ob-poc.client_group` | `id` | Change to `client_group(id)` |
| `accounting.service_contracts(contract_id)` | `ob-poc.legal_contracts` | `contract_id` | Change to `legal_contracts(contract_id)` |
| `documents` (for deal_documents) | `ob-poc.document_catalog` | `doc_id` | Change to `document_catalog(doc_id)` |
| `kyc_cases` | `kyc.cases` | `case_id` | Change to `kyc.cases(case_id)` |
| `cbu_resource_instances(instance_id)` | `ob-poc.cbu_resource_instances` | `instance_id` | ✓ Correct |
| `products(product_id)` | `ob-poc.products` | `product_id` | ✓ Correct |
| `services(service_id)` | `ob-poc.services` | `service_id` | ✓ Correct |
| `entities(entity_id)` | `ob-poc.entities` | `entity_id` | ✓ Correct |
| `cbus(cbu_id)` | `ob-poc.cbus` | `cbu_id` | ✓ Correct |

---

## Phase 1: Schema Migration

### Migration File: `migrations/076_deal_record_fee_billing.sql`

14 new tables in dependency order:
1. `deals` - Hub entity
2. `deal_participants` - Regional LEIs under the deal
3. `deal_contracts` - Links to legal_contracts
4. `deal_rate_cards` - Negotiated product pricing
5. `deal_rate_card_lines` - Individual fee lines with CHECK constraints
6. `deal_slas` - Service level agreements
7. `deal_documents` - Links to document_catalog
8. `deal_ubo_assessments` - UBO/KYC tracking per entity
9. `deal_onboarding_requests` - Handoff to Ops
10. `fee_billing_profiles` - Bridge commercial → operational
11. `fee_billing_account_targets` - Links to cbu_resource_instances
12. `fee_billing_periods` - Billing windows
13. `fee_billing_period_lines` - Calculated fee lines
14. `deal_events` - Audit trail

### Key Schema Corrections (from FK validation)

```sql
-- Use client_group(id) not client_groups(group_id)
primary_client_group_id UUID NOT NULL REFERENCES "ob-poc".client_group(id),

-- Use legal_contracts not accounting.service_contracts
contract_id UUID NOT NULL REFERENCES "ob-poc".legal_contracts(contract_id),

-- Use document_catalog(doc_id) for document links
document_id UUID NOT NULL REFERENCES "ob-poc".document_catalog(doc_id),

-- Use kyc.cases for KYC case links
kyc_case_id UUID REFERENCES kyc.cases(case_id),
```

---

## Phase 2: Verb Definitions

### New Domain: `deal` (30 verbs)

| Verb | Description | Behavior |
|------|-------------|----------|
| `deal.create` | Create deal record | plugin |
| `deal.get` | Get deal by ID | crud |
| `deal.list` | List deals with filters | crud |
| `deal.search` | Search by name/reference | plugin |
| `deal.update` | Update deal fields | crud |
| `deal.update-status` | Transition deal status (state machine) | plugin |
| `deal.cancel` | Cancel a deal | plugin |
| `deal.add-participant` | Add regional entity | plugin |
| `deal.remove-participant` | Remove participant | plugin |
| `deal.list-participants` | List participants | crud |
| `deal.add-contract` | Link contract to deal | plugin |
| `deal.remove-contract` | Unlink contract | plugin |
| `deal.list-contracts` | List contracts | crud |
| `deal.create-rate-card` | Create negotiated rate card | plugin |
| `deal.add-rate-card-line` | Add fee line to rate card | plugin |
| `deal.update-rate-card-line` | Modify rate card line | plugin |
| `deal.remove-rate-card-line` | Remove fee line | plugin |
| `deal.list-rate-card-lines` | List fee lines | crud |
| `deal.propose-rate-card` | Submit for client review | plugin |
| `deal.counter-rate-card` | Client counter-offer (clones) | plugin |
| `deal.agree-rate-card` | Finalize rate card | plugin |
| `deal.add-sla` | Add SLA to deal | plugin |
| `deal.remove-sla` | Remove SLA | plugin |
| `deal.list-slas` | List SLAs | crud |
| `deal.add-document` | Link document | plugin |
| `deal.update-document-status` | Update document status | plugin |
| `deal.list-documents` | List documents | crud |
| `deal.add-ubo-assessment` | Link UBO assessment | plugin |
| `deal.update-ubo-assessment` | Update assessment status | plugin |
| `deal.request-onboarding` | Create onboarding request | plugin |
| `deal.request-onboarding-batch` | Batch onboarding (transactional) | plugin |
| `deal.update-onboarding-status` | Update onboarding status | plugin |
| `deal.list-onboarding-requests` | List onboarding requests | crud |
| `deal.summary` | Full deal summary with nested data | plugin |
| `deal.timeline` | Event timeline for audit | crud |

### New Domain: `billing` (14 verbs)

| Verb | Description | Behavior |
|------|-------------|----------|
| `billing.create-profile` | Create billing profile | plugin |
| `billing.activate-profile` | Activate billing | plugin |
| `billing.suspend-profile` | Suspend billing | plugin |
| `billing.close-profile` | Close billing profile | plugin |
| `billing.get-profile` | Get profile with targets | crud |
| `billing.list-profiles` | List profiles | crud |
| `billing.add-account-target` | Link resource instance | plugin |
| `billing.remove-account-target` | Soft-remove target | plugin |
| `billing.list-account-targets` | List targets | crud |
| `billing.create-period` | Create billing period | plugin |
| `billing.calculate-period` | Run fee calculation | plugin |
| `billing.review-period` | Mark as reviewed | plugin |
| `billing.approve-period` | Approve for invoicing | plugin |
| `billing.generate-invoice` | Generate invoice | plugin |
| `billing.dispute-period` | Client dispute | plugin |
| `billing.period-summary` | Get period detail | crud |
| `billing.revenue-summary` | Aggregated revenue report | plugin |

---

## Phase 3: State Machines

### Deal Status Transitions
```
PROSPECT → [QUALIFYING, CANCELLED]
QUALIFYING → [NEGOTIATING, CANCELLED]
NEGOTIATING → [CONTRACTED, QUALIFYING, CANCELLED]
CONTRACTED → [ONBOARDING, CANCELLED]
ONBOARDING → [ACTIVE, CANCELLED]
ACTIVE → [WINDING_DOWN]
WINDING_DOWN → [OFFBOARDED]
// OFFBOARDED and CANCELLED are terminal
```

### Rate Card Status Transitions
```
DRAFT → PROPOSED → COUNTER_OFFERED ⟷ (can counter multiple times)
        ↓           ↓
      AGREED      AGREED
        ↓
     (immutable)
```

### Billing Period Status Transitions
```
PENDING → CALCULATING → CALCULATED → REVIEWED → APPROVED → INVOICED
                           ↓           ↓          ↓
                        DISPUTED    DISPUTED   DISPUTED
```

---

## Phase 4: Rust Implementation

### File Structure

```
rust/
├── crates/ob-poc-types/src/
│   ├── deal.rs                 # Deal, DealParticipant, DealRateCard, etc.
│   └── billing.rs              # FeeBillingProfile, BillingPeriod, etc.
├── src/domain_ops/
│   ├── deal_ops.rs             # CustomOperation impls for deal.*
│   └── billing_ops.rs          # CustomOperation impls for billing.*
└── config/verbs/
    ├── deal.yaml               # 34 verb definitions
    └── billing.yaml            # 17 verb definitions
```

### Key Implementation Notes

1. **Event Recording**: Every mutating operation writes to `deal_events`
2. **Rate Card Immutability**: Once status = AGREED, lines are frozen
3. **Closed Loop Validation**: Account targets must reference correct CBU's resources
4. **Batch Operations**: `deal.request-onboarding-batch` is transactional
5. **Fee Calculation**: Start with BPS and FLAT pricing models, add TIERED later

---

## Phase 5: Execution Steps

### Step 1: Create Migration
```bash
# Create migration file
touch migrations/076_deal_record_fee_billing.sql
# Apply migration
psql -d data_designer -f migrations/076_deal_record_fee_billing.sql
```

### Step 2: Create Verb YAML
```bash
# Create verb definition files
touch rust/config/verbs/deal.yaml
touch rust/config/verbs/billing.yaml
```

### Step 3: Create Type Definitions
```bash
# Add type files
touch rust/crates/ob-poc-types/src/deal.rs
touch rust/crates/ob-poc-types/src/billing.rs
```

### Step 4: Implement Custom Ops
```bash
# Add operation files
touch rust/src/domain_ops/deal_ops.rs
touch rust/src/domain_ops/billing_ops.rs
```

### Step 5: Register and Build
```bash
cd rust
cargo build
cargo sqlx prepare --workspace
```

### Step 6: Populate Embeddings
```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

### Step 7: Update Documentation
- Add to CLAUDE.md: Deal Record section
- Regenerate master-schema.sql

### Step 8: Verify
```bash
cargo x pre-commit
cargo x check --db
```

---

## Relationship Diagram

```
deals (hub)
  ├── deal_participants          → entities (regional LEIs)
  ├── deal_contracts             → legal_contracts
  ├── deal_rate_cards            → products
  │   └── deal_rate_card_lines   (fee lines with CHECK constraints)
  ├── deal_slas                  → products, services
  ├── deal_documents             → document_catalog
  ├── deal_ubo_assessments       → entities, kyc.cases
  ├── deal_onboarding_requests   → cbus, products, kyc.cases
  ├── fee_billing_profiles       → legal_contracts, deal_rate_cards, cbus, products, entities
  │   └── fee_billing_account_targets → cbu_resource_instances, deal_rate_card_lines
  │       └── fee_billing_periods
  │           └── fee_billing_period_lines
  └── deal_events                (audit trail)
```

---

## Testing Checklist

- [ ] Create deal → add participants → add contract → create rate card with lines
- [ ] Propose → counter → counter again → agree rate card
- [ ] Request onboarding (single and batch)
- [ ] Create billing profile → add account targets → activate
- [ ] Create period → calculate → review → approve → generate invoice
- [ ] Verify closed loop: rate_card_line → billing_account_target → period_line uses consistent rates
- [ ] Test all state machine transitions (valid and invalid)
- [ ] Test CHECK constraints on deal_rate_card_lines
- [ ] Test unique constraints (one primary participant, no duplicate onboarding requests)
