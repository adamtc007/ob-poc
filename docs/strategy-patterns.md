# ob-poc Strategy & Patterns

> Core architectural decisions and their rationale
> This is the "why" - implementation details are in /docs

---

## 1. Data Strategy Overview

### The Core Insight

**Everything is an Entity with Relationships.**

Traditional custody systems have dozens of disconnected tables: clients, counterparties, funds, people, companies, etc. ob-poc unifies these into a single entity model where:

- Every legal person (natural or corporate) is an `entity`
- Relationships between entities are typed edges (ownership, role, agreement)
- Context determines meaning (same entity can be client, counterparty, UBO)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DATA PHILOSOPHY                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   TRADITIONAL APPROACH              ob-poc APPROACH                          │
│   ────────────────────              ───────────────                          │
│                                                                              │
│   ┌─────────┐ ┌─────────┐          ┌──────────────────────────────────────┐ │
│   │ CLIENTS │ │COUNTPTY │          │             ENTITIES                 │ │
│   └─────────┘ └─────────┘          │  (unified: person, company, fund)    │ │
│   ┌─────────┐ ┌─────────┐          └──────────────────────────────────────┘ │
│   │  FUNDS  │ │ PEOPLE  │                         │                         │
│   └─────────┘ └─────────┘                         │                         │
│   ┌─────────┐ ┌─────────┐                         ▼                         │
│   │COMPANIES│ │ TRUSTS  │          ┌──────────────────────────────────────┐ │
│   └─────────┘ └─────────┘          │          RELATIONSHIPS               │ │
│                                    │  (ownership, role, agreement, etc.)  │ │
│   Each has own schema,             └──────────────────────────────────────┘ │
│   own KYC, own documents                         │                         │
│                                                  ▼                         │
│                                    ┌──────────────────────────────────────┐ │
│                                    │            CONTEXT                   │ │
│                                    │  (CBU determines role/perspective)   │ │
│                                    └──────────────────────────────────────┘ │
│                                                                              │
│   BENEFIT: Same entity, same KYC, multiple contexts                         │
│   Goldman Sachs is ONE entity that can be:                                  │
│     - Counterparty to Fund A (ISDA)                                         │
│     - Investor in Fund B (shareholder)                                      │
│     - Service provider to Fund C (prime broker)                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### CBU (Client Business Unit)

**What it is:** The organizing principle. A CBU represents a BNY client entity (fund, company, trust) that we're onboarding or servicing.

**Why it exists:** 
- Provides **context** for all operations
- Everything happens "within" a CBU scope
- KYC, documents, roles, products all attach to a CBU
- Enables multi-client isolation in same database

**Key insight:** CBU is NOT just a client record. It's a **lens** through which we view entity relationships.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CBU AS LENS                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Without CBU context:              With CBU context:                        │
│   ────────────────────              ─────────────────                        │
│                                                                              │
│   ┌───┐   ┌───┐   ┌───┐            Viewing as "Acme Fund" CBU:              │
│   │ A │───│ B │───│ C │                                                      │
│   └───┘   └───┘   └───┘            ┌───────────────────────────────────┐    │
│   │       │       │                │  Acme Fund (ME)                   │    │
│   └───────┴───────┘                │    │                              │    │
│   (who relates to whom?)           │    ├── John (Director, UBO 40%)   │    │
│                                    │    ├── Jane (Signatory)           │    │
│                                    │    ├── ManCo (Manager)            │    │
│                                    │    └── Goldman (Counterparty)     │    │
│                                    └───────────────────────────────────┘    │
│                                                                              │
│   CBU provides: perspective, ownership, role semantics                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**CBU Hierarchy:**
- CBUs can have parent CBUs (fund → ManCo → holding company)
- Enables group-level views
- Products/services can be shared across CBU hierarchy

---

### UBO (Ultimate Beneficial Owner)

**What it is:** Natural persons who ultimately own/control ≥25% of an entity.

