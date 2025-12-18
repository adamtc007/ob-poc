# TODO: KYC/UBO Convergence Test Harness

## ⛔ MANDATORY FIRST STEP

**Before writing ANY code, read:**
- `/docs/KYC-UBO-SOLUTION-OVERVIEW.md` - Full solution architecture
- `/TODO-KYC-UBO-CONVERGENCE.md` - Verb specifications
- `/rust/src/bin/batch_test_harness.rs` - Reference for CLAP harness pattern

---

## Objective

Create an `xtask` command and test harness to validate the KYC/UBO convergence implementation using three progressively complex scenarios:

1. **Simple Fund** - Allianz ManCo → Fund (2-level, 100% ownership)
2. **Hedge Fund LLP** - Multiple partners, complex percentages
3. **Trust Structure** - Settlor, trustee, beneficiaries with discretionary interests

Each scenario exercises the full convergence loop:
```
Allege → Link Proofs → Verify → Assert Converged → Evaluate → Decision
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  xtask ubo-test                                                             │
│  ├── scenario-1  (Simple Fund)                                              │
│  ├── scenario-2  (Hedge Fund LLP)                                           │
│  ├── scenario-3  (Trust Structure)                                          │
│  ├── all         (Run all scenarios)                                        │
│  └── clean       (Remove test data)                                         │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  rust/src/bin/ubo_test_harness.rs                                           │
│  ├── Setup: Create entities, CBU, mock proofs                               │
│  ├── Execute: Run DSL sequence via server                                   │
│  ├── Verify: Check convergence state, assertions, decision                  │
│  └── Report: JSON output for CI integration                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Scenario 1: Simple Allianz Fund

### Ownership Structure

```
┌─────────────────────────┐
│      Allianz SE         │  Ultimate Parent (DE)
│   [limited_company]     │
└───────────▲─────────────┘
            │
       ownership: 100%
       status: → PROVEN
            │
┌───────────┴─────────────┐
│  Allianz Global         │  ManCo (DE)
│  Investors GmbH         │
│   [manco]               │
└───────────▲─────────────┘
            │
       ownership: 100%
       status: → PROVEN
            │
┌───────────┴─────────────┐
│  Allianz Dynamic        │  Fund (LU)
│  Multi Asset Strategy   │
│   [fund]                │
└─────────────────────────┘
         CBU
```

### Test Data

```json
{
  "scenario": "simple_fund",
  "entities": [
    {
      "type": "limited_company",
      "name": "Allianz SE",
      "jurisdiction": "DE",
      "ref": "allianz_se"
    },
    {
      "type": "manco", 
      "name": "Allianz Global Investors GmbH",
      "jurisdiction": "DE",
      "ref": "allianz_gi"
    },
    {
      "type": "fund",
      "name": "Allianz Dynamic Multi Asset Strategy",
      "jurisdiction": "LU",
      "ref": "allianz_fund"
    }
  ],
  "cbu": {
    "name": "Allianz Dynamic Multi Asset Strategy",
    "jurisdiction": "LU",
    "ref": "cbu_allianz"
  },
  "allegations": [
    {
      "from": "allianz_fund",
      "to": "allianz_gi",
      "type": "ownership",
      "percentage": 100
    },
    {
      "from": "allianz_gi", 
      "to": "allianz_se",
      "type": "ownership",
      "percentage": 100
    }
  ],
  "proofs": [
    {
      "ref": "proof_1",
      "type": "shareholder_register",
      "edge": ["allianz_fund", "allianz_gi"],
      "observed_percentage": 100
    },
    {
      "ref": "proof_2",
      "type": "shareholder_register", 
      "edge": ["allianz_gi", "allianz_se"],
      "observed_percentage": 100
    }
  ],
  "expected": {
    "converged": true,
    "beneficial_owners": [],
    "decision": "CLEARED"
  }
}
```

### DSL Sequence

```clojure
;; ═══════════════════════════════════════════════════════════════════════════
;; SCENARIO 1: Simple Allianz Fund
;; Expected: Full convergence, no BOs (corporate chain), CLEARED
;; ═══════════════════════════════════════════════════════════════════════════

