;; DSL v3.0 Example: Zenith Capital UBO Discovery Workflow
;; This example demonstrates the new unified syntax and declarative verbs

(define-kyc-investigation
  :id "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0

  ;; 1. GRAPH DEFINITION - Entity declarations
  (entity
    :id "company-zenith-spv-001"
    :label "Company"
    :props {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
      :entity-type "Limited Partnership"
    })

  (entity
    :id "alpha-holdings-sg"
    :label "Company"
    :props {
      :legal-name "Alpha Holdings Pte Ltd"
      :registration-number "SG-789012"
      :jurisdiction "SG"
      :entity-type "Private Limited Company"
    })

  (entity
    :id "person-john-smith"
    :label "Person"
    :props {
      :legal-name "John Smith"
      :nationality "US"
      :date-of-birth "1975-03-15"
      :passport-number "US123456789"
    })

  ;; 2. OWNERSHIP RELATIONSHIPS
  (edge
    :from "person-john-smith"
    :to "alpha-holdings-sg"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 100.0
      :share-class "Ordinary Shares"
      :voting-rights 100.0
    }
    :evidence ["doc-sg-registry-001" "doc-share-certificate-001"])

  (edge
    :from "alpha-holdings-sg"
    :to "company-zenith-spv-001"
    :type "HAS_OWNERSHIP"
    :props {
      :percent 45.0
      :share-class "Class A Units"
      :voting-rights 45.0
    }
    :evidence ["doc-cayman-registry-001" "doc-partnership-agreement-001"])

  ;; 3. KYC/COMPLIANCE ACTIONS
  (kyc.collect_document
    :target "company-zenith-spv-001"
    :doc-type "certificate_incorporation"
    :doc-id "doc-cayman-registry-001"
    :status "VERIFIED"
    :collected-at "2025-11-10T09:15:00Z")

  (kyc.collect_document
    :target "alpha-holdings-sg"
    :doc-type "certificate_incorporation"
    :doc-id "doc-sg-registry-001"
    :status "VERIFIED"
    :collected-at "2025-11-10T09:30:00Z")

  (kyc.verify
    :customer-id "person-john-smith"
    :method "enhanced_due_diligence"
    :doc-types ["passport" "utility_bill" "bank_statement"]
    :verified-at "2025-11-10T10:00:00Z")

  (kyc.screen_sanctions
    :target "person-john-smith"
    :databases ["OFAC" "UN" "EU" "HMT"]
    :status "CLEAR"
    :screened-at "2025-11-10T10:15:00Z")

  (compliance.fatca_check
    :entity "company-zenith-spv-001"
    :status "NON_US"
    :classification "PASSIVE_NFFE"
    :checked-at "2025-11-10T10:20:00Z")

  ;; 4. UBO CALCULATION
  (ubo.calc
    :target "company-zenith-spv-001"
    :threshold 25.0
    :prongs ["ownership" "voting"]
    :jurisdiction "KY"
    :calculated-at "2025-11-10T10:30:00Z")

  ;; 5. DECLARATIVE OUTCOME - This is the calculated state
  (ubo.outcome
    :target "company-zenith-spv-001"
    :at "2025-11-10T10:30:00Z"
    :threshold 25.0
    :ubos [
      {
        :entity "person-john-smith"
        :effective-percent 45.0
        :prongs {
          :ownership 45.0
          :voting 45.0
        }
        :paths [
          ["person-john-smith" "alpha-holdings-sg" "company-zenith-spv-001"]
        ]
        :confidence "HIGH"
        :evidence ["doc-sg-registry-001" "doc-cayman-registry-001" "doc-share-certificate-001"]
      }
    ]
    :unresolved [])

  ;; 6. CBU MAPPING - Assign roles to CBU context
  (role.assign
    :entity "person-john-smith"
    :role "UltimateBeneficialOwner"
    :cbu "CBU-ZENITH-001"
    :period {
      :start "2025-11-10"
    }
    :confidence "HIGH"
    :effective-percent 45.0
    :evidence ["ubo.outcome:zenith-capital-ubo-discovery"])

  ;; 7. COMPLIANCE VERIFICATION
  (compliance.verify
    :framework "FATF"
    :jurisdiction "KY"
    :status "COMPLIANT"
    :checks ["sanctions_screening" "pep_check" "adverse_media" "ubo_identification"]
    :verified-at "2025-11-10T11:00:00Z")

  ;; 8. CASE MANAGEMENT
  (case.update
    :id "CBU-ZENITH-001"
    :status "UBO_IDENTIFIED"
    :add-products ["CUSTODY" "FUND_ACCOUNTING"]
    :updated-at "2025-11-10T11:15:00Z")

  ;; 9. WORKFLOW TRANSITION
  (workflow.transition
    :from "UBO_DISCOVERY"
    :to "COMPLIANCE_REVIEW"
    :reason "UBO successfully identified and verified"
    :timestamp "2025-11-10T11:30:00Z"))
