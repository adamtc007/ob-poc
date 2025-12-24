# CLAUDE TODO: Operations Runbook & User Manual

## Overview

Create a living documentation set that serves as:
1. **Operations Runbook** - Step-by-step session examples
2. **Product Socialization** - Business-friendly explanations
3. **Reference Manual** - Verb dictionary appendix

**Audience:** Agile product team, business analysts, operations users

**Format:** Markdown files in `docs/runbook/` that can be rendered as a documentation site

## Document Structure

```
docs/runbook/
├── README.md                    # Overview and navigation
├── 01-introduction.md           # What is ob-poc, key concepts
├── 02-getting-started.md        # First session, basic flow
├── 03-individual-onboarding.md  # Runbook: Individual client
├── 04-corporate-onboarding.md   # Runbook: Corporate with UBO
├── 05-kyc-workflow.md           # Runbook: KYC case lifecycle
├── 05a-kyc-async-requests.md    # Runbook: Fire-and-forget document requests
├── 06-product-subscription.md   # Runbook: Adding products
├── 07-trading-setup.md          # Runbook: Trading profile & SSIs
├── 08-auto-onboarding.md        # Runbook: Auto-complete feature
├── 09-journey-stages.md         # Semantic stage map explained
├── 10-troubleshooting.md        # Common issues and solutions
├── appendix-a-verb-dictionary.md # Auto-generated verb reference
├── appendix-b-entity-types.md   # Entity type reference
└── appendix-c-glossary.md       # Business terms glossary
```

## Document Specifications

### 01-introduction.md

```markdown
# Enterprise Onboarding Platform (ob-poc)

## What Is This?

The Enterprise Onboarding Platform streamlines client onboarding for custody 
banks and broker-dealers. It combines:

- **Natural Language Interface** - Chat with an AI agent to perform operations
- **Domain-Specific Language** - Precise, auditable commands
- **Visual Journey Tracking** - See onboarding progress at a glance
- **Regulatory Compliance** - Built-in KYC/UBO workflows

## Key Concepts

### Client Business Unit (CBU)
The central entity representing a client relationship. Everything hangs off 
the CBU: entities (people, companies), products, cases, agreements.

### The Onboarding Journey
A structured progression through stages:
1. Client Setup → 2. Product Selection → 3. KYC Review → 4. Trading Setup → ...

### DSL (Domain-Specific Language)
Commands like `(cbu.create name:"Alpha Fund" jurisdiction:US)` that the 
system executes. You can type these directly or let the AI agent generate them.

### Symbols (@references)
Named handles to entities: `@alpha-fund`, `@kyc-case`. Use these to chain 
operations together.

## How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│  YOU                              SYSTEM                        │
│  ───                              ──────                        │
│  "Create a new fund             → Generates DSL                 │
│   called Alpha Fund"            → Validates                     │
│                                  → Executes                     │
│                                  → Returns: @alpha-fund         │
│                                                                 │
│  "Add custody product"          → (cbu.add-product              │
│                                     cbu-id:@alpha-fund          │
│                                     product:CUSTODY)            │
│                                  → Services provisioned         │
│                                  → Journey updates              │
└─────────────────────────────────────────────────────────────────┘
```
```

### 02-getting-started.md

```markdown
# Getting Started

## Your First Session

### Step 1: Open the Application
Navigate to the ob-poc UI. You'll see:
- **Left Panel**: Context (current CBU, linked entities, journey)
- **Center**: Chat interface and DSL editor
- **Right**: Graph visualization

### Step 2: Create a Client
Type in the chat:
```
Create a new individual client called John Smith, US jurisdiction
```

The agent responds with DSL:
```lisp
(cbu.create name:"John Smith" jurisdiction:US client-type:INDIVIDUAL) -> @john-smith
```

Click **Execute** or type `/execute`.

### Step 3: Check the Journey
The left panel now shows:
```
✓ Client Setup (complete)
○ Product Selection (next)
○ KYC Review
○ ...
```

### Step 4: Add a Product
```
Add custody product to John Smith
```

Generates and executes:
```lisp
(cbu.add-product cbu-id:@john-smith product:CUSTODY)
```

Journey updates:
```
✓ Client Setup
✓ Product Selection
◐ KYC Review (in progress - missing: kyc_case)
```

### Step 5: Continue the Journey
Either:
- Click stages in the journey panel
- Ask the agent: "What's next?"
- Use auto-complete: `(onboarding.auto-complete cbu-id:@john-smith dry-run:true)`
```

### 03-individual-onboarding.md (RUNBOOK)

```markdown
# Runbook: Individual Client Onboarding