;; Phase 1: Setup (entities should exist from seed)
(cbu.ensure :name "Allianz Dynamic Multi Asset Strategy" 
            :jurisdiction "LU" 
            :as @cbu)

;; Phase 2: Allegations
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

;; Phase 3: Link Proofs
(ubo.link-proof :cbu @cbu 
                :edge [("fund" "Allianz Dynamic") ("manco" "Allianz GI")]
                :proof @proof_1
                :proof-type "shareholder_register")

(ubo.link-proof :cbu @cbu 
                :edge [("manco" "Allianz GI") ("company" "Allianz SE")]
                :proof @proof_2
                :proof-type "shareholder_register")

;; Phase 4: Verify
(ubo.verify :cbu @cbu :as @verify_result)

;; Phase 5: Check Status
(ubo.status :cbu @cbu :as @status)
;; Expected: converged = true

;; Phase 6: Assertions
(ubo.assert :cbu @cbu :converged true)
(ubo.assert :cbu @cbu :no-expired-proofs true)

;; Phase 7: Evaluate
(ubo.evaluate :cbu @cbu :as @eval)
;; Expected: No natural person BOs (corporate chain terminates at Allianz SE)

(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)

;; Phase 8: Decision
(kyc.decision :cbu @cbu :status "CLEARED" :review-in "12m" :as @decision)
```

### Verification Points

- [ ] 2 edges created with status='alleged'
- [ ] After link-proof: status='pending'
- [ ] After verify: status='proven' (both edges)
- [ ] `ubo.status` returns `converged: true`
- [ ] `ubo.assert :converged true` passes
- [ ] `ubo.evaluate` identifies no natural person BOs
- [ ] `kyc.decision` records CLEARED
- [ ] Assertion log contains 4 passed assertions

---

## Scenario 2: Hedge Fund LLP (Complex)

### Ownership Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         APEX CAPITAL LLP                                    │
│                         [partnership]                                       │
│                         Jurisdiction: UK                                    │
└───────────────────────────────▲─────────────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   ownership: 45%          ownership: 35%          ownership: 20%
   status: PROVEN          status: PROVEN          status: PROVEN
        │                       │                       │
┌───────┴───────┐       ┌───────┴───────┐       ┌───────┴───────┐
│ Wellington    │       │ Sarah Chen    │       │ Marcus Webb   │
│ Partners Ltd  │       │ [person]      │       │ [person]      │
│ [company]     │       │ UK            │       │ UK            │
└───────▲───────┘       └───────────────┘       └───────────────┘
        │                    BO: 35%                 BO: 20%
   ownership: 100%
   status: PROVEN
        │
┌───────┴───────┐
│ James         │
│ Wellington    │
│ [person] UK   │
└───────────────┘
    BO: 45%


                                │
                                │ manages
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    APEX GLOBAL OPPORTUNITIES FUND                           │
│                    [fund]                                                   │
│                    Jurisdiction: Cayman Islands                             │
│                                  CBU                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Test Data

```json
{
  "scenario": "hedge_fund_llp",
  "entities": [
    {
      "type": "fund",
      "name": "Apex Global Opportunities Fund",
      "jurisdiction": "KY",
      "ref": "apex_fund"
    },
    {
      "type": "partnership",
      "name": "Apex Capital LLP", 
      "jurisdiction": "UK",
      "ref": "apex_llp"
    },
    {
      "type": "limited_company",
      "name": "Wellington Partners Ltd",
      "jurisdiction": "UK",
      "ref": "wellington_ltd"
    },
    {
      "type": "proper_person",
      "name": "James Wellington",
      "jurisdiction": "UK",
      "ref": "james_wellington"
    },
    {
      "type": "proper_person",
      "name": "Sarah Chen",
      "jurisdiction": "UK",
      "ref": "sarah_chen"
    },
    {
      "type": "proper_person",
      "name": "Marcus Webb",
      "jurisdiction": "UK",
      "ref": "marcus_webb"
    }
  ],
  "cbu": {
    "name": "Apex Global Opportunities Fund",
    "jurisdiction": "KY",
    "ref": "cbu_apex"
  },
  "allegations": [
    {
      "from": "apex_fund",
      "to": "apex_llp",
      "type": "ownership",
      "percentage": 100,
      "note": "Fund owned by IM partnership"
    },
    {
      "from": "apex_llp",
      "to": "wellington_ltd",
      "type": "ownership",
      "percentage": 45
    },
    {
      "from": "apex_llp",
      "to": "sarah_chen",
      "type": "ownership",
      "percentage": 35
    },
    {
      "from": "apex_llp",
      "to": "marcus_webb",
      "type": "ownership",
      "percentage": 20
    },
    {
      "from": "wellington_ltd",
      "to": "james_wellington",
      "type": "ownership",
      "percentage": 100
    }
  ],
  "control": [
    {
      "from": "apex_llp",
      "to": "sarah_chen",
      "type": "control",
      "role": "managing_partner"
    }
  ],
  "proofs": [
    {
      "ref": "proof_fund_llp",
      "type": "partnership_agreement",
      "edge": ["apex_fund", "apex_llp"],
      "observed_percentage": 100
    },
    {
      "ref": "proof_llp_wellington",
      "type": "partnership_agreement",
      "edge": ["apex_llp", "wellington_ltd"],
      "observed_percentage": 45
    },
    {
      "ref": "proof_llp_chen",
      "type": "partnership_agreement",
      "edge": ["apex_llp", "sarah_chen"],
      "observed_percentage": 35
    },
    {
      "ref": "proof_llp_webb",
      "type": "partnership_agreement",
      "edge": ["apex_llp", "marcus_webb"],
      "observed_percentage": 20
    },
    {
      "ref": "proof_wellington_james",
      "type": "shareholder_register",
      "edge": ["wellington_ltd", "james_wellington"],
      "observed_percentage": 100
    }
  ],
  "expected": {
    "converged": true,
    "beneficial_owners": [
      {"name": "James Wellington", "effective_percentage": 45},
      {"name": "Sarah Chen", "effective_percentage": 35}
    ],
    "control_persons": [
      {"name": "Sarah Chen", "role": "managing_partner"}
    ],
    "below_threshold": [
      {"name": "Marcus Webb", "effective_percentage": 20}
    ],
    "decision": "CLEARED"
  }
}
```

### DSL Sequence

```clojure
;; ═══════════════════════════════════════════════════════════════════════════
;; SCENARIO 2: Hedge Fund LLP
;; Expected: 2 BOs (James 45%, Sarah 35%), 1 control person, CLEARED
;; ═══════════════════════════════════════════════════════════════════════════

