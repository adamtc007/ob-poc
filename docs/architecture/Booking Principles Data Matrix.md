# Booking Principles Data Matrix and Cross-Reference Model

## Version 1.0

## 1. Purpose

This artefact is intended to make the Booking Principles concept easier to visualise.

It shows the capability in three complementary views:

1. **Client-side attribute matrix** — the client metadata relevant to eligibility
2. **Product-side attribute matrix** — the product metadata relevant to sellability / booking / servicing
3. **Cross-reference rule linkage map** — the rule layer that evaluates combinations of client and product facts in context

A fourth section then shows the **conceptual data entity model** as a separate view, so the audience can distinguish between:

* the **attributes being compared**,
* the **rules that join them**,
* and the **entities needed to persist the model**.

This is useful because the business often thinks in matrix form, while solution design needs a proper entity and rule model.

---

## 2. How to read this model

The intended logic is:

> **Client attributes** + **Product attributes** + **Booking / jurisdiction context** + **Rule set** = **Eligibility decision**

So the rule layer is the bridge between the two sides.

This should not be implemented as a single giant spreadsheet. Instead:

* the **left side** is the client profile,
* the **right side** is the product profile,
* the **middle** is the rule / policy / condition linkage,
* and the **output** is a decision with rationale, conditions, and missing-information markers.

---

## 3. Visual summary

```text
+---------------------------+      +-----------------------------+      +---------------------------+
|   Client Attribute Set    | ---> |  Eligibility Rule Layer     | ---> |  Decision / Conditions    |
|                           | <--- |  (policy / booking logic)   | <--- |  / Escalation / Gaps      |
+---------------------------+      +-----------------------------+      +---------------------------+
               ^                                  ^
               |                                  |
               |                                  |
      +-------------------+             +----------------------------+
      | Booking Context   |             | Product Attribute Set      |
      | / Jurisdiction    |             |                            |
      +-------------------+             +----------------------------+
```

This is the cleanest way to explain the concept to both business and engineering audiences.

---

## 4. Client-side attribute matrix

The client-side profile captures the facts that materially influence whether a product may be sold, booked, onboarded, and serviced.

| Client attribute group           | Example attributes                                                               | Why it matters to Booking Principles                                       |
| -------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Identity and legal structure     | client legal entity, legal form, domicile, incorporation country                 | Determines legal/jurisdictional permissibility and contracting context     |
| Group / relationship context     | parent group, affiliate structure, relationship type, existing BNY footprint     | Helps assess relationship scope and reuse of existing servicing context    |
| CBU / operating model            | CBU type, business unit role, service-consuming unit, operational footprint      | Determines who is actually subscribing to / consuming the service          |
| Sector classification            | pension fund, asset manager, insurer, retail fund, sovereign, broker, corporate  | Product eligibility often varies by sector and client type                 |
| Market / customer classification | institutional, professional, eligible counterparty, regulated client markers     | Used to test target-market and distribution constraints                    |
| Nature and purpose               | stated purpose, intended use case, expected activity                             | Used to determine whether the requested product use is aligned to policy   |
| Geography                        | operating countries, target service geographies, cross-border footprint          | Key for jurisdiction restrictions and local-service constraints            |
| Source of funds / wealth         | regulated pension contributions, asset-management revenue, fund subscriptions    | Relevant to AML sensitivity and policy gating                              |
| Risk posture                     | AML tier, sanctions posture, adverse-media markers, high-risk country indicators | Can prohibit, condition, or escalate certain products or booking paths     |
| Ownership / control posture      | UBO completeness, control complexity, trust/nominee markers                      | Important where policy depends on resolved ownership/control understanding |
| Regulatory status                | licensed / regulated status, entity authorisations, market participation markers | Some products require specific client regulatory standing                  |
| Evidence completeness            | core KYC complete, tax forms complete, constitutional docs complete              | Drives whether an allowed outcome is unconditional or conditional          |
| Existing relationship state      | already onboarded, existing entitlements, prior approvals                        | Can accelerate or constrain product expansion decisions                    |

---

## 5. Product-side attribute matrix