## Scenario
Onboard **Jane Doe**, a US individual, for Custody services.

## Prerequisites
- User has UI access
- Session is active

## Steps

### 1. Create the CBU
**Chat:** "Create an individual client Jane Doe from the US"

**DSL Generated:**
```lisp
(cbu.create 
  name:"Jane Doe" 
  jurisdiction:US 
  client-type:INDIVIDUAL) -> @jane-doe
```

**Result:** CBU created, `@jane-doe` symbol available

---

### 2. Add Proper Person Entity
**Chat:** "Add Jane as the main person on this account"

**DSL Generated:**
```lisp
(entity.create-proper-person
  cbu-id:@jane-doe
  first-name:"Jane"
  last-name:"Doe"
  nationality:US
  date-of-birth:"1985-03-15") -> @jane-entity
```

**Result:** Person entity created and linked to CBU

---

### 3. Assign Account Holder Role
**Chat:** "Make Jane the account holder"

**DSL Generated:**
```lisp
(cbu.assign-role
  cbu-id:@jane-doe
  entity-id:@jane-entity
  role-type:ACCOUNT_HOLDER
  effective-date:"2024-01-01")
```

**Result:** Role assigned, entity appears in graph with role badge

---

### 4. Subscribe to Custody
**Chat:** "Add custody product"

**DSL Generated:**
```lisp
(cbu.add-product cbu-id:@jane-doe product:CUSTODY)
```

**Result:** 
- Custody product added
- Services auto-provisioned (Safekeeping, Settlement, Corporate Actions, etc.)
- Journey advances to KYC Review

---

### 5. Start KYC Case
**Chat:** "Start the KYC process"

**DSL Generated:**
```lisp
(kyc-case.create
  cbu-id:@jane-doe
  case-type:NEW_CLIENT) -> @kyc-case
```

**Result:** KYC case created in INTAKE status

---

### 6. Add Entity Workstream
**Chat:** "Create a workstream for Jane's KYC"

**DSL Generated:**
```lisp
(entity-workstream.create
  case-id:@kyc-case
  entity-id:@jane-entity
  workstream-type:INDIVIDUAL) -> @workstream
```

**Result:** Workstream created, KYC stage shows progress

---

### 7. Complete KYC Checks
**Chat:** "Run screening and complete the workstream"

**DSL Generated:**
```lisp
(screening.run workstream-id:@workstream)
(entity-workstream.complete workstream-id:@workstream)
(kyc-case.update-status case-id:@kyc-case new-status:APPROVED)
```

**Result:** KYC approved, journey advances

---

### 8. Setup Trading Profile
**Chat:** "Import a default trading profile"

**DSL Generated:**
```lisp
(trading-profile.import
  cbu-id:@jane-doe
  profile-path:"config/seed/trading_profiles/individual_us.yaml") -> @profile
```

**Result:** Trading profile configured with instrument universe

---

### 9. Create SSI
**Chat:** "Create a settlement instruction for USD securities"

**DSL Generated:**
```lisp
(cbu-custody.create-ssi
  cbu-id:@jane-doe
  name:"Jane Doe USD SSI"
  type:SECURITIES
  currency:USD
  safekeeping-account:"SAFE-001"
  safekeeping-bic:"CUSTUS33") -> @ssi
```

**Result:** SSI created, settlement stage advances

---

### 10. Verify Completion
**Chat:** "What's the onboarding status?"

**Response:**
```
Onboarding Progress: 100%

✓ Client Setup - Complete
✓ Product Selection - CUSTODY active
✓ KYC Review - Case APPROVED
✓ Instrument Universe - Configured
✓ Settlement Instructions - 1 SSI active
✓ Lifecycle Resources - Provisioned

Jane Doe is fully onboarded for Custody services.
```

## Symbols Created
| Symbol | Entity Type | Description |
|--------|-------------|-------------|
| @jane-doe | CBU | Client business unit |
| @jane-entity | Entity | Proper person record |
| @kyc-case | KYC Case | Onboarding KYC case |
| @workstream | Workstream | KYC entity workstream |
| @profile | Trading Profile | Trading configuration |
| @ssi | SSI | Settlement instruction |

## Time Estimate
- Manual: 15-20 minutes
- With auto-complete: 2-3 minutes
```

### 04-corporate-onboarding.md (RUNBOOK)

```markdown
# Runbook: Corporate Client with UBO Chain

