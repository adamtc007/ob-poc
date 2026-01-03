# KYC Control Model

This document describes the ownership and control analysis framework used for Ultimate Beneficial Owner (UBO) identification in the KYC platform.

## Overview

The KYC Control Model implements a comprehensive framework for identifying individuals who ultimately own or control legal entities. This is a regulatory requirement under AML/KYC frameworks including FATF recommendations, EU AMLD, and FinCEN regulations.

## Two Paths to UBO Status

An individual qualifies as a UBO through one of two independent paths:

```
                    ┌─────────────────────────────────────────┐
                    │           UBO QUALIFICATION             │
                    │                                         │
                    │   Path 1: OWNERSHIP    Path 2: CONTROL  │
                    │   ≥25% direct or       Board appoint,   │
                    │   indirect shares      voting rights,   │
                    │                        trust powers     │
                    └─────────────────────────────────────────┘
                              │                    │
                              ▼                    ▼
                    ┌─────────────────┐  ┌─────────────────┐
                    │  Share Registry │  │  Control Rights │
                    │  capital.*      │  │  board.*        │
                    │                 │  │  trust.*        │
                    │                 │  │  partnership.*  │
                    └─────────────────┘  └─────────────────┘
```

### Path 1: Ownership (≥25% Threshold)

Ownership is calculated through share capital:

| Ownership Type | Calculation | Example |
|----------------|-------------|---------|
| Direct | Shares held directly | Person holds 30% of Company A |
| Indirect | Multiplicative through chain | Person → 50% of HoldCo → 60% of OpCo = 30% indirect |
| Combined | Sum of direct + indirect | 15% direct + 12% indirect = 27% total |

**Key Principle**: Voting shares must sum to 100% at each entity (reconciliation).

### Path 2: Control (Regardless of Ownership)

Control can exist without ownership through:

| Control Vector | Description | Typical Threshold |
|----------------|-------------|-------------------|
| Board Appointment Rights | Power to appoint/remove directors | Majority of board |
| Voting Rights | Contractual voting agreements | >50% voting power |
| Veto Rights | Ability to block major decisions | Any veto power |
| GP Status | General Partner in partnership | GP has control regardless of capital |
| Trust Powers | Trustee discretion, protector powers | Discretionary trust |

## Epistemic Model

The system tracks the certainty of ownership/control claims through four states:

```
DISCOVERED → ALLEGED → EVIDENCED → VERIFIED
     │           │           │           │
     ▼           ▼           ▼           ▼
  System      Client      Document    Analyst
  found it    claims it   supports    confirmed
```

### State Transitions

| From | To | Trigger | Example |
|------|-----|---------|---------|
| - | DISCOVERED | System inference | UBO chain tracing finds natural person |
| DISCOVERED | ALLEGED | Client attestation | Client confirms ownership structure |
| ALLEGED | EVIDENCED | Document extraction | Share certificate uploaded |
| EVIDENCED | VERIFIED | Analyst review | Analyst confirms document validity |

### Confidence Scoring

Each state has an associated confidence level:

| State | Base Confidence | Modifiers |
|-------|-----------------|-----------|
| DISCOVERED | 0.3 | +0.1 if from registry lookup |
| ALLEGED | 0.5 | +0.1 if consistent with other data |
| EVIDENCED | 0.7 | +0.1 per corroborating document |
| VERIFIED | 0.9 | +0.1 if registry-confirmed |

## Entity Type Decision Matrix

Different entity types have different control mechanisms:

### Corporations (Limited Companies)

| Control Mechanism | How Measured | DSL Domain |
|-------------------|--------------|------------|
| Share ownership | % of voting shares | `capital.*` |
| Board control | Appointment rights | `board.*` |
| Voting agreements | Contractual rights | `control.*` |

### Partnerships

| Control Mechanism | How Measured | DSL Domain |
|-------------------|--------------|------------|
| GP status | Partner type | `partnership.*` |
| Capital percentage | Capital account | `partnership.*` |
| Profit/loss allocation | P&L ratio | `partnership.*` |

**Key Principle**: GP has presumptive control regardless of capital percentage.

### Trusts

| Control Mechanism | How Measured | DSL Domain |
|-------------------|--------------|------------|
| Trustee discretion | Trust deed provisions | `trust.*` |
| Protector powers | Reserved powers | `trust.*` |
| Appointor power | Power to appoint trustee | `trust.*` |
| Beneficiary interest | Fixed vs discretionary | `trust.*` |

**Key Principle**: Discretionary trusts require identifying persons with effective control, not just named beneficiaries.

### Funds (SICAV, Unit Trust, etc.)

