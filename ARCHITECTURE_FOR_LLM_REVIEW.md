# Architecture for LLM Review: ob-poc Domain Model Alignment Audit

## 1. Executive Summary

This document describes the semantic architecture of **ob-poc**, a custody banking onboarding and fund administration platform. The system manages the lifecycle of investment fund structures from prospect research through KYC/AML approval, covering UBO resolution, sanctions/PEP screening, document collection, deal origination, and trading mandate configuration.

**Scale:** ~1,418 canonical verbs across ~60 domains, 23,405 intent patterns, ~306 database tables, 20+ jurisdiction-specific fund structure templates (constellations), 6 state machines governing lifecycle flows.

**Core abstractions:**
- **CBU (Client Business Unit):** The atomic operating unit — a single fund or trading entity requiring custody services. Industry equivalent: a custody account mandate or sub-custodian service agreement.
- **Constellation:** A jurisdiction-specific template defining required roles, dependencies, and onboarding steps for a fund structure type (e.g., Luxembourg UCITS SICAV, Irish ICAV, US 40-Act Open-End).
- **SemOS (Semantic Operating System):** An immutable governance registry providing ABAC-controlled verb surfaces, attribute definitions, and context-aware verb resolution.

**Operator vocabulary layer:** The system deliberately uses business-friendly terms ("structure", "party", "case", "mandate") in its UI and NLU pipeline, mapping them to technical DSL primitives (`cbu.*`, `entity.*`, `kyc-case.*`, `trading-profile.*`). This dual vocabulary is intentional.

---

## 2. Entity Catalogue

### 2.1 Client Group

| Aspect | Detail |
|--------|--------|
| **Business definition** | A commercial client relationship — the top-level grouping of all entities and funds under a single institutional client (e.g., "Allianz Global Investors", "BlackRock") |
| **Industry equivalent** | Relationship Management Entity, Master Client Record, Commercial Client |
| **Lifecycle states** | prospect → researching → ubo_mapped → control_mapped → cbus_identified → onboarding → active → dormant → offboarded |
| **Key transitions** | Research (GLEIF import) kicks off discovery; UBO mapping gates CBU identification; all cases approved gates "active" |
| **Relationships** | 1:N entities (via client_group_entity), 1:N CBUs (via cbu_entity_roles), 1:N anchors by role (ultimate_parent, governance_controller, regulatory_anchor) |
| **Regulatory surface** | Group-level UBO determination, consolidated risk assessment, AML group risk rating |

### 2.2 Entity (Legal Entity / Natural Person)

| Aspect | Detail |
|--------|--------|
| **Business definition** | Any legal or natural person in the system — companies, individuals, trusts, partnerships, nominees |
| **Industry equivalent** | Party, Counterparty, Legal Entity (ISO 17442 LEI holder), Natural Person |
| **Entity sub-types** | company, person, trust (discretionary, charitable, fixed_interest, unit), partnership, nominee, government_entity |
| **KYC lifecycle states** | empty → placeholder → filled → workstream_open → screening_complete → evidence_collected → verified → approved |
| **Key transitions** | Placeholder entities auto-created during structure setup; GLEIF enriches with LEI data; KYC workstream opens per-entity screening |
| **Relationships** | M:N ownership (entity_relationships, type=ownership with percentage), M:N control (type=control with control_type: board_member/executive/voting_rights/veto_rights/appointee), M:N trust roles (type=trust_role with role: settlor/trustee/beneficiary/protector), 1:N to CBUs via cbu_entity_roles |
| **Regulatory surface** | KYC/CDD subject, sanctions screening target, PEP screening target, UBO candidate (natural persons with ownership >= threshold), BODS 0.4 interest type coding |
| **Notable feature** | Temporal ownership — relationships have effective_from/effective_to dates enabling point-in-time queries |

### 2.3 CBU (Client Business Unit)

