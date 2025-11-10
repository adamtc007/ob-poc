;;; ============================================================================
;;; CUSTODY PRODUCT ONBOARDING - COMPLETE DSL EXAMPLE (v3.0)
;;; ============================================================================
;;;
;;; This example demonstrates the complete onboarding flow for a Client Business
;;; Unit (CBU) to the Custody product, including service discovery and resource
;;; provisioning for all standard custody business services.
;;;
;;; Client: Global Investment Partners LLC
;;; Product: CUSTODY
;;; Services: Safekeeping, Security Movement, Trade Capture, Reconciliation,
;;;          Special Settlement Instructions, Custody Reporting
;;; Resources: Platform systems, engines, and infrastructure components
;;;
;;; Timeline: Complete onboarding workflow from initial case creation to
;;;          full operational readiness
;;; ============================================================================

;;; ----------------------------------------------------------------------------
;;; PHASE 1: CASE CREATION
;;; Initialize the onboarding case for the client
;;; State: CREATED
;;; ----------------------------------------------------------------------------

(case.create
  :cbu-id "CBU-CUSTODY-2024-001"
  :nature-purpose "Institutional asset management firm requiring comprehensive custody services for multi-asset class portfolios including equities, fixed income, and alternative investments across global markets"
  :client-name "Global Investment Partners LLC"
  :client-type "INSTITUTIONAL_INVESTMENT_MANAGER"
  :jurisdiction "US"
  :assets-under-management "15000000000"
  :regulatory-status "SEC_REGISTERED_INVESTMENT_ADVISER")

;;; Result:
;;; - CBU ID: CBU-CUSTODY-2024-001
;;; - Status: CREATED
;;; - Onboarding workflow initiated

;;; ----------------------------------------------------------------------------
;;; PHASE 2: PRODUCT SELECTION
;;; Add the Custody product to the onboarding case
;;; State: CREATED → PRODUCTS_ADDED
;;; ----------------------------------------------------------------------------

(case.update
  :id "CBU-CUSTODY-2024-001"
  :add-products ["CUSTODY"]
  :justification "Client requires institutional-grade custody services for safekeeping of investment assets, trade settlement, and comprehensive reporting"
  :expected-volume "Daily trade volume: 500-1000 transactions"
  :asset-classes ["EQUITIES", "FIXED_INCOME", "ALTERNATIVES", "CASH"]
  :markets ["US", "EUROPE", "ASIA_PACIFIC"])

;;; Result:
;;; - Status: CREATED → PRODUCTS_ADDED
;;; - Products: [CUSTODY]

;;; ----------------------------------------------------------------------------
;;; PHASE 3: SERVICE DISCOVERY
;;; Discover and plan all services required for Custody product
;;; State: PRODUCTS_ADDED → SERVICES_PLANNED
;;; ----------------------------------------------------------------------------

(services.plan
  :services [
    {
      :name "SAFEKEEPING"
      :description "Secure custody and safekeeping of client assets"
      :sla "99.9% availability"
      :criticality "HIGH"
      :dependencies ["SECURITY_MASTER", "POSITION_KEEPING"]
    },
    {
      :name "SECURITY_MOVEMENT"
      :description "Asset transfer and movement processing"
      :sla "T+1 settlement"
      :criticality "HIGH"
      :dependencies ["SETTLEMENT_ENGINE", "COUNTERPARTY_NETWORK"]
    },
    {
      :name "TRADE_CAPTURE"
      :description "Trade instruction capture and validation"
      :sla "Real-time processing"
      :criticality "MEDIUM"
      :dependencies ["ORDER_MANAGEMENT", "RISK_ENGINE"]
    },
    {
      :name "RECONCILIATION"
      :description "Daily position and cash reconciliation"
      :sla "T+1 reconciliation"
      :criticality "HIGH"
      :dependencies ["POSITION_KEEPING", "CASH_MANAGEMENT"]
    },
    {
      :name "REPORTING"
      :description "Custody reporting and analytics"
      :sla "Daily reports by 8AM ET"
      :criticality "MEDIUM"
      :dependencies ["DATA_WAREHOUSE", "REPORTING_ENGINE"]
    }
  ])

