# Architecture Brain Dump: ISDA, CSA, and Custodian Role in OTC Derivatives

## Date: 2025-01-05
## Context: Trading View visualization, Instrument Matrix, Collateral Management

---

## ISDA Master Agreement Structure

The ISDA Master Agreement is a **bilateral framework** between two trading counterparties for OTC derivatives. It does NOT involve the custodian directly.

```
┌─────────────────────────────────────────────────────────────────────┐
│                     ISDA MASTER AGREEMENT                           │
│                   (Bilateral: Party A ↔ Party B)                    │
├─────────────────────────────────────────────────────────────────────┤
│  Schedule        │ Customizations, elections, amendments            │
│  Definitions     │ 2006 ISDA Definitions (or 2021 for rates)        │
│  Confirmations   │ Trade-specific terms                             │
│  CSA (Annex)     │ Credit Support Annex - collateral terms          │
└─────────────────────────────────────────────────────────────────────┘
```

---

## CSA: Credit Support Annex

### What CSA Actually Stands For

**Credit Support Annex** - NOT "Collateral Service Agreement" or "Credit Service Agreement"

The CSA is a voluntary annex to the ISDA Master Agreement that governs collateral arrangements to mitigate counterparty credit risk.

### CSA Key Terms

| Term | Definition |
|------|------------|
| **Eligible Collateral** | What assets can be posted (cash, government bonds, corporate bonds, equities) |
| **Threshold** | Exposure amount allowed before collateral posting required (e.g., $10M) |
| **Minimum Transfer Amount (MTA)** | Smallest collateral movement (e.g., $500K) - avoids operational churn |
| **Independent Amount (IA)** | Additional collateral beyond mark-to-market (trade-specific or counterparty-specific) |
| **Haircut** | Discount applied to non-cash collateral (e.g., 2% for govt bonds, 15% for equities) |
| **Valuation Agent** | Who calculates the exposure (usually the dealer) |
| **Valuation Date/Time** | When MTM is calculated (e.g., 4pm London) |
| **Notification Time** | Deadline for margin calls (e.g., 10am) |
| **Settlement Day** | When collateral must be delivered (T+1 typically) |
| **Dispute Resolution** | Process when parties disagree on valuation |

### VM vs IM CSA

Post-2008 regulations created two distinct collateral regimes:

| Type | Full Name | Purpose | Regulatory Driver |
|------|-----------|---------|-------------------|
| **VM** | Variation Margin | Covers daily MTM changes | EMIR, Dodd-Frank |
| **IM** | Initial Margin | Covers potential future exposure in default | BCBS-IOSCO UMR |

**Critical Distinction**:
- **VM**: Can be rehypothecated (reused by the receiving party)
- **IM**: Must be **segregated** with independent third-party custodian - CANNOT be rehypothecated

This is why IM drives significant custodian involvement.

---

## The Custodian's Role (BNY's Position)

### Key Insight: BNY is NOT Party to the CSA

The CSA is bilateral between trading counterparties. BNY enters through **separate documentation**:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DOCUMENTATION STACK                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ISDA Master + CSA                                                 │
│   (Party A ↔ Party B)                                               │
│        │                                                            │
│        │ References "Custodian" as agent                            │
│        ▼                                                            │
│   ┌─────────────────────────────────────────────────────────┐       │
│   │  CUSTODY AGREEMENT                                      │       │
│   │  (Client ↔ BNY)                                         │       │
│   │  - Safekeeping of assets                                │       │
│   │  - Asset servicing (income, corp actions)               │       │
│   │  - Reporting                                            │       │
│   └─────────────────────────────────────────────────────────┘       │
│        │                                                            │
│        │ For IM segregation (regulatory requirement)                │
│        ▼                                                            │
│   ┌─────────────────────────────────────────────────────────┐       │
│   │  ACCOUNT CONTROL AGREEMENT (ACA)                        │       │
│   │  aka Triparty Agreement / Control Agreement             │       │
│   │  (Pledgor + Secured Party + Custodian)                  │       │
│   │  - Segregation requirements                             │       │
│   │  - Release conditions                                   │       │
│   │  - Default procedures                                   │       │
│   └─────────────────────────────────────────────────────────┘       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Three Ways BNY Participates

#### 1. Custody Agreement (Foundation)
Standard safekeeping and administration of client assets. This exists regardless of derivatives activity.

#### 2. Custodian as Agent (VM Context)
Under standard CSA terms, a party may appoint a custodian as agent to hold collateral on their behalf.