| Aspect | Detail |
|--------|--------|
| **Business definition** | The atomic unit of custody service delivery — typically a single fund, sub-fund, or segregated mandate requiring its own KYC case, trading profile, and custody account |
| **Industry equivalent** | Custody Account, Fund Account, Sub-custodian Mandate, Client Portfolio |
| **Lifecycle** | Created (DISCOVERED) → VALIDATED → ACTIVE; soft-delete enforced |
| **Key properties** | name, jurisdiction, client_type (FUND, CORPORATE), nature_purpose, commercial_client_entity_id |
| **Relationships** | M:N entities via cbu_entity_roles (management_company, depositary, investment_manager, general_partner, etc.), 1:N KYC cases, 1:N trading profiles, 1:1 client group (via commercial client entity), M:N structure links (parent/child for cross-border structures) |
| **Regulatory surface** | KYC case subject, sanctions screening scope, document requirement scope, tollgate approval unit |
| **Notable feature** | Idempotent creation by fund-entity-id — prevents duplicate CBUs for the same legal entity |

### 2.4 KYC Case

| Aspect | Detail |
|--------|--------|
| **Business definition** | A single KYC/CDD assessment for a CBU — the container for all due diligence activity (screening, evidence collection, risk rating, approval) |
| **Industry equivalent** | CDD Case, KYC Review, Customer Due Diligence Assessment, AML Case File |
| **Lifecycle states** | intake → discovery → assessment → review → approved/rejected/withdrawn/expired/refer_to_regulator/do_not_onboard |
| **Key transitions** | Risk rating required for assessment→review; review→approved requires all entity workstreams clear; refer_to_regulator is a terminal escalation; blocked→discovery via reopen |
| **Terminal states** | approved, rejected, withdrawn, expired, refer_to_regulator, do_not_onboard |
| **Relationships** | 1:1 CBU, 1:N entity workstreams, 1:N tollgate evaluations, 0:1 deal (optional commercial linkage), 1:N case events |
| **Regulatory surface** | The primary regulatory artifact — maps to CDD obligations under 4AMLD/5AMLD/6AMLD, FATF Recommendations, FinCEN CDD Rule |
| **Case types** | INITIAL (new onboarding), PERIODIC (scheduled review), EVENT_TRIGGERED (material change), REMEDIATION |

### 2.5 UBO (Ultimate Beneficial Owner)

| Aspect | Detail |
|--------|--------|
| **Business definition** | A natural person who ultimately owns or controls a legal entity, typically defined by ownership >= 25% (configurable threshold) or through control mechanisms |
| **Industry equivalent** | Ultimate Beneficial Owner (4AMLD Art. 3(6)), Beneficial Owner (FinCEN CDD Rule), Controlling Person (CRS) |
| **Epistemic lifecycle** | undiscovered → alleged → provable → proved → approved |
| **Key transitions** | alleged = ownership claim exists but unverified; provable = evidence collected; proved = evidence verified; approved = case-level approval |
| **Ownership types** | DIRECT, INDIRECT, BENEFICIAL, DIRECTLY_CONSOLIDATED, ULTIMATELY_CONSOLIDATED |
| **Control types** | board_member, executive, voting_rights, veto_rights, appointee |
| **Trust roles** | settlor, trustee, beneficiary (fixed/discretionary/contingent), protector |
| **Chain terminus types** | LISTED_COMPANY, GOVERNMENT_ENTITY, REGULATED_FUND, WIDELY_HELD, EXCHANGE_TRADED |
| **Notable features** | Snapshot capture for deterministic replay; diff between determination runs; convergence model for ownership supersession; deceased marking with cascade; configurable threshold (default 25%, down to 5% for chain computation); BODS 0.4 interest type interoperability |

### 2.6 Screening

| Aspect | Detail |
|--------|--------|
| **Business definition** | Compliance screening of an entity against sanctions lists, PEP databases, and adverse media sources |
| **Industry equivalent** | Name Screening, AML Screening, Sanctions Check, PEP Check, KYC Screening |
| **Lifecycle states** | not_started → sanctions_pending/clear/hit → pep_pending/clear/hit → media_pending/clear/hit → all_clear / escalated → resolved |
| **Screening types** | SANCTIONS, PEP, ADVERSE_MEDIA, CREDIT, CRIMINAL, REGULATORY, CONSOLIDATED |
| **Hit resolution** | HIT_PENDING_REVIEW → HIT_CONFIRMED (triggers red flag) / HIT_DISMISSED (false positive) |
| **Relationships** | 1:1 entity workstream, 0:N red flags, 0:1 review notes |
| **Notable feature** | Bulk refresh across all workstreams in a case; rescreen from all_clear back to not_started |