**Why it matters:** Regulatory requirement (AML/KYC). Must identify real humans behind corporate structures.

**The challenge:** Ownership chains can be deep and complex.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           UBO DISCOVERY                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   The Problem:                                                               │
│   ────────────                                                               │
│                                                                              │
│   Who owns "Acme Fund"?                                                     │
│                                                                              │
│       ┌─────────────┐                                                       │
│       │  Acme Fund  │                                                       │
│       └──────┬──────┘                                                       │
│              │ 100% owned by                                                │
│              ▼                                                              │
│       ┌─────────────┐                                                       │
│       │  Acme ManCo │                                                       │
│       └──────┬──────┘                                                       │
│              │ 100% owned by                                                │
│              ▼                                                              │
│       ┌─────────────┐                                                       │
│       │ Acme HoldCo │                                                       │
│       └──────┬──────┘                                                       │
│              │                                                               │
│       ┌──────┴──────┐                                                       │
│       │             │                                                       │
│       ▼             ▼                                                       │
│   ┌───────┐    ┌───────┐                                                    │
│   │ John  │    │ Jane  │                                                    │
│   │ (60%) │    │ (40%) │   ◀── THESE are the UBOs                          │
│   └───────┘    └───────┘                                                    │
│                                                                              │
│   Our Approach:                                                              │
│   ─────────────                                                              │
│                                                                              │
│   1. Holdings (investor register) capture direct shareholdings              │
│   2. Entity relationships capture ownership edges                           │
│   3. GLEIF API provides corporate hierarchy (LEI → parent → ultimate)       │
│   4. Trigger: holding ≥25% → auto-create entity_relationship               │
│   5. Graph traversal finds natural persons at chain end                     │
│                                                                              │
│   Key Tables:                                                                │
│   - holdings (direct positions)                                             │
│   - entity_relationships (ownership edges with %)                           │
│   - gleif_relationships (external hierarchy data)                           │
│   - entity_workstreams (KYC review per discovered entity)                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Dual-Use Holdings Pattern:**
- `usage_type = 'TA'`: Transfer Agency - tracking client's investors for KYC-as-a-service
- `usage_type = 'UBO'`: Intra-group holdings for beneficial ownership discovery
- Same table, different purpose, unified BODS export

---

### Documents & Attribute Dictionary

**The Problem:** KYC requires collecting specific documents and data points that vary by:
- Entity type (person vs company vs trust)
- Jurisdiction (LU vs IE vs KY vs UK)
- Risk rating (low vs medium vs high)
- Product type (custody vs fund admin vs collateral)

**Traditional approach:** Hardcoded logic scattered across codebase.

**Our approach:** Configuration-driven attribute dictionary.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      ATTRIBUTE DICTIONARY STRATEGY                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   attribute_catalogue                                                        │
│   ───────────────────                                                        │
│   Defines WHAT can be collected:                                            │
│     - passport_number (type: string, PII: true)                             │
│     - date_of_birth (type: date, PII: true)                                 │
│     - company_registration_number (type: string)                            │
│     - source_of_wealth (type: enum[inheritance, business, ...])             │
│                                                                              │
│   document_catalogue                                                         │
│   ──────────────────                                                         │
│   Defines WHAT documents exist:                                             │
│     - PASSPORT (for: proper_person)                                         │
│     - UTILITY_BILL (for: proper_person, address proof)                      │
│     - CERT_OF_INCORPORATION (for: limited_company)                          │
│     - TRUST_DEED (for: trust)                                               │
│                                                                              │
│   requirement_matrix                                                         │
│   ──────────────────                                                         │
│   Defines WHEN things are required:                                         │
│                                                                              │
│   ┌────────────────┬──────────────┬────────────┬───────────┬─────────────┐  │
│   │ Entity Type    │ Jurisdiction │ Risk Level │ Attribute │ Required?   │  │
│   ├────────────────┼──────────────┼────────────┼───────────┼─────────────┤  │
│   │ proper_person  │ *            │ *          │ passport  │ YES         │  │
│   │ proper_person  │ *            │ HIGH       │ source_of │ YES         │  │
│   │                │              │            │ _wealth   │             │  │
│   │ limited_company│ LU           │ *          │ reg_number│ YES         │  │
│   │ limited_company│ KY           │ *          │ reg_number│ OPTIONAL    │  │
│   │ trust          │ *            │ *          │ trust_deed│ YES         │  │
│   └────────────────┴──────────────┴────────────┴───────────┴─────────────┘  │
│                                                                              │
│   BENEFIT: Change requirements via config, not code                         │
│   BENEFIT: Audit trail of what was required vs collected                    │
│   BENEFIT: Different rules for different jurisdictions                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Products, Services & Service Resources

