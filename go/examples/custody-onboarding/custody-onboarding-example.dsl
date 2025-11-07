;;; ============================================================================
;;; CUSTODY PRODUCT ONBOARDING - COMPLETE DSL EXAMPLE
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
  (cbu.id "CBU-CUSTODY-2024-001")
  (nature-purpose "Institutional asset management firm requiring comprehensive custody services for multi-asset class portfolios including equities, fixed income, and alternative investments across global markets")
  (client.name "Global Investment Partners LLC")
  (client.type "INSTITUTIONAL_INVESTMENT_MANAGER")
  (jurisdiction "US")
  (assets-under-management "15000000000")
  (regulatory-status "SEC_REGISTERED_INVESTMENT_ADVISER"))

;;; Result:
;;; - CBU ID: CBU-CUSTODY-2024-001
;;; - Status: CREATED
;;; - Onboarding workflow initiated

;;; ----------------------------------------------------------------------------
;;; PHASE 2: PRODUCT SELECTION
;;; Add the Custody product to the onboarding case
;;; State: CREATED → PRODUCTS_ADDED
;;; ----------------------------------------------------------------------------

(products.add
  (product "CUSTODY")
  (justification "Client requires institutional-grade custody services for safekeeping of investment assets, trade settlement, and comprehensive reporting")
  (expected-volume "Daily trade volume: 500-1000 transactions")
  (asset-classes "EQUITIES" "FIXED_INCOME" "ALTERNATIVES" "CASH")
  (markets "US" "EUROPE" "ASIA_PACIFIC"))

;;; Result:
;;; - Product CUSTODY added to case
;;; - Product requirements documented
;;; - Ready for service discovery

;;; ----------------------------------------------------------------------------
;;; PHASE 3: SERVICE DISCOVERY
;;; Discover and configure all business services for Custody product
;;; State: PRODUCTS_ADDED → SERVICES_DISCOVERED
;;; ----------------------------------------------------------------------------

(services.discover
  (product "CUSTODY")
  (services
    (service
      (name "Safekeeping")
      (description "Asset safekeeping and segregation service")
      (business-requirements
        (segregation "FULL_CLIENT_SEGREGATION")
        (asset-types "PHYSICAL_CERTIFICATES" "ELECTRONIC_SECURITIES")
        (custody-model "GLOBAL_CUSTODY")
        (nominee-services "REQUIRED")))

    (service
      (name "SecurityMovement")
      (description "Security movement and control service")
      (business-requirements
        (movement-types "DVP" "RVP" "FREE_DELIVERY")
        (settlement-cycles "T+0" "T+1" "T+2" "T+3")
        (counterparties "PRIME_BROKERS" "CENTRAL_DEPOSITORIES" "CUSTODIANS")
        (currencies "USD" "EUR" "GBP" "JPY" "CHF")))

    (service
      (name "TradeCapture")
      (description "Trade capture and control service")
      (business-requirements
        (capture-methods "ELECTRONIC_FEEDS" "MANUAL_INPUT" "FILE_UPLOAD")
        (validation-rules "POSITION_LIMITS" "SETTLEMENT_DATE_VALIDATION" "COUNTERPARTY_LIMITS")
        (trade-sources "EXECUTION_MANAGEMENT_SYSTEMS" "PRIME_BROKERS" "DIRECT_INPUT")
        (asset-classes "ALL_SUPPORTED")))

    (service
      (name "Reconciliation")
      (description "Position and cash reconciliation service")
      (business-requirements
        (reconciliation-frequency "DAILY" "INTRADAY")
        (external-sources "PRIME_BROKERS" "CENTRAL_DEPOSITORIES" "MARKET_DATA_VENDORS")
        (tolerance-levels "ZERO_TOLERANCE_CASH" "POSITION_VARIANCE_0.01%")
        (exception-handling "AUTOMATED_MATCHING" "MANUAL_RESEARCH" "ESCALATION_WORKFLOW")))

    (service
      (name "SpecialSettlementInstructions")
      (description "Special Settlement Instructions management")
      (business-requirements
        (instruction-types "STANDING_SSI" "TRADE_SPECIFIC_SSI" "EXCEPTION_SSI")
        (maintenance-workflow "CLIENT_SELF_SERVICE" "OPERATIONS_MAINTAINED")
        (validation-rules "COUNTERPARTY_VERIFICATION" "ACCOUNT_VALIDATION" "LIMIT_CHECKS")
        (approval-workflow "DUAL_APPROVAL_REQUIRED")))

    (service
      (name "CustodyReporting")
      (description "Comprehensive custody reporting service")
      (business-requirements
        (report-types "POSITION_STATEMENTS" "TRANSACTION_REPORTS" "PERFORMANCE_ATTRIBUTION" "REGULATORY_REPORTS")
        (frequency "DAILY" "WEEKLY" "MONTHLY" "QUARTERLY" "ON_DEMAND")
        (delivery-methods "SECURE_PORTAL" "EMAIL" "SFTP" "API")
        (formats "PDF" "EXCEL" "CSV" "XML" "JSON")
        (customization "CLIENT_BRANDED" "CUSTOM_LAYOUTS" "TAILORED_CONTENT")))))

