;; =============================================================================
;; SCENARIO 06: Full OTC Derivatives Setup (Phase 3.5)
;; Comprehensive OTC onboarding with ISDA, CSA, collateral accounts, and
;; trade confirmation configuration
;; =============================================================================

;; --- Setup: Create the CBU ---
(cbu.ensure
  :name "Quantum Macro Fund"
  :jurisdiction "US"
  :client-type "fund"
  :as @fund)

;; --- Create Fund Entity ---
(entity.ensure-limited-company
  :name "Quantum Macro Master Fund LP"
  :jurisdiction "DE"
  :as @fund-entity)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @fund-entity
  :role "ASSET_OWNER")

;; =============================================================================
;; COUNTERPARTY SETUP
;; =============================================================================

;; --- Goldman Sachs ---
(counterparty.ensure
  :name "Goldman Sachs"
  :lei "784F5XWPLTWKTBV3E584"
  :counterparty-type "BANK"
  :jurisdiction "US"
  :as @cp-gs)

(counterparty.add-bic
  :counterparty-id @cp-gs
  :bic "GABORUSMXXX"
  :type "SWIFT"
  :is-primary true)

;; --- JP Morgan ---
(counterparty.ensure
  :name "JP Morgan"
  :lei "8I5DZWZKVSZI1NUHU748"
  :counterparty-type "BANK"
  :jurisdiction "US"
  :as @cp-jpm)

(counterparty.add-bic
  :counterparty-id @cp-jpm
  :bic "CHASUS33XXX"
  :type "SWIFT"
  :is-primary true)

;; --- Morgan Stanley ---
(counterparty.ensure
  :name "Morgan Stanley"
  :lei "IGJSJL3JD5P30I6NJZ34"
  :counterparty-type "BANK"
  :jurisdiction "US"
  :as @cp-ms)

(counterparty.add-bic
  :counterparty-id @cp-ms
  :bic "MSTCUS44XXX"
  :type "SWIFT"
  :is-primary true)

;; =============================================================================
;; ISDA MASTER AGREEMENTS
;; =============================================================================

;; --- Goldman Sachs ISDA ---
(isda.establish
  :cbu-id @fund
  :counterparty-id @cp-gs
  :version "2002"
  :governing-law "NY"
  :effective-date "2025-01-15"
  :as @isda-gs)

;; Add product coverage
(isda.add-product-scope
  :isda-id @isda-gs
  :product-type "IRS")

(isda.add-product-scope
  :isda-id @isda-gs
  :product-type "XCCY")

(isda.add-product-scope
  :isda-id @isda-gs
  :product-type "FX_FORWARD")

;; --- JP Morgan ISDA ---
(isda.establish
  :cbu-id @fund
  :counterparty-id @cp-jpm
  :version "2002"
  :governing-law "NY"
  :effective-date "2025-01-15"
  :as @isda-jpm)

(isda.add-product-scope
  :isda-id @isda-jpm
  :product-type "IRS")

(isda.add-product-scope
  :isda-id @isda-jpm
  :product-type "CDS")

;; --- Morgan Stanley ISDA (English Law) ---
(isda.establish
  :cbu-id @fund
  :counterparty-id @cp-ms
  :version "2002"
  :governing-law "ENGLISH"
  :effective-date "2025-01-15"
  :as @isda-ms)

(isda.add-product-scope
  :isda-id @isda-ms
  :product-type "IRS")

(isda.add-product-scope
  :isda-id @isda-ms
  :product-type "SWAPTION")

;; =============================================================================
;; CREDIT SUPPORT ANNEXES (CSAs)
;; =============================================================================

;; --- Goldman Sachs VM CSA ---
(csa.establish
  :isda-id @isda-gs
  :csa-type "VM"
  :our-threshold 0
  :their-threshold 0
  :mta 500000
  :rounding 10000
  :threshold-ccy "USD"
  :interest-benchmark "SOFR"
  :effective-date "2025-01-15"
  :as @csa-gs-vm)

;; Eligible collateral for Goldman VM CSA
(csa.add-eligible-collateral
  :csa-id @csa-gs-vm
  :asset-class "CASH"
  :currency "USD"
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-gs-vm
  :asset-class "GOVT_BOND"
  :currency "USD"
  :haircut-pct 2.0)

;; --- JP Morgan VM CSA ---
(csa.establish
  :isda-id @isda-jpm
  :csa-type "VM"
  :our-threshold 0
  :their-threshold 0
  :mta 500000
  :rounding 10000
  :threshold-ccy "USD"
  :interest-benchmark "SOFR"
  :effective-date "2025-01-15"
  :as @csa-jpm-vm)

(csa.add-eligible-collateral
  :csa-id @csa-jpm-vm
  :asset-class "CASH"
  :currency "USD"
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-jpm-vm
  :asset-class "CASH"
  :currency "EUR"
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-jpm-vm
  :asset-class "GOVT_BOND"
  :currency "USD"
  :haircut-pct 2.0)