**The Hierarchy:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PRODUCT → SERVICE → RESOURCE HIERARCHY                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   PRODUCT (what we sell)                                                    │
│   ──────────────────────                                                     │
│   High-level offering: Custody, Fund Accounting, Collateral Management      │
│   Attached to: CBU                                                          │
│   Example: "Acme Fund has Custody product"                                  │
│                                                                              │
│        │                                                                     │
│        │ 1:N                                                                │
│        ▼                                                                     │
│                                                                              │
│   SERVICE (what we do)                                                      │
│   ────────────────────                                                       │
│   Specific capability within product:                                       │
│     Custody → Settlement, Safekeeping, Corporate Actions, Tax Reclaim      │
│     Fund Accounting → NAV Calculation, Investor Reporting, Audit Support   │
│   Attached to: CBU + Product                                                │
│                                                                              │
│        │                                                                     │
│        │ 1:N                                                                │
│        ▼                                                                     │
│                                                                              │
│   SERVICE RESOURCE (what we use)                                            │
│   ──────────────────────────────                                             │
│   Concrete instance that delivers service:                                  │
│     Settlement → SSI (Standard Settlement Instructions)                     │
│     Settlement → Depot Account at Clearstream                               │
│     Safekeeping → Subcustodian relationship                                 │
│   Has: configuration, status, effective dates                               │
│                                                                              │
│   ─────────────────────────────────────────────────────────────────────     │
│                                                                              │
│   EXAMPLE:                                                                   │
│                                                                              │
│   Acme Fund (CBU)                                                           │
│   └── Custody (Product)                                                     │
│       ├── Settlement (Service)                                              │
│       │   ├── SSI for USD (Resource)                                        │
│       │   ├── SSI for EUR (Resource)                                        │
│       │   └── Clearstream depot 12345 (Resource)                            │
│       └── Safekeeping (Service)                                             │
│           └── State Street subcustody (Resource)                            │
│                                                                              │
│   WHY THIS MATTERS:                                                          │
│   - Products drive pricing/billing                                          │
│   - Services drive operational workflows                                     │
│   - Resources drive actual execution                                         │
│   - All three need KYC/due diligence                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Trading Matrix

**The Problem:** For custody/settlement, we need to know:
- What instruments can this client trade?
- On which markets?
- With which counterparties?
- Under which agreements?
- Who provides what service?