### 2.7 Document / Evidence

| Aspect | Detail |
|--------|--------|
| **Business definition** | Three-layer document model: Requirement (what is needed), Document (logical identity), Version (each submission) |
| **Industry equivalent** | KYC Document, CDD Evidence, Supporting Documentation |
| **Lifecycle states** | missing → requested → received → in_qa → verified / rejected → expired |
| **Rejection codes** | Quality (UNREADABLE, CUTOFF, GLARE), Mismatch (WRONG_DOC_TYPE, WRONG_PERSON), Validity (EXPIRED, UNDATED), Data (DOB_MISMATCH, NAME_MISMATCH), Authenticity (SUSPECTED_ALTERATION) |
| **Special transitions** | waived (manual override with justification), reinstate (undo waiver) |
| **Relationships** | N:1 entity (per-entity requirements), 1:N versions (immutable submissions), 0:1 QA reviewer |

### 2.8 Deal Record

| Aspect | Detail |
|--------|--------|
| **Business definition** | Commercial origination hub linking sales, contracting, rate negotiation, and onboarding into a single lifecycle |
| **Industry equivalent** | Sales Opportunity, Commercial Deal, MSA/Fee Agreement, Revenue Pipeline Record |
| **Lifecycle states** | PROSPECT → QUALIFYING → NEGOTIATING → CONTRACTED → ONBOARDING → ACTIVE → WINDING_DOWN → OFFBOARDED (CANCELLED from any state) |
| **Rate card lifecycle** | DRAFT → PROPOSED → COUNTER_OFFERED ↔ REVISED → AGREED → SUPERSEDED |
| **Pricing models** | BPS (basis points on AUM), FLAT (fixed fee), TIERED (volume-based), PER_TRANSACTION |
| **Relationships** | 1:N participants (sales_owner, relationship_manager, legal_counsel), 1:N contracts, 1:N rate cards with lines, 1:N onboarding requests → CBUs, 1:N fee billing profiles |

### 2.9 Contract

| Aspect | Detail |
|--------|--------|
| **Business definition** | Legal contract (MSA) between the custodian and client, carrying product-level subscriptions that gate CBU onboarding |
| **Industry equivalent** | Master Service Agreement, Custody Agreement, Global Custodian Contract |
| **Onboarding gate** | CBU can only be subscribed if contract covers the required product — no contract = no onboarding |
| **Relationships** | 1:N contract_products (with rate_card_id), 1:N cbu_subscriptions (FK enforced) |

### 2.10 Trading Profile (Mandate)

| Aspect | Detail |
|--------|--------|
| **Business definition** | The trading configuration for a CBU — instruments, markets, currencies, settlement routes, booking rules, counterparties |
| **Industry equivalent** | Investment Mandate, Trading Authorization Profile, Custody Service Configuration, Market Access Profile |
| **Operator term** | "Mandate" (UI never shows "trading-profile") |
| **Key components** | Instrument universe (equities, fixed income, OTC), market access (MICs), settlement instructions (SSIs), booking rules (priority-ordered), ISDA/CSA agreements |

### 2.11 Tollgate

| Aspect | Detail |
|--------|--------|
| **Business definition** | A quality gate evaluation that checks whether all prerequisites are met before advancing a KYC case (screening coverage, evidence completeness, document verification) |
| **Industry equivalent** | Compliance Gate, Quality Checkpoint, Due Diligence Tollgate, Approval Gate |
| **Trigger** | Depends on case reaching "review" state |
| **Checks** | Document coverage, screening status, evidence verification, red flag status |

### 2.12 Booking Principal

| Aspect | Detail |
|--------|--------|
| **Business definition** | The legal entity through which the custodian books transactions — links client mandates to custodian operating entities across jurisdictions |
| **Industry equivalent** | Booking Entity, Operating Company, Custodian Branch, Sub-custodian |
| **Lifecycle** | Draft → Active → Suspended → Retired |
| **Relationships** | 1:N client-principal relationships, 1:N service availability (three-lane model), 1:N eligibility rulesets |

