; ===============================================================================
; ULTIMATE BENEFICIAL OWNERSHIP (UBO) DISCOVERY WORKFLOW
; ===============================================================================
; This DSL example demonstrates a complete UBO identification and verification
; workflow for a complex corporate entity. It follows financial services best
; practices for AML/CFT compliance and regulatory requirements.
;
; Entity: TechGlobal Holdings S.à r.l. (Luxembourg)
; Regulatory Framework: EU 5th Money Laundering Directive (5MLD)
; Ownership Threshold: 25% (EU standard)
; ===============================================================================

; =============================================================================
; PHASE 1: ENTITY DATA COLLECTION
; =============================================================================

; Step 1.1: Collect comprehensive entity information
(ubo.collect-entity-data
  (entity_name "TechGlobal Holdings S.à r.l.")
  (jurisdiction "LU")
  (entity_type "LLC")
  (registration_number "B123456")
  (business_purpose "Technology investment holding company"))

; Step 1.2: Retrieve declared ownership structure from corporate registry
(ubo.get-ownership-structure
  (entity_id @attr{entity-uuid-techglobal})
  (depth_limit 10)
  (include_voting_rights true)
  (include_control_agreements true))

; Step 1.3: Map initial ownership relationships
(attributes.define
  (attr-id @attr{ownership-parent-corp})
  (name "ownership.parent_corporation")
  (value "InnovateTech Partners Ltd (Cyprus)")
  (percentage 45.0)
  (link_type "DIRECT_SHARE"))

(attributes.define
  (attr-id @attr{ownership-venture-fund})
  (name "ownership.venture_fund")
  (value "GlobalVenture Fund II L.P. (Delaware)")
  (percentage 30.0)
  (link_type "DIRECT_SHARE"))

(attributes.define
  (attr-id @attr{ownership-management})
  (name "ownership.management_entity")
  (value "TechFounders Management LLC (Delaware)")
  (percentage 25.0)
  (link_type "DIRECT_SHARE"))

; =============================================================================
; PHASE 2: RECURSIVE OWNERSHIP UNROLLING
; =============================================================================

; Step 2.1: Unroll complex ownership structures recursively
(ubo.unroll-structure
  (entity_id @attr{entity-uuid-techglobal})
  (consolidation_method "ADDITIVE")
  (max_depth 15)
  (stop_at_natural_persons true)
  (threshold_cutoff 5.0))

; Step 2.2: Unroll InnovateTech Partners Ltd (Cyprus) - 45% shareholder
(ubo.collect-entity-data
  (entity_name "InnovateTech Partners Ltd")
  (jurisdiction "CY")
  (entity_type "CORPORATION")
  (parent_entity_id @attr{entity-uuid-techglobal}))

(ubo.get-ownership-structure
  (entity_id @attr{entity-uuid-innovatetech})
  (depth_limit 5))

; InnovateTech Partners ownership breakdown:
; - Maria Kowalski (Poland): 60% = 27% indirect in TechGlobal (45% × 60%)
; - Chen Wei Holdings Ltd (Singapore): 40% = 18% indirect in TechGlobal (45% × 40%)

; Step 2.3: Unroll GlobalVenture Fund II L.P. (Delaware) - 30% shareholder
(ubo.collect-entity-data
  (entity_name "GlobalVenture Fund II L.P.")
  (jurisdiction "US")
  (entity_type "PARTNERSHIP")
  (parent_entity_id @attr{entity-uuid-techglobal}))

; GlobalVenture Fund complex structure:
; - Limited Partners (various institutions): 95%
; - General Partner (GlobalVenture Management LLC): 5% (but full control)

; Step 2.4: Unroll TechFounders Management LLC - 25% shareholder
(ubo.collect-entity-data
  (entity_name "TechFounders Management LLC")
  (jurisdiction "US")
  (entity_type "LLC")
  (parent_entity_id @attr{entity-uuid-techglobal}))

; TechFounders Management ownership:
; - Alex Johnson (US citizen): 40% = 10% indirect in TechGlobal (25% × 40%)
; - Sarah Chen (US citizen): 35% = 8.75% indirect in TechGlobal (25% × 35%)
; - Tech Employees Trust: 25% = 6.25% indirect in TechGlobal (25% × 25%)

; =============================================================================
; PHASE 3: UBO IDENTIFICATION AND THRESHOLD APPLICATION
; =============================================================================

; Step 3.1: Calculate total indirect ownership for all natural persons
(ubo.calculate-indirect-ownership
  (person_id @attr{person-uuid-maria-kowalski})
  (target_entity_id @attr{entity-uuid-techglobal})
  (calculation_method "PATH_MULTIPLICATION")
  (result_percentage 27.0))