| Control Mechanism | How Measured | DSL Domain |
|-------------------|--------------|------------|
| Management shares | Non-redeemable voting | `capital.*` |
| Investment manager | IM appointment | `fund.*` |
| Board composition | Directors | `board.*` |

## Role-to-Entity-Type Constraints

Not all roles apply to all entity types. The system enforces:

| Role | Valid Entity Types | Category |
|------|-------------------|----------|
| SHAREHOLDER | limited_company, fund | Ownership |
| DIRECTOR | limited_company, fund, manco | Control |
| COMPANY_SECRETARY | limited_company | Executive |
| UBO | proper_person | Terminal |
| GENERAL_PARTNER | partnership | Control |
| LIMITED_PARTNER | partnership | Ownership |
| SETTLOR | trust | Trust Role |
| TRUSTEE | trust | Trust Role |
| BENEFICIARY | trust | Trust Role |
| PROTECTOR | trust | Trust Role |
| INVESTMENT_MANAGER | fund, manco | Fund Management |
| MANAGEMENT_COMPANY | fund | Fund Management |

### Validation Rules

```
IF role.category IN ('OWNERSHIP_CHAIN', 'CONTROL_CHAIN')
   AND role.name = 'UBO'
THEN entity.type MUST BE 'proper_person'

IF role.name IN ('GENERAL_PARTNER', 'LIMITED_PARTNER')
THEN target_entity.type MUST BE 'partnership'

IF role.name IN ('SETTLOR', 'TRUSTEE', 'BENEFICIARY', 'PROTECTOR')
THEN target_entity.type MUST BE 'trust'
```

## Capital Structure Model

### Share Classes

Share classes define ownership rights:

```yaml
share_class:
  name: "Ordinary Shares"
  share_type: ORDINARY | PREFERENCE | MANAGEMENT | CARRIED_INTEREST
  voting_rights_per_share: 1.0  # Can be 0 for non-voting
  issued_shares: 1000000
  par_value: 0.01
  currency: EUR
```

### Holdings

Holdings track who owns what:

```yaml
holding:
  holder_entity_id: uuid
  share_class_id: uuid
  shares_held: 300000
  ownership_percentage: 30.0  # Calculated: shares_held / issued_shares
  verification_status: ALLEGED | EVIDENCED | VERIFIED
  acquisition_date: 2024-01-15
```

### Reconciliation Principle

At each entity, voting shares must sum to 100%:

```sql
SELECT issuer_entity_id,
       SUM(shares_held) as total_held,
       sc.issued_shares,
       CASE WHEN SUM(shares_held) = sc.issued_shares THEN 'RECONCILED'
            WHEN SUM(shares_held) < sc.issued_shares THEN 'UNALLOCATED_SHARES'
            WHEN SUM(shares_held) > sc.issued_shares THEN 'OVER_ALLOCATED'
       END as status
FROM holdings h
JOIN share_classes sc ON h.share_class_id = sc.id
GROUP BY issuer_entity_id, sc.issued_shares
```

## Board Composition Model

### Board Positions

```yaml
board_composition:
  entity_id: uuid  # The company
  person_entity_id: uuid  # The director
  position_type: CHAIR | EXECUTIVE_DIRECTOR | NON_EXECUTIVE | INDEPENDENT
  committee_memberships: [AUDIT, REMUNERATION, NOMINATION, RISK]
  voting_weight: 1.0  # Usually 1, but can vary
  appointed_by_entity_id: uuid  # Who appointed this director
```

### Appointment Rights

Appointment rights grant control without ownership:

```yaml
appointment_right:
  granting_entity_id: uuid  # Who grants the right
  receiving_entity_id: uuid  # Who receives the right
  right_type: BOARD_APPOINTMENT | BOARD_REMOVAL | VETO | RESERVED_MATTER
  positions_controllable: 3  # Number of directors appointable
  requires_consent: false
```

### Control Calculation

```
Board Control = (Positions Appointable / Total Board Seats) × 100

IF Board Control > 50% THEN entity has BOARD_CONTROL
IF Veto Rights exist THEN entity has NEGATIVE_CONTROL
```

## Trust Provisions Model

### Trust Roles

```yaml
trust_provision:
  trust_entity_id: uuid
  role_holder_entity_id: uuid
  role_type: SETTLOR | TRUSTEE | PROTECTOR | BENEFICIARY | ENFORCER
  discretion_level: ABSOLUTE | LIMITED | NONE
  interest_type: FIXED | DISCRETIONARY | CONTINGENT  # For beneficiaries
  interest_percentage: 25.0  # For fixed beneficiaries
```