---

## 3. Verb Surface & Vocabulary Map

### 3.1 Verb Statistics

| Metric | Count |
|--------|-------|
| Total canonical verbs | ~1,418 |
| Total intent patterns (embeddings) | ~23,405 |
| Verb domains | ~60 |
| Operator macros | ~54 (18 multi-verb) |
| Jurisdiction-specific constellations | 20 |

### 3.2 Top Domain Verb Alignment Table

| Domain | DSL Verb | Plain English | Industry-Standard Term | Potential LLM Friction |
|--------|----------|---------------|----------------------|----------------------|
| **cbu** | `cbu.create` | Create a fund structure | Open Custody Account / Create Mandate | "CBU" is non-standard; LLM may not map to custody account |
| **cbu** | `cbu.assign-role` | Assign entity to fund role | Appoint Service Provider / Assign Mandate Role | Role cardinality model is domain-specific |
| **entity** | `entity.create` | Create a legal entity record | Party Registration / Entity Setup | Clear mapping |
| **entity** | `entity.ensure-or-placeholder` | Create placeholder entity | Provisional Party Record | "Placeholder" concept may confuse — it's a draft entity |
| **kyc-case** | `kyc-case.create` | Open KYC case | Initiate CDD Assessment | Clear mapping |
| **kyc-case** | `kyc-case.set-risk-rating` | Assign risk rating | Customer Risk Assessment | Standard CDD concept |
| **kyc-case** | `kyc-case.escalate` | Escalate to regulator | Regulatory Referral / SAR Filing Trigger | "refer_to_regulator" terminal state is domain-specific |
| **ubo** | `ubo.add-ownership` | Record ownership relationship | Register Beneficial Ownership Interest | Clear under 4AMLD |
| **ubo** | `ubo.trace-chains` | Trace ownership to natural persons | UBO Determination / Corporate Veil Piercing | "Pierce the veil" maps well |
| **ubo** | `ubo.mark-terminus` | Mark where ownership chain stops | Exempt Entity Designation (listed/government) | Good 4AMLD alignment |
| **screening** | `screening.sanctions` | Run sanctions check | OFAC/EU/UN Sanctions Screening | Clear mapping |
| **screening** | `screening.pep` | Run PEP screening | Politically Exposed Person Check | Clear mapping |
| **screening** | `screening.adverse-media` | Run adverse media check | Adverse Media Screening / Negative News Search | Clear mapping |
| **screening** | `screening.review-hit` | Adjudicate screening hit | Alert Disposition / Match Resolution | Standard compliance workflow |
| **document** | `document.solicit` | Request document from entity | Document Request / Evidence Solicitation | Clear mapping |
| **document** | `document.verify` | QA approve document version | Document Verification / Evidence Acceptance | Clear mapping |
| **document** | `document.reject` | QA reject with reason code | Document Rejection with Remediation Code | Rejection taxonomy is comprehensive |
| **deal** | `deal.create` | Create sales opportunity | Open Commercial Opportunity | Clear mapping |
| **deal** | `deal.create-rate-card` | Start fee negotiation | Fee Schedule Proposal | Custody-specific but standard |
| **deal** | `deal.agree-rate-card` | Finalize fee agreement | Fee Schedule Agreement | Clear mapping |
| **contract** | `contract.subscribe` | Subscribe CBU to contract product | Mandate Activation / Service Subscription | Gate function is domain-specific |
| **gleif** | `gleif.import-tree` | Import corporate hierarchy from GLEIF | LEI Hierarchy Lookup / Corporate Structure Import | Well-known (ISO 17442) |
| **ownership** | `ownership.trace-chain` | Trace ownership/control chain | Ownership Chain Resolution | Clear mapping |
| **control** | `control.build-graph` | Build control graph | Corporate Control Structure Analysis | Graph model may need explanation |
| **session** | `session.load-galaxy` | Load all CBUs under client | Load Client Portfolio Scope | "Galaxy" is system-specific astronomy metaphor |
| **billing** | `billing.create-profile` | Create billing configuration | Fee Profile Setup | Clear mapping |
| **billing** | `billing.calculate-fees` | Run fee calculation | Fee Computation / Invoice Generation | Clear mapping |
| **trading-profile** | `trading-profile.import` | Import trading mandate | Mandate Configuration Import | Clear mapping |
| **tollgate** | `tollgate.evaluate` | Run compliance quality gate | Due Diligence Checkpoint Evaluation | "Tollgate" is semi-standard |
| **fund** | `fund.create-umbrella` | Create umbrella fund structure | Umbrella Fund Registration | Jurisdiction-specific (UCITS/AIFMD) |