**The Solution:** Trading Matrix - a multi-dimensional configuration.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           TRADING MATRIX                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   Dimensions:                                                                │
│   ───────────                                                                │
│   - CBU (who)                                                               │
│   - Asset Class (what: equity, bond, derivative, FX)                        │
│   - Market (where: NYSE, LSE, Eurex)                                        │
│   - Counterparty (with whom: broker, dealer)                                │
│   - Settlement Route (how: Clearstream, Euroclear, Fed)                     │
│   - Agreement (under what: ISDA, GMSLA, MRA)                                │
│                                                                              │
│   Matrix Entry:                                                              │
│   ─────────────                                                              │
│   "Acme Fund CAN trade US Equities on NYSE                                  │
│    via Goldman Sachs, settling at DTC,                                      │
│    under existing custody agreement"                                         │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                        TRADING PROFILE                               │   │
│   │─────────────────────────────────────────────────────────────────────│   │
│   │                                                                      │   │
│   │  cbu_trading_profiles                                               │   │
│   │  └── asset_class: EQUITY                                            │   │
│   │      └── markets: [NYSE, NASDAQ]                                    │   │
│   │          └── counterparties: [Goldman, Morgan Stanley]              │   │
│   │              └── settlement_routes: [DTC]                           │   │
│   │                  └── agreements: [custody_agreement_001]            │   │
│   │                                                                      │   │
│   │  This enables:                                                       │   │
│   │  - Trade eligibility checks                                         │   │
│   │  - Automated routing decisions                                       │   │
│   │  - Regulatory reporting                                              │   │
│   │  - Fee calculation                                                   │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   WHY NOT JUST A BIG TABLE?                                                  │
│   ─────────────────────────                                                  │
│   - Combinatorial explosion (10 asset classes × 50 markets × ...)           │
│   - Instead: hierarchical rules with inheritance                            │
│   - "Trade equities globally" expands to specific markets                   │
│   - Exceptions override defaults                                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Agent & MCP Integration Strategy

### The Core Philosophy

**LLM does DISCOVERY. DSL does EXECUTION.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     AGENT INTEGRATION PHILOSOPHY                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   THE PROBLEM WITH PURE LLM AGENTS:                                         │
│   ──────────────────────────────────                                         │
│                                                                              │
│   User: "Onboard Acme Fund"                                                 │
│                                                                              │
│   Pure LLM approach:                                                         │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ LLM generates SQL directly:                                          │   │
│   │ INSERT INTO clients (name) VALUES ('Acme Fund');                    │   │
│   │ INSERT INTO products (client_id, type) VALUES (1, 'custody');       │   │
│   │ -- Maybe wrong table? Wrong schema? Missing fields?                 │   │
│   │ -- No validation, no audit, no consistency                          │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   PROBLEMS:                                                                  │
│   - LLM can hallucinate table/column names                                  │
│   - No business rule validation                                             │
│   - No audit trail                                                          │
│   - Hard to review/approve                                                  │
│   - Non-deterministic                                                       │
│                                                                              │
│   ─────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│   OUR APPROACH: LLM → DSL → Execution                                       │
│   ──────────────────────────────────                                         │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ LLM generates DSL (constrained vocabulary):                         │   │
│   │                                                                      │   │
│   │ (cbu.ensure :name "Acme Fund" :jurisdiction "LU" :as @fund)         │   │
│   │ (cbu.add-product :cbu-id @fund :product-type "custody")             │   │
│   │                                                                      │   │
│   │ DSL is:                                                              │   │
│   │ - Validated against known verbs                                      │   │
│   │ - Type-checked (cbu-id must be UUID or resolvable name)             │   │
│   │ - Auditable (exact operations logged)                               │   │
│   │ - Reviewable (human can read and approve)                           │   │
│   │ - Deterministic (same DSL = same result)                            │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   The DSL acts as a CONSTRAINT LAYER between fuzzy LLM and crisp database  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Research Macros: Fuzzy → Deterministic Bridge

