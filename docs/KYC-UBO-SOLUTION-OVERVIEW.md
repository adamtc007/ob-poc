# KYC/UBO Solution Overview

## Observation-Based Convergence Model for Beneficial Ownership Verification

**Version:** 1.0  
**Date:** December 2024  
**Status:** Architecture Definition

---

## Executive Summary

This document describes an observation-based approach to Know Your Customer (KYC) and Ultimate Beneficial Owner (UBO) verification that diverges from traditional checkbox-driven compliance workflows. Instead of treating KYC as a form-filling exercise, we model it as a **graph convergence problem** where client allegations must be reconciled with documentary evidence until the ownership model stabilizes.

### Key Differentiators

| Traditional KYC | Observation-Based KYC |
|-----------------|----------------------|
| Flat list of beneficial owners | Graph model with ownership chains |
| "Verified ☑" checkbox | Evidence-linked assertions |
| Analyst judgment (opaque) | Declarative rules (transparent) |
| Point-in-time snapshot | Continuous convergence state |
| Audit: "Who approved" | Audit: "Why approved" (full causation) |

---

## Part 1: Conceptual Model

### 1.1 The Core Insight

KYC is fundamentally about answering: **"Who really owns/controls this client?"**

Traditional systems collect a list of names and percentages. Our model recognizes that:

1. **Ownership is a graph**, not a list (chains of entities)
2. **Client disclosures are allegations**, not facts
3. **Documents provide observations** that may confirm or contradict allegations
4. **Convergence** is achieved when allegations match observations
5. **Decisions** should only be made on converged, evaluated graphs

### 1.2 The Convergence Loop

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONVERGENCE LOOP                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   TRIGGER (onboarding / periodic / event)                                   │
│       │                                                                     │
│       ▼                                                                     │
│   ┌───────────────┐                                                         │
│   │   ALLEGED     │  Client discloses ownership structure                   │
│   │   GRAPH       │  "Fund owned 100% by ManCo, ManCo owned by HoldCo"      │
│   └───────┬───────┘                                                         │
│           │                                                                 │
│           ▼                                                                 │
│   ┌───────────────┐                                                         │
│   │   PROOFS      │  Documents linked to specific edges                     │
│   │   LINKED      │  Shareholder registers, certs of incorporation          │
│   └───────┬───────┘                                                         │
│           │                                                                 │
│           ▼                                                                 │
│   ┌───────────────┐                                                         │
│   │  OBSERVATIONS │  What documents actually say                            │
│   │  EXTRACTED    │  "Register shows: ManCo owns 100% of Fund"              │
│   └───────┬───────┘                                                         │
│           │                                                                 │
│           ▼                                                                 │
│   ┌───────────────┐                                                         │
│   │   COMPARE     │  Allegation vs. Observation per edge                    │
│   │   (VERIFY)    │  Match? → PROVEN | Mismatch? → DISPUTED                 │
│   └───────┬───────┘                                                         │
│           │                                                                 │
│           ▼                                                                 │
│   ┌───────────────┐     ┌─────────────────────────────────────┐             │
│   │  CONVERGED?   │─NO─►│  Surface discrepancies              │             │
│   │               │     │  Request missing proofs             │             │
│   └───────┬───────┘     │  Identify unknown owners            │             │
│           │             │  Loop back to ALLEGED               │             │
│          YES            └─────────────────────────────────────┘             │
│           │                                                                 │
│           ▼                                                                 │
│   ┌───────────────┐                                                         │
│   │   EVALUATE    │  Threshold rules + Red flag checks                      │
│   │               │  (Only on converged graph)                              │
│   └───────┬───────┘                                                         │
│           │                                                                 │
│           ▼                                                                 │
│   ┌───────────────┐                                                         │
│   │   DECISION    │  CLEARED / REJECTED / CONDITIONAL                       │
│   │   + SCHEDULE  │  Set review date for periodic reassessment              │
│   └───────────────┘                                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Graph Model

The ownership structure is modeled as a directed acyclic graph (DAG):