(ubo.calculate-indirect-ownership
  (person_id @attr{person-uuid-alex-johnson})
  (target_entity_id @attr{entity-uuid-techglobal})
  (calculation_method "PATH_MULTIPLICATION")
  (result_percentage 10.0))

; Step 3.2: Identify control prong individuals (regardless of ownership %)
(ubo.identify-control-prong
  (entity_id @attr{entity-uuid-techglobal})
  (control_types ["CEO", "BOARD_MAJORITY", "VOTING_CONTROL", "MANAGEMENT_AGREEMENT"])
  (include_indirect_control true))

; Control prong identification results:
; - Dr. Hans Mueller (CEO of TechGlobal): Senior Managing Official
; - GlobalVenture Management LLC: Controls 30% through GP role

; Step 3.3: Apply EU 5MLD regulatory thresholds
(ubo.apply-thresholds
  (ownership_results @attr{calculated-ownership-data})
  (control_results @attr{control-prong-data})
  (regulatory_framework "EU_5MLD")
  (ownership_threshold 25.0)
  (include_control_prong true))

; Step 3.4: Resolve final UBO list
(ubo.resolve-ubos
  (entity_id @attr{entity-uuid-techglobal})
  (ownership_threshold 25.0)
  (jurisdiction_rules "EU_5MLD")
  (consolidate_family_interests false)
  (include_dormant_shareholders false))

; Final UBO Results:
; UBO #1: Maria Kowalski (27% ownership via InnovateTech Partners)
; UBO #2: Dr. Hans Mueller (0% ownership but CEO control prong)
; UBO #3: [To be determined from GlobalVenture Fund analysis]

; =============================================================================
; PHASE 4: IDENTITY VERIFICATION
; =============================================================================

; Step 4.1: Verify Maria Kowalski (Poland) - 27% ownership UBO
(ubo.verify-identity
  (ubo_id @attr{ubo-uuid-maria-kowalski})
  (document_list [
    "polish_national_id",
    "passport",
    "proof_of_address_poland",
    "bank_statement"
  ])
  (verification_level "ENHANCED")
  (require_biometric_check true)
  (address_verification_required true))

; Step 4.2: Verify Dr. Hans Mueller (Germany) - CEO control prong UBO
(ubo.verify-identity
  (ubo_id @attr{ubo-uuid-hans-mueller})
  (document_list [
    "german_personalausweis",
    "passport",
    "proof_of_address_luxembourg",
    "employment_contract",
    "board_appointment_letter"
  ])
  (verification_level "SUPERIOR")
  (management_verification true)
  (source_of_wealth_required true))

; Step 4.3: Verify additional UBOs from fund structure
(ubo.verify-identity
  (ubo_id @attr{ubo-uuid-globalventure-controller})
  (document_list [
    "us_drivers_license",
    "ssn_verification",
    "proof_of_address_us",
    "sec_form_adv"
  ])
  (verification_level "ENHANCED")
  (investment_adviser_check true))

; =============================================================================
; PHASE 5: COMPLIANCE SCREENING
; =============================================================================

; Step 5.1: Screen Maria Kowalski against all relevant databases
(ubo.screen-person
  (ubo_id @attr{ubo-uuid-maria-kowalski})
  (screening_lists [
    "EU_SANCTIONS",
    "OFAC_SDN",
    "OFAC_SECTORAL",
    "UN_SANCTIONS",
    "UK_SANCTIONS",
    "PEP_DATABASE_DOWJONES",
    "ADVERSE_MEDIA_LEXISNEXIS"
  ])
  (screening_intensity "COMPREHENSIVE")
  (fuzzy_matching_enabled true)
  (ongoing_monitoring true))

; Step 5.2: Screen Dr. Hans Mueller (enhanced screening for control person)
(ubo.screen-person
  (ubo_id @attr{ubo-uuid-hans-mueller})
  (screening_lists [
    "EU_SANCTIONS",
    "OFAC_FULL",
    "GERMAN_BFI_DATABASE",
    "PEP_DATABASE_COMPREHENSIVE",
    "ADVERSE_MEDIA_DEEP_SEARCH",
    "CRIMINAL_BACKGROUND_CHECK"
  ])
  (screening_intensity "DEEP")
  (management_person_screening true)
  (country_specific_checks ["DE", "LU"])
  (professional_sanctions_check true))

