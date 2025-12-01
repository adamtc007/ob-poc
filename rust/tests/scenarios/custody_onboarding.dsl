;; =============================================================================
;; Custody Onboarding Integration Test
;;
;; Tests the three-layer custody model:
;; - Layer 1: Universe (what the CBU trades)
;; - Layer 2: SSI Data (account information)
;; - Layer 3: Booking Rules (ALERT-style routing)
;; =============================================================================

;; Create a test CBU for custody onboarding
(cbu.ensure
  :name "Acme Pension Fund"
  :jurisdiction "US"
  :client-type "FUND"
  :as @cbu)

;; =============================================================================
;; LAYER 1: Define what they trade (Universe)
;; =============================================================================

;; US Equities
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

;; UK Equities - settle in both GBP and USD
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]
  :settlement-types ["DVP"])

;; Government Bonds
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "GOVT_BOND"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

;; =============================================================================
;; LAYER 2: Create SSI Data (Pure account info)
;; =============================================================================

;; Primary US Safekeeping
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary Safekeeping"
  :type "SECURITIES"
  :safekeeping-account "ACME-SAFE-001"
  :safekeeping-bic "BABOROCP"
  :safekeeping-name "Acme Pension Safekeeping"
  :cash-account "ACME-USD-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi-us)

;; UK GBP Safekeeping
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "UK GBP Safekeeping"
  :type "SECURITIES"
  :safekeeping-account "ACME-UK-001"
  :safekeeping-bic "MIDLGB22"
  :cash-account "ACME-GBP-001"
  :cash-bic "MIDLGB22"
  :cash-currency "GBP"
  :pset-bic "CABOROCP"
  :effective-date "2024-12-01"
  :as @ssi-uk)

;; Activate SSIs
(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk)

;; =============================================================================
;; LAYER 3: Define Booking Rules (ALERT-style routing)
;; =============================================================================

;; Specific rules (priority 10)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "US Equity DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-us-eq)

(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-uk
  :name "UK Equity GBP DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP"
  :as @rule-uk-gbp)

;; Cross-currency: UK equities settling in USD
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "UK Equity USD Settlement"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-uk-usd)

;; Government bonds
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "US Govt Bond DVP"
  :priority 10
  :instrument-class "GOVT_BOND"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-govt)

;; Fallback rule (priority 50)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "USD Fallback"
  :priority 50
  :currency "USD"
  :as @rule-fallback)

;; Ultimate fallback (priority 100)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "Ultimate Fallback"
  :priority 100
  :as @rule-ultimate)

;; =============================================================================
;; Validation (plugin ops)
;; =============================================================================

;; Validate that booking rules cover all universe combinations
(cbu-custody.validate-booking-coverage :cbu-id @cbu)

;; Derive what coverage is required based on universe
(cbu-custody.derive-required-coverage :cbu-id @cbu)

;; Lookup SSIs for specific trade characteristics
(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XLON" :currency "USD" :settlement-type "DVP")
