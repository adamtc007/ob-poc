;; ============================================================================
;; OTC ISDA/CSA Onboarding
;; ============================================================================
;; intent: Set up OTC derivatives trading with ISDA Master Agreement and CSA
;;
;; OTC (Over-The-Counter) derivatives require legal documentation between
;; counterparties. This example shows onboarding a fund with:
;; - ISDA Master Agreement (legal framework)
;; - Credit Support Annex (CSA) for collateral terms
;; - Trading eligibility setup
;;
;; ARCHITECTURE NOTE: Trading profiles are the source of truth for trading config.
;; ISDA/CSA are configured via trading-profile authoring verbs, then materialized
;; to operational tables (custody.isda_agreements, custody.csa_agreements).

;; ----------------------------------------------------------------------------
;; Step 1: Create the Fund
;; ----------------------------------------------------------------------------

;; intent: Create fund that will trade OTC derivatives
;; macro: structure.setup
(cbu.create
  :name "Global Macro Opportunities Fund"
  :type "FUND"
  :jurisdiction "IE"
  :legal-form "ICAV"
  :as @fund)

;; ----------------------------------------------------------------------------
;; Step 2: Create the Counterparty
;; ----------------------------------------------------------------------------

;; intent: Create the OTC counterparty (investment bank)
;; macro: party.create
(entity.create
  :name "Goldman Sachs International"
  :type "LEGAL"
  :jurisdiction "GB"
  :lei "W22LROWP2IHZNBB6K528"
  :as @counterparty)

;; ----------------------------------------------------------------------------
;; Step 3: Create Trading Profile (Document-Based Approach)
;; ----------------------------------------------------------------------------
;; Trading profiles are JSONB documents that hold the full trading configuration.
;; They go through a lifecycle: DRAFT -> VALIDATED -> PENDING_REVIEW -> ACTIVE
;; The `materialize` verb projects the document to operational tables.

;; intent: Create a draft trading profile for the fund
(trading.profile.create-draft
  :cbu-id @fund
  :notes "OTC Derivatives trading setup"
  :as @profile)

;; intent: Set base currency
(trading.profile.set-base-currency
  :profile-id @profile
  :currency "EUR")

;; intent: Add OTC instrument classes to the universe
(trading.profile.add-instrument-class
  :profile-id @profile
  :class-code "FX_FORWARD"
  :isda-asset-classes ["FX"])

(trading.profile.add-instrument-class
  :profile-id @profile
  :class-code "IRS"
  :isda-asset-classes ["RATES"])

(trading.profile.add-instrument-class
  :profile-id @profile
  :class-code "CDS"
  :isda-asset-classes ["CREDIT"])

(trading.profile.add-instrument-class
  :profile-id @profile
  :class-code "EQUITY_SWAP"
  :isda-asset-classes ["EQUITY"])

;; ----------------------------------------------------------------------------
;; Step 4: Add ISDA Master Agreement to Profile
;; ----------------------------------------------------------------------------
;; ISDA agreements are configured in the trading profile document.
;; The `add-isda-config` verb adds the counterparty relationship.

;; intent: Add ISDA configuration for Goldman Sachs
(trading.profile.add-isda-config
  :profile-id @profile
  :counterparty-entity-id @counterparty
  :counterparty-name "Goldman Sachs International"
  :counterparty-lei "W22LROWP2IHZNBB6K528"
  :governing-law "ENGLISH"
  :agreement-date "2024-01-15")

;; intent: Add product coverage to the ISDA (what can be traded under this ISDA)
(trading.profile.add-isda-coverage
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :asset-class "FX"
  :base-products ["FX_FORWARD" "FX_OPTION"])

(trading.profile.add-isda-coverage
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :asset-class "RATES"
  :base-products ["IRS" "XCCY_SWAP"])

(trading.profile.add-isda-coverage
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :asset-class "CREDIT"
  :base-products ["CDS"])

(trading.profile.add-isda-coverage
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :asset-class "EQUITY"
  :base-products ["EQUITY_SWAP" "VARIANCE_SWAP"])

;; ----------------------------------------------------------------------------
;; Step 5: Add Credit Support Annex (CSA) to ISDA
;; ----------------------------------------------------------------------------
;; CSA defines collateral terms for variation margin (VM) and/or initial margin (IM).

;; intent: Add VM CSA to the Goldman ISDA
(trading.profile.add-csa-config
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :csa-type "VM"
  :threshold-currency "USD"
  :threshold-amount 0
  :minimum-transfer-amount 500000)

;; intent: Add eligible collateral types
(trading.profile.add-csa-collateral
  :profile-id @profile
  :counterparty-ref "Goldman Sachs International"
  :collateral-type "CASH"
  :currencies ["USD" "EUR" "GBP"]
  :haircut-pct 0)

(trading.profile.add-csa-collateral
  :profile-id @profile
  :counterparty-ref "Goldman Sachs International"
  :collateral-type "GOVT_BOND"
  :issuers ["US" "DE" "GB"]
  :min-rating "A-"
  :haircut-pct 2.0)

;; ----------------------------------------------------------------------------
;; Step 6: Set Up Standing Settlement Instructions (SSIs)
;; ----------------------------------------------------------------------------
;; SSIs define where collateral and settlements are routed.
;; OTC_COLLATERAL type SSIs are specifically for margin transfers.

;; intent: Add USD collateral SSI
(trading.profile.add-standing-instruction
  :profile-id @profile
  :ssi-type "OTC_COLLATERAL"
  :ssi-name "USD-COLLATERAL"
  :cash-account "001-234567"
  :cash-bic "CITIUS33"
  :cash-currency "USD")

;; intent: Add EUR collateral SSI
(trading.profile.add-standing-instruction
  :profile-id @profile
  :ssi-type "OTC_COLLATERAL"
  :ssi-name "EUR-COLLATERAL"
  :cash-account "001-234568"
  :cash-bic "DEUTDEFF"
  :cash-currency "EUR")

;; ----------------------------------------------------------------------------
;; Step 7: Link SSIs to CSA
;; ----------------------------------------------------------------------------

;; intent: Associate USD SSI with Goldman CSA for margin transfers
(trading.profile.link-csa-ssi
  :profile-id @profile
  :counterparty-ref "Goldman Sachs International"
  :ssi-name "USD-COLLATERAL")

;; ----------------------------------------------------------------------------
;; Step 8: Validate and Activate
;; ----------------------------------------------------------------------------
;; Profile goes through validation before activation.

;; intent: Validate the profile is complete and ready
(trading-profile.validate-go-live-ready
  :profile-id @profile
  :strictness "STANDARD")

;; intent: Submit for approval (transitions DRAFT -> PENDING_REVIEW after validation)
;; NOTE: In a real workflow, ops team would validate first, then submit
(trading-profile.submit
  :profile-id @profile
  :submitted-by "onboarding-system"
  :notes "OTC derivatives setup complete")

;; intent: Approve and activate (transitions PENDING_REVIEW -> ACTIVE)
;; NOTE: In production, this would be a separate approval step
(trading-profile.approve
  :profile-id @profile
  :approved-by "client-approver"
  :notes "Approved for go-live")

;; intent: Materialize to operational tables (projects document to custody.* tables)
(trading-profile.materialize
  :profile-id @profile
  :sections ["isda" "universe"])

;; ----------------------------------------------------------------------------
;; Step 9: Verify Setup
;; ----------------------------------------------------------------------------

;; intent: List ISDAs for this CBU
(isda.list
  :cbu-id @fund)

;; intent: Get the active trading profile to verify configuration
(trading-profile.get-active
  :cbu-id @fund)
