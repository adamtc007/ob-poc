# ARCHITECTURE_FOR_LLM_REVIEW.md

> **Generated:** 2026-03-23 | **Source:** ob-poc repository | **Target audience:** LLM-powered architecture reviewers unfamiliar with this codebase
> **Reviewer note:** You have NO filesystem access. This document is ALL you see. Every entity lifecycle, state transition, verb, and constellation slot is documented here.

---

## 1. Executive Summary

ob-poc is a **custody/fund-services onboarding platform** built in Rust (Axum) with a React/TypeScript frontend. It models the complete lifecycle of institutional client onboarding for a global custodian bank: from prospect identification through GLEIF corporate-hierarchy import, Ultimate Beneficial Owner (UBO) discovery and ownership-chain mapping, KYC case management per entity, multi-layered compliance screening (sanctions, PEP, adverse media), document evidence collection with QA verification, tollgate approval, and post-approval trading/settlement infrastructure setup including trading profiles, custody accounts, booking principals, SSIs, and fee billing.

The system manages approximately 306 PostgreSQL tables across two principal schemas (`"ob-poc"` for business data, `sem_reg*` for governance metadata). It exposes 1,263 canonical DSL verbs organized into approximately 30 domains, with 23,405 embedding vectors (BGE-small-en-v1.5, 384-dim) enabling sub-15ms local semantic verb discovery. The verb surface is structured into 25 constellation maps across 14 constellation families, governed by 9 YAML-defined state machines and organized into 6 discovery universes.

The distinguishing architectural feature is the **Semantic OS (SemOS)** layer: an immutable snapshot registry with 16+ governed object types that acts as the single source of truth for what verbs, attributes, entity types, policies, and evidence requirements exist at any point in time. Every user utterance flows through SemOS context resolution before verb selection, creating an enforced closed loop:

1. **SemOS** resolves the current context envelope (allowed verbs, pruned verbs with structured reasons, fingerprint)
2. **Sage** (observation-plane + polarity pre-classification) classifies the utterance intent
3. **Coder** (verb metadata index + structured resolution) selects the specific verb within the SemOS-constrained surface
4. **Execute** runs the verb against the database, changing entity state
5. **SemOS** re-resolves on the next turn, reflecting the changed state

This loop is enforced at every entry point with no bypass paths. Direct DSL execution validates every verb FQN against the SemOS context envelope. Pending mutation confirmations perform TOCTOU (time-of-check-to-time-of-use) rechecks against a fresh envelope before executing stale DSL. Discovery selections are validated against the SemOS discovery surface before mutating session state. The grounded action surface extracts `current_state` from the SemOS envelope and feeds it to the lifecycle filter in the verb surface computation pipeline.

A standalone **BPMN-Lite** durable orchestration service (23-opcode fiber VM, gRPC API, 123+ tests) handles long-running workflows spanning days or weeks (document solicitation, human approvals, timer-based escalations). It compiles BPMN XML into bytecode executed by user-space fibers that can be serialized, persisted, and resumed deterministically.

**Key architectural principles:**
- Every utterance is a delta against current entity state (not a free-form command)
- Constellations are entity topology (who relates to whom), not verb groupings
- Documents are entity state (evidence for an entity), not separate topology nodes
- The agent is a router (maps utterance to valid verb), not a reasoner
- LLM is used only for argument extraction, never for verb selection
- Undo is composite-level (state transitions), not factual (entity attributes are facts, not states)

---

## 2. Entity Catalogue

### 2.1 Client Group

**Business Definition:** A Client Group represents the top-level commercial relationship between the custodian bank and an institutional client. It is the virtual container for all entities (companies, persons, funds) that form a single client's corporate hierarchy. The group is the anchor for UBO discovery, ownership chain tracing, and CBU identification — every downstream onboarding entity depends on a grounded group.

**Industry Equivalents:** Corresponds to a "Client Master Record" or "Relationship Group" in custody platforms (e.g., BNY Mellon's client hierarchy model, State Street's relationship mapping). In GLEIF terms, it maps to a parent-child corporate hierarchy rooted at an LEI-bearing entity.