**Key Legal Point**: The pledgor's obligation to transfer collateral is satisfied when assets are delivered to the appointed custodian. This is critical for:
- Settlement finality
- Timing of perfection of security interest
- Operational workflow (client doesn't move assets directly to counterparty)

#### 3. Account Control Agreement (IM Context - Triparty)
For regulatory IM under BCBS-IOSCO Uncleared Margin Rules (UMR):

```
                    ┌─────────────┐
                    │   PLEDGOR   │
                    │ (Collateral │
                    │   Giver)    │
                    └──────┬──────┘
                           │
              Posts IM     │
                           ▼
                    ┌─────────────┐
                    │  CUSTODIAN  │◄─── Account Control Agreement
                    │   (BNY)     │     governs this relationship
                    │ Segregated  │
                    │  Account    │
                    └──────┬──────┘
                           │
              Security     │  On default, Secured Party
              Interest     │  can direct release
                           ▼
                    ┌─────────────┐
                    │  SECURED    │
                    │   PARTY     │
                    │ (Collateral │
                    │  Receiver)  │
                    └─────────────┘
```

The ACA ensures:
- Collateral is legally segregated (bankruptcy remote)
- Secured party has perfected security interest
- Clear procedures for release (return, substitution, default)
- Custodian is protected from conflicting instructions

---

## Operational Flow: Margin Call Lifecycle

```
Day 0 (Trade Date)
    │
    ▼
Day 1+ (Valuation Date)
    │
    ├── 4:00 PM: MTM snapshot taken
    │
    ▼
    Valuation Agent calculates exposure
    │
    ├── Exposure > Threshold + MTA?
    │       │
    │       ├── NO → No call
    │       │
    │       └── YES → Generate margin call
    │
    ▼
Day 2 (Call Date)
    │
    ├── 10:00 AM: Margin call issued
    │
    ▼
    Counterparty receives call
    │
    ├── Agrees? → Proceeds to settlement
    │
    └── Disputes? → Resolution process
    │
    ▼
Day 3 (Settlement Date, T+1 from call)
    │
    ├── Pledgor instructs custodian
    │
    ├── Custodian transfers to:
    │       ├── VM: Counterparty's account (or their custodian)
    │       └── IM: Segregated account at triparty custodian
    │
    └── Confirmation sent to both parties
```

---

## Data Model Implications

### Current Schema Gaps

The `custody.csa_agreements` table exists but may need enrichment:

```sql
-- Current (simplified)
CREATE TABLE custody.csa_agreements (
    csa_id UUID PRIMARY KEY,
    isda_id UUID REFERENCES custody.isda_agreements,
    csa_type VARCHAR(20),  -- 'VM', 'VM_IM'
    threshold_amount DECIMAL,
    threshold_currency VARCHAR(3),
    -- ...
);

-- Should also capture:
ALTER TABLE custody.csa_agreements ADD COLUMN IF NOT EXISTS
    minimum_transfer_amount DECIMAL,
    independent_amount DECIMAL,
    valuation_agent VARCHAR(20),  -- 'PARTY_A', 'PARTY_B', 'BOTH'
    valuation_time TIME,
    valuation_timezone VARCHAR(50),
    notification_time TIME,
    settlement_days INTEGER DEFAULT 1,
    dispute_resolution_method VARCHAR(50);

-- Eligible collateral schedule (complex - needs separate table)
CREATE TABLE custody.csa_eligible_collateral (
    eligible_id UUID PRIMARY KEY,
    csa_id UUID REFERENCES custody.csa_agreements,
    collateral_type VARCHAR(50),  -- 'CASH', 'GOVT_BOND', 'CORP_BOND', 'EQUITY'
    currencies VARCHAR(3)[],
    issuer_constraints JSONB,  -- e.g., {"min_rating": "A-", "countries": ["US", "GB", "DE"]}
    haircut_pct DECIMAL(5,2),
    concentration_limit_pct DECIMAL(5,2),
    is_active BOOLEAN DEFAULT true
);
```

### Triparty/ACA Tracking

For IM segregation, need to track the control agreement:

```sql
CREATE TABLE custody.account_control_agreements (
    aca_id UUID PRIMARY KEY,
    csa_id UUID REFERENCES custody.csa_agreements,
    
    -- Three parties
    pledgor_entity_id UUID REFERENCES "ob-poc".entities,
    secured_party_entity_id UUID REFERENCES "ob-poc".entities,
    custodian_entity_id UUID REFERENCES "ob-poc".entities,  -- Usually BNY
    
    -- Account details
    segregated_account_id UUID REFERENCES custody.accounts,
    account_type VARCHAR(20),  -- 'PLEDGE', 'THIRD_PARTY', 'TRIPARTY'
    
    -- Legal
    governing_law VARCHAR(20),
    effective_date DATE,
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

---

## Visualization Implications

### Trading View: ISDA/CSA Hierarchy

```
ISDA Agreement (Goldman, NY Law)
    │
    ├── CSA (VM)
    │     ├── Threshold: $10M
    │     ├── MTA: $500K
    │     └── Eligible: Cash USD/EUR, UST
    │
    └── CSA (IM) ─────────────────┐
          ├── Threshold: $50M     │
          └── Segregated Account  │
                    │             │
                    ▼             │
              ┌───────────┐       │
              │   ACA     │◄──────┘
              │ (Triparty)│
              ├───────────┤
              │ Pledgor: CBU
              │ Secured: Goldman
              │ Custodian: BNY
              │ Account: XXX-123
              └───────────┘
```

### Node Types to Add

| Node Type | Description | Parent Edge |
|-----------|-------------|-------------|
| `CSA_VM` | Variation Margin CSA | ISDA → CSA_VM |
| `CSA_IM` | Initial Margin CSA | ISDA → CSA_IM |
| `ACA` | Account Control Agreement | CSA_IM → ACA |
| `SEGREGATED_ACCOUNT` | IM segregated custody account | ACA → Account |

### Edge Types

| Edge | From | To | Meaning |
|------|------|-----|---------|
| `ISDA_HAS_VM_CSA` | ISDA | CSA_VM | ISDA has variation margin annex |
| `ISDA_HAS_IM_CSA` | ISDA | CSA_IM | ISDA has initial margin annex |
| `CSA_GOVERNED_BY_ACA` | CSA_IM | ACA | IM segregation control agreement |
| `ACA_USES_ACCOUNT` | ACA | Account | Segregated account for IM |
| `PLEDGOR_IN_ACA` | Entity | ACA | Entity is pledgor (role edge) |
| `SECURED_IN_ACA` | Entity | ACA | Entity is secured party (role edge) |
| `CUSTODIAN_IN_ACA` | Entity | ACA | Entity is custodian (role edge) |

---

## Collateral Optimization (Future Enhancement)

BNY offers collateral optimization services that sit on top of this:

```
┌─────────────────────────────────────────────────────────────────────┐
│                  COLLATERAL OPTIMIZATION                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Client has multiple CSAs with different counterparties             │
│  Each CSA has different eligible collateral schedules               │
│                                                                     │
│  Optimization asks: Given available inventory, what's the           │
│  cheapest-to-deliver collateral allocation across all CSAs?         │
│                                                                     │
│  Factors:                                                           │
│  - Haircuts (lower = more efficient)                                │
│  - Funding cost of assets                                           │
│  - Concentration limits                                             │
│  - Substitution rights                                              │
│  - Operational constraints                                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

This is a service/resource that would appear in the Onboarding view when derived from OTC instrument matrix entries.

---

## Regulatory Context

### Why This Matters (Post-2008)

| Regulation | Jurisdiction | Requirement |
|------------|--------------|-------------|
| **EMIR** | EU | Mandatory VM for OTC derivatives |
| **Dodd-Frank** | US | Mandatory VM, central clearing mandate |
| **BCBS-IOSCO UMR** | Global | Mandatory IM for uncleared OTC derivatives |

**UMR Phase-In** (by AANA threshold):
- Phase 1 (2016): €3T+ 
- Phase 6 (2022): €8B+
- Result: Many more buy-side firms now need IM segregation

This drove massive growth in triparty custody for collateral management - a key BNY business line.

---

## Summary

1. **CSA = Credit Support Annex** (not "Collateral Service Agreement")
2. **CSA is bilateral** between trading counterparties - custodian not a party
3. **BNY enters via separate agreements**: Custody Agreement + Account Control Agreement
4. **VM collateral**: Can be held at counterparty's custodian, rehypothecation allowed
5. **IM collateral**: MUST be segregated at independent custodian, NO rehypothecation
6. **ACA/Triparty Agreement**: Three-way contract (Pledgor + Secured Party + Custodian) for IM
7. **Visualization**: ISDA → CSA_VM / CSA_IM → ACA → Segregated Account

This is foundational knowledge for the Trading View and explains why OTC derivatives in the instrument matrix drive Collateral Management product requirements in Onboarding.

---

## References

- ISDA Master Agreement (2002)
- ISDA Credit Support Annex (English Law / New York Law variants)
- BCBS-IOSCO "Margin requirements for non-centrally cleared derivatives" (2020 revision)
- EMIR Regulatory Technical Standards on margin
- Dodd-Frank Title VII