;; Phase 1: Setup
(cbu.ensure :name "Apex Global Opportunities Fund" 
            :jurisdiction "KY" 
            :as @cbu)

;; Phase 2: Allegations - Ownership
(ubo.allege :cbu @cbu 
            :from ("fund" "Apex Global Opportunities Fund")
            :to ("partnership" "Apex Capital LLP")
            :type "ownership" 
            :percentage 100
            :source "client_disclosure")

(ubo.allege :cbu @cbu 
            :from ("partnership" "Apex Capital LLP")
            :to ("company" "Wellington Partners Ltd")
            :type "ownership" 
            :percentage 45
            :source "client_disclosure")

(ubo.allege :cbu @cbu 
            :from ("partnership" "Apex Capital LLP")
            :to ("person" "Sarah Chen")
            :type "ownership" 
            :percentage 35
            :source "client_disclosure")

(ubo.allege :cbu @cbu 
            :from ("partnership" "Apex Capital LLP")
            :to ("person" "Marcus Webb")
            :type "ownership" 
            :percentage 20
            :source "client_disclosure")

(ubo.allege :cbu @cbu 
            :from ("company" "Wellington Partners Ltd")
            :to ("person" "James Wellington")
            :type "ownership" 
            :percentage 100
            :source "client_disclosure")

;; Phase 2b: Allegations - Control
(ubo.allege :cbu @cbu 
            :from ("partnership" "Apex Capital LLP")
            :to ("person" "Sarah Chen")
            :type "control" 
            :role "managing_partner"
            :source "partnership_agreement")

;; Phase 3: Link Proofs
(ubo.link-proof :cbu @cbu 
                :edge [("fund" "Apex Global") ("partnership" "Apex Capital")]
                :proof @proof_fund_llp
                :proof-type "partnership_agreement")