**The Pattern:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RESEARCH MACRO PATTERN                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   User wants: "Find all PEPs connected to Acme and flag them"               │
│                                                                              │
│   This requires:                                                             │
│   1. DISCOVERY: What entities are connected to Acme? (fuzzy)                │
│   2. FILTERING: Which are PEPs? (fuzzy - needs screening)                   │
│   3. ACTION: Create tasks for each (deterministic)                          │
│                                                                              │
│   Research Macro Syntax:                                                     │
│   ──────────────────────                                                     │
│   @research {                                                                │
│       Find all UBOs and directors of Acme Fund                              │
│       who are politically exposed persons                                    │
│       and create enhanced due diligence tasks for each                      │
│   }                                                                          │
│                                                                              │
│   Execution:                                                                 │
│   ──────────                                                                 │
│                                                                              │
│   ┌──────────────┐                                                          │
│   │  @research   │                                                          │
│   │   block      │                                                          │
│   └──────┬───────┘                                                          │
│          │                                                                   │
│          ▼                                                                   │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │  LLM PHASE (discovery)                                                │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  Context: Schema, verb reference, current CBU                        │  │
│   │  Task: Generate DSL that answers the query                           │  │
│   │  Output: Valid DSL statements                                        │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│          │                                                                   │
│          │ Generated DSL                                                    │
│          ▼                                                                   │
│   ┌──────────────────────────────────────────────────────────────────────┐  │
│   │  DSL PHASE (deterministic execution)                                  │  │
│   │──────────────────────────────────────────────────────────────────────│  │
│   │  (entity.list-ubos :cbu-id "Acme Fund" :as @ubos)                    │  │
│   │  (entity.list-by-role :cbu-id "Acme Fund" :role "director" :as @dirs)│  │
│   │  @foreach @ubos + @dirs as @person {                                 │  │
│   │      (screening.check-pep :entity-id @person.entity_id :as @result)  │  │
│   │      @if @result.is_pep {                                            │  │
│   │          (task.create :entity-id @person.entity_id                   │  │
│   │                       :type "ENHANCED_DUE_DILIGENCE")                │  │
│   │      }                                                               │  │
│   │  }                                                                   │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│          │                                                                   │
│          ▼                                                                   │
│   ┌──────────────┐                                                          │
│   │   Results    │                                                          │
│   │  (audited,   │                                                          │
│   │  deterministic)                                                         │
│   └──────────────┘                                                          │
│                                                                              │
│   KEY INSIGHT:                                                               │
│   - LLM picks WHICH verbs to use (reasoning)                                │
│   - DSL ensures HOW they execute (correctness)                              │
│   - Audit log shows exact operations performed                              │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### MCP (Model Context Protocol) Integration