The product-side profile captures the facts that determine whether the product is permitted, through which route, and under what conditions.

| Product attribute group     | Example attributes                                                             | Why it matters to Booking Principles                                |
| --------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------------- |
| Product identity            | product family, product variant, commercial product name                       | Defines the offer under evaluation                                  |
| Service mapping             | linked service bundle, internal service taxonomy, resource dependencies        | Connects commercial intent to operational delivery                  |
| Target market               | intended client segments, supported sectors, permitted client types            | Core eligibility logic is often driven by target-market definitions |
| Prohibited market segments  | prohibited client types, excluded sectors, retail prohibition markers          | Allows explicit non-eligibility rules                               |
| Jurisdiction coverage       | supported jurisdictions, restricted countries, prohibited distribution markets | Controls where the product may be sold or serviced                  |
| Legal / booking constraints | permitted BNY legal entities, permitted branches, local licensing dependencies | Determines the valid booking / contracting pathway                  |
| Regulatory overlays         | regime tags, institutional-only markers, distribution constraints              | Reflects product governance and regulatory perimeter                |
| Risk characteristics        | risk tier, reputational sensitivity, sanctions-sensitive features              | Can require extra review or prohibit certain client combinations    |
| Documentation requirements  | mandatory agreements, disclosures, tax forms, legal opinions                   | Drives conditional obligations attached to approval                 |
| Approval requirements       | product governance sign-off, local compliance sign-off, legal approval         | Important for conditional and escalated outcomes                    |
| Operational serviceability  | supported currencies, markets, ops model, implementation complexity            | A product may be legally sellable but not operationally serviceable |
| Lifecycle / governance      | launch status, retired status, effective dates, product owner                  | Ensures rules apply only to currently governed offerings            |

---

## 6. Booking-context matrix

Because client and product alone are not sufficient, the decision also needs an explicit booking context.

| Booking-context group          | Example attributes                                    | Why it matters                                             |
| ------------------------------ | ----------------------------------------------------- | ---------------------------------------------------------- |
| BNY legal entity               | contracting entity, booking entity                    | A product may be allowed only through certain BNY entities |
| Branch / office                | branch, local office, service location                | Local permissions and servicing constraints often apply    |
| Contracting / governing law    | contract location, governing law                      | Legal path may vary by jurisdiction                        |
| Distribution context           | where product is marketed / offered                   | Cross-border distribution rules may apply                  |
| Settlement / operating markets | custody market, collateral market, settlement market  | Downstream market support may constrain availability       |
| Local overlays                 | country policy overlays, tax overlays, legal overlays | Adds local nuance to otherwise global rules                |

---

## 7. Cross-reference rule linkage map

This is the middle layer that connects client-side and product-side facts.

### 7.1 Rule linkage concept

A rule typically evaluates one or more client attributes against one or more product attributes, sometimes with booking-context qualifiers.

Conceptually:

```text
Client Attribute(s)
    + Product Attribute(s)
    + Booking Context Attribute(s)
    + Policy Source / Rule Definition
    = Decision fragment
```

Multiple decision fragments combine into the final eligibility result.

---

### 7.2 Example rule linkage map

| Rule family                         | Client-side inputs                                    | Product-side inputs                                 | Booking-context inputs               | Typical outcome                        |
| ----------------------------------- | ----------------------------------------------------- | --------------------------------------------------- | ------------------------------------ | -------------------------------------- |
| Sector eligibility                  | sector, client type                                   | target sectors, prohibited sectors                  | optional jurisdiction                | allowed / prohibited                   |
| Investor classification eligibility | investor classification, regulated status             | permitted client classes, institutional-only marker | distribution jurisdiction            | allowed / prohibited / conditional     |
| Source-of-funds sensitivity         | source-of-funds category, AML risk tier               | product risk tier, sensitivity markers              | booking entity / region              | allowed / escalate / prohibit          |
| Jurisdiction eligibility            | domicile, operating geography, distribution footprint | supported jurisdictions, excluded jurisdictions     | booking country, offering country    | allowed / prohibited                   |
| Booking-entity validity             | client geography, relationship region                 | permitted booking entities, licensing dependencies  | proposed BNY entity / branch         | allowed / conditional / prohibit       |
| Nature-and-purpose alignment        | intended use case, relationship purpose               | product objective, permitted use cases              | service model                        | allowed / conditional                  |
| Evidence prerequisite rule          | evidence completeness, tax doc status                 | mandatory documents, disclosure requirements        | jurisdiction                         | conditional / insufficient information |
| Ownership / control sensitivity     | UBO completeness, complex ownership markers           | product sensitivity / policy restrictions           | jurisdiction / legal entity          | escalate / prohibit / conditional      |
| Operational supportability          | service region, client operating model                | supported currencies, supported markets, ops model  | service location / settlement market | allowed / conditional / prohibit       |
| Existing relationship reuse         | existing onboarding status, prior approvals           | expansion rules, product governance conditions      | existing BNY entity                  | fast-track / conditional               |