(ubo.link-proof :cbu @cbu 
                :edge [("partnership" "Apex Capital") ("company" "Wellington Partners")]
                :proof @proof_llp_wellington
                :proof-type "partnership_agreement")

(ubo.link-proof :cbu @cbu 
                :edge [("partnership" "Apex Capital") ("person" "Sarah Chen")]
                :proof @proof_llp_chen
                :proof-type "partnership_agreement")

(ubo.link-proof :cbu @cbu 
                :edge [("partnership" "Apex Capital") ("person" "Marcus Webb")]
                :proof @proof_llp_webb
                :proof-type "partnership_agreement")

(ubo.link-proof :cbu @cbu 
                :edge [("company" "Wellington Partners") ("person" "James Wellington")]
                :proof @proof_wellington_james
                :proof-type "shareholder_register")

;; Phase 4: Verify
(ubo.verify :cbu @cbu :as @verify_result)

;; Phase 5: Status
(ubo.status :cbu @cbu :as @status)

;; Phase 6: Assertions
(ubo.assert :cbu @cbu :converged true)
(ubo.assert :cbu @cbu :no-expired-proofs true)

;; Phase 7: Evaluate
(ubo.evaluate :cbu @cbu :as @eval)
;; Expected BOs (>25%): James Wellington (45%), Sarah Chen (35%)
;; Expected control: Sarah Chen (managing_partner)
;; Below threshold: Marcus Webb (20%)

(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)