**Why MCP:**
- Standardized way for LLMs to access external tools
- Claude/other models can call our DSL as MCP tools
- Enables multi-tool workflows (DSL + web search + file ops)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        MCP INTEGRATION STRATEGY                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   MCP Server (rust/src/mcp/)                                                │
│   ──────────────────────────                                                 │
│                                                                              │
│   Exposes DSL operations as MCP tools:                                      │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  Tool: execute_dsl                                                   │   │
│   │  ─────────────────                                                   │   │
│   │  Input: { "dsl": "(cbu.ensure :name \"Acme\" ...)" }                │   │
│   │  Output: { "success": true, "result": {...}, "captured": {...} }    │   │
│   │                                                                      │   │
│   │  Tool: query_entities                                                │   │
│   │  ────────────────────                                                │   │
│   │  Input: { "cbu_id": "...", "entity_type": "proper_person" }         │   │
│   │  Output: { "entities": [...] }                                       │   │
│   │                                                                      │   │
│   │  Tool: get_verb_reference                                            │   │
│   │  ────────────────────────                                            │   │
│   │  Input: { "domain": "cbu" }                                         │   │
│   │  Output: { "verbs": ["ensure", "add-product", ...], "args": {...} } │   │
│   │                                                                      │   │
│   │  Tool: validate_dsl                                                  │   │
│   │  ─────────────────                                                   │   │
│   │  Input: { "dsl": "..." }                                            │   │
│   │  Output: { "valid": true/false, "errors": [...] }                   │   │
│   │                                                                      │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   Usage Pattern:                                                             │
│   ──────────────                                                             │
│                                                                              │
│   1. LLM calls get_verb_reference to understand available operations        │
│   2. LLM generates DSL based on user request                                │
│   3. LLM calls validate_dsl to check syntax                                 │
│   4. If invalid, LLM fixes and revalidates                                  │
│   5. LLM calls execute_dsl to run validated DSL                             │
│   6. LLM interprets results for user                                        │
│                                                                              │
│   This keeps LLM in the loop for reasoning while DSL ensures correctness   │
│                                                                              │
│   ─────────────────────────────────────────────────────────────────────────  │
│                                                                              │
│   MULTI-TOOL WORKFLOWS:                                                      │
│   ─────────────────────                                                      │
│                                                                              │
│   User: "Research Acme's ultimate parent using GLEIF and set up KYC"        │
│                                                                              │
│   LLM orchestrates:                                                          │
│   1. execute_dsl → (gleif.trace-ownership :entity-id "Acme" :as @chain)    │
│   2. [LLM analyzes ownership chain, identifies ultimate parent]             │
│   3. execute_dsl → (kyc.open-case :cbu-id "Acme" :case-type "ONBOARDING")  │
│   4. execute_dsl → (kyc.add-entity :case-id @case :entity-id @ultimate...)  │
│   5. [LLM summarizes actions taken]                                         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Agent Modes

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            AGENT MODES                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   MODE 1: LEXICON (Fast, Local, Deterministic)                              │
│   ─────────────────────────────────────────────                              │
│   - No LLM API call                                                         │
│   - Pattern matching on known phrases                                       │
│   - Best for: simple commands, known patterns                               │
│   - Example: "Add John as director" → lexicon matches → DSL generated       │
│                                                                              │
│   MODE 2: LLM (Flexible, API, Reasoning)                                    │
│   ─────────────────────────────────────                                      │
│   - Claude API call with context                                            │
│   - Can handle novel requests                                               │
│   - Best for: complex queries, multi-step reasoning                         │
│   - Example: "Set up standard Luxembourg fund structure"                    │
│                                                                              │
│   MODE 3: RESEARCH MACRO (Hybrid)                                           │
│   ────────────────────────────────                                           │
│   - LLM for discovery/planning                                              │
│   - DSL for execution                                                       │
│   - Best for: exploratory queries, conditional logic                        │
│   - Example: @research { find compliance gaps }                             │
│                                                                              │
│   MODE 4: CONDUCTOR (Multi-turn with State)                                 │
│   ─────────────────────────────────────────                                  │
│   - Maintains conversation state                                            │
│   - Captures persist across turns                                           │
│   - Confirmation before execution                                           │
│   - Best for: interactive onboarding sessions                               │
│                                                                              │
│   SELECTION LOGIC:                                                           │
│   ────────────────                                                           │
│   1. Try lexicon first (fast path)                                          │
│   2. If no match → LLM mode                                                 │
│   3. If @research block → research macro mode                               │
│   4. If interactive session → conductor mode                                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. egui Do's and Don'ts

### Critical Rules

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         egui DO'S AND DON'TS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   egui is IMMEDIATE MODE. This changes everything.                          │
│                                                                              │
│   RETAINED MODE (React, etc):      IMMEDIATE MODE (egui):                   │
│   ────────────────────────         ─────────────────────                    │
│   - State stored in UI             - State stored externally                │
│   - UI updates on state change     - UI rebuilt every frame                 │
│   - Event handlers                 - Direct conditionals                    │
│   - Virtual DOM diffing            - No diffing needed                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