## Scenario
Onboard **Acme Holdings Ltd**, a UK corporate with:
- Parent company (Acme Holdings)
- Operating subsidiary (Acme Trading)
- Two UBOs (controlling persons)

## Steps

### 1. Create Corporate CBU
```lisp
(cbu.create 
  name:"Acme Holdings Ltd" 
  jurisdiction:GB 
  client-type:CORPORATE) -> @acme
```

### 2. Create Entity Structure

**Parent Company:**
```lisp
(entity.create-limited-company
  cbu-id:@acme
  name:"Acme Holdings Ltd"
  jurisdiction:GB
  registration-number:"12345678"
  incorporation-date:"2010-05-20") -> @acme-parent
```

**Subsidiary:**
```lisp
(entity.create-limited-company
  cbu-id:@acme
  name:"Acme Trading Ltd"
  jurisdiction:GB
  registration-number:"87654321") -> @acme-trading
```

**UBO 1:**
```lisp
(entity.create-proper-person
  cbu-id:@acme
  first-name:"Robert"
  last-name:"Smith"
  nationality:GB) -> @ubo1
```

**UBO 2:**
```lisp
(entity.create-proper-person
  cbu-id:@acme
  first-name:"Sarah"
  last-name:"Jones"
  nationality:GB) -> @ubo2
```

### 3. Establish Ownership Chain
```lisp
; Parent owns subsidiary
(entity.add-ownership
  owner-id:@acme-parent
  owned-id:@acme-trading
  ownership-percentage:100
  ownership-type:DIRECT)

; UBOs own parent
(entity.add-ownership
  owner-id:@ubo1
  owned-id:@acme-parent
  ownership-percentage:60
  ownership-type:DIRECT)

(entity.add-ownership
  owner-id:@ubo2
  owned-id:@acme-parent
  ownership-percentage:40
  ownership-type:DIRECT)
```

### 4. Assign Roles
```lisp
(cbu.assign-role cbu-id:@acme entity-id:@acme-parent role-type:ACCOUNT_HOLDER)
(cbu.assign-role cbu-id:@acme entity-id:@ubo1 role-type:UBO)
(cbu.assign-role cbu-id:@acme entity-id:@ubo2 role-type:UBO)
(cbu.assign-role cbu-id:@acme entity-id:@acme-trading role-type:TRADING_ENTITY)
```

### 5. Graph Visualization
The graph now shows:
```
        @acme (CBU)
            │
    ┌───────┴───────┐
    │               │
@acme-parent    @acme-trading
[ACCOUNT_HOLDER] [TRADING_ENTITY]
    │ owns 100%
    │
    ├──────┬──────┐
    │      │      │
  @ubo1  @ubo2
  [UBO]  [UBO]
   60%    40%
```

### 6. KYC with Multiple Workstreams
```lisp
(kyc-case.create cbu-id:@acme case-type:NEW_CLIENT) -> @kyc

; Workstream per entity requiring KYC
(entity-workstream.create case-id:@kyc entity-id:@acme-parent) -> @ws-parent
(entity-workstream.create case-id:@kyc entity-id:@ubo1) -> @ws-ubo1
(entity-workstream.create case-id:@kyc entity-id:@ubo2) -> @ws-ubo2
```

### 7. Complete Onboarding
Use auto-complete for remaining steps:
```lisp
(onboarding.auto-complete cbu-id:@acme)
```

## Expected Result
- 4 entities in graph with ownership edges
- 3 KYC workstreams (one per KYC-required entity)
- Full custody product stack provisioned
- 100% journey completion
```

### 08-auto-onboarding.md (RUNBOOK)

```markdown
# Runbook: Automated Onboarding

## Overview
The `onboarding.auto-complete` verb automatically progresses through 
onboarding stages by generating and executing DSL for missing entities.

## When to Use
- **Demo/POC**: Quickly populate a CBU for demonstration
- **Testing**: Generate test data rapidly
- **Gap Filling**: Complete partially onboarded clients

## Basic Usage

### Preview Mode (Dry Run)
See what would be created without executing:
```lisp
(onboarding.auto-complete cbu-id:@fund dry-run:true)
```