;; Phase 8: Decision
(kyc.decision :cbu @cbu :status "CLEARED" :review-in "12m" :as @decision)
```

### Verification Points

- [ ] 6 ownership edges + 1 control edge created
- [ ] All edges verified and proven
- [ ] `ubo.traverse` calculates: James=45%, Sarah=35%, Marcus=20%
- [ ] 2 beneficial owners identified (>25% threshold)
- [ ] 1 control person identified (managing_partner)
- [ ] Marcus Webb below threshold (20%)
- [ ] Decision: CLEARED

---

## Scenario 3: Trust Structure

### Ownership Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     HARTLEY FAMILY TRUST                                    │
│                     [trust]                                                 │
│                     Jurisdiction: Jersey                                    │
└───────────────────────────────▲─────────────────────────────────────────────┘
                                │
    ┌───────────────────────────┼───────────────────────────────┐
    │                           │                               │
 settlor                    trustee                      beneficiaries
    │                           │                               │
┌───┴───────────┐       ┌───────┴───────┐       ┌───────────────┴───────────────┐
│ Edward        │       │ Channel       │       │                               │
│ Hartley       │       │ Islands       │       │    ┌───────────┐  ┌───────────┐
│ [person]      │       │ Trust Co Ltd  │       │    │ Victoria  │  │ William   │
│ (deceased)    │       │ [company]     │       │    │ Hartley   │  │ Hartley   │
└───────────────┘       └───────────────┘       │    │ [person]  │  │ [person]  │
                              │                 │    │ 50% fixed │  │ 50% fixed │
                         control                │    └───────────┘  └───────────┘
                              │                 │       BO: 50%        BO: 50%
                              ▼                 │
                        ┌───────────┐           │
                        │ Margaret  │◄──────────┘
                        │ Thompson  │     protector
                        │ [person]  │
                        └───────────┘
                                │
                                │ trust owns 100%
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    HARTLEY CAPITAL FUND                                     │
│                    [fund]                                                   │
│                    Jurisdiction: Jersey                                     │
│                                  CBU                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Test Data

```json
{
  "scenario": "trust_structure",
  "entities": [
    {
      "type": "fund",
      "name": "Hartley Capital Fund",
      "jurisdiction": "JE",
      "ref": "hartley_fund"
    },
    {
      "type": "trust",
      "name": "Hartley Family Trust",
      "jurisdiction": "JE",
      "ref": "hartley_trust"
    },
    {
      "type": "proper_person",
      "name": "Edward Hartley",
      "jurisdiction": "UK",
      "status": "deceased",
      "ref": "edward_hartley"
    },
    {
      "type": "limited_company",
      "name": "Channel Islands Trust Co Ltd",
      "jurisdiction": "JE",
      "ref": "trustee_co"
    },
    {
      "type": "proper_person",
      "name": "Victoria Hartley",
      "jurisdiction": "UK",
      "ref": "victoria_hartley"
    },
    {
      "type": "proper_person",
      "name": "William Hartley",
      "jurisdiction": "UK",
      "ref": "william_hartley"
    },
    {
      "type": "proper_person",
      "name": "Margaret Thompson",
      "jurisdiction": "UK",
      "ref": "margaret_thompson"
    }
  ],
  "cbu": {
    "name": "Hartley Capital Fund",
    "jurisdiction": "JE",
    "ref": "cbu_hartley"
  },
  "allegations": [
    {
      "from": "hartley_fund",
      "to": "hartley_trust",
      "type": "ownership",
      "percentage": 100
    }
  ],
  "trust_roles": [
    {
      "trust": "hartley_trust",
      "person": "edward_hartley",
      "role": "settlor"
    },
    {
      "trust": "hartley_trust",
      "company": "trustee_co",
      "role": "trustee"
    },
    {
      "trust": "hartley_trust",
      "person": "victoria_hartley",
      "role": "beneficiary",
      "interest_type": "fixed",
      "percentage": 50
    },
    {
      "trust": "hartley_trust",
      "person": "william_hartley",
      "role": "beneficiary",
      "interest_type": "fixed",
      "percentage": 50
    },
    {
      "trust": "hartley_trust",
      "person": "margaret_thompson",
      "role": "protector"
    }
  ],
  "proofs": [
    {
      "ref": "proof_fund_trust",
      "type": "trust_deed",
      "edge": ["hartley_fund", "hartley_trust"],
      "observed_percentage": 100
    },
    {
      "ref": "proof_trust_deed",
      "type": "trust_deed",
      "note": "Covers all trust roles"
    }
  ],
  "expected": {
    "converged": true,
    "beneficial_owners": [
      {"name": "Victoria Hartley", "effective_percentage": 50, "via": "beneficiary_fixed"},
      {"name": "William Hartley", "effective_percentage": 50, "via": "beneficiary_fixed"}
    ],
    "control_persons": [
      {"name": "Channel Islands Trust Co Ltd", "role": "trustee"},
      {"name": "Margaret Thompson", "role": "protector"}
    ],
    "decision": "CLEARED",
    "enhanced_dd": true,
    "reason": "trust_structure"
  }
}
```

### DSL Sequence

```clojure
;; ═══════════════════════════════════════════════════════════════════════════
;; SCENARIO 3: Trust Structure
;; Expected: 2 BOs (beneficiaries), 2 control persons, Enhanced DD, CLEARED
;; ═══════════════════════════════════════════════════════════════════════════

;; Phase 1: Setup
(cbu.ensure :name "Hartley Capital Fund" 
            :jurisdiction "JE" 
            :as @cbu)

;; Phase 2: Allegations - Ownership (Fund → Trust)
(ubo.allege :cbu @cbu 
            :from ("fund" "Hartley Capital Fund")
            :to ("trust" "Hartley Family Trust")
            :type "ownership" 
            :percentage 100
            :source "client_disclosure")

;; Phase 2b: Allegations - Trust Roles
(ubo.allege :cbu @cbu 
            :from ("trust" "Hartley Family Trust")
            :to ("person" "Edward Hartley")
            :type "trust_role" 
            :role "settlor"
            :source "trust_deed")

(ubo.allege :cbu @cbu 
            :from ("trust" "Hartley Family Trust")
            :to ("company" "Channel Islands Trust Co Ltd")
            :type "trust_role" 
            :role "trustee"
            :source "trust_deed")

(ubo.allege :cbu @cbu 
            :from ("trust" "Hartley Family Trust")
            :to ("person" "Victoria Hartley")
            :type "trust_role" 
            :role "beneficiary"
            :interest "fixed"
            :percentage 50
            :source "trust_deed")

(ubo.allege :cbu @cbu 
            :from ("trust" "Hartley Family Trust")
            :to ("person" "William Hartley")
            :type "trust_role" 
            :role "beneficiary"
            :interest "fixed"
            :percentage 50
            :source "trust_deed")

