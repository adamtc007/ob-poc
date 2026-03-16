# Booking Principles / Client–Product Eligibility
## Vision and Scope v1.1

## Executive Framing for Product and Engineering

**Booking Principles** should be positioned as a strategic control and enablement capability for institutional onboarding, product governance, and service design.

For a Product audience, it solves a familiar business problem: commercial intent often gets ahead of delivery reality. A product may be commercially negotiated, but whether it can actually be sold, contracted, booked, onboarded, and serviced depends on a combination of client characteristics, product constraints, jurisdiction rules, legal-entity permissions, and operational readiness.

For an Engineering audience, this is not just a reporting matrix or a UI screen. It is a governed decision capability built on structured metadata profiles, policy-aligned rule evaluation, evidence-aware conditions, and explainable outputs that can be consumed by deterministic orchestration.

In practical terms, this capability sits between **Deal Map**, **Client / CBU profile**, **KYC / UBO posture**, and **Product / Service / Resource provisioning**. It answers the critical question:

> **Can BNY validly offer and operationalize this product for this client, through this legal and jurisdictional pathway, under current policy and servicing constraints?**

That makes Booking Principles a bridge between:

- commercial design,
- control policy,
- onboarding execution,
- and service provisioning reality.

It should therefore be treated as a first-class domain capability rather than an informal spreadsheet exercise.

---

## Product Value Proposition

From a Product perspective, the value is straightforward:

- reduce late-stage onboarding failure,
- expose non-sellable or non-serviceable combinations earlier,
- make client/product eligibility transparent,
- shorten decision cycles for straightforward cases,
- route edge cases into governed escalation paths,
- and create a reusable decision service that can be embedded across onboarding and product workflows.

This allows product managers and onboarding leads to move from vague policy interpretation toward explicit, governed eligibility decisions.

---

## Engineering Value Proposition

From an Engineering perspective, the capability provides:

- a clear fact model for eligibility-relevant client and product attributes,
- separation of metadata, rule logic, evidence state, and decision output,
- auditable and deterministic rule evaluation,
- explicit support for incomplete-information states,
- and an interface suitable for DSL/runbook integration.

This avoids burying critical business rules inside manual process notes, UI-only validations, or uncontrolled spreadsheet logic. It also aligns strongly with the OB-POC principle that major onboarding decisions should be represented as structured, inspectable, and replayable control outcomes.

---

## 1. Purpose

This document defines the **vision, scope, core capabilities, and conceptual metadata model** for a **Booking Principles / Client–Product Eligibility** capability within the OB-POC domain.

The purpose of this capability is to determine, in a governed and auditable manner:

> **Can BNY offer, contract, onboard, book, and service a given product or service for a given client context?**

This is not a narrow sales-rule table. It is a cross-domain control capability that sits between:

- commercial product negotiation,
- client and CBU profiling,
- KYC / AML / sanctions posture,
- legal entity and branch booking constraints,
- product governance,
- jurisdiction policy,
- and operational service readiness.

It converts fragmented policy, product, legal, compliance, and operational constraints into a structured decision plane that can support both:

- **human review and governance**, and
- **agentic / deterministic onboarding orchestration**.

---

## 2. Problem Statement

Institutional banking products and services are not universally sellable, bookable, or serviceable across all clients.

A client may appear commercially attractive, but eligibility may depend on many intersecting factors, including:

- client region, domicile, and operating footprint,
- client legal form and CBU type,
- sector classification,
- investor or customer category,
- source of funds / source of wealth,
- nature and purpose of the relationship,
- risk and policy posture,
- product family and service variant,
- instrument and market characteristics,
- booking entity and branch permissions,
- local jurisdiction restrictions,
- tax, distribution, and regulatory constraints,
- and supporting documentation or approvals.

In many institutions these controls are fragmented across spreadsheets, policy documents, legal memos, onboarding playbooks, product governance packs, and tribal knowledge.

This creates several recurring problems:

### 2.1 Fragmented decision ownership

No single governed capability answers the end-to-end question of whether a product may be sold and serviced for a client.

### 2.2 Inconsistent decisions

Different onboarding teams, product teams, and regions may interpret the same constraints differently.

### 2.3 Weak auditability