**Output:**
```json
{
  "steps_executed": 5,
  "dry_run": true,
  "steps": [
    {
      "entity_type": "kyc_case",
      "stage": "KYC_REVIEW",
      "dsl": "(kyc-case.create :cbu-id \"...\" :case-type \"NEW_CLIENT\")"
    },
    {
      "entity_type": "trading_profile",
      "stage": "INSTRUMENT_UNIVERSE",
      "dsl": "(trading-profile.import :cbu-id \"...\" ...)"
    }
    // ...
  ]
}
```

### Execute Mode
Actually create the entities:
```lisp
(onboarding.auto-complete cbu-id:@fund)
```

### Stop at Specific Stage
Complete only up to KYC:
```lisp
(onboarding.auto-complete cbu-id:@fund target-stage:KYC_REVIEW)
```

### Limit Steps
Safety limit for large onboardings:
```lisp
(onboarding.auto-complete cbu-id:@fund max-steps:5)
```

## Example Session

```
> (cbu.create name:"Quick Demo Fund" jurisdiction:US) -> @demo
Created CBU: Quick Demo Fund (@demo)

> (cbu.add-product cbu-id:@demo product:CUSTODY)
Added CUSTODY to Quick Demo Fund
Services provisioned: 8

> (semantic.get-state cbu-id:@demo)
Progress: 2/6 stages (33%)
Missing: kyc_case, trading_profile, cbu_ssi, ...

> (onboarding.auto-complete cbu-id:@demo)
{
  "steps_executed": 4,
  "steps_succeeded": 4,
  "steps_failed": 0,
  "target_reached": true,
  "remaining_missing": []
}

> (semantic.get-state cbu-id:@demo)
Progress: 6/6 stages (100%)
```

## Limitations

### Entities Requiring User Input
Some entities need human selection:
- `entity_workstream` - Which entity to create workstream for
- `isda_agreement` - Counterparty selection
- `holding` - Investor entity selection

Auto-complete stops when it hits these with a message:
```
"DSL requires user selection - cannot auto-complete"
```

### No Rollback
If a step fails, previously created entities remain. Use dry-run first.

### Sequential Execution
Steps run one at a time. A future enhancement could parallelize independent stages.
```

### 05a-kyc-async-requests.md (RUNBOOK)

```markdown
# Runbook: Fire-and-Forget Document Requests

## Overview
The async request system uses a **fire-and-forget** pattern for document collection.
Instead of blocking workflows, you create requests that are tracked independently
and can be checked via case state queries.

## Key Concept: Domain Coherence
Requests are **embedded in their parent workstreams**, not returned as flat lists.
When you query case state, each workstream shows its `awaiting` requests inline:

```json
{
  "case_status": "DISCOVERY",
  "workstreams": [
    {
      "entity_name": "John Smith",
      "status": "COLLECT",
      "awaiting": [
        {"type": "PASSPORT", "due": "2024-02-01", "overdue": false},
        {"type": "ADDRESS_PROOF", "due": "2024-01-15", "overdue": true}
      ]
    }
  ]
}
```

## Request Types

| Type | Description | Typical Due Period |
|------|-------------|-------------------|
| `PASSPORT` | Identity document | 5 days |
| `ADDRESS_PROOF` | Utility bill, bank statement | 5 days |
| `CERT_OF_INC` | Certificate of incorporation | 7 days |
| `REG_OF_DIRS` | Register of directors | 7 days |
| `OWNERSHIP_CHART` | UBO structure diagram | 10 days |
| `SOURCE_OF_WEALTH` | Wealth origin documentation | 14 days |
| `BANK_STATEMENT` | Financial statements | 7 days |
| `TAX_RESIDENCY_CERT` | Tax domicile proof | 10 days |

## Step-by-Step Workflow

### 1. Create Requests (Fire)
**Chat:** "Request passport and address proof from John Smith's workstream"

**DSL Generated:**
```lisp
(request.create
  workstream-id:@ws-john
  request-type:PASSPORT
  due-in-days:5) -> @req-passport

(request.create
  workstream-id:@ws-john
  request-type:ADDRESS_PROOF
  due-in-days:5) -> @req-address
```

**Result:** Requests created with PENDING status, timestamps set

---

### 2. Check Case State (Forget... then Remember)
**Chat:** "What's the status of the KYC case?"

**DSL Generated:**
```lisp
(kyc-case.state case-id:@kyc-case)
```

**Response:**
```
KYC Case: NEW_CLIENT | Status: DISCOVERY

Entity Workstreams:
├─ John Smith [COLLECT] ⏳
│  └─ awaiting: PASSPORT (due 2024-02-01), ADDRESS_PROOF (due 2024-02-01)
├─ Acme Holdings [VERIFY] ✓
│  └─ awaiting: (none)
└─ Sarah Jones [COLLECT] ⚠️
   └─ awaiting: PASSPORT (due 2024-01-20) ⚠️ OVERDUE
```