(ubo.allege :cbu @cbu 
            :from ("trust" "Hartley Family Trust")
            :to ("person" "Margaret Thompson")
            :type "trust_role" 
            :role "protector"
            :source "trust_deed")

;; Phase 3: Link Proofs
(ubo.link-proof :cbu @cbu 
                :edge [("fund" "Hartley Capital") ("trust" "Hartley Family")]
                :proof @proof_fund_trust
                :proof-type "trust_deed")

;; Trust deed covers all trust role edges
(ubo.link-proof :cbu @cbu 
                :edge [("trust" "Hartley Family") ("person" "Edward Hartley")]
                :proof @proof_trust_deed
                :proof-type "trust_deed")

(ubo.link-proof :cbu @cbu 
                :edge [("trust" "Hartley Family") ("company" "Channel Islands Trust")]
                :proof @proof_trust_deed
                :proof-type "trust_deed")

(ubo.link-proof :cbu @cbu 
                :edge [("trust" "Hartley Family") ("person" "Victoria Hartley")]
                :proof @proof_trust_deed
                :proof-type "trust_deed")

(ubo.link-proof :cbu @cbu 
                :edge [("trust" "Hartley Family") ("person" "William Hartley")]
                :proof @proof_trust_deed
                :proof-type "trust_deed")

(ubo.link-proof :cbu @cbu 
                :edge [("trust" "Hartley Family") ("person" "Margaret Thompson")]
                :proof @proof_trust_deed
                :proof-type "trust_deed")

;; Phase 4: Verify
(ubo.verify :cbu @cbu :as @verify_result)

;; Phase 5: Status
(ubo.status :cbu @cbu :as @status)

;; Phase 6: Assertions
(ubo.assert :cbu @cbu :converged true)
(ubo.assert :cbu @cbu :no-expired-proofs true)

;; Phase 7: Evaluate
(ubo.evaluate :cbu @cbu :as @eval)
;; Expected BOs: Victoria (50%), William (50%) - fixed beneficiaries
;; Expected control: Trustee company, Protector Margaret
;; Note: Trust structure triggers Enhanced DD

(ubo.assert :cbu @cbu :thresholds-pass true)
(ubo.assert :cbu @cbu :no-blocking-flags true)

;; Phase 8: Decision (with enhanced DD flag)
(kyc.decision :cbu @cbu :status "CLEARED" :review-in "6m" :enhanced-dd true :as @decision)
```

### Verification Points

- [ ] 1 ownership edge + 5 trust_role edges created
- [ ] All edges verified and proven
- [ ] Beneficiaries identified as BOs (fixed interest = ownership-equivalent)
- [ ] Trustee identified as control person
- [ ] Protector identified as control person
- [ ] Settlor recorded (deceased, not control)
- [ ] Enhanced DD flag set due to trust structure
- [ ] Review period shortened (6m vs 12m)
- [ ] Decision: CLEARED

---

## Test Harness Implementation

### File: `rust/xtask/src/ubo_test.rs`

```rust
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "ubo-test")]
#[command(about = "KYC/UBO convergence test harness")]
pub struct UboTestCli {
    #[command(subcommand)]
    command: UboTestCommand,
}

#[derive(Subcommand)]
enum UboTestCommand {
    /// Run Scenario 1: Simple Allianz Fund
    Scenario1 {
        #[arg(long, default_value = "http://localhost:3000")]
        server_url: String,
        
        #[arg(long)]
        json_output: bool,
    },
    
    /// Run Scenario 2: Hedge Fund LLP
    Scenario2 {
        #[arg(long, default_value = "http://localhost:3000")]
        server_url: String,
        
        #[arg(long)]
        json_output: bool,
    },
    
    /// Run Scenario 3: Trust Structure
    Scenario3 {
        #[arg(long, default_value = "http://localhost:3000")]
        server_url: String,
        
        #[arg(long)]
        json_output: bool,
    },
    
    /// Run all scenarios
    All {
        #[arg(long, default_value = "http://localhost:3000")]
        server_url: String,
        
        #[arg(long)]
        json_output: bool,
    },
    