;;; Result:
;;; - Status: PRODUCTS_ADDED → SERVICES_PLANNED
;;; - Discovered: 5 core custody services
;;; - Dependencies identified for each service

;;; ----------------------------------------------------------------------------
;;; PHASE 4: RESOURCE PROVISIONING
;;; Provision all required resources for planned services
;;; State: SERVICES_PLANNED → RESOURCES_PROVISIONED
;;; ----------------------------------------------------------------------------

(resources.plan
  :resources [
    {
      :type "CUSTODY_PLATFORM"
      :name "Custody Core Platform"
      :owner "CustodyTech"
      :capacity "10TB storage, 32GB RAM"
      :environment "PRODUCTION"
    },
    {
      :type "SETTLEMENT_ENGINE"
      :name "Global Settlement Engine"
      :owner "SettlementOps"
      :capacity "1000 TPS"
      :environment "PRODUCTION"
    },
    {
      :type "RISK_ENGINE"
      :name "Real-time Risk Engine"
      :owner "RiskTech"
      :capacity "500 risk calculations/second"
      :environment "PRODUCTION"
    },
    {
      :type "REPORTING_DB"
      :name "Custody Reporting Database"
      :owner "DataOps"
      :capacity "5TB storage"
      :environment "PRODUCTION"
    }
  ])

;;; Result:
;;; - Status: SERVICES_PLANNED → RESOURCES_PROVISIONED
;;; - Provisioned: 4 core platform resources
;;; - Assigned ownership and capacity planning

;;; ----------------------------------------------------------------------------
;;; PHASE 5: COMPLIANCE VERIFICATION
;;; Verify regulatory compliance for custody operations
;;; State: RESOURCES_PROVISIONED → COMPLIANCE_VERIFIED
;;; ----------------------------------------------------------------------------

(compliance.verify
  :framework "SEC_CUSTODY_RULES"
  :jurisdiction "US"
  :status "COMPLIANT"
  :checks ["custody_rule_compliance", "segregation_requirements", "audit_trail", "reporting_requirements"]
  :verified-at "2024-11-10T15:30:00Z"
  :certification-period {
    :start "2024-11-10"
    :end "2025-11-10"
  })

;;; Result:
;;; - Status: RESOURCES_PROVISIONED → COMPLIANCE_VERIFIED
;;; - Regulatory compliance confirmed
;;; - Annual certification established

;;; ----------------------------------------------------------------------------
;;; PHASE 6: FINAL CASE CLOSURE
;;; Complete the onboarding process
;;; State: COMPLIANCE_VERIFIED → COMPLETE
;;; ----------------------------------------------------------------------------

(case.close
  :id "CBU-CUSTODY-2024-001"
  :status "COMPLETE"
  :completion-reason "All custody services successfully onboarded and operational"
  :go-live-date "2024-11-15"
  :completed-at "2024-11-10T16:00:00Z"
  :next-review-date "2025-02-10")

;;; Result:
;;; - Status: COMPLIANCE_VERIFIED → COMPLETE
;;; - CBU fully onboarded to Custody product
;;; - Go-live scheduled for 2024-11-15
;;; - Quarterly review scheduled

;;; ----------------------------------------------------------------------------
;;; WORKFLOW TRANSITION SUMMARY
;;; State flow: CREATED → PRODUCTS_ADDED → SERVICES_PLANNED →
;;;            RESOURCES_PROVISIONED → COMPLIANCE_VERIFIED → COMPLETE
;;; ----------------------------------------------------------------------------

(workflow.transition
  :from "COMPLIANCE_VERIFIED"
  :to "COMPLETE"
  :reason "Custody onboarding successfully completed"
  :timestamp "2024-11-10T16:00:00Z"
  :metadata {
    :total-duration "45 days"
    :services-count 5
    :resources-count 4
    :compliance-frameworks ["SEC_CUSTODY_RULES"]
  })

;;; ============================================================================
;;; END OF CUSTODY ONBOARDING EXAMPLE
;;; ============================================================================