### Trust Control Analysis

| Role | Control Indicator | UBO Treatment |
|------|-------------------|---------------|
| Trustee | Full discretion | May be UBO if absolute discretion |
| Protector | Veto powers | UBO if can replace trustee |
| Settlor | Reserved powers | UBO if powers retained |
| Fixed Beneficiary | ≥25% interest | UBO by ownership |
| Discretionary Beneficiary | Potential benefit | Depends on class size |

## Partnership Capital Model

### Partner Types

```yaml
partnership_capital:
  partnership_entity_id: uuid
  partner_entity_id: uuid
  partner_type: GENERAL_PARTNER | LIMITED_PARTNER | SPECIAL_LIMITED_PARTNER
  capital_committed: 10000000.00
  capital_contributed: 7500000.00
  capital_percentage: 15.0
  profit_share_percentage: 20.0  # May differ from capital
  has_management_rights: true  # GP always true, LP usually false
```

### GP Control Rule

```
IF partner_type = 'GENERAL_PARTNER'
THEN has_control = TRUE  # Regardless of capital percentage

Control flows from GP → GP's owners
```

### LP Ownership

Limited Partners are treated like shareholders for ownership purposes:

```
IF partner_type = 'LIMITED_PARTNER'
   AND capital_percentage >= 25%
THEN qualifies_as_ubo = TRUE  # By ownership path
```

## Tollgate Decision Engine

Tollgates are checkpoints in the KYC workflow that evaluate verification completeness.

### Tollgate Types

| Type | When Evaluated | Pass Criteria |
|------|----------------|---------------|
| DISCOVERY_COMPLETE | After UBO tracing | All ownership chains traced to natural persons |
| EVIDENCE_COMPLETE | After document collection | All required docs for each UBO collected |
| VERIFICATION_COMPLETE | After analyst review | All UBOs verified, no unresolved discrepancies |
| DECISION_READY | Before final decision | All tollgates passed, risk assessment complete |
| PERIODIC_REVIEW | Scheduled trigger | No material changes, all data current |

### Evaluation Metrics

```yaml
tollgate_evaluation:
  case_id: uuid
  evaluation_type: DISCOVERY_COMPLETE
  metrics:
    total_ubos_found: 3
    ubos_verified: 2
    ubos_pending: 1
    ownership_reconciled: true
    control_mapped: true
    unresolved_discrepancies: 0
  threshold:
    min_ubos_verified_pct: 100
    ownership_must_reconcile: true
    max_unresolved_discrepancies: 0
  result: PASS | FAIL | OVERRIDE
  evaluated_at: timestamp
```

### Threshold Configuration

Thresholds are configurable per jurisdiction and risk level:

```yaml
tollgate_threshold:
  jurisdiction: LU
  risk_level: HIGH
  tollgate_type: VERIFICATION_COMPLETE
  thresholds:
    min_verified_percentage: 100  # All UBOs must be verified
    ownership_reconciliation_required: true
    control_mapping_required: true
    max_epistemic_gaps: 0  # No DISCOVERED-only items
    min_confidence_score: 0.85
```

### Override Mechanism

When business necessity requires proceeding despite failed tollgate:

```yaml
tollgate_override:
  evaluation_id: uuid
  reason: "Time-critical transaction, mitigating controls in place"
  approved_by: analyst_id
  expiry_date: 2024-03-31  # Overrides are time-limited
  conditions:
    - "Enhanced monitoring for 90 days"
    - "Second-line review within 30 days"
```

## Unified Control Analysis

The `control.analyze` verb provides comprehensive analysis across all vectors:

```lisp
(control.analyze
    :entity-id @target-company
    :include-ownership true
    :include-board true
    :include-trust-roles true
    :include-partnership true
    :depth 5)
```

### Output Structure

```yaml
control_analysis:
  entity_id: uuid
  analysis_date: timestamp
  
  ownership_path:
    ubos_by_ownership:
      - person_id: uuid
        effective_ownership: 35.0
        chain: [company_a, holdco_b, person]
        verification_status: VERIFIED
        
  control_path:
    ubos_by_control:
      - person_id: uuid
        control_type: BOARD_APPOINTMENT
        control_description: "Appoints 3 of 5 directors"
        verification_status: EVIDENCED
        
  combined_ubos:
    - person_id: uuid
      qualifies_via: [OWNERSHIP, CONTROL]
      highest_confidence: 0.92
      
  reconciliation_status:
    all_shares_allocated: true
    board_fully_mapped: true
    trust_roles_complete: true
    
  epistemic_summary:
    discovered: 1
    alleged: 0
    evidenced: 2
    verified: 3
```