---

### 7.3 Rule outcome types

Each rule should be able to produce a structured outcome such as:

* `allow`
* `allow_with_conditions`
* `escalate`
* `prohibit`
* `insufficient_information`

Rules should also return:

* rationale text or rationale code
* policy source reference
* required evidence or approval items
* blocking or non-blocking severity

---

## 8. Decision assembly model

The final decision is not usually a single rule hit. It is assembled from multiple rule evaluations.

| Stage                  | Description                                                            |
| ---------------------- | ---------------------------------------------------------------------- |
| Fact collection        | Gather client, product, and booking-context attributes                 |
| Rule selection         | Determine which rules apply to this product/client/context combination |
| Rule evaluation        | Evaluate all applicable rules                                          |
| Outcome consolidation  | Merge results into one decision state                                  |
| Condition generation   | Attach required evidence, documents, approvals, or remediation         |
| Explanation generation | Produce rationale and rule lineage                                     |
| Persisted decision     | Store decision, conditions, gaps, and audit metadata                   |

---

## 9. Conceptual entity model

This section shows the underlying entity model as a separate artefact.

### 9.1 Core entities

| Entity                   | Purpose                                                                |
| ------------------------ | ---------------------------------------------------------------------- |
| `ClientProfile`          | Stores eligibility-relevant client attributes                          |
| `ProductProfile`         | Stores eligibility-relevant product attributes                         |
| `BookingContextProfile`  | Stores legal/jurisdictional/servicing context for the proposed path    |
| `EligibilityRule`        | Stores rule definitions and linkage logic                              |
| `PolicySource`           | Stores the originating policy/governance source for rules              |
| `EligibilityDecision`    | Stores the resulting decision for a specific evaluated combination     |
| `DecisionRationale`      | Stores rule-by-rule reasoning and explanation fragments                |
| `DecisionCondition`      | Stores obligations attached to conditional outcomes                    |
| `MissingInformationItem` | Stores facts/evidence still required                                   |
| `ApprovalRequirement`    | Stores formal approval gates attached to products, rules, or decisions |

---

### 9.2 Relationship view

```text
ClientProfile ---------\
                        \
                         >---- EligibilityDecision ----< DecisionRationale
                        /                 |
ProductProfile --------/                  |
                                          +----< DecisionCondition
BookingContextProfile -/                  |
                                          +----< MissingInformationItem

EligibilityRule -------> (applied during evaluation) <------- PolicySource

ApprovalRequirement ---> ProductProfile / EligibilityRule / EligibilityDecision
```

This shows the distinction between:

* **profiles** (facts)
* **rules** (logic)
* **sources** (governance origin)
* **decision records** (outputs)
* **conditions / gaps / approvals** (follow-on obligations)

---

### 9.3 Suggested entity metadata shape

The following sections expand the conceptual entities into a more review-ready data model. The intent is not to lock every physical implementation decision, but to provide enough detail for serious data review, challenge, and eventual schema design.

### `ClientProfile`

**Purpose**
Stores the eligibility-relevant client and CBU-side facts used by Booking Principles evaluation.

**Key identifiers / references**