; Step 5.3: Screen fund-related UBOs
(ubo.screen-person
  (ubo_id @attr{ubo-uuid-globalventure-controller})
  (screening_lists [
    "OFAC_FULL",
    "FINRA_SANCTIONS",
    "SEC_SANCTIONS",
    "CFTC_SANCTIONS",
    "PEP_DATABASE_US",
    "ADVERSE_MEDIA_FINANCIAL_SERVICES"
  ])
  (screening_intensity "COMPREHENSIVE")
  (investment_manager_screening true)
  (finra_background_check true))

; =============================================================================
; PHASE 6: RISK ASSESSMENT
; =============================================================================

; Step 6.1: Assess individual UBO risk profiles
(ubo.assess-risk
  (entity_id @attr{entity-uuid-techglobal})
  (ubo_list @attr{verified-ubo-list})
  (risk_factors [
    "COMPLEX_OWNERSHIP_STRUCTURE",
    "MULTI_JURISDICTION_PRESENCE",
    "PRIVATE_EQUITY_INVOLVEMENT",
    "TECHNOLOGY_SECTOR_EXPOSURE",
    "EU_REGULATORY_COMPLIANCE"
  ])
  (jurisdiction_risk_matrix "EU_5MLD_STANDARD")
  (industry_risk_profile "TECHNOLOGY_MEDIUM")
  (customer_risk_appetite "MEDIUM_HIGH"))

; Risk Assessment Results:
; Overall Entity Risk: MEDIUM-HIGH
; - Maria Kowalski: MEDIUM (EU jurisdiction, no PEP/sanctions hits)
; - Dr. Hans Mueller: MEDIUM-HIGH (Control person, management role)
; - Fund Structure: HIGH (Complex US fund structure, multiple LPs)

; Step 6.2: Generate risk-based mitigation requirements
(compliance.screen
  (entity_id @attr{entity-uuid-techglobal})
  (risk_rating "MEDIUM_HIGH")
  (mitigation_requirements [
    "ENHANCED_DUE_DILIGENCE",
    "SENIOR_MANAGEMENT_APPROVAL",
    "QUARTERLY_UBO_REVIEW",
    "FUND_STRUCTURE_DOCUMENTATION",
    "ONGOING_MONITORING_ENHANCED"
  ])
  (approval_authority "HEAD_OF_COMPLIANCE")
  (review_frequency "QUARTERLY"))

; =============================================================================
; PHASE 7: ONGOING MONITORING SETUP
; =============================================================================

; Step 7.1: Configure ongoing monitoring for entity structure changes
(ubo.monitor-changes
  (entity_id @attr{entity-uuid-techglobal})
  (monitoring_frequency "MONTHLY")
  (alert_thresholds [
    ("OWNERSHIP_CHANGE", 5.0),
    ("NEW_SHAREHOLDER", 10.0),
    ("CONTROL_CHANGE", "ANY"),
    ("SANCTIONS_HIT", "IMMEDIATE"),
    ("ADVERSE_MEDIA", "HIGH_RISK_ONLY")
  ])
  (data_sources [
    "CORPORATE_REGISTRY_LU",
    "CORPORATE_REGISTRY_CY",
    "SEC_FILINGS",
    "SANCTIONS_DATABASES",
    "PEP_DATABASES",
    "ADVERSE_MEDIA_FEEDS"
  ])
  (escalation_matrix "UBO_MONITORING_PROTOCOL"))

; Step 7.2: Schedule periodic UBO data refresh
(ubo.refresh-data
  (entity_id @attr{entity-uuid-techglobal})
  (refresh_frequency "QUARTERLY")
  (full_reverification_frequency "ANNUALLY")
  (data_sources [
    "BENEFICIAL_OWNERSHIP_REGISTRIES",
    "CORPORATE_FILINGS",
    "FUND_ADMINISTRATOR_REPORTS",
    "VERIFICATION_PROVIDERS"
  ])
  (auto_trigger_conditions [
    "SIGNIFICANT_OWNERSHIP_CHANGE",
    "CONTROL_STRUCTURE_CHANGE",
    "REGULATORY_FRAMEWORK_UPDATE"
  ]))

; Step 7.3: Set up regulatory reporting requirements
(compliance.monitor
  (entity_id @attr{entity-uuid-techglobal})
  (reporting_requirements [
    ("LU_AML_AUTHORITY", "ANNUAL"),
    ("EU_5MLD_REPORTING", "ANNUAL"),
    ("INTERNAL_COMPLIANCE", "QUARTERLY")
  ])
  (documentation_requirements [
    "UBO_IDENTIFICATION_REPORT",
    "VERIFICATION_EVIDENCE_FILE",
    "SCREENING_RESULTS_ARCHIVE",
    "RISK_ASSESSMENT_DOCUMENTATION"
  ])
  (retention_period "10_YEARS"))