A decision is often made, but the supporting rationale, policy basis, and evidence trail are not captured in a durable, explainable structure.

### 2.4 Late-stage failure

Commercial negotiations may progress before legal, compliance, or servicing constraints are tested, resulting in expensive rework or client disappointment.

### 2.5 Poor reusability in automation

Policy prose and informal matrix logic are difficult for deterministic orchestration engines and agents to consume safely.

---

## 3. Vision

The target state is a governed **Booking Principles Capability** that maintains structured metadata profiles for both **clients** and **products**, evaluates them against policy and operational rules, and returns an explainable decision.

That decision should answer questions such as:

- Is this client eligible for this product?
- Which BNY legal entity may contract or book it?
- In which jurisdiction(s) may the service be offered?
- What conditions, approvals, or documents are required?
- Is the outcome allowed, conditional, escalated, prohibited, or indeterminate?

The long-term vision is to make booking-principles evaluation a reusable enterprise control service that can be invoked by:

- deal design,
- onboarding,
- product governance,
- KYC / AML review,
- service provisioning,
- agentic guidance flows,
- and deterministic DSL-driven runbooks.

---

## 4. Scope

### 4.1 In scope

This capability covers the structured determination of **client–product permissibility** using governed metadata and rules.

Specifically in scope:

- client profile modelling relevant to product eligibility,
- product profile modelling relevant to sellability / booking / servicing,
- booking entity and jurisdiction constraint modelling,
- policy and rule evaluation,
- decision outcomes with rationale,
- evidence and prerequisite requirement generation,
- escalation and exception signalling,
- audit trail support,
- integration into onboarding and product-service-resource selection.

### 4.2 Out of scope

The following are adjacent but not the primary responsibility of this capability:

- full KYC / AML adjudication,
- credit approval,
- pricing negotiation,
- legal document drafting,
- downstream operational fulfilment,
- transaction monitoring,
- full tax advice,
- customer profitability analysis.

These domains may provide inputs into booking-principles evaluation, or may consume its decisions, but are not replaced by it.

---

## 5. Core Concept

At its heart, Booking Principles is a governed decision function:

> **Client Profile × Product Profile × Booking Context × Policy Rules × Evidence State → Decision**

The output is typically not just a binary yes/no. It should support at least the following outcome classes:

- **Allowed**
- **Allowed with Conditions**
- **Requires Escalation**
- **Prohibited**
- **Insufficient Information**

Each outcome should carry structured rationale and references to the rules, policy sources, or missing evidence that drove the result.

---

## 6. Business Outcomes

A mature Booking Principles capability should deliver the following outcomes:

### 6.1 Earlier eligibility screening

Commercial and onboarding teams can identify impossible or restricted combinations before deep downstream effort is invested.

### 6.2 Consistent policy application

Rules are applied in a structured and repeatable way across teams and regions.

### 6.3 Explainable decisions

Every decision can be traced back to profile facts, rules, policy sources, and evidence state.

### 6.4 Better orchestration

The result can directly drive onboarding paths, required documents, escalation queues, and service provisioning eligibility.

### 6.5 Better metadata governance

Client and product attributes relevant to eligibility become first-class governed data assets rather than implicit spreadsheet columns.

---

## 7. Core Capabilities

### 7.1 Client profile capture and normalization

Maintain a governed metadata profile describing the client in terms relevant to product permissibility.

### 7.2 Product profile capture and normalization

Maintain a governed metadata profile describing the product or service in terms relevant to target market, restrictions, and booking constraints.

### 7.3 Booking context modelling

Represent the legal entity, branch, jurisdiction, market, and servicing context in which the product would be contracted and delivered.

### 7.4 Rule and policy evaluation

Evaluate structured rules against client, product, and booking-context metadata.

### 7.5 Condition and prerequisite generation

Return required approvals, documentation, and policy conditions when a product is only conditionally permissible.

### 7.6 Escalation detection

Identify combinations that require regional, legal, compliance, tax, or product-governance review.

### 7.7 Explainability and auditability

Persist rationale, matched rules, missing data, and decision lineage.

### 7.8 Reusable decision service

Expose the result as a callable service for onboarding workflows, deal design, research, and agentic guidance.

---

## 8. Conceptual Solution Approach