╔═════════════════════════════════════════════════════════════════════════════╗
║                              DO's                                            ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  ✓ Store ALL state outside egui (in your App struct)                        ║
║                                                                              ║
║    struct MyApp {                                                            ║
║        selected_entity: Option<Uuid>,   // ← State here                     ║
║        entities: Vec<Entity>,           // ← Data here                      ║
║        show_modal: bool,                // ← UI state here too              ║
║    }                                                                         ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✓ Use response.clicked() / response.changed() for interactions             ║
║                                                                              ║
║    if ui.button("Click me").clicked() {                                     ║
║        self.counter += 1;  // Mutate state directly                         ║
║    }                                                                         ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✓ Keep update() function fast (called every frame ~60fps)                  ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✓ Use ctx.request_repaint() sparingly (only when async data arrives)       ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✓ Use egui_extras::Table for large lists (virtualizes rows)                ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✓ Use channels (mpsc) for async operations                                 ║
║                                                                              ║
║    // In App struct:                                                         ║
║    rx: Receiver<ApiResult>,                                                  ║
║                                                                              ║
║    // In update():                                                           ║
║    while let Ok(result) = self.rx.try_recv() {                              ║
║        self.data = result;                                                   ║
║    }                                                                         ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✓ Use ui.id_source() or ui.push_id() for dynamic lists                     ║
║                                                                              ║
║    for (i, item) in items.iter().enumerate() {                              ║
║        ui.push_id(i, |ui| {                                                 ║
║            ui.label(&item.name);                                            ║
║        });                                                                   ║
║    }                                                                         ║
║                                                                              ║
╚═════════════════════════════════════════════════════════════════════════════╝

╔═════════════════════════════════════════════════════════════════════════════╗
║                              DON'Ts                                          ║
╠═════════════════════════════════════════════════════════════════════════════╣
║                                                                              ║
║  ✗ DON'T do async/await inside update()                                     ║
║                                                                              ║
║    // WRONG - blocks UI thread                                              ║
║    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {                ║
║        let data = fetch_data().await;  // ← BLOCKS!                         ║
║    }                                                                         ║
║                                                                              ║
║    // RIGHT - spawn task, receive via channel                               ║
║    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {                ║
║        if ui.button("Load").clicked() {                                     ║
║            let tx = self.tx.clone();                                        ║
║            tokio::spawn(async move {                                        ║
║                let data = fetch_data().await;                               ║
║                tx.send(data).ok();                                          ║
║            });                                                               ║
║        }                                                                     ║
║    }                                                                         ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✗ DON'T allocate in hot paths                                              ║
║                                                                              ║
║    // WRONG - allocates every frame                                         ║
║    ui.label(format!("Count: {}", self.count));                              ║
║                                                                              ║
║    // BETTER - for static strings                                           ║
║    ui.label("Count: ");                                                     ║
║    ui.label(self.count.to_string());  // Still allocates, but less         ║
║                                                                              ║
║    // BEST - cache formatted string if count rarely changes                 ║
║    if self.count_changed {                                                  ║
║        self.count_str = format!("Count: {}", self.count);                   ║
║        self.count_changed = false;                                          ║
║    }                                                                         ║
║    ui.label(&self.count_str);                                               ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✗ DON'T store UI state in egui widgets                                     ║
║                                                                              ║
║    // WRONG - expects widget to remember                                    ║
║    ui.text_edit_singleline(&mut String::new());  // Resets every frame!    ║
║                                                                              ║
║    // RIGHT - state in App                                                  ║
║    ui.text_edit_singleline(&mut self.search_text);                          ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✗ DON'T use thread::sleep() or blocking I/O                                ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✗ DON'T create windows/panels conditionally without persistence            ║
║                                                                              ║
║    // WRONG - window state lost when hidden                                 ║
║    if self.show_window {                                                    ║
║        Window::new("Settings")...                                           ║
║    }                                                                         ║
║                                                                              ║
║    // RIGHT - window always exists, just open/closed                        ║
║    Window::new("Settings")                                                  ║
║        .open(&mut self.show_settings)  // Controls visibility              ║
║        .show(ctx, |ui| { ... });                                            ║
║                                                                              ║
║  ─────────────────────────────────────────────────────────────────────────  ║
║                                                                              ║
║  ✗ DON'T forget to clone data for closures                                  ║
║                                                                              ║
║    // WRONG - borrow checker error                                          ║
║    if ui.button("Save").clicked() {                                         ║
║        save(&self.data);  // Can't borrow self mutably and immutably       ║
║    }                                                                         ║
║                                                                              ║
║    // RIGHT - clone what you need                                           ║
║    let data_to_save = self.data.clone();                                    ║
║    if ui.button("Save").clicked() {                                         ║
║        save(&data_to_save);                                                  ║
║    }                                                                         ║
║                                                                              ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