## DSL Verb Reference

### Capital Domain

| Verb | Description |
|------|-------------|
| `capital.define-share-class` | Define a share class for an entity |
| `capital.record-issuance` | Record share issuance event |
| `capital.record-transfer` | Record share transfer between holders |
| `capital.record-redemption` | Record share redemption |
| `capital.get-cap-table` | Get current capitalization table |
| `capital.reconcile` | Verify shares sum to 100% |
| `capital.calculate-ownership` | Calculate effective ownership through chains |

### Board Domain

| Verb | Description |
|------|-------------|
| `board.appoint-director` | Appoint a director to a board |
| `board.remove-director` | Remove a director from a board |
| `board.grant-appointment-right` | Grant board appointment rights |
| `board.revoke-appointment-right` | Revoke board appointment rights |
| `board.get-composition` | Get current board composition |
| `board.analyze-control` | Analyze who controls the board |
| `board.list-appointment-rights` | List all appointment rights |

### Trust Domain

| Verb | Description |
|------|-------------|
| `trust.add-role` | Add a trust role holder |
| `trust.update-role` | Update trust role details |
| `trust.remove-role` | Remove a trust role holder |
| `trust.list-roles` | List all roles for a trust |
| `trust.analyze-control` | Determine who controls the trust |
| `trust.get-beneficiary-interests` | Get beneficiary interest breakdown |
| `trust.identify-ubos` | Identify UBOs from trust structure |

### Partnership Domain

| Verb | Description |
|------|-------------|
| `partnership.add-partner` | Add a partner with capital commitment |
| `partnership.record-contribution` | Record capital contribution |
| `partnership.record-distribution` | Record capital distribution |
| `partnership.withdraw-partner` | Withdraw a partner |
| `partnership.list-partners` | List all partners |
| `partnership.reconcile` | Verify capital accounts balance |
| `partnership.analyze-control` | Analyze GP control chain |

### Tollgate Domain

| Verb | Description |
|------|-------------|
| `tollgate.evaluate` | Run tollgate evaluation |
| `tollgate.get-metrics` | Get evaluation metrics |
| `tollgate.set-threshold` | Set tollgate thresholds |
| `tollgate.override` | Override failed tollgate |
| `tollgate.list-evaluations` | List evaluations for a case |
| `tollgate.get-decision-readiness` | Check if case is decision-ready |

### Control Domain (Unified)

| Verb | Description |
|------|-------------|
| `control.analyze` | Comprehensive control analysis |
| `control.build-graph` | Build full control graph |
| `control.identify-ubos` | Identify all UBOs across vectors |
| `control.trace-chain` | Trace specific control chain |
| `control.reconcile-ownership` | Reconcile all ownership data |

## Implementation Notes

### Database Schema

The control model uses the following key tables:

| Table | Schema | Purpose |
|-------|--------|---------|
| `share_classes` | kyc | Share class definitions |
| `capital_holdings` | kyc | Who owns what shares |
| `capital_events` | kyc | Share issuance/transfer history |
| `board_compositions` | kyc | Director positions |
| `appointment_rights` | kyc | Board appointment rights |
| `trust_provisions` | kyc | Trust role assignments |
| `partnership_capital` | kyc | Partner capital accounts |
| `tollgate_evaluations` | kyc | Tollgate evaluation results |
| `tollgate_thresholds` | kyc | Configurable thresholds |
| `tollgate_overrides` | kyc | Override records |

### Integration Points

1. **UBO Domain**: Control analysis feeds into `ubo.register-ubo` and `ubo.verify-ubo`
2. **Observation Domain**: Epistemic states map to `observation.record` confidence scores
3. **Verification Domain**: Control analysis triggers verification challenges
4. **KYC Case Domain**: Tollgate evaluations drive case workflow

### Performance Considerations

- Ownership chain tracing is O(n) where n = chain depth (typically ≤10)
- Board control calculation is O(1) per entity
- Reconciliation is O(m) where m = number of share classes
- Full control graph build is O(e) where e = total entities in CBU

## Regulatory Alignment

This model supports compliance with:

| Regulation | Requirement | How Met |
|------------|-------------|---------|
| EU AMLD 5/6 | 25% ownership threshold | Configurable via tollgate thresholds |
| FATF R.10 | Identify natural persons | `control.identify-ubos` terminates at persons |
| UK PSC Register | Control definition | Board appointment rights + voting |
| US FinCEN CDD | Beneficial ownership | Combined ownership + control paths |
| CSSF Circular 17/650 | Fund UBO identification | Trust + fund management roles |