;;; Result:
;;; - 6 core custody services identified and configured
;;; - Business requirements documented for each service
;;; - Service specifications ready for resource mapping

;;; ----------------------------------------------------------------------------
;;; PHASE 4: RESOURCE DISCOVERY AND PROVISIONING
;;; Map services to implementation resources and provision infrastructure
;;; State: SERVICES_DISCOVERED → RESOURCES_DISCOVERED
;;; ----------------------------------------------------------------------------

(resources.discover
  (service-mappings

    ;; Safekeeping Service Resources
    (service "Safekeeping"
      (resources
        (resource
          (name "CustodyMainPlatform")
          (type "PRIMARY_PLATFORM")
          (owner "CustodyTech")
          (configuration
            (environment "PRODUCTION")
            (high-availability "ACTIVE_ACTIVE")
            (backup-strategy "REAL_TIME_REPLICATION")
            (security-controls "SOC2_TYPE2" "PCI_DSS")))

        (resource
          (name "PhysicalVaultSystem")
          (type "VAULT_MANAGEMENT")
          (owner "VaultOperations")
          (configuration
            (locations "NEW_YORK" "LONDON" "SINGAPORE")
            (security-level "CLASS_5_VAULT")
            (access-controls "BIOMETRIC_DUAL_CUSTODY")
            (chain-of-custody "BLOCKCHAIN_LEDGER")))

        (resource
          (name "NomineeServicesSystem")
          (type "NOMINEE_MANAGEMENT")
          (owner "CustodyTech")
          (configuration
            (nominee-entities "CUSTODY_TECH_NOMINEES_LLC")
            (beneficial-ownership-tracking "REAL_TIME")
            (regulatory-reporting "AUTOMATED")
            (audit-trail "IMMUTABLE_LOG")))))

    ;; Security Movement Service Resources
    (service "SecurityMovement"
      (resources
        (resource
          (name "SecurityMovementEngine")
          (type "MOVEMENT_PROCESSOR")
          (owner "SettlementTech")
          (configuration
            (processing-capacity "10000_MOVEMENTS_PER_DAY")
            (stp-rate "99.5%")
            (settlement-networks "DTC" "EUROCLEAR" "CLEARSTREAM" "CREST")
            (exception-handling "AUTOMATED_RETRY_WITH_ESCALATION")))

        (resource
          (name "SSIManagementService")
          (type "SSI_ENGINE")
          (owner "SettlementTech")
          (configuration
            (ssi-database-size "UNLIMITED")
            (validation-rules "REAL_TIME_VERIFICATION")
            (change-approval-workflow "DUAL_APPROVAL")
            (audit-logging "COMPREHENSIVE")))))

    ;; Trade Capture Service Resources
    (service "TradeCapture"
      (resources
        (resource
          (name "TradeCaptureAndRoutingSystem")
          (type "TRADE_PROCESSOR")
          (owner "TradingTech")
          (configuration
            (throughput "50000_TRADES_PER_DAY")
            (latency "SUB_100MS")
            (connectivity "FIX_4.2" "FIX_4.4" "SWIFT" "API")
            (validation-engine "REAL_TIME_CHECKS")
            (error-handling "AUTOMATED_CORRECTION_WITH_ALERTS")))))

    ;; Reconciliation Service Resources
    (service "Reconciliation"
      (resources
        (resource
          (name "ReconciliationPlatform")
          (type "RECONCILIATION_ENGINE")
          (owner "ReconciliationTech")
          (configuration
            (matching-algorithms "FUZZY_LOGIC" "EXACT_MATCH" "THRESHOLD_MATCHING")
            (data-sources "INTERNAL_SYSTEMS" "EXTERNAL_FEEDS" "FILE_IMPORTS")
            (processing-schedule "CONTINUOUS" "BATCH_EOD")
            (exception-management "WORKFLOW_DRIVEN")
            (reporting "DASHBOARD_REAL_TIME" "SCHEDULED_REPORTS")))))

    ;; SSI Service Resources
    (service "SpecialSettlementInstructions"
      (resources
        (resource
          (name "SSIManagementService")
          (type "SSI_REPOSITORY")
          (owner "SettlementTech")
          (configuration
            (instruction-storage "HIERARCHICAL_BY_COUNTERPARTY")
            (maintenance-interface "WEB_PORTAL" "API" "BULK_UPLOAD")
            (approval-workflow "MAKER_CHECKER")
            (version-control "FULL_AUDIT_TRAIL")
            (integration "REAL_TIME_VALIDATION")))))

    ;; Custody Reporting Service Resources
    (service "CustodyReporting"
      (resources
        (resource
          (name "CustodyReportingEngine")
          (type "REPORTING_PLATFORM")
          (owner "ReportingTech")
          (configuration
            (report-generation "SCHEDULED_AND_ON_DEMAND")
            (data-sources "CUSTODY_MAIN_PLATFORM" "EXTERNAL_MARKET_DATA")
            (output-formats "PDF" "EXCEL" "XML" "JSON")
            (distribution "SECURE_PORTAL" "EMAIL" "SFTP")
            (customization "TEMPLATE_BASED_CUSTOMIZATION")))

        (resource
          (name "CustodyMainPlatform")
          (type "DATA_SOURCE")
          (owner "CustodyTech")
          (configuration
            (data-feeds "REAL_TIME_POSITIONS" "TRANSACTION_HISTORY" "CORPORATE_ACTIONS")
            (api-access "RESTful_API" "GraphQL")
            (data-quality "VALIDATED_AND_RECONCILED")
            (historical-retention "7_YEARS"))))))

;;; Result:
;;; - 8 implementation resources identified and mapped
;;; - Resource configurations specified
;;; - Infrastructure requirements documented
;;; - Resource provisioning ready to begin

;;; ----------------------------------------------------------------------------
;;; PHASE 5: ATTRIBUTES AND CONFIGURATION
;;; Define and populate custody-specific attributes for operational setup
;;; State: RESOURCES_DISCOVERED → ATTRIBUTES_POPULATED
;;; ----------------------------------------------------------------------------

(attributes.populate
  (custody-account-attributes
    (var (attr-id "custody.account.number") (source "generated") (format "CUST-{client-id}-{sequence}"))
    (var (attr-id "custody.account.type") (value "INSTITUTIONAL_SEGREGATED"))
    (var (attr-id "custody.account.base-currency") (value "USD"))
    (var (attr-id "custody.account.multicurrency-enabled") (value "true"))
    (var (attr-id "custody.account.overdraft-limit") (value "0"))
    (var (attr-id "custody.account.settlement-cycles") (value "T+0,T+1,T+2,T+3")))

  (safekeeping-attributes
    (var (attr-id "safekeeping.segregation-level") (value "FULL_CLIENT_SEGREGATION"))
    (var (attr-id "safekeeping.nominee-structure") (value "CUSTODY_TECH_NOMINEES_LLC"))
    (var (attr-id "safekeeping.physical-certificate-handling") (value "VAULT_STORAGE_WITH_DIGITIZATION"))
    (var (attr-id "safekeeping.insurance-coverage") (value "LLOYDS_OF_LONDON_500M_USD")))

  (settlement-attributes
    (var (attr-id "settlement.primary-markets") (value "US,UK,EU,APAC"))
    (var (attr-id "settlement.dvp-enabled") (value "true"))
    (var (attr-id "settlement.rvp-enabled") (value "true"))
    (var (attr-id "settlement.free-delivery-enabled") (value "true"))
    (var (attr-id "settlement.cut-off-times") (value "US:2PM_EST,EU:12PM_CET,APAC:11AM_JST")))

  (reporting-attributes
    (var (attr-id "reporting.daily-positions-enabled") (value "true"))
    (var (attr-id "reporting.transaction-reporting-enabled") (value "true"))
    (var (attr-id "reporting.performance-attribution-enabled") (value "true"))
    (var (attr-id "reporting.regulatory-reporting-enabled") (value "true"))
    (var (attr-id "reporting.custom-branding-enabled") (value "true"))
    (var (attr-id "reporting.delivery-methods") (value "SECURE_PORTAL,EMAIL,SFTP"))
    (var (attr-id "reporting.formats-supported") (value "PDF,EXCEL,CSV,XML,JSON")))

  (ssi-attributes
    (var (attr-id "ssi.standing-instructions-enabled") (value "true"))
    (var (attr-id "ssi.trade-specific-instructions-enabled") (value "true"))
    (var (attr-id "ssi.client-maintenance-portal-enabled") (value "true"))
    (var (attr-id "ssi.dual-approval-required") (value "true"))
    (var (attr-id "ssi.real-time-validation-enabled") (value "true")))

  (reconciliation-attributes
    (var (attr-id "reconciliation.daily-frequency-enabled") (value "true"))
    (var (attr-id "reconciliation.intraday-frequency-enabled") (value "true"))
    (var (attr-id "reconciliation.cash-tolerance") (value "0.00"))
    (var (attr-id "reconciliation.position-tolerance") (value "0.01"))
    (var (attr-id "reconciliation.automated-matching-enabled") (value "true"))
    (var (attr-id "reconciliation.exception-escalation-enabled") (value "true"))))

;;; Result:
;;; - 25+ custody-specific attributes populated
;;; - Operational parameters configured
;;; - Account structures defined
;;; - Service configurations established

;;; ----------------------------------------------------------------------------
;;; PHASE 6: FINAL BINDING AND ACTIVATION
;;; Bind all configuration values and activate custody services
;;; State: ATTRIBUTES_POPULATED → VALUES_BOUND → COMPLETED
;;; ----------------------------------------------------------------------------

(values.bind
  ;; Account Setup Bindings
  (bind (attr-id "custody.account.number") (value "CUST-GIP-001"))
  (bind (attr-id "custody.account.opened-date") (value "2024-03-01"))
  (bind (attr-id "custody.account.relationship-manager") (value "Sarah.Johnson@custodytech.com"))
  (bind (attr-id "custody.account.operations-contact") (value "operations.team@custodytech.com"))

  ;; Platform Integration Bindings
  (bind (attr-id "platform.main-custody.client-code") (value "GIP001"))
  (bind (attr-id "platform.trade-capture.connectivity.fix-session") (value "CUSTTECH_GIP001"))
  (bind (attr-id "platform.reconciliation.client-hierarchy") (value "GIP001.MASTER"))
  (bind (attr-id "platform.reporting.client-portal-url") (value "https://custody.custodytech.com/clients/GIP001"))

  ;; SSI Setup Bindings
  (bind (attr-id "ssi.default.usd.account") (value "12345678901"))
  (bind (attr-id "ssi.default.usd.bank") (value "JPMorgan Chase Bank N.A."))
  (bind (attr-id "ssi.default.usd.swift") (value "CHASUS33"))
  (bind (attr-id "ssi.default.eur.account") (value "98765432101"))
  (bind (attr-id "ssi.default.eur.bank") (value "Deutsche Bank AG"))
  (bind (attr-id "ssi.default.eur.swift") (value "DEUTDEFF"))

  ;; Regulatory and Compliance Bindings
  (bind (attr-id "compliance.lei-code") (value "549300ABCDEFGHIJK123"))
  (bind (attr-id "compliance.regulatory.primary") (value "SEC"))
  (bind (attr-id "compliance.regulatory.secondary") (value "FINRA,CFTC"))
  (bind (attr-id "compliance.tax-reporting.enabled") (value "true"))
  (bind (attr-id "compliance.fatca-reporting.enabled") (value "true")))

(workflow.complete
  (onboarding-id "CBU-CUSTODY-2024-001")
  (completion-date "2024-03-01")
  (services-activated "Safekeeping,SecurityMovement,TradeCapture,Reconciliation,SpecialSettlementInstructions,CustodyReporting")
  (go-live-date "2024-03-04")
  (relationship-manager "Sarah Johnson")
  (operations-contact "Custody Operations Team")
  (client-notification-sent "true")
  (documentation-complete "true")
  (training-completed "true")
  (connectivity-tested "true")
  (reconciliation-validated "true")
  (reporting-verified "true"))

;;; ============================================================================
;;; ONBOARDING COMPLETE
;;; ============================================================================
;;;
;;; Final Status: COMPLETED
;;; Services Activated: 6 core custody services
;;; Resources Provisioned: 8 platform resources
;;; Attributes Configured: 25+ operational parameters
;;; Go-Live Date: March 4, 2024
;;;
;;; The client Global Investment Partners LLC is now fully onboarded to the
;;; Custody product with comprehensive service coverage:
;;;
;;; ✅ Safekeeping - Multi-jurisdiction asset custody with full segregation
;;; ✅ Security Movement - Global settlement with multi-currency support
;;; ✅ Trade Capture - High-volume trade processing with real-time validation
;;; ✅ Reconciliation - Daily and intraday position matching with exception management
;;; ✅ Special Settlement Instructions - Comprehensive SSI management with client portal
;;; ✅ Custody Reporting - Full suite of standard and custom reports
;;;
;;; All implementation resources are provisioned and configured:
;;; • CustodyMainPlatform - Primary custody system
;;; • TradeCaptureAndRoutingSystem - Trade processing engine
;;; • SecurityMovementEngine - Settlement processing
;;; • ReconciliationPlatform - Position matching system
;;; • SSIManagementService - Settlement instructions repository
;;; • CustodyReportingEngine - Comprehensive reporting platform
;;; • PhysicalVaultSystem - Physical certificate storage
;;; • NomineeServicesSystem - Beneficial ownership management
;;;
;;; The onboarding workflow demonstrates the complete DSL-as-State pattern
;;; where each phase builds upon the previous accumulated DSL, creating a
;;; complete audit trail and executable configuration for the custody
;;; relationship.
;;; ============================================================================