;; --- Morgan Stanley IM CSA (UMR compliant with third-party custody) ---
(csa.establish
  :isda-id @isda-ms
  :csa-type "IM"
  :threshold-ccy "USD"
  :interest-benchmark "SOFR"
  :segregation-required true
  :third-party-custodian "State Street"
  :effective-date "2025-01-15"
  :as @csa-ms-im)

;; IM-eligible collateral (more restrictive)
(csa.add-eligible-collateral
  :csa-id @csa-ms-im
  :asset-class "CASH"
  :currency "USD"
  :haircut-pct 0)

(csa.add-eligible-collateral
  :csa-id @csa-ms-im
  :asset-class "GOVT_BOND"
  :currency "USD"
  :haircut-pct 1.0)

(csa.add-eligible-collateral
  :csa-id @csa-ms-im
  :asset-class "AGENCY"
  :currency "USD"
  :haircut-pct 4.0)

;; =============================================================================
;; COLLATERAL ACCOUNTS
;; =============================================================================

;; --- Goldman VM Collateral Accounts ---
(collateral.ensure-account
  :cbu-id @fund
  :counterparty-id @cp-gs
  :csa-id @csa-gs-vm
  :account-type "VM_POSTED"
  :currency "USD"
  :account-number "COLL-GS-POST-001"
  :as @coll-gs-posted)

(collateral.ensure-account
  :cbu-id @fund
  :counterparty-id @cp-gs
  :csa-id @csa-gs-vm
  :account-type "VM_RECEIVED"
  :currency "USD"
  :account-number "COLL-GS-RECV-001"
  :as @coll-gs-received)

;; --- JP Morgan VM Collateral Accounts ---
(collateral.ensure-account
  :cbu-id @fund
  :counterparty-id @cp-jpm
  :csa-id @csa-jpm-vm
  :account-type "VM_POSTED"
  :currency "USD"
  :account-number "COLL-JPM-POST-001"
  :as @coll-jpm-posted)

(collateral.ensure-account
  :cbu-id @fund
  :counterparty-id @cp-jpm
  :csa-id @csa-jpm-vm
  :account-type "VM_RECEIVED"
  :currency "USD"
  :account-number "COLL-JPM-RECV-001"
  :as @coll-jpm-received)

;; --- Morgan Stanley IM Collateral (Third-party segregated) ---
(collateral.ensure-account
  :cbu-id @fund
  :counterparty-id @cp-ms
  :csa-id @csa-ms-im
  :account-type "POSTED_IM"
  :currency "USD"
  :account-number "COLL-MS-IM-001"
  :custodian "State Street"
  :is-third-party true
  :as @coll-ms-im-posted)

(collateral.ensure-account
  :cbu-id @fund
  :counterparty-id @cp-ms
  :csa-id @csa-ms-im
  :account-type "RECEIVED_IM"
  :currency "USD"
  :account-number "COLL-MS-IM-RECV-001"
  :custodian "State Street"
  :is-third-party true
  :as @coll-ms-im-received)

;; =============================================================================
;; TRADE CONFIRMATION CONFIGURATION
;; =============================================================================

;; --- Goldman: DTCC for rates, MarkitWire backup ---
(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-gs
  :method "DTCC_GTR"
  :product-type "IRS"
  :is-primary true)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-gs
  :method "DTCC_GTR"
  :product-type "XCCY"
  :is-primary true)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-gs
  :method "MARKITWIRE"
  :product-type "IRS"
  :is-primary false)

;; --- JP Morgan: DTCC for rates and credit ---
(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-jpm
  :method "DTCC_GTR"
  :product-type "IRS"
  :is-primary true)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-jpm
  :method "DTCC_GTR"
  :product-type "CDS"
  :is-primary true)

;; --- Morgan Stanley: MarkitWire for rates ---
(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-ms
  :method "MARKITWIRE"
  :product-type "IRS"
  :is-primary true)

(confirmation.configure
  :cbu-id @fund
  :counterparty-id @cp-ms
  :method "MARKITWIRE"
  :product-type "SWAPTION"
  :is-primary true)

;; =============================================================================
;; QUERY: List all OTC setup for verification
;; =============================================================================

(counterparty.list :cbu-id @fund)
(isda.list :cbu-id @fund)
(csa.list :cbu-id @fund)
(collateral.query-accounts :cbu-id @fund)
(confirmation.query :cbu-id @fund)

;; =============================================================================
;; EXPECTED RESULTS:
;; - 1 CBU with fund entity
;; - 3 counterparties (GS, JPM, MS) with LEIs and BICs
;; - 3 ISDA master agreements (2x NY law, 1x English law)
;; - 3 CSAs (2x VM, 1x IM with third-party custody)
;; - 6 collateral accounts
;; - 8 confirmation configurations
;; =============================================================================