The recommended solution approach is to model Booking Principles as a governed control plane composed of:

1. **Client-side metadata profiles**
2. **Product-side metadata profiles**
3. **Booking / legal / jurisdiction context profiles**
4. **Rule definitions and policy mappings**
5. **Decision outputs with conditions and rationale**
6. **Evidence obligations and approvals**

This should not be implemented as a single monolithic spreadsheet matrix. Instead, it should be built as a structured metadata and rule framework that can produce a matrix-style decision view when needed.

### 8.1 Why structured profiles matter

A traditional matrix often hides complexity and becomes difficult to maintain. A structured metadata approach is stronger because:

- client facts can be governed and reused,
- product constraints can be versioned and owned,
- rules can be evaluated deterministically,
- decisions can be audited,
- missing information can be surfaced explicitly,
- and downstream orchestration can consume the result safely.

### 8.2 Decision layering

The solution should separate:

- **facts** — client, product, and context metadata,
- **rules** — policy or control logic,
- **evidence** — documents, attestations, approvals, and derived facts,
- **decision** — outcome, rationale, and next actions.

This separation aligns well with your broader OB-POC architecture and avoids conflating governed facts with rule interpretation.

---

## 9. Conceptual Metadata Model

## 9.1 Client-side profile metadata

The client-side profile should capture the attributes needed to determine whether a product may be offered, booked, and serviced.

### A. Identity and organizational context

- client master identifier
- client legal entity name
- legal form
- domicile country
- incorporation country
- operating countries / regions
- parent group / group structure reference
- CBU type
- relationship type to BNY

### B. Sector and business classification

- sector classification
- sub-sector classification
- market segment
- institutional type
- public/private status
- regulated/unregulated status
- fund / pension / insurer / asset manager / broker / sovereign / corporate / charity / family office / SPV markers

### C. Nature and purpose

- stated relationship purpose
- intended products / services sought
- business activity summary
- expected transaction types
- expected servicing model
- strategic relationship category

### D. Geography and distribution context

- target servicing regions
- target booking jurisdictions
- investor / customer base geography
- cross-border distribution footprint
- local presence markers
- jurisdiction sensitivity markers

### E. Regulatory and classification metadata

- client regulatory classification
- investor classification
- professional / eligible / institutional markers
- financial counterparty / non-financial counterparty markers where relevant
- exchange / market participation markers
- licensing / authorization status

### F. Risk and policy posture

- KYC status
- AML risk tier
- sanctions posture
- PEP / adverse-media sensitivity markers
- source-of-funds category
- source-of-wealth category where relevant
- high-risk jurisdiction indicators
- policy exception flags
- onboarding complexity indicator

### G. Ownership and control context

- UBO completeness status
- controlling-persons identified flag
- ownership complexity marker
- nominee / trust / layered holding indicators
- publicly listed exception markers where applicable

### H. Documentation and evidence posture

- core KYC evidence completeness
- tax documentation status
- constitutional documents completeness
- signatory / authority completeness
- policy attestation requirements
- known missing mandatory evidence

### I. Relationship and servicing metadata

- existing BNY relationship status
- existing service footprint
- prior approved products
- servicing model category
- operational support model
- dependency on specific locations / teams

---

## 9.2 Product-side profile metadata

The product-side profile should represent the governance, sellability, booking, servicing, and restriction characteristics of the product.

### A. Product identity and taxonomy

- product identifier
- product family
- product variant
- commercial product name
- service bundle mapping
- linked internal service and resource taxonomy

### B. Product purpose and intended market

- product objective
- intended client segment
- intended sector coverage
- permitted client types
- prohibited client types
- target market definition
- sophistication / complexity marker

### C. Legal and booking characteristics

- permitted BNY contracting entities
- permitted booking entities
- branch restrictions
- jurisdiction availability
- jurisdiction exclusions
- cross-border sale restrictions
- local licensing dependencies
- local representation requirements

### D. Instrument / market characteristics

- asset class / instrument type
- market / exchange dependencies
- clearing / settlement model
- custody network dependencies
- margin / collateral dependencies
- leverage / derivatives markers where relevant

### E. Regulatory and compliance characteristics