### 3.3 Macro Surface (Key Composite Operations)

| Macro FQN | Operator Label | Expands To | Business Meaning |
|-----------|---------------|------------|-----------------|
| `structure.setup` | Set up Structure | `cbu.create` + role assignments | Create a new fund and assign required service providers |
| `case.open` | Open Case | `kyc-case.create` | Initiate KYC review for a structure |
| `screening.full` | Full Screening | `screening.sanctions` + `screening.pep` + `screening.adverse-media` | Run all three standard compliance screenings |
| `kyc.full-review` | Full KYC Review | `case.open` → `screening.full` → `kyc.collect-documents` | End-to-end KYC workflow |
| `struct.lux.ucits.sicav` | Luxembourg UCITS SICAV | 13-step onboarding: CBU + ManCo + depositary + IM + UBO + case + mandate | Complete SICAV fund onboarding |
| `party.assign` | Assign Party | `cbu.assign-role` | Assign entity to role in fund structure |
| `mandate.create` | Create Mandate | `trading-profile.create` | Set up trading authorization for a structure |
| `ubo.discover` | Discover UBO | `ubo.trace-chains` | Find ultimate beneficial owners |
| `struct.hedge.cross-border` | Cross-Border Hedge Setup | Multi-jurisdiction CBU creation with structure links | Feeder/master/aggregator cross-border structure |

---

## 4. Domain Architecture

### 4.1 The Constellation Model

The constellation is the system's most distinctive architectural feature. Each constellation is a **jurisdiction-specific dependency graph** defining:

1. **Slots** — typed positions (CBU, entity, case, mandate, entity_graph) with cardinality (root, mandatory, optional, recursive)
2. **Dependencies** — DAG edges between slots (`depends_on`, with optional `min_state`)
3. **State machines** — each slot may reference a lifecycle state machine
4. **Verb bindings** — each slot declares which verbs are available in which states (`when: empty`, `when: filled`, `when: [placeholder, filled]`)
5. **Overlays** — data enrichments loaded alongside the slot (workstreams, screenings, evidence, red_flags)

**Example: Luxembourg UCITS SICAV constellation (`struct.lux.ucits.sicav`):**

```
cbu (root)
├── management_company (mandatory, entity, entity_kyc_lifecycle)
├── depositary (mandatory, entity, entity_kyc_lifecycle)
├── investment_manager (optional, entity, entity_kyc_lifecycle)
├── ownership_chain (recursive, entity_graph, ubo_epistemic_lifecycle, max_depth=5)
├── case (optional, kyc_case_lifecycle, depends_on: management_company)
│   └── tollgate (optional, depends_on: case at min_state=intake)
└── mandate (optional, depends_on: cbu at filled AND case at intake)
```

**20 constellations cover 4 jurisdictions:**
- **Luxembourg:** UCITS SICAV, AIF RAIF, PE SCSp
- **Ireland:** UCITS ICAV, AIF ICAV, Hedge ICAV
- **UK:** Authorised OEIC, AUT, ACS, LTAF, PE LP, Manager LLP
- **US:** 40-Act Open-End, 40-Act Closed-End, ETF 40-Act, Private Fund Delaware LP
- **Cross-border:** Hedge Cross-Border, PE Cross-Border

**Family selection logic:** When a user says "onboard a Luxembourg SICAV", the system matches jurisdiction (LU) + structure type (ucits) to the `fund_onboarding` family, which selects `struct.lux.ucits.sicav`.

### 4.2 The SemOS Governance Surface

SemOS provides four governance layers:

1. **Verb Surface** — which verbs are available given session context (agent mode, workflow phase, SemReg CCIR, lifecycle state, actor gating). ~30 safe-harbor verbs when fail-closed.
2. **Context Resolution** — 12-step pipeline resolving subject → entity type → views → verbs → attributes → preconditions → policies → access decisions → governance signals → confidence
3. **Immutable Registry** — 13 object types (attribute_def, entity_type_def, verb_contract, taxonomy, membership_rule, view_def, policy_rule, evidence_requirement, document_type_def, observation_def, derivation_spec, relationship_type_def, taxonomy_node) stored as immutable snapshots
4. **ABAC** — attribute-based access control with actor_type (Agent, Analyst, Governance, System), clearance levels, access purposes, and jurisdiction constraints

### 4.3 The Onboarding DAG (6-Layer Model)

The composite state model projects the group's onboarding progress as a 6-layer dependency DAG:

| Layer | Name | Gate Condition | Key Verbs |
|-------|------|----------------|-----------|
| 0 | Group Identity | GLEIF import → UBO mapped → control chain | `gleif.import-tree`, `ubo.discover`, `control.build-graph` |
| 1 | CBU Identification | CBUs created from group hierarchy | `cbu.create`, `cbu.create-from-client-group` |
| 2 | KYC Case Opening | Per-CBU case created | `kyc-case.create`, `kyc.open-case` |
| 3 | Screening | Sanctions + PEP + adverse media per entity | `screening.sanctions`, `screening.pep`, `screening.adverse-media` |
| 4 | Document Collection | Per-entity evidence requirements met | `document.solicit`, `document.verify` |
| 5 | Tollgate / Approval | Quality gate passed, case approved | `tollgate.evaluate`, `kyc-case.update-status` → approved |

Each layer has a `LayerState` (Complete, InProgress, NotStarted, Blocked). The system computes next-likely-verbs from the "as-is → to-be" gap at each layer.

### 4.4 Utterance-to-Execution Pipeline

```
User utterance
    → CompoundSignals extraction (jurisdiction, action, structure nouns)
    → ECIR noun taxonomy (deterministic Tier -1, ~120 nouns)
    → ScenarioIndex (Tier -2A, journey-level compound intents, score 0.97)
    → MacroIndex (Tier -2B, macro search parity, score 0.96)
    → Operator macros (Tier 0, exact/fuzzy business vocabulary, score 1.0/0.95)
    → Learned patterns (Tiers 1-3)
    → Semantic embedding search (Tier 6, BGE-small-en-v1.5, 384-dim)
    → Phonetic fallback (Tier 7)
    → SemOS verb surface filtering (pre-constraint, not post-filter)
    → Composite state boost (+/-0.15 from group lifecycle position)
    → Ambiguity resolution (margin >= 0.05 required for clear winner)
    → LLM argument extraction (only LLM call in pipeline)
    → Deterministic DSL assembly
    → SemOS enforcement at execution gate
    → Runbook compilation and execution
```

### 4.5 Group-Level Ownership Architecture

Ownership and control resolution operates at the **group level**, not per-CBU:

1. **GLEIF Import:** Fetches corporate hierarchy from LEI registry (ISO 17442), populates `entity_relationships` with parent-child links
2. **UBO Discovery:** Traverses ownership chains to natural persons, applying configurable threshold (default 25%, overridable to 5%)
3. **Control Graph:** Maps control relationships (board, executive, voting rights, veto, appointee) separately from ownership
4. **Trust Structures:** Separate role taxonomy (settlor, trustee, beneficiary, protector) with interest type coding (fixed, discretionary, contingent)
5. **Chain Terminus:** Explicitly marks where ownership tracing stops (listed company, government entity, regulated fund, widely held, exchange traded)

All CBUs under a group inherit the group's ownership/control determination.

---

## 5. Alignment Risk Register