    /// Clean test data
    Clean {
        #[arg(long, default_value = "http://localhost:3000")]
        server_url: String,
    },
    
    /// Seed test entities (run before scenarios)
    Seed {
        #[arg(long, default_value = "http://localhost:3000")]
        server_url: String,
    },
}

#[derive(Debug, Serialize)]
struct ScenarioResult {
    scenario: String,
    success: bool,
    duration_ms: u64,
    edges_created: usize,
    edges_proven: usize,
    converged: bool,
    beneficial_owners: Vec<String>,
    control_persons: Vec<String>,
    decision: Option<String>,
    errors: Vec<String>,
}

impl UboTestCli {
    pub async fn run(&self) -> anyhow::Result<()> {
        match &self.command {
            UboTestCommand::Scenario1 { server_url, json_output } => {
                run_scenario_1(server_url, *json_output).await
            }
            UboTestCommand::Scenario2 { server_url, json_output } => {
                run_scenario_2(server_url, *json_output).await
            }
            UboTestCommand::Scenario3 { server_url, json_output } => {
                run_scenario_3(server_url, *json_output).await
            }
            UboTestCommand::All { server_url, json_output } => {
                run_all_scenarios(server_url, *json_output).await
            }
            UboTestCommand::Clean { server_url } => {
                clean_test_data(server_url).await
            }
            UboTestCommand::Seed { server_url } => {
                seed_test_entities(server_url).await
            }
        }
    }
}

async fn run_scenario_1(server_url: &str, json_output: bool) -> anyhow::Result<()> {
    // Implementation: Execute DSL sequence for Scenario 1
    // 1. Ensure CBU exists
    // 2. Execute allegations
    // 3. Link proofs (mock proofs with matching observations)
    // 4. Verify
    // 5. Run assertions
    // 6. Evaluate
    // 7. Decision
    // 8. Verify expected state
    todo!()
}

// Similar implementations for scenario_2, scenario_3, all, clean, seed
```

### File: `rust/xtask/src/main.rs` (add subcommand)

```rust
// Add to existing xtask CLI
UboTest(ubo_test::UboTestCli),