---

### 3. Fulfill a Request
When document is received and cataloged, it auto-fulfills matching requests:

**Chat:** "Catalog John's passport"

**DSL Generated:**
```lisp
(document.catalog
  cbu-id:@acme
  entity-id:@john
  doc-type:PASSPORT
  title:"John Smith Passport"
  file-path:"/uploads/john-passport.pdf")
```

**Result:** 
- Document cataloged
- Matching PASSPORT request auto-fulfilled
- Workstream `awaiting` count decreases

---

### 4. Handle Overdue Requests

**Remind:**
```lisp
(request.remind request-id:@req-address)
```

**Extend deadline:**
```lisp
(request.extend request-id:@req-address extend-days:7)
```

**Escalate:**
```lisp
(request.escalate request-id:@req-address reason:"No response after 3 reminders")
```

**Waive (with justification):**
```lisp
(request.waive request-id:@req-address reason:"Alternative verification via bank")
```

---

### 5. Bulk Request Creation
For standard document sets:

**Chat:** "Request all standard KYC documents for corporate workstreams"

**DSL Generated:**
```lisp
(request.create-batch
  workstream-ids:[@ws-acme @ws-subsidiary]
  request-types:[CERT_OF_INC REG_OF_DIRS OWNERSHIP_CHART]
  due-in-days:7)
```

---

### 6. Workstream-Level State Query
For detailed workstream view:

**DSL:**
```lisp
(entity-workstream.state workstream-id:@ws-john)
```

**Response:**
```json
{
  "entity_name": "John Smith",
  "entity_type": "PROPER_PERSON",
  "status": "COLLECT",
  "awaiting_requests": [
    {
      "request_id": "...",
      "type": "ADDRESS_PROOF",
      "status": "PENDING",
      "due_date": "2024-02-01",
      "is_overdue": false,
      "remind_count": 0
    }
  ],
  "screenings": [
    {"type": "PEP", "status": "CLEAR"},
    {"type": "SANCTIONS", "status": "CLEAR"}
  ]
}
```

## Status Flow

```
PENDING → REMINDED → ESCALATED → FULFILLED
                  ↘           ↗
                   WAIVED ───┘
```

| Status | Meaning |
|--------|---------|
| `PENDING` | Request created, awaiting response |
| `REMINDED` | Follow-up reminder sent |
| `ESCALATED` | Escalated to senior analyst/manager |
| `FULFILLED` | Document received and linked |
| `WAIVED` | Requirement waived with justification |

## Integration with Journey

The journey stage map considers outstanding requests:

```
◐ KYC Review (in progress)
    └─ awaiting 3 documents across 2 workstreams
```

When all workstreams have no awaiting requests and screening is complete,
the KYC stage can advance to APPROVED.

## Symbols Created

| Symbol | Entity Type | Description |
|--------|-------------|-------------|
| @req-passport | Request | Document request for passport |
| @req-address | Request | Document request for address proof |

## Agent Prompt Context

When a KYC case is linked to the session, the agent automatically receives
embedded request context in its prompt:

```
KYC Context:
├─ Case: NEW_CLIENT | DISCOVERY
├─ Workstream: John Smith [COLLECT]
│  └─ awaiting: PASSPORT, ADDRESS_PROOF
└─ 2 total requests pending
```

This enables natural conversation:
- "What documents are we still waiting for?"
- "Remind John about his passport"
- "Waive the address proof - we verified via bank"
```

### appendix-a-verb-dictionary.md

This should be **auto-generated** from the YAML configs. Create a script:

```markdown
# Appendix A: Verb Dictionary

> **Note:** This appendix is auto-generated from `config/verbs/*.yaml`
> Last updated: {timestamp}

## Domains