* client_profile_id
* client_id
* party_id
* legal_entity_id
* cbu_id
* group_id
* onboarding_request_id where evaluation is request-scoped

**Core classification attributes**

* client_type_code
* cbu_type_code
* sector_code
* sub_sector_code
* institutional_type_code
* market_segment_code
* investor_classification_code
* regulatory_classification_code
* relationship_type_code

**Jurisdiction and geography attributes**

* domicile_country_code
* incorporation_country_code
* primary_operating_country_code
* operating_region_code
* service_region_preference_code
* target_distribution_region_code
* high_risk_jurisdiction_flag

**Nature, purpose, and business activity attributes**

* nature_and_purpose_code
* intended_use_case_code
* business_activity_code
* expected_activity_profile_code
* expected_transaction_pattern_code
* strategic_relationship_category_code

**Risk and policy posture attributes**

* aml_risk_tier_code
* sanctions_risk_tier_code
* pep_flag
* adverse_media_flag
* source_of_funds_code
* source_of_wealth_code where relevant
* policy_exception_flag
* enhanced_due_diligence_required_flag
* onboarding_complexity_code

**Ownership and control attributes**

* ubo_completeness_status_code
* ownership_complexity_code
* control_structure_code
* nominee_structure_flag
* trust_structure_flag
* publicly_listed_exception_flag
* controlling_persons_identified_flag

**Evidence and document posture attributes**

* kyc_evidence_status_code
* constitutional_document_status_code
* tax_document_status_code
* authority_document_status_code
* evidence_completeness_status_code
* outstanding_mandatory_evidence_count

**Relationship and servicing attributes**

* existing_client_flag
* existing_service_footprint_code
* servicing_model_code
* relationship_owner_reference
* prior_approval_reuse_flag

**Lifecycle / governance attributes**

* profile_status_code
* effective_from
* effective_to
* created_at
* updated_at
* created_by
* updated_by
* source_system_code
* source_confidence_code where externally researched

**Suggested child structures / dependent tables**

* `ClientProfileJurisdiction` for multi-country / multi-region coverage
* `ClientProfileClassification` for multi-valued classifications
* `ClientProfileRiskFlag` for discrete risk markers
* `ClientProfileEvidenceStatus` for per-evidence-family state

---

### `ProductProfile`

**Purpose**
Stores the eligibility-relevant product, service-bundle, and governance-side facts used by Booking Principles evaluation.

**Key identifiers / references**

* product_profile_id
* product_id
* product_family_id
* product_variant_id
* service_bundle_id
* service_catalog_id where relevant
* product_owner_party_id

**Core classification attributes**

* product_family_code
* product_variant_code
* product_category_code
* service_model_code
* target_market_code
* complexity_tier_code
* product_governance_classification_code
* institutional_only_flag
* retail_prohibited_flag

**Target-market and eligibility attributes**

* target_client_type_rule_code
* target_sector_rule_code
* prohibited_client_type_rule_code
* prohibited_sector_rule_code
* permitted_investor_classification_rule_code
* prohibited_investor_classification_rule_code
* permitted_regulatory_status_rule_code

**Jurisdiction and booking attributes**

* global_product_flag
* supported_jurisdiction_rule_code
* prohibited_jurisdiction_rule_code
* cross_border_distribution_rule_code
* permitted_booking_entity_rule_code
* permitted_branch_rule_code
* local_licensing_required_flag
* local_presence_required_flag

**Instrument / market / serviceability attributes**

* asset_class_code
* instrument_type_code
* settlement_model_code
* custody_market_support_code
* collateral_support_code
* margin_support_code
* supported_currency_rule_code
* supported_market_rule_code
* operational_support_model_code
* implementation_complexity_code
* manual_servicing_intensity_code

**Risk and control attributes**

* product_risk_tier_code
* reputational_sensitivity_code
* aml_sensitivity_code
* sanctions_sensitivity_code
* restricted_country_sensitivity_flag
* enhanced_review_required_flag

**Documentation and approval attributes**

* mandatory_contract_document_rule_code
* mandatory_disclosure_rule_code
* tax_document_requirement_rule_code
* legal_opinion_requirement_flag
* committee_approval_rule_code
* second_line_approval_rule_code
* exception_approval_rule_code

