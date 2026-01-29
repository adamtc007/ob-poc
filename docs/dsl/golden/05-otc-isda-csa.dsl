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
;; Step 3: Create Trading Profile
;; ----------------------------------------------------------------------------

;; intent: Set up trading profile for derivatives
;; macro: mandate.setup
(trading-profile.create
  :cbu-id @fund
  :name "OTC Derivatives"
  :strategy "MACRO"
  :as @profile)

;; intent: Enable OTC instruments
(trading-profile.add-instrument
  :profile-id @profile
  :instrument "OTC_FX_FORWARD")

(trading-profile.add-instrument
  :profile-id @profile
  :instrument "OTC_IRS")

(trading-profile.add-instrument
  :profile-id @profile
  :instrument "OTC_CDS")

(trading-profile.add-instrument
  :profile-id @profile
  :instrument "OTC_EQUITY_SWAP")

;; ----------------------------------------------------------------------------
;; Step 4: Create ISDA Master Agreement
;; ----------------------------------------------------------------------------

;; intent: Create ISDA Master Agreement with counterparty
(isda.create
  :cbu-id @fund
  :counterparty-id @counterparty
  :version "2002"
  :governing-law "ENGLISH"
  :execution-date "2024-01-15"
  :as @isda)

;; intent: Add schedule terms
(isda.set-schedule
  :isda-id @isda
  :netting-agreement true
  :cross-default true
  :cross-default-threshold 10000000
  :currency "USD")

;; ----------------------------------------------------------------------------
;; Step 5: Create Credit Support Annex (CSA)
;; ----------------------------------------------------------------------------

;; intent: Create CSA for collateral terms
(csa.create
  :isda-id @isda
  :type "VM"
  :as @csa)

;; intent: Set collateral parameters for fund
(csa.set-party-terms
  :csa-id @csa
  :party-id @fund
  :threshold 0
  :minimum-transfer-amount 500000
  :rounding 10000
  :eligible-collateral ["CASH_USD" "CASH_EUR" "UST"])

;; intent: Set collateral parameters for counterparty
(csa.set-party-terms
  :csa-id @csa
  :party-id @counterparty
  :threshold 0
  :minimum-transfer-amount 500000
  :rounding 10000
  :eligible-collateral ["CASH_USD" "CASH_EUR" "UST" "GILT"])

;; ----------------------------------------------------------------------------
;; Step 6: Set Up Settlement Instructions
;; ----------------------------------------------------------------------------

;; intent: Create SSI for USD collateral
(ssi.create
  :cbu-id @fund
  :currency "USD"
  :purpose "COLLATERAL"
  :beneficiary-name "Global Macro Opportunities Fund"
  :beneficiary-account "001-234567"
  :beneficiary-bank-bic "CITIUS33"
  :correspondent-bank-bic "CHASUS33"
  :as @ssi_usd)

;; intent: Create SSI for EUR collateral
(ssi.create
  :cbu-id @fund
  :currency "EUR"
  :purpose "COLLATERAL"
  :beneficiary-name "Global Macro Opportunities Fund"
  :beneficiary-account "001-234568"
  :beneficiary-bank-bic "DEUTDEFF"
  :as @ssi_eur)

;; ----------------------------------------------------------------------------
;; Step 7: Link SSIs to CSA
;; ----------------------------------------------------------------------------

;; intent: Associate settlement instructions with CSA
(csa.add-ssi :csa-id @csa :ssi-id @ssi_usd)
(csa.add-ssi :csa-id @csa :ssi-id @ssi_eur)

;; ----------------------------------------------------------------------------
;; Step 8: Verify Setup
;; ----------------------------------------------------------------------------

;; intent: Get complete ISDA/CSA details
(isda.get :id @isda :include-csa true :include-ssis true)

;; intent: Check trading eligibility
(trading-profile.check-eligibility
  :profile-id @profile
  :counterparty-id @counterparty
  :instrument "OTC_IRS")
