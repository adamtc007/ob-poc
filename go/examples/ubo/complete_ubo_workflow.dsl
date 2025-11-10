;; ===============================================================================
;; ULTIMATE BENEFICIAL OWNERSHIP (UBO) DISCOVERY WORKFLOW (v3.0)
;; ===============================================================================
;; This DSL example demonstrates a complete UBO identification and verification
;; workflow for a complex corporate entity using v3.0 EBNF compliant syntax.
;; It follows financial services best practices for AML/CFT compliance.
;;
;; Entity: TechGlobal Holdings S.à r.l. (Luxembourg)
;; Regulatory Framework: EU 5th Money Laundering Directive (5MLD)
;; Ownership Threshold: 25% (EU standard)
;; ===============================================================================

(define-kyc-investigation
  :id "techglobal-ubo-discovery-2024"
  :target-entity "techglobal-holdings-sarl"
  :jurisdiction "LU"
  :ubo-threshold 25.0
  :regulatory-framework ["EU5MLD", "LU_AML_LAW"]
  :investigation-date "2024-11-10"

  ;; =============================================================================
  ;; PHASE 1: ENTITY DECLARATIONS
  ;; =============================================================================

  ;; Primary Target Entity
  (entity
    :id "techglobal-holdings-sarl"
    :label "Company"
    :props {
      :legal-name "TechGlobal Holdings S.à r.l."
      :jurisdiction "LU"
      :entity-type "LLC"
      :registration-number "B123456"
      :incorporation-date "2020-03-15"
      :business-purpose "Technology investment holding company"
      :registered-address "12 Rue de la Paix, L-1234 Luxembourg"
    })

  ;; Parent Corporation (Cyprus)
  (entity
    :id "innovatetech-partners-cy"
    :label "Company"
    :props {
      :legal-name "InnovateTech Partners Ltd"
      :jurisdiction "CY"
      :entity-type "Private Limited Company"
      :registration-number "HE123456"
      :incorporation-date "2018-05-22"
      :business-purpose "Technology investment and development"
    })

  ;; Venture Fund (Delaware)
  (entity
    :id "globalventure-fund-ii"
    :label "Company"
    :props {
      :legal-name "GlobalVenture Fund II L.P."
      :jurisdiction "US-DE"
      :entity-type "Limited Partnership"
      :registration-number "DE-7890123"
      :fund-type "Venture Capital Fund"
    })

  ;; Management Entity
  (entity
    :id "techfounders-management"
    :label "Company"
    :props {
      :legal-name "TechFounders Management LLC"
      :jurisdiction "US-DE"
      :entity-type "LLC"
      :registration-number "DE-4567890"
    })

  ;; Individual UBOs
  (entity
    :id "person-alice-chen"
    :label "Person"
    :props {
      :legal-name "Alice Chen"
      :nationality "US"
      :date-of-birth "1975-08-12"
      :residence "Singapore"
      :occupation "Technology Entrepreneur"
    })

  (entity
    :id "person-robert-mueller"
    :label "Person"
    :props {
      :legal-name "Robert Mueller"
      :nationality "DE"
      :date-of-birth "1968-11-03"
      :residence "Berlin, Germany"
      :occupation "Venture Capital Partner"
    })

  ;; =============================================================================
  ;; PHASE 2: OWNERSHIP RELATIONSHIPS
  ;; =============================================================================

  ;; Direct ownership from parent corporation (45%)
  (edge
    :from "innovatetech-partners-cy"
    :to "techglobal-holdings-sarl"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 45.0
      :share-class "Class A Shares"
      :voting-rights 45.0
      :acquisition-date "2020-03-15"
    }
    :evidence ["shareholder-register-2024", "share-certificate-001"])

  ;; Direct ownership from venture fund (30%)
  (edge
    :from "globalventure-fund-ii"
    :to "techglobal-holdings-sarl"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 30.0
      :share-class "Class B Shares"
      :voting-rights 30.0
      :acquisition-date "2020-06-01"
    }
    :evidence ["investor-agreement-2020", "cap-table-q2-2024"])

  ;; Management ownership (25%)
  (edge
    :from "techfounders-management"
    :to "techglobal-holdings-sarl"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 25.0
      :share-class "Management Shares"
      :voting-rights 25.0
      :vesting-schedule "4-year-cliff"
    }
    :evidence ["management-agreement-2020", "stock-option-plan"])

  ;; Indirect ownership through parent corporation
  (edge
    :from "person-alice-chen"
    :to "innovatetech-partners-cy"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 60.0
      :share-class "Ordinary Shares"
      :voting-rights 60.0
      :control-type "DIRECT"
    }
    :evidence ["cy-company-registry", "shareholders-agreement"])

  (edge
    :from "person-robert-mueller"
    :to "innovatetech-partners-cy"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 40.0
      :share-class "Ordinary Shares"
      :voting-rights 40.0
      :control-type "DIRECT"
    }
    :evidence ["cy-company-registry", "shareholders-agreement"])

  ;; Fund management structure
  (edge
    :from "person-robert-mueller"
    :to "globalventure-fund-ii"
    :type "HAS_CONTROL"
    :props {
      :percent 100.0
      :control-type "GENERAL_PARTNER"
      :management-fee 2.0
      :carried-interest 20.0
    }
    :evidence ["fund-documents", "gp-agreement"])

  ;; Management entity control
  (edge
    :from "person-alice-chen"
    :to "techfounders-management"
    :type "HAS_CONTROL"
    :props {
      :percent 100.0
      :control-type "MANAGING_MEMBER"
      :voting-rights 100.0
    }
    :evidence ["llc-operating-agreement", "management-resolutions"])

  ;; =============================================================================
  ;; PHASE 3: KYC AND COMPLIANCE VERIFICATION
  ;; =============================================================================

  ;; Enhanced Due Diligence for the target entity
  (kyc.verify
    :customer-id "techglobal-holdings-sarl"
    :method "enhanced_due_diligence"
    :required-documents [
      "certificate_incorporation",
      "memorandum_articles",
      "board_resolution",
      "beneficial_ownership_declaration",
      "financial_statements"
    ]
    :jurisdiction-requirements ["LU_AML", "EU5MLD"]
    :verified-at "2024-11-10T10:00:00Z")

  ;; KYC for individual UBOs
  (kyc.verify
    :customer-id "person-alice-chen"
    :method "simplified_due_diligence"
    :required-documents ["passport", "utility_bill", "bank_reference"]
    :pep-check true
    :verified-at "2024-11-10T10:30:00Z")

  (kyc.verify
    :customer-id "person-robert-mueller"
    :method "simplified_due_diligence"
    :required-documents ["passport", "utility_bill", "professional_reference"]
    :pep-check true
    :verified-at "2024-11-10T10:45:00Z")

  ;; Sanctions screening for all entities
  (kyc.screen_sanctions
    :target "techglobal-holdings-sarl"
    :databases ["EU", "UN", "OFAC", "HMT"]
    :status "CLEAR"
    :screened-at "2024-11-10T11:00:00Z")

  (kyc.screen_sanctions
    :target "person-alice-chen"
    :databases ["EU", "UN", "OFAC", "HMT"]
    :status "CLEAR"
    :screened-at "2024-11-10T11:15:00Z")

  (kyc.screen_sanctions
    :target "person-robert-mueller"
    :databases ["EU", "UN", "OFAC", "HMT"]
    :status "CLEAR"
    :screened-at "2024-11-10T11:30:00Z")

  ;; PEP checks
  (kyc.check_pep
    :target "person-alice-chen"
    :status "NOT_PEP"
    :databases ["PEP_DATABASE_GLOBAL", "ADVERSE_MEDIA"]
    :checked-at "2024-11-10T11:45:00Z")

  (kyc.check_pep
    :target "person-robert-mueller"
    :status "NOT_PEP"
    :databases ["PEP_DATABASE_GLOBAL", "ADVERSE_MEDIA"]
    :checked-at "2024-11-10T12:00:00Z")

  ;; Compliance framework verification
  (compliance.verify
    :framework "EU5MLD"
    :jurisdiction "LU"
    :status "COMPLIANT"
    :checks [
      "beneficial_ownership_disclosure",
      "customer_due_diligence",
      "ongoing_monitoring",
      "suspicious_transaction_reporting"
    ]
    :verified-at "2024-11-10T12:15:00Z")

  ;; =============================================================================
  ;; PHASE 4: UBO CALCULATION
  ;; =============================================================================

  ;; Calculate UBO ownership using multiple prongs
  (ubo.calc
    :target "techglobal-holdings-sarl"
    :threshold 25.0
    :prongs ["ownership", "voting", "control"]
    :calculation-method "combined"
    :jurisdiction "LU"
    :max-depth 5
    :calculated-at "2024-11-10T13:00:00Z")

  ;; =============================================================================
  ;; PHASE 5: DECLARATIVE UBO OUTCOME
  ;; =============================================================================

  ;; Authoritative UBO determination results
  (ubo.outcome
    :target "techglobal-holdings-sarl"
    :at "2024-11-10T13:00:00Z"
    :threshold 25.0
    :jurisdiction "LU"
    :regulatory-framework "EU5MLD"
    :ubos [
      {
        :entity "person-alice-chen"
        :effective-percent 52.0
        :prongs {
          :ownership 52.0
          :voting 52.0
          :control 70.0
        }
        :paths [
          ["person-alice-chen", "innovatetech-partners-cy", "techglobal-holdings-sarl"],
          ["person-alice-chen", "techfounders-management", "techglobal-holdings-sarl"]
        ]
        :control-mechanisms ["direct_shareholding", "management_control"]
        :confidence "HIGH"
        :evidence [
          "cy-company-registry",
          "shareholders-agreement",
          "llc-operating-agreement",
          "shareholder-register-2024"
        ]
      },
      {
        :entity "person-robert-mueller"
        :effective-percent 48.0
        :prongs {
          :ownership 48.0
          :voting 48.0
          :control 30.0
        }
        :paths [
          ["person-robert-mueller", "innovatetech-partners-cy", "techglobal-holdings-sarl"],
          ["person-robert-mueller", "globalventure-fund-ii", "techglobal-holdings-sarl"]
        ]
        :control-mechanisms ["direct_shareholding", "fund_management"]
        :confidence "HIGH"
        :evidence [
          "cy-company-registry",
          "fund-documents",
          "gp-agreement",
          "cap-table-q2-2024"
        ]
      }
    ]
    :unresolved []
    :calculation-notes "Both individuals exceed 25% threshold through multiple ownership paths"
    :review-date "2025-11-10")

  ;; =============================================================================
  ;; PHASE 6: CBU ROLE ASSIGNMENTS
  ;; =============================================================================

  ;; Assign Alice Chen as primary UBO
  (role.assign
    :entity "person-alice-chen"
    :role "UltimateBeneficialOwner"
    :cbu "CBU-TECHGLOBAL-001"
    :effective-percent 52.0
    :control-level "PRIMARY"
    :period {
      :start "2024-11-10"
      :review-date "2025-11-10"
    }
    :evidence ["ubo.outcome:techglobal-ubo-discovery-2024"]
    :confidence "HIGH")

  ;; Assign Robert Mueller as secondary UBO
  (role.assign
    :entity "person-robert-mueller"
    :role "UltimateBeneficialOwner"
    :cbu "CBU-TECHGLOBAL-001"
    :effective-percent 48.0
    :control-level "SECONDARY"
    :period {
      :start "2024-11-10"
      :review-date "2025-11-10"
    }
    :evidence ["ubo.outcome:techglobal-ubo-discovery-2024"]
    :confidence "HIGH")

  ;; =============================================================================
  ;; PHASE 7: ONGOING MONITORING SETUP
  ;; =============================================================================

  ;; Schedule periodic UBO review
  (compliance.schedule-review
    :target "techglobal-holdings-sarl"
    :review-type "UBO_PERIODIC_REVIEW"
    :frequency "ANNUAL"
    :next-review-date "2025-11-10"
    :triggers [
      "ownership_change_above_5_percent",
      "control_structure_change",
      "regulatory_requirement_change"
    ]
    :scheduled-at "2024-11-10T14:00:00Z")

  ;; Enable continuous monitoring
  (compliance.enable-monitoring
    :target "techglobal-holdings-sarl"
    :monitoring-types [
      "adverse_media_screening",
      "sanctions_list_updates",
      "pep_status_changes",
      "corporate_registry_updates"
    ]
    :frequency "DAILY"
    :enabled-at "2024-11-10T14:15:00Z")

  ;; =============================================================================
  ;; PHASE 8: FINAL WORKFLOW TRANSITION
  ;; =============================================================================

  ;; Complete the UBO discovery workflow
  (workflow.transition
    :from "UBO_CALCULATION"
    :to "UBO_VERIFIED"
    :reason "UBO identification complete with 2 individuals above 25% threshold"
    :timestamp "2024-11-10T14:30:00Z"
    :metadata {
      :total-entities-analyzed 7
      :ubos-identified 2
      :ownership-paths-traced 4
      :compliance-frameworks ["EU5MLD", "LU_AML"]
      :confidence-level "HIGH"
      :next-review "2025-11-10"
    }))