- regulatory regime tags
- product-governance classification
- restricted distribution markers
- retail prohibition markers
- institutional-only marker
- investor eligibility preconditions
- policy-sensitive features

### F. Risk and control characteristics

- inherent risk tier
- reputational sensitivity marker
- AML / sanctions sensitivity marker
- restricted-country sensitivity
- high-risk-sector prohibitions
- enhanced review requirements

### G. Documentation and approval requirements

- mandatory contractual documents
- product disclosure requirements
- suitability / appropriateness attestations where relevant
- tax forms or elections
- legal opinion requirements
- exception approval requirements
- committee or second-line approval dependencies

### H. Operational serviceability metadata

- supported operating regions
- supported currencies
- supported service windows
- operational prerequisites
- client data prerequisites
- dependency on downstream platforms
- manual servicing intensity marker
- implementation complexity marker

### I. Lifecycle and governance metadata

- product status
- launch / retire dates
- version or policy effective date
- product owner
- compliance owner
- legal owner
- governance source references

---

## 9.3 Booking-context metadata

A client and product alone are often insufficient. The proposed implementation should also maintain a booking-context profile.

Suggested metadata includes:

- proposed BNY legal entity
- proposed booking branch / office
- contracting location
- service delivery location
- governing law preference
- onboarding region
- booking market
- settlement market
- product distribution jurisdiction
- local policy overlays

---

## 9.4 Conceptual entity model

The capability should be represented as a governed conceptual model rather than a single flat matrix.

At a minimum, the following conceptual entities are recommended.

### A. ClientProfile

Represents the eligibility-relevant profile of the client or client business unit.

Suggested fields / groupings:

- client_profile_id
- client_id / party_id
- cbu_id where applicable
- legal_entity_id
- profile_status
- domicile_country
- operating_region_set
- sector_code
- client_type_code
- investor_classification_code
- source_of_funds_code
- nature_and_purpose_code
- aml_risk_tier
- sanctions_risk_flag
- ubo_completeness_status
- evidence_completeness_status
- effective_from / effective_to

### B. ProductProfile

Represents the eligibility-relevant profile of a commercial product, service bundle, or governed offering.

Suggested fields / groupings:

- product_profile_id
- product_id
- product_family_code
- product_variant_code
- service_bundle_id
- target_client_type_set
- prohibited_client_type_set
- supported_sector_set
- prohibited_sector_set
- supported_jurisdiction_set
- prohibited_jurisdiction_set
- permitted_booking_entity_set
- regulatory_tag_set
- mandatory_document_set
- approval_requirement_set
- product_status
- effective_from / effective_to

### C. BookingContextProfile

Represents the proposed commercial and operational pathway through which the product would be contracted, booked, and serviced.

Suggested fields / groupings:

- booking_context_id
- proposed_bny_legal_entity_id
- proposed_branch_id
- contracting_country
- booking_country
- service_region
- governing_law_code
- distribution_jurisdiction_set
- settlement_market_set
- local_policy_overlay_set
- operating_model_code

### D. EligibilityRule

Represents a governed rule or constraint used to evaluate permissibility.

Suggested fields / groupings:

- eligibility_rule_id
- rule_code
- rule_name
- rule_type
- policy_source_id
- applies_to_product_scope
- applies_to_client_scope
- applies_to_jurisdiction_scope
- severity
- outcome_mode
- condition_expression / structured predicate reference
- rationale_template
- effective_from / effective_to
- owner_team

### E. PolicySource

Represents the originating policy, legal memo, governance pack, or operating standard from which a rule is derived.

Suggested fields / groupings:

- policy_source_id
- source_type
- source_name
- source_version
- issuing_team
- jurisdiction_scope
- effective_date
- review_date
- citation_reference

### F. EligibilityDecision

Represents the outcome of evaluating a client/product/booking-context combination.

Suggested fields / groupings:

- eligibility_decision_id
- decision_timestamp
- client_profile_id
- product_profile_id
- booking_context_id
- decision_outcome
- decision_summary
- rules_matched_count
- escalation_required_flag
- insufficient_information_flag
- decision_version
- evaluator_id / service_id

### G. DecisionRationale

Represents structured explanation for the decision.

Suggested fields / groupings:

- decision_rationale_id
- eligibility_decision_id
- matched_rule_id
- rationale_type
- rationale_text
- source_policy_id
- fact_reference_payload
- priority_order

### H. DecisionCondition

Represents obligations or conditions attached to a non-simple approval outcome.

Suggested fields / groupings:

- decision_condition_id
- eligibility_decision_id
- condition_type
- condition_code
- condition_text
- required_before_stage
- owner_team
- satisfaction_status

### I. MissingInformationItem

Represents missing attributes, evidence, or unresolved choices preventing a definitive decision.

Suggested fields / groupings:

- missing_information_item_id
- eligibility_decision_id
- missing_item_type
- missing_attribute_code
- missing_evidence_code
- severity
- remediation_hint

### J. ApprovalRequirement

Represents formal review or approval gates attached to a product, jurisdiction, or decision path.

Suggested fields / groupings:

- approval_requirement_id
- scope_type
- scope_reference_id
- approval_type
- approval_owner_team
- mandatory_flag
- evidence_required_flag

---

## 10. Conceptual matrix view

Although the recommended implementation is metadata-driven, the business will often still want a matrix-style representation.

That matrix should be treated as a **derived decision view**, not the canonical source of truth.

A conceptual matrix may be rendered across dimensions such as:

- client sector
- client type / CBU type
- region / jurisdiction
- source-of-funds category
- nature-and-purpose category
- product family
- product variant
- booking entity
- outcome
- conditions
- escalation owner

This gives business stakeholders a familiar visual artifact while preserving a structured underlying data model.

---

## 11. Decision Model

A Booking Principles decision should produce a structured response containing:

### 10.1 Decision outcome

- allowed
- allowed_with_conditions
- requires_escalation
- prohibited
- insufficient_information

### 10.2 Decision basis

- matched rules
- rule conflicts if any
- source policy references
- product profile references
- client profile facts used
- booking-context facts used

### 10.3 Conditions and obligations

- required documents
- required approvals
- required attestations
- required client classifications
- required remediation steps

### 10.4 Missing information

- missing client attributes
- missing product attributes
- missing evidence
- unresolved jurisdiction choice
- unresolved booking-entity choice

### 10.5 Audit metadata

- decision timestamp
- decision version / rule-set version
- evaluator identity or service id
- explanation summary
- structured rationale payload

---

## 12. Rule Types

The solution should support multiple conceptual rule types.

### 11.1 Client eligibility rules

Example: only certain client sectors or classifications may access a product.

### 11.2 Jurisdiction rules

Example: a product may not be marketed or serviced into a specific jurisdiction.

### 11.3 Booking-entity rules

Example: only specific BNY legal entities may contract or book the product.

### 11.4 Risk and policy rules

Example: high-risk source-of-funds or high-risk jurisdictions may require escalation or prohibition.

### 11.5 Documentation prerequisite rules

Example: the product is allowed only when specific tax forms, constitutional docs, or legal opinions are present.

### 11.6 Operational capability rules

Example: a product may be legally sellable but not operationally supported in the relevant region.

### 11.7 Combined rules

Example: a product is allowed for pension clients in one jurisdiction through one booking entity, but prohibited for retail-fund structures in another.

---

## 13. Relationship to the OB-POC Domain Model

This capability fits naturally into the broader OB-POC architecture.

### 12.1 Relationship to Deal Map

The Deal Map expresses the commercial product relationship:

- what was negotiated,
- for which client entity set,
- via which rate cards and commercial packaging,
- and with which legal counterparties.

Booking Principles tests whether the negotiated commercial intent is actually permissible and executable.

### 12.2 Relationship to Onboarding Request

The Onboarding Request wraps a set of CBUs and requested services. Booking Principles should influence:

- whether the request can proceed,
- which product/service/resource combinations are eligible,
- what prerequisites are required,
- and whether escalation is needed before provisioning.

### 12.3 Relationship to KYC / UBO

KYC and UBO do not get replaced. Instead, their outputs inform the client-side profile:

- risk posture,
- ownership complexity,
- UBO completeness,
- source-of-funds confidence,
- and policy sensitivities.

### 12.4 Relationship to Product / Service / Resource taxonomy

The product profile should map into the existing product-service-resource hierarchy so that commercial products and operational services remain aligned.