**Lifecycle / governance attributes**

* product_status_code
* launch_date
* retire_date
* effective_from
* effective_to
* product_owner_reference
* compliance_owner_reference
* legal_owner_reference
* source_policy_set_reference
* created_at
* updated_at

**Suggested child structures / dependent tables**

* `ProductProfileJurisdictionRule`
* `ProductProfileBookingEntityRule`
* `ProductProfileDocumentRequirement`
* `ProductProfileApprovalRequirement`
* `ProductProfileOperationalConstraint`

---

### `BookingContextProfile`

**Purpose**
Stores the proposed pathway through which the product would be contracted, booked, distributed, and serviced.

**Key identifiers / references**

* booking_context_id
* onboarding_request_id
* client_id
* cbu_id
* product_id

**Legal / entity routing attributes**

* proposed_bny_legal_entity_id
* proposed_booking_entity_id
* proposed_contracting_entity_id
* proposed_branch_id
* proposed_service_entity_id

**Geographic / jurisdictional attributes**

* contracting_country_code
* booking_country_code
* service_delivery_country_code
* service_region_code
* offering_jurisdiction_code
* distribution_jurisdiction_code
* settlement_market_code
* custody_market_code

**Legal and policy context attributes**

* governing_law_code
* local_policy_overlay_code
* local_tax_overlay_code
* local_licensing_context_code
* cross_border_service_flag

**Operational context attributes**

* operating_model_code
* servicing_team_code
* implementation_region_code
* downstream_platform_context_code
* support_window_code

**Lifecycle / governance attributes**

* context_status_code
* effective_from
* effective_to
* created_at
* updated_at

**Suggested child structures / dependent tables**

* `BookingContextJurisdiction`
* `BookingContextPolicyOverlay`
* `BookingContextMarketSupport`

---

### `EligibilityRule`

**Purpose**
Stores the governed rule definition or rule reference used to evaluate client/product/context combinations.

**Key identifiers / references**

* eligibility_rule_id
* rule_code
* rule_family_code
* policy_source_id
* approval_requirement_id where rule directly creates an approval

**Classification attributes**

* rule_name
* rule_description
* rule_type_code
* rule_scope_code
* severity_code
* blocking_flag
* active_flag

**Applicability attributes**

* applies_to_product_family_code
* applies_to_product_variant_code
* applies_to_client_type_code
* applies_to_sector_code
* applies_to_investor_classification_code
* applies_to_jurisdiction_code
* applies_to_booking_entity_code
* applies_to_service_model_code

**Evaluation attributes**

* outcome_mode_code
* predicate_reference
* rule_expression_format_code
* rule_priority
* conflict_resolution_code
* rationale_template
* remediation_template

**Governance attributes**

* owner_team_code
* issuing_function_code
* effective_from
* effective_to
* review_due_date
* version_no
* created_at
* updated_at

**Suggested child structures / dependent tables**

* `EligibilityRuleCondition`
* `EligibilityRuleApplicability`
* `EligibilityRuleOutcome`
* `EligibilityRulePolicyReference`

---

### `PolicySource`

**Purpose**
Stores the policy, legal, governance, or operating source from which rules are derived.

**Key identifiers / references**

* policy_source_id
* document_id
* document_version_id
* semantic_registry_entry_id where governed in Semantic OS

**Classification attributes**

* source_type_code
* source_family_code
* source_name
* source_short_code
* issuing_team_code
* jurisdiction_scope_code
* domain_scope_code

**Version / publication attributes**

* source_version
* publication_status_code
* effective_date
* review_date
* superseded_by_policy_source_id
* citation_reference

**Interpretation attributes**

* operational_interpretation_reference
* legal_text_reference
* policy_summary

**Lifecycle / governance attributes**

* active_flag
* created_at
* updated_at

---

### `EligibilityDecision`

**Purpose**
Stores the resulting decision for a specific client/product/booking-context evaluation.

**Key identifiers / references**