| # | Category | System Term | Industry Standard | Risk Level | Assessment |
|---|----------|-------------|-------------------|------------|------------|
| 1 | **Terminology** | CBU (Client Business Unit) | Custody Account / Fund Mandate | HIGH | "CBU" is entirely proprietary. No LLM has trained on this term in a custody context. Every interaction about fund structures will require mapping. The system mitigates this with the operator vocabulary layer ("structure"), but the DSL and internal discussions use CBU exclusively. |
| 2 | **Terminology** | "Galaxy" / "Book" / "Universe" | Client Portfolio / Scope / Full Client Set | MEDIUM | Astronomy-derived navigation metaphors. "Load the allianz galaxy" means "load all funds for Allianz". LLMs may interpret "galaxy" literally. |
| 3 | **Terminology** | "Mandate" (trading-profile) | Investment Mandate / Trading Authorization | LOW | "Mandate" is industry-standard in European custody but may confuse US-centric LLMs that associate "mandate" with legal requirements rather than trading authorizations. |
| 4 | **Structural** | UBO Epistemic Lifecycle (undiscovered→alleged→provable→proved→approved) | Not standard — most systems use binary (identified/verified) | MEDIUM | The 5-state epistemic model is richer than industry norms. FATF and 4AMLD describe UBO identification and verification as a 2-step process. The system's "alleged" and "provable" intermediate states encode evidence completeness, which is typically handled as metadata rather than lifecycle states. This is a deliberate innovation. |
| 5 | **Structural** | Constellation (jurisdiction-specific onboarding template) | No direct equivalent — closest is "onboarding playbook" or "regulatory template" | HIGH | The constellation concept is entirely novel. LLMs have no training data for this abstraction. They will need explicit guidance that a "constellation" = a typed dependency graph of roles, entities, and lifecycle states, parameterized by jurisdiction and fund type. |
| 6 | **Lifecycle** | KYC Case: "refer_to_regulator" and "do_not_onboard" as terminal states | SAR Filing (separate workflow), Client Rejection | MEDIUM | Most KYC platforms separate the SAR filing decision from case management. Embedding "refer_to_regulator" as a terminal KYC case state couples these concerns. Industry practice typically treats STR/SAR as a parallel workflow triggered by case findings, not a case terminal state. |
| 7 | **Lifecycle** | Screening lifecycle: sequential sanctions→PEP→media | Usually parallel screening | MEDIUM | The state machine models screening as sequential (sanctions first, then PEP, then media). Industry practice typically runs all three in parallel. The system does support initiating them independently, but the canonical lifecycle encodes a preferred ordering. |
| 8 | **Structural** | Document model: 3-layer (Requirement→Document→Version) | Usually 2-layer (Document→Version) | LOW | The explicit "Requirement" layer (what is needed) separate from "Document" (what exists) is a deliberate enhancement. Industry systems often conflate requirements with document tracking. This separation enables better gap analysis but may confuse LLMs expecting simpler models. |
| 9 | **Terminology** | "Tollgate" | Compliance Gate / Quality Checkpoint / Sign-off | LOW | "Tollgate" is used in some consulting/PMO contexts but is not standard AML/KYC terminology. Most compliance systems call this an "approval gate" or "quality checkpoint." |
| 10 | **Structural** | Deal Record as commercial origination hub | Usually separate CRM and onboarding systems | MEDIUM | Integrating sales pipeline (deal.create, rate card negotiation) with KYC onboarding in a single system is unusual. Most custody banks use separate CRM (Salesforce) and KYC (Fenergo/Pega) platforms. The integration is a deliberate design choice. |
| 11 | **Deliberate Innovation** | Constellation-per-jurisdiction onboarding | Most systems use configurable checklists | HIGH | Instead of configurable checklists, the system uses typed dependency graphs with role cardinality, state machine bindings, and placeholder entity support. This is materially more sophisticated than industry norms and represents genuine innovation. LLMs will need calibration. |
| 12 | **Deliberate Innovation** | Composite state → verb prediction (as-is → to-be gap analysis) | No equivalent in standard custody platforms | HIGH | The system uses the group's composite lifecycle state to predict which verb the user wants next and bias intent resolution accordingly. This is an AI-native design pattern with no traditional equivalent. |
| 13 | **Structural** | UBO chain terminus marking (LISTED_COMPANY, WIDELY_HELD, etc.) | Exempt entity categories under 4AMLD Art. 3(6) | LOW | Good regulatory alignment. The terminus types map to 4AMLD exemptions. LLMs should understand these as regulatory carve-outs for publicly listed companies, government entities, and regulated funds. |
| 14 | **Terminology** | "Red Flag" (red_flags table) | Risk Indicator / Suspicion Alert | LOW | "Red flag" is commonly used in AML context but the system uses it as a formal entity with CRUD operations, which is more structured than typical narrative-based risk flagging. |
| 15 | **Lifecycle** | BODS 0.4 interest type interoperability | Beneficial Ownership Data Standard | LOW | Good standards alignment. The system codes ownership interest types (shareholding, votingRights, appointmentOfBoard) per the BODS standard, enabling interoperability with OpenOwnership and similar registries. |