---

## 14. Proposed Implementation Approach

### 13.1 Phase 1 — Conceptual model and metadata dictionary

Define:

- client-side eligibility attributes,
- product-side eligibility attributes,
- booking-context attributes,
- decision outcomes,
- rule taxonomy,
- and ownership model for governed metadata.

### 13.2 Phase 2 — Minimal viable rule engine

Implement a constrained rule layer capable of evaluating:

- sector,
- jurisdiction,
- booking entity,
- client type,
- risk posture,
- and documentation prerequisites.

### 13.3 Phase 3 — Decision service integration

Expose evaluation as a callable capability for:

- onboarding,
- product selection,
- deal design,
- and agent-guided review.

### 13.4 Phase 4 — Explainability and audit trail

Persist full rationale, rule references, missing data, and conditions for downstream review and control evidence.

### 13.5 Phase 5 — Expanded policy coverage

Add richer jurisdictional, tax, regulatory, and operational overlays as governed profiles mature.

---

## 15. Design Principles

The recommended design principles are:

### 14.1 Facts first

Capture governed metadata profiles before over-engineering rule syntax.

### 14.2 Separate facts from rules

Do not bury client or product truth inside rule text.

### 14.3 Prefer explainable outcomes

The system must say not only **what** the result is, but **why**.

### 14.4 Support incomplete-information states

A meaningful answer may be “insufficient information” rather than false certainty.

### 14.5 Treat conditions as first-class outputs

Conditional allowance is a core business reality and should not be modelled as an afterthought.

### 14.6 Align with deterministic orchestration

The output should be consumable by DSL runbooks and onboarding workflows without ambiguity.

---

## 16. Example Conceptual Decision

**Input:**

- Client sector: Pension Fund
- CBU type: Institutional Asset Owner
- Region: EMEA
- Source of funds: Regulated pension contributions
- Nature and purpose: Global custody and collateral services
- Product: Cross-border custody bundle with collateral support
- Proposed booking entity: BNY entity A
- Service region: Luxembourg / UK

**Possible output:**

- Outcome: `allowed_with_conditions`
- Conditions:
  - tax forms required
  - contractual schedule required
  - local product governance approval for one jurisdiction
- Rationale:
  - client type is permitted
  - jurisdictions are generally supported
  - one service feature requires additional local sign-off
- Missing information:
  - final contracting branch not yet confirmed

This is the type of output the capability should consistently produce.

---

## 17. Target End State

The target end state is an enterprise-grade capability in which:

- client and product eligibility metadata are governed,
- policy logic is structured and versioned,
- decisions are explainable,
- onboarding and deal design can invoke the service early,
- and downstream service provisioning consumes a clear permissibility result.

In that state, Booking Principles becomes a key bridge between:

- commercial intent,
- compliance and legal constraints,
- product governance,
- and executable onboarding reality.

---

## 18. BNY-facing positioning summary

For BNY Product and Engineering stakeholders, Booking Principles should be described as a reusable decision capability that transforms product-policy complexity into operationally usable control outcomes.

It gives the firm a governed way to answer, early and consistently:

- whether a product is appropriate for a client profile,
- whether it can be booked through the intended BNY entity,
- whether jurisdictional and policy constraints permit it,
- what evidence and approvals are still required,
- and whether onboarding should proceed, pause, escalate, or stop.

In strategic terms, it reduces dependency on fragmented spreadsheets and tribal interpretation, while enabling deterministic onboarding orchestration to consume eligibility outcomes as structured inputs.

That is why Booking Principles belongs as a core capability in the OB-POC control and data architecture.

---

## 19. Summary

Booking Principles should be treated as a distinct business and data capability, not as an informal spreadsheet exercise.

It solves a real institutional-banking problem:

> **determining whether a given client context is eligible for a given product, through a given legal and jurisdictional pathway, with clear conditions, rationale, and auditability**.

For OB-POC, this capability provides a natural governed decision layer between:

- Deal Map,
- Client / CBU profiling,
- KYC / UBO evidence,
- and Product / Service / Resource provisioning.

That makes it strategically important not only for onboarding control, but for the broader vision of deterministic, agent-assisted institutional client lifecycle orchestration.