```
                    ┌─────────────────┐
                    │   Allianz SE    │  (Ultimate Parent)
                    │   [company]     │
                    └────────▲────────┘
                             │
                        ownership: 100%
                        proof: register_002
                        status: PROVEN
                             │
                    ┌────────┴────────┐
                    │  Allianz GI     │  (ManCo)
                    │  GmbH [manco]   │
                    └────────▲────────┘
                             │
                        ownership: 100%
                        proof: register_001
                        status: PROVEN
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
    ┌─────┴─────┐      ┌─────┴─────┐      ┌─────┴─────┐
    │  Fund A   │      │  Fund B   │      │  Fund C   │
    │  [fund]   │      │  [fund]   │      │  [fund]   │
    └───────────┘      └───────────┘      └───────────┘
```

**Node Types (Entities):**
- `proper_person` - Natural person
- `limited_company` - Corporate entity
- `partnership` - Partnership structure
- `trust` - Trust arrangement
- `fund` - Investment fund
- `manco` - Management company

**Edge Types:**
- `ownership` - A owns X% of B
- `control` - A controls B (role: CEO, Director, etc.)
- `trust_role` - A is trustee/beneficiary/settlor of Trust T

### 1.4 Edge State Machine

Each edge in the graph progresses through states:

```
┌──────────────────────────────────────────────────────────────────┐
│  EDGE STATES                                                     │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ALLEGED ─────────────────────────────────────────┐              │
│     │                                             │              │
│     │ proof linked                                │ no proof     │
│     ▼                                             │ required     │
│  PENDING ─────────────────────────────────────────┤              │
│     │                                             │              │
│     ├─ proof confirms allegation ─────────────────┤              │
│     │                                             ▼              │
│     │                                          PROVEN ◄──────────│
│     │                                             │              │
│     └─ proof contradicts allegation               │              │
│                    │                              │              │
│                    ▼                              │              │
│                DISPUTED                           │              │
│                    │                              │              │
│                    ├─ client updates allegation ──┘              │
│                    │   (new allegation, re-verify)               │
│                    │                                             │
│                    └─ accept proof as truth ─────►PROVEN         │
│                       (allegation corrected)                     │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 1.5 Assertions as Declarative Gates

Rather than imperative workflow steps, we use **assertions** to declare what must be true:

```clojure
;; These are declarative gates - they don't DO anything,
;; they VERIFY that something is true

(ubo.assert :cbu @cbu :converged true)
;; PASSES: silently continues
;; FAILS:  returns structured error with blocking items

(ubo.assert :cbu @cbu :no-expired-proofs true)
(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)
```

This is the **declarative intent** - the DSL expresses WHAT must be true, not HOW to achieve it.

---

## Part 2: Integration with Client Onboarding

### 2.1 Onboarding Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      CLIENT ONBOARDING FLOW                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. ONBOARDING REQUEST                                                      │
│     └── Client requests services (custody, fund accounting, etc.)           │
│                                                                             │
│  2. CBU CREATION                                                            │
│     └── Client Business Unit created with jurisdiction, products            │
│                                                                             │
│  3. ENTITY REGISTRATION                                                     │
│     └── Fund, ManCo, related entities registered                            │
│     └── Role assignments (ASSET_OWNER, MANAGEMENT_COMPANY, etc.)            │
│                                                                             │
│  4. KYC CASE INITIATION ◄─────────────────────────────────────────────┐     │
│     └── UBO graph initialized (empty, no allegations yet)             │     │
│                                                                       │     │
│  5. ALLEGATION COLLECTION                                             │     │
│     └── Client discloses ownership structure                          │     │
│     └── Edges added to graph with status=ALLEGED                      │     │
│                                                                       │     │
│  6. PROOF COLLECTION                                                  │     │
│     └── Documents requested (RFI if needed)                           │     │
│     └── Documents linked to specific edges                            │     │
│                                                                       │     │
│  7. VERIFICATION                                                      │     │
│     └── Observations extracted from proofs                            │     │
│     └── Compared to allegations                                       │     │
│     └── Discrepancies surfaced                                        │     │
│                                                                       │     │
│  8. CONVERGENCE CHECK                                                 │     │
│     └── All edges PROVEN? Continue : Loop to step 5 ──────────────────┘     │
│                                                                             │
│  9. EVALUATION                                                              │
│     └── Threshold calculation (identify beneficial owners)                  │
│     └── Red flag checks (PEP, sanctions, adverse media)                     │
│                                                                             │
│  10. TOLLGATE (if required)                                                 │
│      └── L1/L2/L3 review based on risk level                                │
│                                                                             │
│  11. DECISION                                                               │
│      └── CLEARED → activate services                                        │
│      └── REJECTED → decline onboarding                                      │
│      └── CONDITIONAL → activate with restrictions                           │
│                                                                             │
│  12. SCHEDULE REVIEW                                                        │
│      └── Set periodic review date (12m standard, 6m enhanced)               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 DSL Integration

The entire onboarding flow is DSL-native:

```clojure
;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 1: CBU SETUP (from template.batch)
;; ═══════════════════════════════════════════════════════════════════════════