; =============================================================================
; PHASE 8: COMPLIANCE DOCUMENTATION AND AUDIT TRAIL
; =============================================================================

; Step 8.1: Generate comprehensive UBO compliance documentation
(audit.log
  (event "UBO_DISCOVERY_COMPLETE")
  (entity_id @attr{entity-uuid-techglobal})
  (timestamp @attr{completion-timestamp})
  (compliance_officer @attr{officer-uuid})
  (regulatory_framework "EU_5MLD")
  (ubos_identified 3)
  (verification_status "COMPLETE")
  (screening_status "CLEAR")
  (risk_rating "MEDIUM_HIGH")
  (approval_status "APPROVED")
  (next_review_due @attr{next-review-date}))

; Step 8.2: Bind all collected attribute values
(values.bind
  (bind (attr-id @attr{entity-legal-name})
        (value "TechGlobal Holdings S.à r.l."))
  (bind (attr-id @attr{entity-jurisdiction})
        (value "LU"))
  (bind (attr-id @attr{ubo-count})
        (value 3))
  (bind (attr-id @attr{ownership-threshold-applied})
        (value 25.0))
  (bind (attr-id @attr{regulatory-framework})
        (value "EU_5MLD"))
  (bind (attr-id @attr{compliance-status})
        (value "COMPLIANT"))
  (bind (attr-id @attr{risk-rating-overall})
        (value "MEDIUM_HIGH"))
  (bind (attr-id @attr{monitoring-frequency})
        (value "MONTHLY"))
  (bind (attr-id @attr{next-review-due})
        (value "2024-09-30")))

; =============================================================================
; WORKFLOW COMPLETION AND NEXT STEPS
; =============================================================================

; Step 9.1: Transition case to UBO-compliant state
(workflow.transition
  (from "UBO_DISCOVERY_PENDING")
  (to "UBO_COMPLIANT")
  (condition "ALL_UBOS_VERIFIED_AND_SCREENED")
  (approval_required true)
  (approver @attr{compliance-officer-uuid}))

; Step 9.2: Set up future review triggers
(ubo.trigger-review
  (entity_id @attr{entity-uuid-techglobal})
  (review_reason "PERIODIC_REVIEW")
  (priority "MEDIUM")
  (scheduled_date "2024-12-31")
  (review_scope [
    "OWNERSHIP_STRUCTURE_CHANGES",
    "UBO_VERIFICATION_REFRESH",
    "SCREENING_UPDATE",
    "RISK_REASSESSMENT"
  ]))

; =============================================================================
; SUMMARY OF UBO DISCOVERY RESULTS
; =============================================================================

; Entity: TechGlobal Holdings S.à r.l. (Luxembourg)
; Total UBOs Identified: 3
;
; UBO #1: Maria Kowalski (Polish national)
;   - Ownership: 27% (indirect via InnovateTech Partners Ltd)
;   - Qualification: Ownership threshold (>25%)
;   - Verification: VERIFIED
;   - Screening: CLEAR
;   - Risk: MEDIUM
;
; UBO #2: Dr. Hans Mueller (German national)
;   - Ownership: 0%
;   - Qualification: Control prong (CEO/Senior Managing Official)
;   - Verification: VERIFIED
;   - Screening: CLEAR
;   - Risk: MEDIUM-HIGH
;
; UBO #3: [GlobalVenture Fund Controller]
;   - Ownership: 30% (indirect via fund GP control)
;   - Qualification: Control prong (GP of fund holding 30%)
;   - Verification: VERIFIED
;   - Screening: CLEAR
;   - Risk: HIGH (complex fund structure)
;
; Overall Compliance Status: COMPLIANT
; Overall Risk Rating: MEDIUM-HIGH
; Next Review Due: Q4 2024
; Ongoing Monitoring: ACTIVE (Monthly)
;
; This UBO discovery workflow demonstrates comprehensive compliance with
; EU 5th Money Laundering Directive requirements, including:
; ✅ 25% ownership threshold application
; ✅ Control prong identification
; ✅ Recursive ownership unrolling
; ✅ Enhanced identity verification
; ✅ Comprehensive sanctions/PEP screening
; ✅ Risk-based assessment
; ✅ Ongoing monitoring setup
; ✅ Complete audit trail and documentation
; ===============================================================================