---

## 6. Open Questions for Reviewer

1. **UBO Epistemic Model Validity:** The 5-state epistemic lifecycle (undiscovered→alleged→provable→proved→approved) goes beyond standard 4AMLD requirements. Does the "alleged" vs "provable" distinction carry genuine regulatory or operational value, or does it add complexity without improving compliance outcomes? What is the industry precedent for epistemic state tracking in UBO determination?

2. **Sequential vs Parallel Screening:** The screening state machine encodes a sequential flow (sanctions→PEP→adverse media). In your understanding of industry practice, is there a legitimate risk management reason to order these sequentially (e.g., fail-fast on sanctions before incurring PEP screening costs), or is this a modeling artifact that should allow full parallelism?

3. **Constellation Granularity:** The system models 20 distinct fund structure constellations across 4 jurisdictions. Are there material fund types missing from this taxonomy? Specifically: (a) Cayman Islands structures (SPC, exempted LP), (b) Channel Islands (Jersey/Guernsey), (c) Singapore VCC, (d) Hong Kong OFC — are these omissions likely to matter for a global custody bank?

4. **KYC Case Terminal States:** The "refer_to_regulator" terminal state in the KYC case lifecycle couples case management with SAR/STR filing. In real-world AML compliance, would you expect these to be decoupled — i.e., a case can be approved while simultaneously triggering a defensive SAR filing? How does this affect the system's regulatory fidelity?

5. **CBU-Entity Role Model:** The system uses a flat M:N relationship (cbu_entity_roles) to bind entities to CBU roles (management_company, depositary, investment_manager, general_partner). Does this adequately model the real-world complexity where a single entity may serve multiple roles simultaneously (e.g., a ManCo that is also the AIFM), or where role delegation and sub-delegation occurs?

6. **Ownership Threshold Configurability:** The system defaults to 25% UBO threshold but supports down to 5% for chain computation. Given that different jurisdictions have different thresholds (25% in EU, 10% in some MENA countries, beneficial interest for trusts regardless of percentage), does the single-threshold model with configurable override adequately represent jurisdictional variation?

7. **Trust Structure Modeling:** The system models trust roles (settlor, trustee, beneficiary, protector) as entity relationship types with interest_type coding (fixed, discretionary, contingent). Does this model adequately capture the complexity of trust structures relevant to custody banking — particularly: (a) powers of appointment, (b) revocable vs irrevocable trusts, (c) purpose trusts with no beneficiaries, (d) GRAT/CLAT charitable structures?

8. **Deal-to-KYC Integration:** The system links deals to KYC cases (`kyc-case.create` accepts optional `deal-id`). In practice, at what point in the commercial lifecycle should KYC initiate? Is it correct that a deal can exist in PROSPECT/QUALIFYING without any KYC case, or would a global custodian typically run preliminary checks earlier?

9. **Document Rejection Taxonomy:** The document rejection reason codes (UNREADABLE, WRONG_DOC_TYPE, DOB_MISMATCH, SUSPECTED_ALTERATION, etc.) span quality, mismatch, validity, data, and authenticity categories. Is this taxonomy comprehensive enough for production custody banking operations, or are there common rejection reasons missing?

10. **Cross-Border Structure Topology:** The system models cross-border fund structures via `cbu_structure_links` (parent/child with link_type: feeder/parallel/aggregator). Does this adequately capture the regulatory requirements for master-feeder structures under UCITS V (passport), AIFMD (delegation), and US 40-Act (feeder fund exemptions)?