(cbu.ensure :name "Allianz Dynamic Multi Asset Strategy" 
            :jurisdiction "LU" 
            :as @cbu)

(cbu.assign-role :cbu @cbu 
                 :entity ("fund" "Allianz Dynamic Multi Asset Strategy")
                 :role "ASSET_OWNER")

(cbu.assign-role :cbu @cbu 
                 :entity ("manco" "Allianz Global Investors GmbH")
                 :role "MANAGEMENT_COMPANY")

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 2: KYC - ALLEGATION
;; ═══════════════════════════════════════════════════════════════════════════

(ubo.allege :cbu @cbu 
            :from ("fund" "Allianz Dynamic Multi Asset Strategy")
            :to ("manco" "Allianz Global Investors GmbH")
            :type "ownership" 
            :percentage 100
            :source "client_disclosure")

(ubo.allege :cbu @cbu 
            :from ("manco" "Allianz Global Investors GmbH")
            :to ("company" "Allianz SE")
            :type "ownership" 
            :percentage 100
            :source "client_disclosure")

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 3: KYC - PROOFS
;; ═══════════════════════════════════════════════════════════════════════════

(ubo.link-proof :cbu @cbu 
                :edge [("fund" "Allianz Dynamic") ("manco" "Allianz GI")]
                :proof @shareholder_register_001
                :proof-type "shareholder_register")

(ubo.link-proof :cbu @cbu 
                :edge [("manco" "Allianz GI") ("company" "Allianz SE")]
                :proof @shareholder_register_002
                :proof-type "shareholder_register")

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 4: KYC - VERIFICATION
;; ═══════════════════════════════════════════════════════════════════════════

(ubo.verify :cbu @cbu :as @verification)

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 5: KYC - ASSERTIONS (Declarative Gates)
;; ═══════════════════════════════════════════════════════════════════════════

(ubo.assert :cbu @cbu :converged true)
(ubo.assert :cbu @cbu :no-expired-proofs true)

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 6: KYC - EVALUATION
;; ═══════════════════════════════════════════════════════════════════════════

(ubo.evaluate :cbu @cbu :as @evaluation)