;; ===============================================================================
;; UBO DISCOVERY WORKFLOW COMPLETE
;; ===============================================================================
;;
;; Final Results Summary:
;; - Target Entity: TechGlobal Holdings S.à r.l. (Luxembourg)
;; - UBO Threshold: 25% (EU5MLD standard)
;; - UBOs Identified: 2 individuals
;;   1. Alice Chen: 52% effective ownership (PRIMARY)
;;   2. Robert Mueller: 48% effective ownership (SECONDARY)
;;
;; Ownership Structure:
;; - Direct ownership through 3 intermediate entities
;; - Complex control mechanisms including fund management and LLC control
;; - Multiple ownership paths for each UBO providing redundancy
;;
;; Compliance Status: VERIFIED
;; - All entities and individuals screened (sanctions, PEP, adverse media)
;; - Enhanced due diligence completed for corporate entity
;; - Simplified due diligence completed for individual UBOs
;; - EU 5th Money Laundering Directive requirements satisfied
;;
;; This example demonstrates:
;; ✅ v3.0 EBNF compliant syntax throughout
;; ✅ Complete UBO discovery workflow with complex ownership structures
;; ✅ Multiple ownership paths and control mechanisms
;; ✅ Comprehensive KYC and compliance verification
;; ✅ Declarative UBO outcome with detailed calculation results
;; ✅ Role assignments for CBU mapping
;; ✅ Ongoing monitoring and review scheduling
;; ✅ DSL-as-State pattern with complete audit trail
;; ===============================================================================