- [cbu](#cbu) - Client Business Unit operations
- [entity](#entity) - Entity management
- [kyc](#kyc) - KYC case and workstream operations
- [custody](#custody) - Custody-specific operations
- [onboarding](#onboarding) - Automated onboarding
- [semantic](#semantic) - Journey state queries
- ...

---

## cbu

### cbu.create
Create a new Client Business Unit

**Arguments:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| name | string | ✓ | CBU display name |
| jurisdiction | string | ✓ | ISO country code |
| client-type | enum | | INDIVIDUAL, CORPORATE, FUND |

**Example:**
```lisp
(cbu.create name:"Alpha Fund" jurisdiction:US client-type:FUND) -> @fund
```

---

### cbu.add-product
Subscribe CBU to a product

**Arguments:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| cbu-id | uuid | ✓ | Target CBU |
| product | enum | ✓ | CUSTODY, FUND_ACCOUNTING, etc. |

**Example:**
```lisp
(cbu.add-product cbu-id:@fund product:CUSTODY)
```

---

[... continue for all verbs ...]
```

## Implementation Plan

### Phase 1: Structure & Core Docs
1. Create `docs/runbook/` directory structure
2. Write `README.md` navigation
3. Write `01-introduction.md`
4. Write `02-getting-started.md`

### Phase 2: Runbooks
5. Write `03-individual-onboarding.md`
6. Write `04-corporate-onboarding.md`
7. Write `05-kyc-workflow.md`
8. Write `08-auto-onboarding.md`

### Phase 3: Reference Appendices
9. Create verb dictionary generator script
10. Generate `appendix-a-verb-dictionary.md`
11. Write `appendix-b-entity-types.md`
12. Write `appendix-c-glossary.md`

### Phase 4: Living Document Tooling
13. Add `make docs` target to regenerate appendix
14. Add timestamp/version to generated docs
15. Consider mdBook or similar for HTML rendering

## Verb Dictionary Generator

Create `scripts/generate-verb-dictionary.py` (or Rust):

```python
#!/usr/bin/env python3
"""Generate verb dictionary markdown from YAML configs."""

import yaml
from pathlib import Path
from datetime import datetime

VERB_DIR = Path("rust/config/verbs")
OUTPUT = Path("docs/runbook/appendix-a-verb-dictionary.md")

def main():
    domains = {}
    
    for yaml_file in VERB_DIR.glob("*.yaml"):
        with open(yaml_file) as f:
            config = yaml.safe_load(f)
            if "domains" in config:
                domains.update(config["domains"])
    
    # Generate markdown
    lines = [
        "# Appendix A: Verb Dictionary",
        "",
        f"> Auto-generated from `config/verbs/*.yaml` on {datetime.now().isoformat()}",
        "",
        "## Domains",
        ""
    ]
    
    for domain_name in sorted(domains.keys()):
        lines.append(f"- [{domain_name}](#{domain_name})")
    
    lines.append("")
    lines.append("---")
    lines.append("")
    
    for domain_name, domain in sorted(domains.items()):
        lines.append(f"## {domain_name}")
        lines.append("")
        if "description" in domain:
            lines.append(domain["description"])
            lines.append("")
        
        for verb_name, verb in domain.get("verbs", {}).items():
            lines.append(f"### {domain_name}.{verb_name}")
            lines.append("")
            lines.append(verb.get("description", ""))
            lines.append("")
            
            args = verb.get("args", [])
            if args:
                lines.append("**Arguments:**")
                lines.append("| Name | Type | Required | Description |")
                lines.append("|------|------|----------|-------------|")
                for arg in args:
                    req = "✓" if arg.get("required") else ""
                    lines.append(f"| {arg['name']} | {arg.get('type', 'any')} | {req} | {arg.get('description', '')} |")
                lines.append("")
            
            lines.append("---")
            lines.append("")
    
    OUTPUT.write_text("\n".join(lines))
    print(f"Generated {OUTPUT}")

if __name__ == "__main__":
    main()
```

## Success Criteria

1. Product team can read runbooks and understand the system
2. Operations users can follow step-by-step guides
3. Verb dictionary stays in sync with code
4. Documentation renders nicely in GitHub/mdBook
5. Examples are copy-pasteable and work

## Files to Create

| File | Priority | Description |
|------|----------|-------------|
| `docs/runbook/README.md` | HIGH | Navigation hub |
| `docs/runbook/01-introduction.md` | HIGH | What is this |
| `docs/runbook/02-getting-started.md` | HIGH | First steps |
| `docs/runbook/03-individual-onboarding.md` | HIGH | Key runbook |
| `docs/runbook/04-corporate-onboarding.md` | HIGH | UBO example |
| `docs/runbook/08-auto-onboarding.md` | HIGH | Auto feature |
| `scripts/generate-verb-dictionary.py` | MEDIUM | Keep in sync |
| `docs/runbook/appendix-a-verb-dictionary.md` | MEDIUM | Generated |
| `Makefile` addition | LOW | `make docs` target |