// In match:
Command::UboTest(cli) => cli.run().await,
```

### Test Data Seeding

```rust
async fn seed_test_entities(server_url: &str) -> anyhow::Result<()> {
    // Scenario 1 entities
    create_entity(server_url, "limited_company", "Allianz SE", "DE").await?;
    create_entity(server_url, "manco", "Allianz Global Investors GmbH", "DE").await?;
    create_entity(server_url, "fund", "Allianz Dynamic Multi Asset Strategy", "LU").await?;
    
    // Scenario 2 entities
    create_entity(server_url, "fund", "Apex Global Opportunities Fund", "KY").await?;
    create_entity(server_url, "partnership", "Apex Capital LLP", "UK").await?;
    create_entity(server_url, "limited_company", "Wellington Partners Ltd", "UK").await?;
    create_entity(server_url, "proper_person", "James Wellington", "UK").await?;
    create_entity(server_url, "proper_person", "Sarah Chen", "UK").await?;
    create_entity(server_url, "proper_person", "Marcus Webb", "UK").await?;
    
    // Scenario 3 entities
    create_entity(server_url, "fund", "Hartley Capital Fund", "JE").await?;
    create_entity(server_url, "trust", "Hartley Family Trust", "JE").await?;
    create_entity(server_url, "proper_person", "Edward Hartley", "UK").await?;
    create_entity(server_url, "limited_company", "Channel Islands Trust Co Ltd", "JE").await?;
    create_entity(server_url, "proper_person", "Victoria Hartley", "UK").await?;
    create_entity(server_url, "proper_person", "William Hartley", "UK").await?;
    create_entity(server_url, "proper_person", "Margaret Thompson", "UK").await?;
    
    // Create mock proofs
    create_mock_proofs(server_url).await?;
    
    Ok(())
}
```

---

## Implementation Tasks

### Phase 1: Test Data & Seeding

- [ ] Create `rust/xtask/src/ubo_test.rs`
- [ ] Add `UboTest` subcommand to xtask main.rs
- [ ] Implement `seed_test_entities()` - create all scenario entities
- [ ] Implement `create_mock_proofs()` - create proof documents with observations
- [ ] Implement `clean_test_data()` - FK-aware cleanup

### Phase 2: Scenario 1 (Simple Fund)

- [ ] Implement `run_scenario_1()`
- [ ] DSL execution for allegations
- [ ] DSL execution for proof linking
- [ ] DSL execution for verify + assertions
- [ ] DSL execution for evaluate + decision
- [ ] Verification: check convergence state matches expected
- [ ] Verification: check decision recorded

### Phase 3: Scenario 2 (Hedge Fund LLP)

- [ ] Implement `run_scenario_2()`
- [ ] Handle multiple ownership edges with different percentages
- [ ] Handle control edge (managing_partner)
- [ ] Verify threshold calculation (45%, 35%, 20%)
- [ ] Verify BO identification (only >25%)

### Phase 4: Scenario 3 (Trust)

- [ ] Implement `run_scenario_3()`
- [ ] Handle trust_role edges (settlor, trustee, beneficiary, protector)
- [ ] Verify beneficiary → BO mapping for fixed interests
- [ ] Verify trustee/protector → control person mapping
- [ ] Verify enhanced DD flag
- [ ] Verify shortened review period

### Phase 5: Integration

- [ ] Implement `run_all_scenarios()` - sequential execution with summary
- [ ] Add JSON output mode for CI
- [ ] Add timing metrics
- [ ] Exit codes for CI (0 = all pass, 1 = failures)

### Phase 6: Documentation

- [ ] Add scenario descriptions to test output
- [ ] Add expected vs actual comparison in verbose mode
- [ ] README for running tests

---

## File Summary

| File | Action | Purpose |
|------|--------|---------|
| `rust/xtask/src/ubo_test.rs` | Create | Test harness implementation |
| `rust/xtask/src/main.rs` | Modify | Add UboTest subcommand |
| `rust/xtask/src/test_data/scenario_1.json` | Create | Scenario 1 test data |
| `rust/xtask/src/test_data/scenario_2.json` | Create | Scenario 2 test data |
| `rust/xtask/src/test_data/scenario_3.json` | Create | Scenario 3 test data |

---

## Usage

```bash
# Seed test entities first
cargo xtask ubo-test seed

# Run individual scenarios
cargo xtask ubo-test scenario-1
cargo xtask ubo-test scenario-2
cargo xtask ubo-test scenario-3

# Run all scenarios
cargo xtask ubo-test all

# JSON output for CI
cargo xtask ubo-test all --json-output

# Clean test data
cargo xtask ubo-test clean
```

---

## Success Criteria

### Scenario 1
- [ ] 2 ownership edges created and proven
- [ ] Convergence = true
- [ ] No natural person BOs (corporate chain)
- [ ] Decision = CLEARED

### Scenario 2
- [ ] 6 ownership + 1 control edges created and proven
- [ ] Convergence = true
- [ ] 2 BOs identified: James (45%), Sarah (35%)
- [ ] 1 control person: Sarah (managing_partner)
- [ ] Marcus excluded (20% < 25%)
- [ ] Decision = CLEARED

### Scenario 3
- [ ] 1 ownership + 5 trust_role edges created and proven
- [ ] Convergence = true
- [ ] 2 BOs identified: Victoria (50%), William (50%)
- [ ] 2 control persons: Trustee, Protector
- [ ] Enhanced DD flag = true
- [ ] Review period = 6m
- [ ] Decision = CLEARED

### Overall
- [ ] All scenarios pass in sequence
- [ ] Exit code 0 when all pass
- [ ] JSON output parseable by CI
- [ ] Clean removes all test data

---

## References

- Solution overview: `/docs/KYC-UBO-SOLUTION-OVERVIEW.md`
- Implementation TODO: `/TODO-KYC-UBO-CONVERGENCE.md`
- Batch test harness pattern: `/rust/src/bin/batch_test_harness.rs`
- UBO verbs: `/rust/src/dsl_v2/custom_ops/ubo_*.rs`

---

*Test harness for KYC/UBO convergence validation across three ownership structure patterns.*