### Async Pattern for ob-poc

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ASYNC PATTERN FOR ob-poc UI                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   struct App {                                                               │
│       // State                                                               │
│       session: Arc<RwLock<SharedSession>>,                                  │
│       entities: Vec<Entity>,                                                │
│       loading: bool,                                                        │
│                                                                              │
│       // Async communication                                                 │
│       cmd_tx: Sender<Command>,           // Send commands to backend        │
│       result_rx: Receiver<Result>,       // Receive results                 │
│   }                                                                          │
│                                                                              │
│   impl eframe::App for App {                                                │
│       fn update(&mut self, ctx: &Context, _frame: &mut Frame) {             │
│           // 1. Check for async results (non-blocking)                      │
│           while let Ok(result) = self.result_rx.try_recv() {                │
│               match result {                                                 │
│                   Result::Entities(e) => {                                  │
│                       self.entities = e;                                    │
│                       self.loading = false;                                 │
│                   }                                                          │
│                   Result::DslExecuted(r) => {                               │
│                       self.session.write().last_result = Some(r);          │
│                   }                                                          │
│               }                                                              │
│               ctx.request_repaint();  // Trigger redraw                     │
│           }                                                                  │
│                                                                              │
│           // 2. Render UI                                                    │
│           CentralPanel::default().show(ctx, |ui| {                          │
│               if self.loading {                                             │
│                   ui.spinner();                                             │
│               } else {                                                       │
│                   for entity in &self.entities {                            │
│                       ui.label(&entity.name);                               │
│                   }                                                          │
│               }                                                              │
│                                                                              │
│               // 3. Handle user actions                                      │
│               if ui.button("Refresh").clicked() {                           │
│                   self.loading = true;                                      │
│                   self.cmd_tx.send(Command::LoadEntities).ok();             │
│               }                                                              │
│           });                                                                │
│       }                                                                      │
│   }                                                                          │
│                                                                              │
│   // Backend runs in separate tokio runtime                                 │
│   async fn backend(cmd_rx: Receiver<Command>, result_tx: Sender<Result>) { │
│       while let Ok(cmd) = cmd_rx.recv() {                                   │
│           match cmd {                                                        │
│               Command::LoadEntities => {                                    │
│                   let entities = db::load_entities().await;                 │
│                   result_tx.send(Result::Entities(entities)).ok();          │
│               }                                                              │
│               Command::ExecuteDsl(dsl) => {                                 │
│                   let result = executor::run(&dsl).await;                   │
│                   result_tx.send(Result::DslExecuted(result)).ok();         │
│               }                                                              │
│           }                                                                  │
│       }                                                                      │
│   }                                                                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Reference

| Concept | Key Insight | Where to Look |
|---------|-------------|---------------|
| CBU | Lens/context, not just a record | `cbus` table, `cbu.ensure` verb |
| Entity | Unified model for all legal persons | `entities` table |
| UBO | Graph traversal to natural persons | `entity_relationships`, `gleif_relationships` |
| Holdings | Dual-use: TA (investor KYC) + UBO (ownership) | `kyc.holdings` |
| Products | What we sell (high level) | `products` table |
| Services | What we do (operational) | `services` table |
| Service Resources | Concrete instances (SSI, depot) | `service_resources` table |
| Trading Matrix | Multi-dimensional eligibility | `cbu_trading_profiles` |
| Attribute Dictionary | Config-driven requirements | `attribute_catalogue`, `document_catalogue` |
| Agent | LLM → DSL → Execution | `ob-agentic` crate |
| Research Macro | Fuzzy discovery → deterministic execution | `@research { }` blocks |
| MCP | Expose DSL as tools for LLM | `rust/src/mcp/` |

---

Generated: 2026-01-09