(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 7: KYC - DECISION
;; ═══════════════════════════════════════════════════════════════════════════

(kyc.decision :cbu @cbu :status "CLEARED" :review-in "12m" :as @decision)

;; ═══════════════════════════════════════════════════════════════════════════
;; PHASE 8: SERVICE ACTIVATION
;; ═══════════════════════════════════════════════════════════════════════════

(cbu.add-product :cbu @cbu :product "CUSTODY")
(cbu.add-product :cbu @cbu :product "FUND_ACCOUNTING")
```

### 2.3 Periodic Review Integration

Periodic review is the **same loop**, triggered by time:

```clojure
;; Triggered by scheduler when review_date reached

;; Mark proofs as potentially stale (need re-verification)
(ubo.mark-dirty :cbu @cbu :reason "periodic_review")

;; Re-run convergence check
(ubo.status :cbu @cbu :as @status)

;; If proofs expired or new requirements, surface them
;; Otherwise, re-evaluate and re-decide

(ubo.assert :cbu @cbu :converged true)
(ubo.assert :cbu @cbu :no-expired-proofs true)
(ubo.evaluate :cbu @cbu :as @evaluation)
(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)
(kyc.decision :cbu @cbu :status "CLEARED" :review-in "12m")
```

### 2.4 Event-Driven Review

Same loop, different trigger:

```clojure
;; Triggered by: corporate registry change, adverse media hit, sanctions update

(ubo.trigger-review :cbu @cbu 
                    :reason "ownership_change_detected"
                    :source "corporate_registry_lu")

;; Same convergence loop follows...
```

---

## Part 3: Data Architecture

### 3.1 Core Tables

```sql
-- ═══════════════════════════════════════════════════════════════════════════
-- ENTITIES (Graph Nodes)
-- ═══════════════════════════════════════════════════════════════════════════

-- Base entity table (existing)
CREATE TABLE entities (
    entity_id UUID PRIMARY KEY,
    entity_type_id UUID REFERENCES entity_types,
    name VARCHAR(255),
    jurisdiction VARCHAR(10),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Entity type extensions: proper_persons, limited_companies, trusts, etc.
-- (existing tables)

-- ═══════════════════════════════════════════════════════════════════════════
-- OWNERSHIP GRAPH (Edges)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE ubo_edges (
    edge_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus,
    
    -- From/To entities
    from_entity_id UUID NOT NULL REFERENCES entities,
    to_entity_id UUID NOT NULL REFERENCES entities,
    
    -- Edge type
    edge_type VARCHAR(20) NOT NULL,  -- 'ownership', 'control', 'trust_role'
    
    -- For ownership edges
    percentage DECIMAL(5,2),
    
    -- For control edges
    control_role VARCHAR(50),  -- 'ceo', 'director', 'senior_manager'
    
    -- For trust role edges
    trust_role VARCHAR(50),    -- 'settlor', 'trustee', 'beneficiary', 'protector'
    interest_type VARCHAR(20), -- 'fixed', 'discretionary'
    
    -- Allegation tracking
    alleged_percentage DECIMAL(5,2),
    alleged_at TIMESTAMPTZ,
    alleged_by UUID,  -- user who recorded allegation
    allegation_source VARCHAR(100),  -- 'client_disclosure', 'public_registry'
    
    -- Proof tracking  
    proven_percentage DECIMAL(5,2),
    proven_at TIMESTAMPTZ,
    proof_id UUID REFERENCES proofs,
    
    -- State
    status VARCHAR(20) NOT NULL DEFAULT 'alleged',
    -- 'alleged', 'pending', 'proven', 'disputed'
    
    -- Discrepancy handling
    discrepancy_notes TEXT,
    resolved_at TIMESTAMPTZ,
    resolved_by UUID,
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(cbu_id, from_entity_id, to_entity_id, edge_type)
);

CREATE INDEX idx_ubo_edges_cbu ON ubo_edges(cbu_id);
CREATE INDEX idx_ubo_edges_status ON ubo_edges(cbu_id, status);

-- ═══════════════════════════════════════════════════════════════════════════
-- PROOFS (Evidence)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE proofs (
    proof_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus,
    
    -- Document reference
    document_id UUID,  -- Reference to document store
    proof_type VARCHAR(50) NOT NULL,
    -- 'passport', 'national_id', 'drivers_license',
    -- 'certificate_of_incorporation', 'shareholder_register',
    -- 'trust_deed', 'partnership_agreement', 'articles_of_association'
    
    -- Validity
    valid_from DATE,
    valid_until DATE,
    is_expired BOOLEAN GENERATED ALWAYS AS (valid_until < CURRENT_DATE) STORED,
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'valid',
    -- 'valid', 'expired', 'dirty', 'superseded'
    
    -- Dirty flag for re-verification
    marked_dirty_at TIMESTAMPTZ,
    dirty_reason VARCHAR(100),
    
    -- Metadata
    uploaded_by UUID,
    uploaded_at TIMESTAMPTZ DEFAULT NOW(),
    verified_by UUID,
    verified_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_proofs_cbu ON proofs(cbu_id);
CREATE INDEX idx_proofs_status ON proofs(cbu_id, status);

-- ═══════════════════════════════════════════════════════════════════════════
-- OBSERVATIONS (What proofs say)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE ubo_observations (
    observation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus,
    proof_id UUID NOT NULL REFERENCES proofs,
    edge_id UUID REFERENCES ubo_edges,  -- Which edge this observation relates to
    
    -- What was observed
    subject_entity_id UUID REFERENCES entities,
    attribute_code VARCHAR(50) NOT NULL,
    -- 'ownership_percentage', 'name', 'date_of_birth', 'address', 'role'
    observed_value JSONB NOT NULL,
    
    -- Extraction metadata
    extracted_from JSONB,  -- {page: 3, section: "shareholders", confidence: 0.95}
    extraction_method VARCHAR(50),  -- 'manual', 'ocr', 'api'
    
    -- Audit
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID
);

CREATE INDEX idx_observations_edge ON ubo_observations(edge_id);
CREATE INDEX idx_observations_proof ON ubo_observations(proof_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- KYC DECISIONS
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE kyc_decisions (
    decision_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus,
    
    -- Decision
    status VARCHAR(20) NOT NULL,  -- 'CLEARED', 'REJECTED', 'CONDITIONAL'
    conditions TEXT,  -- If conditional, what conditions
    
    -- Review scheduling
    review_interval INTERVAL,
    next_review_date DATE,
    
    -- Evaluation snapshot at decision time
    evaluation_snapshot JSONB,
    -- {thresholds: {...}, red_flags: {...}, beneficial_owners: [...]}
    
    -- Audit
    decided_by UUID NOT NULL,
    decided_at TIMESTAMPTZ DEFAULT NOW(),
    decision_rationale TEXT,
    
    -- DSL trace
    dsl_execution_id UUID,  -- Reference to DSL execution that produced this
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_kyc_decisions_cbu ON kyc_decisions(cbu_id);
CREATE INDEX idx_kyc_decisions_review ON kyc_decisions(next_review_date);

-- ═══════════════════════════════════════════════════════════════════════════
-- ASSERTION LOG (Audit Trail)
-- ═══════════════════════════════════════════════════════════════════════════

CREATE TABLE ubo_assertion_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus,
    dsl_execution_id UUID,
    
    -- Assertion details
    assertion_type VARCHAR(50) NOT NULL,
    -- 'converged', 'no-expired-proofs', 'thresholds-pass', 'no-blocking-flags'
    expected_value BOOLEAN NOT NULL,
    actual_value BOOLEAN NOT NULL,
    passed BOOLEAN NOT NULL,
    
    -- If failed, why
    failure_details JSONB,
    -- {blocking_edges: [...], expired_proofs: [...], etc.}
    
    -- Audit
    asserted_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_assertion_log_cbu ON ubo_assertion_log(cbu_id);
CREATE INDEX idx_assertion_log_execution ON ubo_assertion_log(dsl_execution_id);
```

### 3.2 Convergence Calculation

The convergence state is computed, not stored:

```sql
-- View: CBU convergence status
CREATE VIEW ubo_convergence_status AS
SELECT 
    cbu_id,
    COUNT(*) AS total_edges,
    COUNT(*) FILTER (WHERE status = 'proven') AS proven_edges,
    COUNT(*) FILTER (WHERE status = 'alleged') AS alleged_edges,
    COUNT(*) FILTER (WHERE status = 'pending') AS pending_edges,
    COUNT(*) FILTER (WHERE status = 'disputed') AS disputed_edges,
    COUNT(*) FILTER (WHERE status = 'proven') = COUNT(*) AS is_converged
FROM ubo_edges
GROUP BY cbu_id;

-- View: Missing proofs
CREATE VIEW ubo_missing_proofs AS
SELECT 
    e.cbu_id,
    e.edge_id,
    e.from_entity_id,
    e.to_entity_id,
    e.edge_type,
    e.status,
    CASE 
        WHEN e.edge_type = 'ownership' THEN 'shareholder_register'
        WHEN e.edge_type = 'control' THEN 'board_resolution'
        WHEN e.edge_type = 'trust_role' THEN 'trust_deed'
    END AS required_proof_type
FROM ubo_edges e
LEFT JOIN proofs p ON e.proof_id = p.proof_id
WHERE e.status IN ('alleged', 'pending')
  AND (e.proof_id IS NULL OR p.status != 'valid');

-- View: Expired proofs
CREATE VIEW ubo_expired_proofs AS
SELECT 
    p.cbu_id,
    p.proof_id,
    p.proof_type,
    p.valid_until,
    e.edge_id,
    e.from_entity_id,
    e.to_entity_id
FROM proofs p
JOIN ubo_edges e ON e.proof_id = p.proof_id
WHERE p.is_expired = true OR p.status = 'dirty';
```

---

## Part 4: DSL Verb Taxonomy

### 4.1 Graph Building Verbs

| Verb | Purpose | Key Arguments |
|------|---------|---------------|
| `ubo.allege` | Add edge to alleged graph | `:cbu`, `:from`, `:to`, `:type`, `:percentage`, `:source` |
| `ubo.update-allegation` | Modify existing allegation | `:cbu`, `:edge`, `:percentage` |
| `ubo.remove-allegation` | Remove edge from graph | `:cbu`, `:edge`, `:reason` |

### 4.2 Proof Management Verbs

| Verb | Purpose | Key Arguments |
|------|---------|---------------|
| `ubo.link-proof` | Attach proof to edge | `:cbu`, `:edge`, `:proof`, `:proof-type` |
| `ubo.unlink-proof` | Remove proof from edge | `:cbu`, `:edge`, `:reason` |
| `ubo.mark-expired` | Mark proof as expired | `:cbu`, `:proof` |
| `ubo.mark-dirty` | Flag proofs for re-verification | `:cbu`, `:reason` |

### 4.3 Verification Verbs

| Verb | Purpose | Key Arguments |
|------|---------|---------------|
| `ubo.verify` | Compare allegations to proofs | `:cbu` |
| `ubo.extract-observation` | Extract data from proof | `:cbu`, `:proof`, `:edge` |
| `ubo.resolve-dispute` | Handle allegation ≠ observation | `:cbu`, `:edge`, `:resolution` |

### 4.4 Query Verbs

| Verb | Purpose | Returns |
|------|---------|---------|
| `ubo.status` | Full convergence state | `{converged, edges, missing_proofs, ...}` |
| `ubo.traverse` | Walk ownership chain | `{chain, effective_percentage}` |
| `ubo.delta` | What's wrong / what's needed | `{discrepancies, missing, actions}` |

### 4.5 Assertion Verbs

| Verb | Purpose | Behavior |
|------|---------|----------|
| `ubo.assert :converged` | All edges proven | Pass silently or fail with details |
| `ubo.assert :no-expired-proofs` | No stale evidence | Pass silently or fail with list |
| `ubo.assert :thresholds-pass` | Jurisdiction rules met | Pass silently or fail with violations |
| `ubo.assert :no-blocking-flags` | No critical red flags | Pass silently or fail with flags |

### 4.6 Evaluation Verbs

| Verb | Purpose | Returns |
|------|---------|---------|
| `ubo.evaluate` | Run all evaluations | `{thresholds, red_flags, beneficial_owners}` |
| `ubo.evaluate-thresholds` | Calculate BO by jurisdiction | `{beneficial_owners, control_persons}` |
| `ubo.check-red-flags` | Run PEP/sanctions/adverse | `{blocking, warning, info}` |

### 4.7 Decision Verbs

| Verb | Purpose | Key Arguments |
|------|---------|---------------|
| `kyc.decision` | Record final decision | `:cbu`, `:status`, `:review-in` |
| `kyc.schedule-review` | Set next review | `:cbu`, `:in`, `:type` |
| `kyc.trigger-review` | Initiate ad-hoc review | `:cbu`, `:reason`, `:source` |

---

## Part 5: Regulatory Alignment

### 5.1 FATF Recommendations

| FATF Requirement | Our Implementation | Compliance |
|------------------|-------------------|------------|
| **R.10** Customer Due Diligence | Full graph model with verification | ✅ Exceeds |
| **R.10(b)** Identify beneficial owners | Chain traversal with threshold calc | ✅ Aligned |
| **R.10(c)** Verify identity | Proof → Observation → Comparison | ✅ Exceeds |
| **R.11** Record-keeping | DSL audit trail with full causation | ✅ Exceeds |
| **R.12** PEPs | Red flag checks integrated | ✅ Aligned |
| **R.24** Transparency of legal persons | Graph model for complex structures | ✅ Aligned |
| **R.25** Transparency of trusts | Trust role edges (settlor, trustee, beneficiary) | ✅ Aligned |

### 5.2 EU AML Directives (4MLD/5MLD/6MLD)

| Requirement | Our Implementation | Compliance |
|-------------|-------------------|------------|
| 25% BO threshold | Configurable per jurisdiction | ✅ |
| Chain calculation | `ubo.traverse` with aggregation | ✅ |
| Control test | Control edges with roles | ✅ |
| Central register format | Graph exportable to standard format | ✅ |
| Enhanced DD triggers | Red flags → escalation paths | ✅ |
| Ongoing monitoring | Periodic review + event triggers | ✅ |

### 5.3 US FinCEN / BSA

| Requirement | Our Implementation | Compliance |
|-------------|-------------------|------------|
| 25% ownership test | Threshold rules by jurisdiction | ✅ |
| Control prong | Control edges separate from ownership | ✅ |
| Certification | Allegation source tracking | ✅ |
| Verification procedures | Proof types configurable | ✅ |

### 5.4 UK PSC Regime

| Requirement | Our Implementation | Compliance |
|-------------|-------------------|------------|
| >25% shares | Ownership edges with percentage | ✅ |
| >25% voting rights | Can model separately if needed | ⚠️ Extension |
| Right to appoint/remove directors | Control edges | ✅ |
| Significant influence or control | Control edges with roles | ✅ |

### 5.5 Audit Trail Requirements

All regulators require ability to demonstrate:

| Question | Our Answer |
|----------|------------|
| "What was decided?" | `kyc_decisions` table |
| "When?" | `decided_at` timestamp |
| "By whom?" | `decided_by` user reference |
| "Based on what evidence?" | `proof_id` references on edges |
| "What checks were performed?" | `ubo_assertion_log` with pass/fail |
| "Can you reproduce the decision?" | Re-run DSL sequence from `dsl_execution_history` |

---

## Part 6: Implementation Strategy

### 6.1 Phase 1: Core Convergence (Weeks 1-2)

**Goal:** Basic graph building and convergence checking

- [ ] `ubo.allege` verb (create edges)
- [ ] `ubo.link-proof` verb (attach proofs)
- [ ] `ubo.verify` verb (compare allegations to proofs)
- [ ] `ubo.status` verb (convergence calculation)
- [ ] Database migrations for `ubo_edges`, `proofs`, `ubo_observations`

**Test:** Simple 2-level ownership chain, prove via shareholder registers

### 6.2 Phase 2: Assertions (Week 3)

**Goal:** Declarative gates for workflow control

- [ ] `ubo.assert` verb with multiple conditions
- [ ] Assertion logging to `ubo_assertion_log`
- [ ] Structured failure responses

**Test:** Attempt decision on non-converged graph → assertion fails with details

### 6.3 Phase 3: Evaluation (Weeks 4-5)

**Goal:** Threshold calculation and red flag checks

- [ ] `ubo.evaluate-thresholds` verb
- [ ] Jurisdiction-specific threshold rules (config-driven)
- [ ] `ubo.check-red-flags` verb
- [ ] Integration with screening services (PEP, sanctions)
- [ ] `ubo.traverse` for chain calculation

**Test:** Complex ownership chain, calculate effective BO percentages

### 6.4 Phase 4: Templates & Agent (Week 6)

**Goal:** Run books and AI-driven workflow

- [ ] KYC templates (standard-kyc, enhanced-dd)
- [ ] Agent prompt integration
- [ ] `kyc.decision` verb
- [ ] Review scheduling

**Test:** Agent-driven complete KYC flow via chat

### 6.5 Phase 5: Triggers (Weeks 7-8)

**Goal:** Periodic and event-driven reviews

- [ ] `ubo.mark-dirty` verb
- [ ] Scheduler integration for periodic reviews
- [ ] Event hooks for corporate registry changes
- [ ] `kyc.trigger-review` verb

**Test:** Automatic review trigger, re-convergence flow

---

## Part 7: Success Criteria

### 7.1 Functional

- [ ] Ownership graph builds correctly from allegations
- [ ] Proofs link to specific edges
- [ ] Verification compares alleged vs. observed
- [ ] Convergence calculated accurately
- [ ] Assertions gate progression
- [ ] Thresholds calculated per jurisdiction
- [ ] Red flags surface correctly
- [ ] Decisions recorded with full audit trail

### 7.2 Regulatory

- [ ] Regulator can trace: decision ← assertions ← proofs ← allegations
- [ ] Full DSL history reproducible
- [ ] Assertion pass/fail logged with reasons
- [ ] BO identification meets jurisdiction thresholds
- [ ] PEP/sanctions screening integrated

### 7.3 Operational

- [ ] Agent can drive KYC workflow via DSL
- [ ] Templates enable consistent processing
- [ ] Periodic reviews trigger automatically
- [ ] Event-driven reviews possible
- [ ] Discrepancies surface and track to resolution

---

## Appendix A: Example Scenarios

### A.1 Simple Fund (Happy Path)

```
Fund ABC (LU)
  └── 100% owned by ManCo GmbH (DE)
        └── 100% owned by HoldCo SE (DE)
              └── 100% owned by Person X (natural person)

Outcome: Person X is beneficial owner (100% effective ownership)
Decision: CLEARED (assuming no red flags)
```

### A.2 Complex Structure with Multiple BOs

```
Fund XYZ (LU)
  ├── 60% owned by ManCo A
  │     └── 100% owned by Person A
  └── 40% owned by ManCo B
        └── 50% owned by Person B
        └── 50% owned by Person C

Outcome: 
- Person A: 60% effective → BO (>25%)
- Person B: 20% effective → not BO
- Person C: 20% effective → not BO

Decision: CLEARED with Person A as sole BO
```

### A.3 Discrepancy Scenario

```
Allegation: Fund 100% owned by ManCo
Proof: Shareholder register shows 70% ManCo, 30% Unknown Entity

Outcome:
- Edge Fund→ManCo: DISPUTED (alleged 100%, observed 70%)
- Discovery: Unknown 30% owner

Actions:
1. Resolve dispute (accept 70% as truth)
2. Request identification of 30% owner
3. Add new allegation for 30% owner
4. Loop until converged
```

### A.4 Trust Structure

```
Fund ABC (LU)
  └── 100% owned by Family Trust
        ├── Settlor: Founder (deceased)
        ├── Trustee: Trust Corp Ltd
        └── Beneficiaries:
              ├── 50% Person A (fixed)
              └── 50% Person B (fixed)

Outcome:
- Person A: 50% effective → BO (>25%)
- Person B: 50% effective → BO (>25%)
- Trust Corp: Control → Control person

Decision: Enhanced DD required (trust structure)
```

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| **Allegation** | Client's claim about ownership/control |
| **Observation** | What evidence actually shows |
| **Convergence** | State where all allegations match observations |
| **Edge** | Relationship in ownership graph (ownership, control, trust role) |
| **Proof** | Documentary evidence linked to an edge |
| **Assertion** | Declarative gate that must be true to proceed |
| **Threshold** | Jurisdiction-specific percentage for BO determination |
| **Red Flag** | Risk indicator (PEP, sanctions, adverse media) |
| **Tollgate** | Approval checkpoint requiring human review |
| **CBU** | Client Business Unit - the client entity being onboarded |
| **UBO** | Ultimate Beneficial Owner |
| **BO** | Beneficial Owner |
| **PEP** | Politically Exposed Person |

---

*Document maintained by Solution Architecture team. Last updated: December 2024*