* eligibility_decision_id
* client_profile_id
* product_profile_id
* booking_context_id
* onboarding_request_id
* deal_id where relevant

**Outcome attributes**

* decision_outcome_code
* decision_summary
* decision_status_code
* escalation_required_flag
* insufficient_information_flag
* conditional_flag
* prohibited_flag

**Evaluation metadata**

* rule_set_version
* rules_evaluated_count
* rules_matched_count
* blocking_rule_count
* non_blocking_condition_count
* decision_timestamp
* evaluator_service_id
* evaluator_run_id
* explanation_payload_reference

**Governance / lifecycle attributes**

* superseded_by_decision_id
* valid_from
* valid_to
* created_at
* created_by

**Suggested child structures / dependent tables**

* `DecisionRationale`
* `DecisionCondition`
* `MissingInformationItem`
* `DecisionAppliedRule`

---

### `DecisionRationale`

**Purpose**
Stores explanation fragments showing why the decision outcome was produced.

**Key identifiers / references**

* decision_rationale_id
* eligibility_decision_id
* eligibility_rule_id
* policy_source_id

**Explanation attributes**

* rationale_type_code
* rationale_sequence_no
* rationale_text
* business_summary_text
* fact_reference_payload
* evidence_reference_payload
* blocking_flag
* severity_code

**Lifecycle attributes**

* created_at

---

### `DecisionCondition`

**Purpose**
Stores conditions, obligations, remediation items, or prerequisites attached to a decision.

**Key identifiers / references**

* decision_condition_id
* eligibility_decision_id
* approval_requirement_id where relevant
* source_rule_id

**Condition attributes**

* condition_type_code
* condition_code
* condition_text
* owner_team_code
* required_before_stage_code
* mandatory_flag
* blocking_until_satisfied_flag
* satisfaction_status_code
* due_date where applicable

**Lifecycle attributes**

* created_at
* satisfied_at
* satisfied_by

---

### `MissingInformationItem`

**Purpose**
Stores the missing facts, unresolved choices, or absent evidence preventing a final or unconditional decision.

**Key identifiers / references**

* missing_information_item_id
* eligibility_decision_id
* source_rule_id

**Gap attributes**

* missing_item_type_code
* missing_attribute_code
* missing_evidence_code
* missing_choice_code
* severity_code
* blocking_flag
* remediation_hint_text
* owner_team_code

**Lifecycle attributes**

* created_at
* resolved_at
* resolved_by

---

### `ApprovalRequirement`

**Purpose**
Stores approval gates relevant to products, rules, or specific decisions.

**Key identifiers / references**

* approval_requirement_id
* policy_source_id
* product_profile_id where product-scoped
* eligibility_rule_id where rule-scoped
* eligibility_decision_id where decision-instantiated

**Approval attributes**

* approval_type_code
* approval_scope_code
* approval_owner_team_code
* mandatory_flag
* evidence_required_flag
* approval_sla_code
* escalation_path_code
* approval_text

**Lifecycle attributes**

* effective_from
* effective_to
* created_at
* updated_at

---

## 9.4 Attribute design guidance

For serious review, the following design assumptions should be explicit.

### A. Separate stable profile facts from decision outcomes

Client, product, and booking-context profiles represent facts or proposed context. Decisions, conditions, and gaps are derived outputs and should remain separate.

### B. Expect multi-valued attributes

Jurisdictions, classifications, approvals, risk markers, supported markets, and evidence states are often multi-valued. The model should therefore allow dependent child tables or associative structures rather than forcing everything into one row.

### C. Prefer governed codes over free text

Sector, client type, jurisdiction, risk tier, investor class, decision outcome, and approval type should all be governed code sets aligned to Semantic OS / attribute dictionary governance where possible.

### D. Support effective dating and versioning

Both profiles and rules need time-bounded validity. This is especially important for product governance and policy-source change.

### E. Preserve explainability

Every decision should be traceable to the facts used, the rules applied, the policy sources cited, and the conditions or gaps produced.

### F. Keep policy source and operational rule distinct

A rule may operationalize a policy, but the source policy document/version should remain separately identifiable.

---