**Lifecycle (state machine: `client_group_lifecycle`, 9 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `prospect` | Identified but not yet researched | → `researching` | `gleif.import-tree`, `client-group.research` |
| `researching` | GLEIF import or manual research underway | → `ubo_mapped` | `ubo.discover`, `ubo.allege` |
| `ubo_mapped` | UBO determination complete | → `control_mapped`, ← `researching` | `control.build-graph`, `ownership.trace-chain` / `ubo.reset` |
| `control_mapped` | Control chain mapped | → `cbus_identified`, ← `ubo_mapped` | `cbu.create`, `cbu.create-from-client-group` / `control.reset` |
| `cbus_identified` | Revenue-generating units identified | → `onboarding` | `kyc-case.create`, `kyc.open-case` |
| `onboarding` | KYC cases open, onboarding in progress | → `active` | `kyc-case.update-status` |
| `active` | All KYC cases approved, fully onboarded | → `dormant`, → `offboarded` | `client-group.suspend` / `client-group.offboard` |
| `dormant` | Temporarily suspended | → `active` | `client-group.reactivate` |
| `offboarded` | Permanently offboarded (terminal) | — | — |

**Reducer conditions:** `has_gleif_import` (scope.gleif_import_count > 0), `has_ubo` (scope.has_ubo_determination = true), `has_control` (scope.has_control_chain = true), `has_cbus` (scope.cbu_count > 0), `has_active_case` (scope.active_case_count > 0), `all_approved` (scope.all_cases_approved = true).

**Relationships:**
- Owns zero-to-many **Entities** via `client_group_entity` junction
- Owns zero-to-many **CBUs** via control chain derivation
- Referenced by **Deals** as `client-group-id`
- Referenced by **Governance** (SLAs, access reviews) as the group root

**Regulatory Surface:** FATF Recommendation 10 (Customer Due Diligence) requires identification of the beneficial owner and understanding the ownership/control structure. 4AMLD Article 30 mandates that member states ensure corporate entities hold adequate, accurate, and current information on beneficial ownership. The client group lifecycle directly models this progression: prospect → research → UBO mapping → control mapping → onboarding.

---

### 2.2 CBU (Client Business Unit)

**Business Definition:** A CBU is the atomic revenue-generating unit in the custody model. It represents a single fund, sub-fund, or investment vehicle that the custodian services. Every entity (person, company) participates in the system through a role on a CBU, and every trading profile, KYC case, and billing profile attaches to a CBU. The CBU is the lens through which the custodian views a client's business — not the legal entity itself, but the economic unit being serviced.

**Industry Equivalents:** Maps to an "Account" or "Fund Account" in traditional custody systems (BNY Mellon NEXEN, State Street Alpha). In SWIFT terms, corresponds to the account structure within a safekeeping account hierarchy. In fund administration, maps to a "Fund Vehicle" or "Sub-Fund" entity.

**Lifecycle:** CBUs do not have a formal state machine in the current implementation. Their lifecycle is implicitly driven by the `client_group_lifecycle` (CBUs are identified during the `cbus_identified` → `onboarding` transition) and the `kyc_case_lifecycle` (CBUs are "active" when their KYC case is approved).

**Key Verbs (25+):** `cbu.create`, `cbu.ensure`, `cbu.read`, `cbu.list`, `cbu.update`, `cbu.rename`, `cbu.set-jurisdiction`, `cbu.add-product`, `cbu.assign-role` (role assignment within fund structures), `cbu.assign-control`, `cbu.assign-ownership`, `cbu.parties`, `cbu.validate-roles`, `cbu.link-structure`, `cbu.submit-for-validation`, `cbu.decide`, `cbu.delete`, `cbu.delete-cascade`, `cbu.create-from-client-group`.

**Relationships:**
- Belongs to a **Client Group** via control chain
- Has zero-to-many **Entity Roles** (depositary, management company, investment manager, etc.) via `cbu_entity_roles`
- Has zero-to-one **KYC Case** via `cases.cbu_id`
- Has zero-to-many **Trading Profiles** via `cbu_trading_profiles`
- Has zero-to-many **Products** via product subscriptions
- Linked to other CBUs via `cbu_structure_links` (feeder/aggregator/parallel relationships)

**Regulatory Surface:** MiFID II mandates that investment firms maintain adequate records of client accounts. CSSF (Luxembourg) Circular 18/698 requires identification of the "value chain" participants (ManCo, depositary, transfer agent) for each fund vehicle.

---

### 2.3 Entity (Legal/Natural Person)

**Business Definition:** An Entity represents any natural person or legal entity that participates in the custody ecosystem. This includes directors, beneficial owners, counterparties, depositaries, management companies, investment managers, auditors, and any other party that holds a role on a CBU or appears in an ownership/control chain. Entities are the subjects of KYC verification — every screening, document requirement, and evidence obligation targets an entity.

**Industry Equivalents:** Maps to "Party" in SWIFT MT messages, "Customer" or "Counterparty" in risk systems, "Legal Entity" in LEI registries. Natural persons map to "Beneficial Owner" records in FATF-aligned systems.

**Lifecycle (state machine: `entity_kyc_lifecycle`, 8 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `empty` | No entity assigned to this slot | → `placeholder`, → `filled` | `entity.ensure-or-placeholder` / `party.add`, `cbu.assign-role` |
| `placeholder` | Placeholder entity created (name only, not yet identified) | → `filled` | `party.search`, `entity.identify`, `cbu.assign-role` |
| `filled` | Entity fully identified with basic data | → `workstream_open` | `kyc-workstream.add` |
| `workstream_open` | KYC workstream open for this entity | → `screening_complete`, → `evidence_collected` | `screening.run` / `evidence.verify` |
| `screening_complete` | All screenings run and clear | → `verified` | `kyc-workstream.close` |
| `evidence_collected` | All evidence verified and complete | → `verified` | `kyc-workstream.close` |
| `verified` | Workstream closed and verified | → `approved` | `case.approve` |
| `approved` | KYC-approved entity (terminal for this cycle) | — | — |

**Reducer overlay sources:** `entity_ref` (cbu_entity_roles), `workstream` (entity_workstreams), `screenings` (per-entity, many), `evidence` (kyc_ubo_evidence, many), `red_flags` (per-case, many), `doc_requests` (per-workstream, many).

**Relationships:**
- Participates in CBUs via roles (depositary, ManCo, investment manager, etc.)
- Subject of zero-to-many **KYC Workstreams** (one per case)
- Subject of zero-to-many **Screenings** (sanctions, PEP, adverse media)
- Subject of zero-to-many **Document Requirements**
- Appears in **UBO Registry** as beneficial owner or intermediate entity
- Appears in **Ownership/Control Chains** as node

**Regulatory Surface:** FATF Rec 10 (CDD on customers), FATF Rec 24-25 (transparency of legal persons and arrangements), 4AMLD Art 13 (identification and verification of beneficial owners), 5AMLD Art 30 (central register of beneficial ownership information).

---

### 2.4 KYC Case

**Business Definition:** A KYC Case is the regulatory work package that tracks the Know Your Customer due diligence process for a single CBU. It progresses through a defined lifecycle from intake through discovery, assessment, review, and ultimately approval or rejection. A case encompasses all entity workstreams, screenings, evidence collection, and tollgate evaluations required for regulatory sign-off.

**Industry Equivalents:** Maps to a "Case" in compliance case management systems (Actimize, NICE, Fenergo), an "Onboarding Request" in custody workflow systems, or a "CDD File" in regulatory frameworks.

**Lifecycle (state machine: `kyc_case_lifecycle`, 11 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `intake` | Case created, initial data gathering | → `discovery` | `kyc-case.update-status` |
| `discovery` | Active investigation and entity identification | → `assessment` | `kyc-case.update-status` |
| `assessment` | Risk assessment in progress | → `review` | `kyc-case.update-status`, `kyc-case.set-risk-rating` |
| `review` | Under compliance officer review | → `approved`, → `rejected`, → `refer_to_regulator` | `kyc-case.update-status` / `kyc-case.close` / `kyc-case.escalate` |
| `blocked` | Blocked by outstanding items | → `discovery`, → `withdrawn` | `kyc-case.reopen` / `kyc-case.close` |
| `approved` | KYC approved — CBU can proceed to trading setup | → `review` (reopen) | `kyc-case.reopen` |
| `rejected` | KYC rejected (terminal) | — | — |
| `withdrawn` | Case withdrawn by client (terminal) | — | — |
| `expired` | Case expired due to inactivity (terminal) | — | — |
| `refer_to_regulator` | Referred to regulatory authority for investigation | — | — |
| `do_not_onboard` | Hard block — entity should not be onboarded (terminal) | — | — |

**Relationships:**
- Belongs to exactly one **CBU** via `cases.cbu_id`
- Contains zero-to-many **Entity Workstreams** (one per entity under review)
- Contains zero-to-many **Tollgate Evaluations** (available when case reaches `review` state)
- Contains zero-to-many **KYC Agreements** (contractual terms)
- Contains zero-to-many **Requests** (information requests to client)

**Regulatory Surface:** FATF Rec 10 prescribes the CDD process lifecycle. 4AMLD Art 14 requires risk-based approach to CDD. The case lifecycle models the prescribed regulatory workflow: identification (intake/discovery), verification (assessment), risk assessment (review), and decision (approved/rejected/referred).

---

### 2.5 Entity Workstream

**Business Definition:** An Entity Workstream is a per-entity KYC investigation track within a case. While a KYC Case is scoped to a CBU, each entity that participates in that CBU (depositary, ManCo, directors, UBOs) requires its own workstream of due diligence — screening, evidence collection, document requests, and risk flag assessment. The workstream is the granular unit where compliance work actually happens.

**Industry Equivalents:** Maps to a "Party Review" or "Subject Review" in compliance case management, or an "Entity Due Diligence Track" in KYC workflow systems.

**Key Verbs:** `entity-workstream.create`, `entity-workstream.read`, `entity-workstream.list-by-case`, `entity-workstream.state`, `entity-workstream.update-status`, `entity-workstream.set-enhanced-dd`, `entity-workstream.set-ubo`, `entity-workstream.complete`, `entity-workstream.block`.

**Relationships:**
- Belongs to exactly one **KYC Case** via `entity_workstreams.case_id`
- Belongs to exactly one **Entity** via `entity_workstreams.entity_id`
- Contains zero-to-many **Screenings** (sanctions, PEP, adverse media)
- Contains zero-to-many **Document Requests**
- Contains zero-to-many **Red Flags** (compliance concerns)

---

### 2.6 UBO Registry Entry

**Business Definition:** A UBO Registry Entry records a beneficial ownership determination for an entity. The UBO epistemic lifecycle models the knowledge state of the ownership claim: from undiscovered, through allegation (initial identification), evidence collection (provable), verification (proved), to regulatory approval. This tracks not just "who owns what" but the evidentiary strength of that claim.

**Industry Equivalents:** Maps to a "Beneficial Ownership Record" in central registries (UK PSC Register, Luxembourg RBE), or a "UBO Determination" in compliance systems. The epistemic lifecycle mirrors the FATF's graduated approach to beneficial ownership identification.

**Lifecycle (state machine: `ubo_epistemic_lifecycle`, 5 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `undiscovered` | No ownership information available | → `alleged` | `ubo.allege` |
| `alleged` | Ownership alleged but not yet evidenced | → `provable` | `ubo.collect-evidence` |
| `provable` | Evidence exists but not yet verified | → `proved` | `ubo.verify` |
| `proved` | Evidence verified, ownership proved | → `approved` | `case.approve` |
| `approved` | Regulatorily approved UBO determination (terminal) | — | — |

**Reducer overlay sources:** `registry` (ubo_registry), `evidence` (kyc_ubo_evidence, many), `screenings` (per-workstream, many).

**Consistency checks:** A "proved" UBO with a blocking screening hit generates a warning. A "provable" UBO lacking fully verified evidence generates a warning.

**Regulatory Surface:** FATF Rec 24-25, 4AMLD Art 30, 5AMLD Art 30(5). The epistemic lifecycle directly models the regulatory requirement to identify, verify, and document beneficial ownership.

---

### 2.7 Screening

**Business Definition:** A Screening represents a single compliance check against an entity — sanctions list screening, Politically Exposed Person (PEP) check, or adverse media scan. Screenings are the core compliance control preventing onboarding of sanctioned or high-risk entities. The screening lifecycle tracks the sequential progression through sanctions, PEP, and media checks, with escalation paths for positive hits.

**Lifecycle (state machine: `screening_lifecycle`, 13 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `not_started` | No screening initiated | → `sanctions_pending`, → `pep_pending`, → `media_pending` | `screening.run`, `screening.sanctions` / `screening.pep` / `screening.adverse-media` |
| `sanctions_pending` | Sanctions check in progress | → `sanctions_clear`, → `sanctions_hit` | `screening.update-status` |
| `sanctions_clear` | Sanctions check clear | → `pep_pending` | `screening.pep` |
| `sanctions_hit` | Sanctions match found | → `escalated` | `screening.escalate` |
| `pep_pending` | PEP check in progress | → `pep_clear`, → `pep_hit` | `screening.update-status` |
| `pep_clear` | PEP check clear | → `media_pending` | `screening.adverse-media` |
| `pep_hit` | PEP match found | → `escalated` | `screening.escalate` |
| `media_pending` | Adverse media check in progress | → `media_clear`, → `media_hit` | `screening.update-status` |
| `media_clear` | Media check clear | → `all_clear` | `screening.complete` |
| `media_hit` | Adverse media match found | → `escalated` | `screening.escalate` |
| `all_clear` | All three checks passed | → `not_started` (re-screen) | `screening.run` |
| `escalated` | Hit escalated for compliance review | → `resolved` | `screening.resolve` |
| `resolved` | Escalation resolved | — | — |

**Regulatory Surface:** OFAC SDN list screening (US), EU Consolidated Sanctions List, FATF Rec 6 (targeted financial sanctions), 4AMLD Art 20-23 (PEP requirements), FATF Rec 12 (PEP enhanced due diligence).

---

### 2.8 Document Requirement, Document, and Document Version

**Business Definition:** The document model uses a three-layer architecture separating what is needed (Requirement), the logical identity (Document), and each physical submission (Version). A Document Requirement defines what the compliance team needs from an entity (e.g., "certified passport copy for director X"). A Document is the stable reference. A Document Version is each immutable submission that undergoes QA verification.

**Lifecycle (state machine: `document_lifecycle`, 8 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `missing` | Requirement exists but no document yet | → `requested`, → `waived` | `document.solicit` / `requirement.waive` |
| `requested` | Document solicited from client | → `received`, ← `missing` | `document.upload` / `document.cancel-request` |
| `received` | Document uploaded | → `in_qa` | `document.review` |
| `in_qa` | Under QA review | → `verified`, → `rejected` | `document.verify` / `document.reject` |
| `verified` | QA approved (terminal until expiry) | → `expired` | `document.expire` |
| `rejected` | QA rejected with reason code | → `requested` (re-solicit) | `document.solicit` |
| `waived` | Manual override — requirement waived | ← `missing` | `requirement.reinstate` |
| `expired` | Document validity lapsed | → `requested` (re-solicit) | `document.solicit` |

**Rejection Reason Codes:** Standardized codes across quality (UNREADABLE, CUTOFF, GLARE, LOW_RESOLUTION), mismatch (WRONG_DOC_TYPE, WRONG_PERSON, SAMPLE_DOC), validity (EXPIRED, NOT_YET_VALID, UNDATED), data (DOB_MISMATCH, NAME_MISMATCH, ADDRESS_MISMATCH), and authenticity (SUSPECTED_ALTERATION, INCONSISTENT_FONTS).

**Industry Equivalents:** The three-layer model (requirement/document/version) aligns with document management in regulated industries. The requirement layer maps to "Document Checklist" or "CDD Requirements Matrix" in compliance systems. The version immutability maps to regulatory requirements for document retention and audit trail.

**Regulatory Surface:** FATF Rec 10 (identity verification documents), 4AMLD Art 40 (document retention), MiFID II Art 16 (record-keeping). The document lifecycle with QA verification, rejection codes, and version immutability directly supports the regulatory expectation of verified, auditable evidence.

---

### 2.9 Deal

**Business Definition:** A Deal Record is the commercial origination hub entity that links the sales pipeline to operational onboarding. It tracks the entire commercial lifecycle from prospect qualification through rate card negotiation, contracting, and handoff to CBU onboarding. The deal connects participants (sales team), contracts (legal agreements), products (what is being sold), rate cards (pricing), and ultimately triggers onboarding requests that create CBUs.

**Lifecycle (state machine: `deal_lifecycle`, 9 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `prospect` | Initial contact | → `qualifying`, → `cancelled` | `deal.create`, `deal.update-status` / `deal.cancel` |
| `qualifying` | Active qualification | → `negotiating`, → `cancelled` | `deal.update-status`, `deal.create-rate-card` / `deal.cancel` |
| `negotiating` | Rate card negotiation | → `contracted`, → `cancelled` | `deal.update-status`, `deal.agree-rate-card` / `deal.cancel` |
| `contracted` | Legal contract signed | → `onboarding`, → `cancelled` | `deal.update-status`, `deal.request-onboarding` / `deal.cancel` |
| `onboarding` | CBU creation triggered | → `active`, → `cancelled` | `deal.update-status` / `deal.cancel` |
| `active` | Fully operational | → `winding_down` | `deal.update-status` |
| `winding_down` | Relationship ending | → `offboarded` | `deal.update-status` |
| `offboarded` | Fully offboarded (terminal) | — | — |
| `cancelled` | Cancelled at any pre-active stage (terminal) | — | — |

**Rate Card Sub-Lifecycle:** `DRAFT → PROPOSED → COUNTER_OFFERED ↔ REVISED → AGREED → SUPERSEDED` with cancellation/rejection from any pre-agreed state. Pricing models: BPS (basis points on AUM), FLAT (fixed fee), TIERED (volume-based), PER_TRANSACTION.

**Industry Equivalents:** Maps to "Opportunity" or "Deal Pipeline" in CRM systems (Salesforce), with rate card negotiation corresponding to RFP/proposal cycles in custody commercial workflows.

---

### 2.10 Contract

**Business Definition:** A Legal Contract represents the master agreement between the custodian and client, covering specific product subscriptions. The contract gates CBU onboarding — no contract with the relevant product means no onboarding of that CBU. Contracts carry products and rate cards, and CBU subscriptions are FK-enforced against contract products.

**Key Verbs:** `contract.create`, `contract.add-product`, `contract.subscribe`, `contract.unsubscribe`, `contract.list-subscriptions`, `contract.for-client`, `contract.terminate`.

---

### 2.11 Trading Profile (Mandate)

**Business Definition:** A Trading Profile defines what a CBU is permitted to trade — the instrument universe, market access, settlement routes, corporate action policy, and booking rules. It is the operational mandate that governs post-onboarding activity. The trading profile goes through a formal approval workflow before activation.

**Lifecycle (state machine: `trading_profile_lifecycle`, 7 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `draft` | Initial configuration | → `submitted` | `trading-profile.submit` |
| `submitted` | Under review | → `approved`, → `rejected`, ← `draft` | `trading-profile.approve` / `trading-profile.reject` / `trading-profile.create-draft` |
| `approved` | Approved for activation | → `active` | `trading-profile.activate` |
| `active` | Live trading enabled | → `suspended`, → `archived` | `trading-profile.archive` |
| `suspended` | Temporarily suspended | → `active` | `trading-profile.activate` |
| `archived` | Permanently archived (terminal) | — | — |
| `rejected` | Rejected, requires revision | → `draft` | `trading-profile.create-draft` |

**Industry Equivalents:** Maps to "Investment Guidelines" or "Trading Mandate" in custody/fund admin systems. The instrument universe corresponds to "Eligible Securities List" in compliance frameworks. The approval workflow maps to mandate sign-off processes.

---

### 2.12 Fund Vehicle

**Business Definition:** A Fund Vehicle represents the legal structure of a fund product — UCITS SICAV, RAIF, ICAV, OEIC, 40-Act, LP, etc. It includes umbrella/sub-fund hierarchies, share classes, feeder/master relationships, and capital events. The fund lifecycle tracks from draft creation through regulatory authorization to active management and eventual termination.

**Lifecycle (state machine: `fund_lifecycle`, 8 states):**

| State | Description | Forward Transitions | Trigger Verbs |
|-------|-------------|-------------------|---------------|
| `draft` | Fund vehicle created | → `registered` | `fund.create`, `fund.ensure` |
| `registered` | Registered with regulator | → `authorized` | `fund.upsert-vehicle` |
| `authorized` | Regulatory authorization received | → `active` | `fund.upsert-vehicle` |
| `active` | Fund actively accepting subscriptions | → `soft_closed` | `fund.upsert-vehicle` |
| `soft_closed` | Closed to new subscriptions | → `active`, → `hard_closed` | `fund.upsert-vehicle` |
| `hard_closed` | Fully closed | → `winding_down` | `fund.upsert-vehicle` |
| `winding_down` | Liquidation in progress | → `terminated` | `fund.delete-vehicle` |
| `terminated` | Fund terminated (terminal) | — | — |

**Industry Equivalents:** Directly maps to fund vehicle types in ALFI (Luxembourg), Irish Funds, Investment Association (UK), and ICI (US) classifications.

---

### 2.13 Booking Principal

**Business Definition:** A Booking Principal is the legal entity through which trades are booked and settled. The three-lane service availability model (booking location, legal entity, principal) determines which entity is used for a given instrument/market/currency combination. Selection involves an evaluation and impact analysis workflow.

**Key Verbs:** `booking-principal.create`, `booking-principal.update`, `booking-principal.retire`, `booking-principal.evaluate`, `booking-principal.select`, `booking-principal.explain`, `booking-principal.coverage-matrix`, `booking-principal.gap-report`, `booking-principal.impact-analysis`.

---

### 2.14 Billing Profile

**Business Definition:** A Fee Billing Profile connects a deal's rate card to actual fee calculation and invoicing. It targets specific CBU accounts, operates on billing periods (monthly/quarterly), and tracks the full billing cycle from period creation through calculation, review, approval, and invoice generation.

**Key Verbs:** `billing.create-profile`, `billing.add-account-target`, `billing.create-period`, `billing.calculate-period`, `billing.review-period`, `billing.approve-period`, `billing.generate-invoice`, `billing.dispute-period`, `billing.revenue-summary`.

---

### 2.15 Tollgate

**Business Definition:** A Tollgate Evaluation is a compliance decision gate that assesses whether a KYC case meets all requirements for approval. It evaluates configurable thresholds (screening clearance, document coverage, risk rating) and produces a decision readiness assessment. Tollgates are only available when the case reaches the `review` state.

**Key Verbs:** `tollgate.evaluate`, `tollgate.evaluate-gate`, `tollgate.read`, `tollgate.get-decision-readiness`, `tollgate.get-metrics`, `tollgate.list-evaluations`, `tollgate.set-threshold`, `tollgate.override`, `tollgate.list-overrides`.

---

### 2.16 SLA (Service Level Agreement)

**Business Definition:** An SLA defines measurable service commitments between the custodian and client — settlement timeliness, reporting frequency, response times. SLAs include commitment definitions, measurement recording, breach detection and escalation, and remediation tracking.

**Key Verbs:** `sla.create`, `sla.bind`, `sla.commit`, `sla.record-measurement`, `sla.activate`, `sla.record-breach`, `sla.report-breach`, `sla.escalate-breach`, `sla.resolve-breach`.

---

### 2.17 Constellation Slot

**Business Definition:** A Constellation Slot is a position in an entity topology map. Each slot has a type (cbu, entity, entity_graph, case, tollgate, mandate), a cardinality (root, mandatory, optional, recursive), optional state machine binding, overlay declarations (evidence, screenings, red_flags), and a `depends_on` DAG defining prerequisites. Slots are NOT verb groupings — they represent WHERE in the entity hierarchy a particular kind of entity lives. Verbs are attached to slots with `when` conditions specifying which slot states permit the verb.

---

## 3. Verb Surface

### 3.1 Scale

The system exposes 1,263 canonical DSL verbs across approximately 30 domains, with 23,405 embedding vectors enabling semantic discovery. Each verb has 5-20 invocation phrases for natural language matching. The verb search pipeline uses a 10-tier priority system:

- Tier -2A: ScenarioIndex (journey-level compound intent, score 0.97)
- Tier -2B: MacroIndex (deterministic macro search, score 0.96)
- Tier -1: ECIR noun taxonomy (99 nouns, deterministic, score 0.95)
- Tier 0: Operator macros (business vocabulary, score 1.0/0.95)
- Tiers 1-7: Learned phrases, semantic search (pgvector), phonetic fallback

Intent hit rates: 64.4% first-attempt, 97.5% two-attempt. Clarification round-trip accuracy: 79.6% with zero cross-domain mismatches.

### 3.2 Verb Alignment Table (Major Domains)

| DSL Verb | Plain English | Industry Equivalent | Constellation | State Precondition |
|----------|---------------|-------------------|---------------|-------------------|
| `client-group.create` | Create a new client group | Open Client Master Record | group.ownership | — |
| `gleif.import-tree` | Import corporate hierarchy from GLEIF | LEI Hierarchy Import | group.ownership | group exists |
| `ubo.discover` | Run UBO discovery algorithm | Beneficial Owner Identification (FATF Rec 24) | group.ownership | GLEIF import done |
| `ubo.allege` | Assert a beneficial ownership claim | UBO Declaration | group.ownership | — |
| `ubo.verify` | Verify UBO with evidence | UBO Verification (4AMLD Art 30) | group.ownership | evidence collected |
| `control.build-graph` | Map corporate control chain | Control Structure Analysis | group.ownership | UBO mapped |
| `cbu.create` | Create a new CBU | Open Fund Account | group.ownership | control mapped |
| `cbu.assign-role` | Assign an entity to a CBU role | Party Role Assignment | struct.* | CBU exists |
| `kyc-case.create` | Open a KYC case for a CBU | Open CDD File | kyc.onboarding | CBU exists |
| `kyc-case.update-status` | Advance KYC case through lifecycle | Progress CDD Review | kyc.onboarding | case exists |
| `kyc-case.set-risk-rating` | Set risk rating on case | Risk Assessment | kyc.onboarding | case in assessment+ |
| `kyc-case.close` | Close/reject a KYC case | Close CDD File | kyc.onboarding | case in review |
| `kyc-case.escalate` | Escalate case to regulator | STR/SAR Filing Referral | kyc.onboarding | case in review |
| `entity-workstream.create` | Open investigation track per entity | Start Party Review | kyc.onboarding | case exists |
| `screening.run` | Run full compliance screening | AML/CFT Screening | kyc.onboarding | workstream exists |
| `screening.sanctions` | Run sanctions list check | OFAC/EU Sanctions Screening | kyc.onboarding | — |
| `screening.pep` | Run PEP check | PEP Due Diligence (FATF Rec 12) | kyc.onboarding | sanctions clear |
| `screening.adverse-media` | Run adverse media scan | Adverse Media Screening | kyc.onboarding | PEP clear |
| `screening.escalate` | Escalate a screening hit | Hit Escalation | kyc.onboarding | hit found |
| `document.solicit` | Request a document from client | Document Request | kyc.onboarding | entity exists |
| `document.upload` | Record document submission | Document Receipt | kyc.onboarding | document requested |
| `document.verify` | QA approve a document version | Document Verification | kyc.onboarding | document in QA |
| `document.reject` | QA reject with reason code | Document Rejection | kyc.onboarding | document in QA |
| `requirement.waive` | Override: waive a requirement | Requirement Waiver | kyc.onboarding | requirement exists |
| `tollgate.evaluate` | Run tollgate decision check | Compliance Gate Evaluation | kyc.onboarding | case in review |
| `deal.create` | Create a new deal record | Open Sales Opportunity | deal.lifecycle | — |
| `deal.create-rate-card` | Start rate card negotiation | Create Fee Proposal | deal.lifecycle | deal in qualifying+ |
| `deal.agree-rate-card` | Finalize negotiation | Accept Fee Schedule | deal.lifecycle | rate card proposed |
| `deal.request-onboarding` | Handoff to CBU creation | Trigger Onboarding | deal.lifecycle | deal contracted |
| `contract.create` | Create legal contract | Create MSA | deal.lifecycle | — |
| `contract.subscribe` | Subscribe CBU to product | Account Opening (product gate) | deal.lifecycle | contract exists |
| `trading-profile.create-draft` | Start trading profile | Create Investment Guidelines | trading.streetside | CBU KYC approved |
| `trading-profile.submit` | Submit for review | Submit Mandate for Approval | trading.streetside | profile in draft |
| `trading-profile.activate` | Go live | Activate Trading Mandate | trading.streetside | profile approved |
| `custody.setup-ssi` | Configure settlement instructions | SSI Setup | trading.streetside | profile exists |
| `fund.create` | Create fund vehicle | Register Fund | fund.administration | — |
| `fund.upsert-vehicle` | Update fund details/status | Amend Fund Registration | fund.administration | fund registered |
| `fund.add-share-class` | Add a share class | Register Share Class | fund.administration | fund exists |
| `billing.create-profile` | Create billing configuration | Set Up Fee Billing | deal.lifecycle | rate card agreed |
| `billing.calculate-period` | Run fee calculation | Calculate Period Fees | deal.lifecycle | period open |
| `sla.create` | Create SLA commitment | Define Service Level | governance.compliance | group exists |
| `red-flag.raise` | Raise compliance concern | Flag Risk Issue | kyc.onboarding | workstream exists |
| `ownership.trace-chain` | Trace ownership chain | Ownership Structure Analysis | group.ownership | entities exist |
| `booking-principal.evaluate` | Evaluate booking principal | Principal Assessment | trading.streetside | CBU exists |
| `session.load-galaxy` | Load all CBUs under apex entity | Load Client Book | — | — |

### 3.3 Macro Table (Multi-Step Workflows)

| Macro FQN | Description | Steps | Jurisdiction |
|-----------|-------------|-------|-------------|
| `struct.lux.ucits.sicav` | Luxembourg UCITS SICAV setup | 13 | LU |
| `struct.lux.aif.raif` | Luxembourg AIF RAIF setup | ~10 | LU |
| `struct.lux.pe.scsp` | Luxembourg PE SCSp setup | ~10 | LU |
| `struct.ie.ucits.icav` | Ireland UCITS ICAV setup | ~10 | IE |
| `struct.uk.authorised.oeic` | UK Authorised OEIC setup | ~10 | UK |
| `struct.us.40act.open-end` | US 40-Act Open-End setup | ~10 | US |
| `struct.hedge.cross-border` | Hedge fund cross-border setup | ~8 | ALL |
| `struct.pe.cross-border` | PE fund cross-border setup | ~8 | ALL |
| `screening.full` | Full screening workflow (sanctions + PEP + adverse media) | 3 | ALL |
| `kyc.full-review` | Complete KYC review macro | ~5 | ALL |
| `kyc.collect-documents` | Document collection macro | ~4 | ALL |
| `case.open` | Open KYC case and trigger initial assessment | ~3 | ALL |

### 3.4 Invocation Phrase Examples

| Verb | Example Natural Language Phrases |
|------|-------------------------------|
| `cbu.create` | "create a fund", "set up a new structure", "add a client business unit", "create a CBU" |
| `session.load-galaxy` | "load the allianz book", "show me the blackrock structures", "load all aviva cbus" |
| `kyc-case.create` | "open a kyc case", "start kyc for this cbu", "create a new case" |
| `screening.run` | "run screening", "screen this entity", "check sanctions" |
| `ubo.discover` | "who owns this company", "find the beneficial owners", "discover UBOs" |
| `document.solicit` | "request a passport", "solicit documents", "ask for identity proof" |
| `deal.create` | "create a deal", "start a new deal record", "open a sales deal" |
| `trading-profile.create-draft` | "set up a trading mandate", "create a trading profile", "draft a mandate" |
| `fund.create` | "create a fund", "set up a new fund vehicle", "register a fund" |
| `contract.create` | "create a contract", "set up a legal agreement", "new master agreement" |

---

## 4. Domain Architecture

### 4.1 Constellation Model Deep Dive

The constellation model is the system's approach to organizing entities into domain-specific topologies. The key principle is that **constellations are entity topology, not verb groupings**. A constellation map defines WHERE entities live in relation to each other — the depositary is a role on a CBU, the KYC case belongs to a CBU, the screening belongs to a workstream which belongs to a case. Verbs are attached to the slots they operate on, with `when` conditions constraining which slot states permit the verb.

**Slot Types:**
- `cbu` — A Client Business Unit position (the root entity in most constellations)
- `entity` — A natural/legal person position (with `entity_kinds` filter)
- `entity_graph` — A recursive graph position (UBO chains, control chains) with `max_depth`
- `case` — A KYC case position
- `tollgate` — A tollgate evaluation position
- `mandate` — A trading profile position

**Cardinality:**
- `root` — Exactly one, the constellation anchor
- `mandatory` — Must be filled for the constellation to be valid
- `optional` — May or may not be present
- `recursive` — Can contain nested instances (UBO chains, control chains)

**Overlays:** Slots can declare overlays — additional data layers that enrich the slot without adding new entity positions. Common overlays: `evidence`, `screenings`, `red_flags`, `ownership`, `control`, `matrix_overlay`, `screening_result`, `doc_requests`.

**Edge Overlays:** Applied to the edges between recursive entity_graph nodes: `ownership` (voting percentage, economic interest), `control` (control chain links).

**Depends-On DAG:** Slots declare dependencies via `depends_on`, which can reference other slots with optional `min_state` constraints. For example, the `onboarding_request` slot in the deal constellation depends on the deal reaching the `contracted` state: `depends_on: [{ slot: deal, min_state: contracted }]`. The tollgate depends on the case reaching `review`: `depends_on: [{ slot: kyc_case, min_state: review }]`.

**State Machine Binding:** Slots can bind to a state machine via `state_machine: <name>`, which governs what verbs are available based on current state. The state machine also enables the verb surface lifecycle filter — verbs that require a state the entity hasn't reached are excluded.

### 4.2 Complete State Machine Inventory

**1. `client_group_lifecycle` (9 states):** prospect → researching → ubo_mapped → control_mapped → cbus_identified → onboarding → active → dormant → offboarded. Revert paths: ubo_mapped → researching, control_mapped → ubo_mapped. Dormant ↔ active.

**2. `kyc_case_lifecycle` (11 states):** intake → discovery → assessment → review → approved/rejected/refer_to_regulator. Blocked → discovery/withdrawn. Approved → review (reopen). Terminal: rejected, withdrawn, expired, refer_to_regulator, do_not_onboard.

**3. `entity_kyc_lifecycle` (8 states):** empty → placeholder → filled → workstream_open → screening_complete/evidence_collected → verified → approved. Uses overlay sources: entity_ref, workstream, screenings, evidence, red_flags, doc_requests.

**4. `ubo_epistemic_lifecycle` (5 states):** undiscovered → alleged → provable → proved → approved. Consistency checks on proved (blocking screening) and provable (missing evidence).

**5. `screening_lifecycle` (13 states):** not_started → sanctions_pending → sanctions_clear → pep_pending → pep_clear → media_pending → media_clear → all_clear. Hit paths: sanctions/pep/media_hit → escalated → resolved. Re-screen: all_clear → not_started.

**6. `document_lifecycle` (8 states):** missing → requested → received → in_qa → verified → expired. Rejection: in_qa → rejected → requested. Waiver: missing ↔ waived. Cancel: requested → missing.

**7. `deal_lifecycle` (9 states):** prospect → qualifying → negotiating → contracted → onboarding → active → winding_down → offboarded. Cancellation from any pre-active state.

**8. `trading_profile_lifecycle` (7 states):** draft → submitted → approved → active → suspended/archived. Rejection: submitted → rejected → draft. Suspension ↔ active.

**9. `fund_lifecycle` (8 states):** draft → registered → authorized → active → soft_closed → hard_closed → winding_down → terminated. Soft_closed ↔ active.

### 4.3 Constellation Map Inventory

| Constellation ID | Jurisdiction | Slot Count | Key Entity Types |
|-----------------|-------------|-----------|------------------|
| `group.ownership` | ALL | 5 (client_group, gleif_import, ubo_discovery, control_chain, cbu_identification) | client-group, entity, company, person |
| `kyc.onboarding` | ALL | 8 (cbu, kyc_case+tollgate, entity_workstream, screening, kyc_agreement, identifier, request) | cbu, entity, case, person, company |
| `kyc.extended` | ALL | Extended KYC with additional slots | cbu, entity, case |
| `deal.lifecycle` | ALL | 10 (deal, participant, deal_contract, contract, deal_product, rate_card, onboarding_request, billing_profile, pricing, contract_template) | deal, entity, contract, company |
| `trading.streetside` | ALL | 12 (cbu, trading_profile, custody, booking_principal, cash_sweep, service_resource, service_intent, booking_location, legal_entity, product, delivery) | cbu, company, entity, mandate |
| `fund.administration` | ALL | 8+ (fund, umbrella, share_class, feeder, investment, capital_event, distribution, nav) | fund, entity |
| `governance.compliance` | ALL | 6+ (group, sla, access_review, regulatory, delegation, ruleset) | client-group, entity, contract |
| `struct.lux.ucits.sicav` | LU | 8+ (cbu, management_company, depositary, transfer_agent, auditor, etc.) | cbu, company |
| `struct.ie.ucits.icav` | IE | 8+ (cbu, ManCo, depositary, administrator, etc.) | cbu, company |
| `struct.uk.authorised.oeic` | UK | 8+ (cbu, ACD, depositary, registrar, etc.) | cbu, company |
| `struct.us.40act.open-end` | US | 8+ (cbu, adviser, custodian, transfer_agent, etc.) | cbu, company |
| `struct.lux.aif.raif` | LU | Fund structure slots | cbu, company |
| `struct.lux.pe.scsp` | LU | PE fund structure slots | cbu, company |
| `struct.ie.aif.icav` | IE | AIF structure slots | cbu, company |
| `struct.ie.hedge.icav` | IE | Hedge fund structure slots | cbu, company |
| `struct.uk.authorised.acs` | UK | ACS structure slots | cbu, company |
| `struct.uk.authorised.aut` | UK | Authorised Unit Trust slots | cbu, company |
| `struct.uk.authorised.ltaf` | UK | Long-Term Asset Fund slots | cbu, company |
| `struct.uk.manager.llp` | UK | Manager LLP slots | cbu, company |
| `struct.uk.pe.lp` | UK | UK PE LP slots | cbu, company |
| `struct.us.40act.closed-end` | US | Closed-end fund slots | cbu, company |
| `struct.us.etf.40act` | US | ETF structure slots | cbu, company |
| `struct.us.private-fund.delaware-lp` | US | Delaware LP slots | cbu, company |
| `struct.hedge.cross-border` | ALL | Cross-border hedge fund | cbu, company |
| `struct.pe.cross-border` | ALL | Cross-border PE fund | cbu, company |

### 4.4 The SemOS Closed Loop

The Semantic OS closed loop ensures that every utterance operates within the bounds of current entity state and governance policy. The loop operates as follows:

**Step 1 — SemOS Context Resolution:** The orchestrator calls `resolve_context()` with the current session state, producing a `SemOsContextEnvelope` containing:
- `allowed_verbs`: HashSet of verb FQNs that pass ABAC, tier, precondition, and lifecycle checks
- `pruned_verbs`: Vec of verbs rejected with structured `PruneReason` (7 variants: AbacDenied, EntityKindMismatch, TierExcluded, TaxonomyNoOverlap, PreconditionFailed, AgentModeBlocked, PolicyDenied)
- `fingerprint`: SHA-256 of sorted allowed verb FQNs (format: `v1:<hex>`)
- `evidence_gaps`, `governance_signals`: Governance intelligence for the agent

**Step 2 — Verb Surface Computation (6-step pipeline):**
1. Registry: Load all ~1,263 verbs from RuntimeVerbRegistry
2. AgentMode: Filter by Research vs Governed mode
3. Scope+Workflow: Apply group scope gate (no group = bootstrap domains only) + workflow phase filter (stage_focus → domain allowlists)
4. SemReg CCIR: Apply `ContextEnvelope.allowed_verbs` as pre-constraint
5. Lifecycle: Filter by `VerbLifecycle.requires_states` vs entity state from `GroundedActionSurface`
6. Rank+CompositeStateBias: Sort by domain, apply `GroupCompositeState` boost, compute `SurfaceFingerprint`

**Step 3 — Sage Pre-Classification:** Observation-plane + polarity pre-classification determines whether the utterance is a read (structure inquiry), write (mutation), or discovery (exploration).

**Step 4 — Coder Verb Resolution:** The verb metadata index + HybridVerbSearcher resolves the utterance to a specific verb FQN within the SemOS-constrained surface. The allowed_verbs from the envelope are threaded into the searcher — disallowed verbs are never returned from any tier.

**Step 5 — Execution:** The verb is compiled into a runbook and executed against the database. Entity state changes.

**Step 6 — State Feedback:** On the next turn, SemOS re-resolves with the changed entity state, producing an updated envelope, and the cycle repeats.

**GroundedActionSurface:** The `current_state` is extracted from the SemOS envelope and fed to `VerbSurfaceContext.entity_state`. This enables the lifecycle filter in Step 5 to fire — verbs requiring states the entity hasn't reached are excluded. Prior to the 2026-03-22 remediation, this was hardcoded to `None`, meaning the lifecycle filter never fired.

### 4.5 OnboardingStateView: The 6-Layer DAG

The OnboardingStateView is a per-turn projection of the group's composite state into an actionable 6-layer DAG. It is computed from live database state and sent on every `ChatResponse` when a group is in scope.

**Layer 0: Group Ownership** — UBO discovery, ownership chain tracing, control graph building. Forward verbs: `gleif.import-tree`, `ubo.discover`, `control.build-graph`. State depends on: group existence, GLEIF import count, UBO determination, control chain.

**Layer 1: CBU Identification** — Revenue-generating unit identification. Forward verbs: `cbu.create`, `cbu.create-from-client-group`. State depends on: CBU count.

**Layer 2: KYC Case Opening** — Per-CBU case management. Forward verbs: `kyc-case.create`, `kyc.open-case`. State depends on: active case count.

**Layer 3: Screening** — Sanctions, PEP, adverse media checks per entity. Forward verbs: `screening.run`, `screening.sanctions`. State depends on: screening completion across entities.

**Layer 4: Document Collection** — Per-entity evidence requirements. Forward verbs: `document.solicit`, `document.upload`. State depends on: document coverage percentage.

**Layer 5: Tollgate / Approval** — Final compliance gate. Forward verbs: `tollgate.evaluate`, `kyc-case.update-status` (to approved). State depends on: all prior layers complete.

**Design Principles:**
- Undo is composite-level (revert a case status from REVIEW to ASSESSMENT) not factual (you cannot "undo" a company name)
- `suggested_utterance` on every verb MUST resolve through the same `HybridVerbSearcher` pipeline — misalignment means clicking a suggestion triggers a different verb
- `VerbDirection` enum: Forward (advance state), Revert (back up state), Query (read without changing)
- `context_reset_hint` fires when utterance is unrelated to the current group context

**Per-CBU State Cards:** Each CBU in scope gets a `CbuStateCard` showing: lifecycle_state, progress_pct, per-phase status (has_case, case_status, has_screening, screening_complete, document_coverage_pct), next_action (single most impactful forward verb), revert_action (composite-level undo).

### 4.6 Evidence Chain

The evidence chain connects governance requirements to physical documents:

1. **AttributeDef** (SemOS registry) — Defines what data attributes exist (e.g., "passport_number", "date_of_birth")
2. **ProofObligationDef** (SemOS registry) — Defines what proof is required for an attribute (e.g., "certified passport copy for natural persons", links to `attributes_proven`)
3. **EvidenceStrategyDef** (SemOS registry) — Defines acceptable evidence strategies (e.g., "original document OR certified copy", freshness requirements, acceptable sources)
4. **DocumentTypeDef** (SemOS registry) — Classifies document types (passport, articles of incorporation, UBO declaration)
5. **document_requirements** (business data) — Per-entity concrete requirements derived from the above chain
6. **document_lifecycle** (state machine) — Tracks each requirement through missing → requested → received → in_qa → verified/rejected/waived/expired

The `attributes_proven` linkage on `ProofObligationDef` connects the obligation back to the specific registry attributes it satisfies, creating a closed proof chain: attribute → obligation → strategy → document type → requirement → lifecycle.

### 4.7 Pipeline Integrity

**Single Envelope Execution:** The SemOS envelope is fetched ONCE before the execution loop and reused for all statements in a batch. This eliminates the per-statement TOCTOU risk that existed before the 2026-03-22 remediation.

**TOCTOU Recheck on Pending Confirmation:** When a user confirms a pending mutation (e.g., typing "yes" to approve a previously proposed DSL statement), the system rechecks the verb against the current SemOS envelope before executing. This prevents executing stale DSL from a prior turn where the verb was allowed but conditions have since changed.

**Discovery Selection Validation:** When the user selects from a discovery surface (e.g., choosing a verb from a disambiguation list), the selection is validated against the SemOS discovery surface before mutating session state. This prevents the selection of verbs that have been pruned since the discovery was presented.

**Direct DSL Bypass:** Removed entirely. The `dsl:` prefix path and `allow_direct_dsl` flag were deleted. All DSL flows through the SemReg-filtered pipeline.

### 4.8 Disambiguation UX

When verb search returns ambiguous results (multiple verbs within the 0.05 ambiguity margin), the system presents a `VerbDisambiguationRequest` with enriched `VerbOption` entries. Each option includes:

- `verb_fqn` — Fully qualified verb name
- `description` — Human-readable description
- `score` — Match confidence (0.0-1.0)
- `suggested_utterance` — Clear phrase the user can say to select this verb (must resolve through pipeline)
- `verb_kind` — "primitive" (single operation), "macro" (multi-step workflow), "query" (read-only)
- `differentiation` — Why this option differs from alternatives (e.g., "Single PEP check for one entity" vs "Full screening workflow, 3 steps")
- `requires_state` — What state the entity must be in (e.g., "Requires KYC case in REVIEW state")
- `produces_state` — What state the entity moves to after execution
- `scope` — "single_entity", "batch", "group"
- `step_count` — Number of steps if this is a macro
- `target_entity_kind` — Entity type this verb operates on (e.g., "cbu", "entity", "case")
- `constellation_slot` — Where in the constellation this verb lives (e.g., "kyc_case", "screening")
- `entity_context` — Human-readable position (e.g., "Operates on the KYC case for this CBU")
- `target_entity_name` — Specific entity name if in scope

The numeric selection path allows users to type a number (e.g., "1") to select the first option, which is routed through the same `POST /api/session/:id/input` unified ingress.

---

## 5. Alignment Risk Register

### 5.1 Terminological Divergences

| System Term | Industry Standard | Notes |
|-------------|-------------------|-------|
| **CBU (Client Business Unit)** | Account / Fund Account | CBU is a business lens, not a legal entity. Industry uses "account" which implies a bank-side identifier; CBU implies the client's economic unit. May confuse reviewers expecting SWIFT account structures. |
| **Structure** (operator macro) | Fund Vehicle / Fund Account | The operator vocabulary maps "structure" to CBU. Industry uses "structure" for corporate hierarchy (holding companies, SPVs), not individual accounts. |
| **Mandate** (operator macro) | Investment Guidelines / Trading Mandate | "Mandate" maps to trading-profile. Industry "mandate" sometimes means the legal authority to act, not the trading configuration. |
| **Party** (operator macro) | Counterparty / Entity / Natural Person | "Party" maps to entity operations. Industry "party" is broader (includes the custodian itself). |
| **Galaxy** | Client Book / Relationship | "Galaxy" (from the ESPER navigation metaphor) means all CBUs under an apex entity. No industry equivalent. |
| **Constellation** | Entity Topology / Entity Relationship Map | Novel term. Industry uses "entity model" or "data model". |
| **Slot** | Position / Role / Field | In constellation context. Industry has no direct equivalent for this topology concept. |
| **Envelope** | Authorization Context / Permission Set | SemOS context envelope. No industry equivalent for a per-turn immutable governance snapshot. |
| **Tollgate** | Compliance Gate / Decision Point / Quality Gate | "Tollgate" is project management terminology. Industry KYC uses "approval gate" or "decision point". |
| **Red Flag** | Risk Indicator / Compliance Alert | Industry standard term but system models it as entity state (overlay on workstream), not a separate entity. |
| **Verb** | Operation / Action / API Endpoint | The DSL verb model has no direct industry equivalent. Closest is a "Business Capability" in TOGAF. |
| **Pack** | Journey Template / Workflow Template | V2 REPL concept. No industry equivalent. |
| **Reducer** | State Derivation Rule / Computed State | From functional programming. No industry equivalent in custody/banking. |
| **Sage** | Intent Classifier / Pre-processor | Novel term for the observation-plane pre-classification layer. |
| **Coder** | Verb Resolver / Action Selector | Novel term for the verb resolution layer. |

### 5.2 Structural Divergences

| Area | Industry Expectation | System Model | Risk Level |
|------|---------------------|-------------|-----------|
| **Account hierarchy** | Multi-level account structure (master account → sub-account → position) with SWIFT-compatible identifiers | CBU is flat, no sub-account hierarchy. Positions are derived from trading profile universe. | Medium — may need sub-account modeling for institutional clients with complex booking structures |
| **Legal entity vs economic entity** | Clear separation between the legal entity (e.g., SICAV SA) and the economic entity (sub-fund) | CBU conflates both. A SICAV and its sub-funds are separate CBUs linked via `cbu_structure_links`. | Medium — industry expects the umbrella to be a different entity type than the sub-fund |
| **Screening vendor integration** | Real-time API integration with screening providers (World-Check, Dow Jones, LexisNexis) | Internal state machine models screening lifecycle but no vendor integration layer | High for production — the screening lifecycle models the workflow, not the data source |
| **Document management** | Enterprise DMS integration (SharePoint, OpenText) with version control and retention | Three-layer model (requirement/document/version) is self-contained with `cargo_ref` URI scheme | Medium — production would need DMS integration |
| **Regulatory reporting** | Automated STR/SAR filing to FIUs, regulatory returns to CSSF/CBI/FCA | `refer_to_regulator` state exists but no actual filing mechanism | High for production — the model captures when to file but not how |
| **Multi-jurisdictional KYC** | Different CDD requirements per jurisdiction (UK enhanced DD, US CIP, LU CRF) | Single `kyc_case_lifecycle` state machine for all jurisdictions | Medium — constellation maps are jurisdiction-specific but the case lifecycle is universal |
| **Client risk rating** | Institutional risk rating models (country risk, product risk, client risk) feeding into CDD level | `kyc-case.set-risk-rating` exists but risk model is simplified | Low — the mechanism exists, the model would need enrichment |
| **Ongoing monitoring** | Continuous screening refresh, periodic reviews (annual/biennial based on risk) | `screening.bulk-refresh` exists; no automated periodic review scheduling | Medium — the verbs exist but the scheduling infrastructure is BPMN-Lite territory |

### 5.3 Lifecycle Divergences

| Entity | Industry Expectation | System Model | Notes |
|--------|---------------------|-------------|-------|
| **KYC Case** | Some frameworks distinguish "Simplified DD", "Standard DD", "Enhanced DD" as separate case types | Single lifecycle with `set-risk-rating` to flag EDD | EDD typically triggers additional requirements, not just a flag |
| **Screening** | Continuous monitoring (not one-time check) with daily/weekly refresh cycles | One-time lifecycle with `bulk-refresh` for re-screening | Industry expects always-on screening, not event-driven |
| **Document** | Documents tied to regulatory obligations with automatic expiry monitoring | Expiry via `document.expire` verb but no automatic monitoring | Industry expects proactive expiry alerts |
| **UBO** | Regulatory registries (UK PSC, LU RBE) with public disclosure obligations | Internal-only epistemic lifecycle | No public registry integration modeled |
| **Deal** | Some custody platforms don't have a formal "deal" concept — onboarding starts directly | Deal is the commercial origination hub, pre-onboarding | Novel for pure custody; standard in full-service banking |

### 5.4 Deliberate Innovations

| Innovation | What It Is | Why It Exists |
|-----------|-----------|--------------|
| **SemOS closed loop** | Every utterance constrained by immutable governance snapshot | Prevents agent hallucination — LLM never selects verbs, only extracts arguments from a pre-constrained surface |
| **Constellation topology** | Entity relationships as navigable graph with state-machine-bound slots | Traditional systems use flat entity-relationship models; constellations enable "what can I do next" reasoning |
| **Epistemic UBO lifecycle** | Tracks confidence level of beneficial ownership claims (alleged → provable → proved → approved) | Industry treats UBO as binary (known/unknown); epistemic lifecycle models the evidentiary progression required by FATF |
| **Verb surface computation** | Per-turn governance-filtered verb set with dual fingerprints | No industry equivalent — most systems have static RBAC, not dynamic state-aware verb filtering |
| **OnboardingStateView** | 6-layer DAG projected as "where am I + what can I do" | Novel UX pattern for compliance workflows — most systems present static task lists, not state-derived action surfaces |
| **BPMN-Lite fiber VM** | User-space fibers compiled from BPMN, serializable for durable persistence | Most custody systems use Camunda/Activiti. The custom VM enables hollow orchestration (no domain logic in the engine) |
| **Three-layer document model** | Requirement → Document → Version with immutable versions and QA lifecycle | Industry DMS typically conflates these. Separating requirement from document enables gap analysis and coverage computation |
| **Composite state bias** | Group-level entity state drives verb scoring ("CBU exists but no case → KYC likely means kyc-case.create") | Novel intent resolution optimization — most NLU systems don't consider entity state when resolving intent |

---

## 6. Open Questions for Reviewer

1. **CBU as lens vs account:** The CBU is described as an "economic unit" (revenue-generating lens), but industry custody accounts have their own lifecycle (opening, dormancy, closure) with regulatory obligations (CRS reporting, FATCA). Does the system need a formal CBU lifecycle state machine, or is the implicit lifecycle through group/KYC status sufficient?

2. **Sub-fund vs umbrella entity typing:** A Luxembourg SICAV umbrella and its sub-funds are both modeled as CBUs linked via `cbu_structure_links`. Industry expects these to be distinct entity types with different regulatory obligations (the umbrella is registered, sub-funds may not be individually authorized). Is this conflation intentional, and does it create regulatory reporting gaps?

3. **Screening as one-time vs continuous:** The screening lifecycle models a sequential one-time check (sanctions → PEP → adverse media → all_clear). Industry best practice (and regulatory expectation under 5AMLD Art 13) is continuous monitoring with daily/weekly refresh cycles. The `screening.bulk-refresh` verb exists, but should the architecture support always-on screening as a first-class concept?

4. **Document expiry automation:** The document lifecycle has an `expired` state triggered by `document.expire`, but no automatic monitoring. FATF Rec 10 and 4AMLD Art 40 imply proactive document freshness management. Should the BPMN-Lite service schedule automatic expiry checks and re-solicitation?

5. **Enhanced Due Diligence (EDD):** The system uses `kyc-case.set-risk-rating` to flag high-risk cases, but EDD typically requires additional evidence requirements, more frequent reviews, and senior management approval. Should EDD be a separate state machine branch or a parameterized extension of the existing lifecycle?

6. **Multi-jurisdictional CDD variation:** The `kyc_case_lifecycle` is universal across jurisdictions, but CDD requirements vary significantly (UK: source of wealth for EDD; US: CIP 326 requirements; LU: CRF professional due diligence). Should the case lifecycle be parameterized by jurisdiction, or do the constellation-level jurisdiction-specific structures (struct.lux.*, struct.ie.*, etc.) provide sufficient differentiation?

7. **Regulatory reporting integration:** The system models `refer_to_regulator` and `escalated` states but has no filing mechanism. For an operational deployment, what regulatory interfaces (FIU filing, CSSF reporting, FCA notifications) would need to be modeled as first-class verbs?

8. **Beneficial ownership public registry integration:** The UBO epistemic lifecycle is internal-only. EU 5AMLD Art 30(5) and UK PSC Register require public disclosure of beneficial ownership. Should the system model outbound registry filing as part of the UBO lifecycle?

9. **Client consent and data subject rights:** The system models extensive PII processing (document collection, screening, entity identification) but does not explicitly model GDPR/data subject rights (consent management, data access requests, erasure). Should this be a separate constellation or integrated into existing lifecycles?

10. **Delegation and power of attorney:** The governance constellation includes `delegation.*` verbs, but the delegation model is not deeply elaborated. In custody operations, delegation chains (investment manager → sub-adviser → execution broker) have regulatory implications (MiFID II delegation rules). Is the current model sufficient for regulatory delegation tracking?

11. **Rate card negotiation round-trip:** The deal rate card lifecycle (DRAFT → PROPOSED → COUNTER_OFFERED ↔ REVISED → AGREED) models bilateral negotiation. Industry custody pricing often involves competitive tenders (RFPs) with multiple rounds. Is the counter-offer model sufficient, or does the system need multi-party negotiation support?

12. **Proof obligation coverage:** The evidence chain (AttributeDef → ProofObligationDef → EvidenceStrategyDef → DocumentTypeDef) is architecturally complete. However, for an industry review: does the `attributes_proven` linkage on ProofObligationDef accurately model how custody banks map regulatory requirements (FATF Rec 10 elements) to specific document types (passport, utility bill, etc.)?

13. **Constellation separation of concerns:** The system enforces zero verb bleed across constellations (verified by cross-audit). However, some industry workflows span multiple constellations (e.g., a deal triggers CBU creation which triggers KYC which requires document collection). How does the system handle cross-constellation workflows — through BPMN orchestration, macro expansion, or manual user navigation?

14. **Booking principal three-lane model:** The `booking-principal.evaluate` and `booking-principal.select` verbs implement a principal selection workflow. In industry, booking principal selection involves regulatory capital considerations (Basel III), netting agreements, and tax optimization. Does the evaluation model incorporate these factors, or is it limited to coverage/availability?
