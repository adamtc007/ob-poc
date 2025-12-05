;; =============================================================================
;; Custody SSI Onboarding - Full Integration Test
;;
;; Tests custody onboarding for a hedge fund with:
;; - 5 Markets: XNYS, XLON, XETR, XTKS, XHKG
;; - 3 Currencies: USD, GBP, EUR (plus local currencies JPY, HKD)
;; - Product: Custody
;; - Includes agent overrides (intermediary chains)
;; =============================================================================

;; Create commercial client entity (head office)
(entity.create-limited-company
  :name "Global Alpha Capital LLC"
  :jurisdiction "US"
  :company-number "DE-12345678"
  :as @head-office)

;; Create the CBU with Custody product
(cbu.ensure
  :name "Global Alpha Master Fund"
  :jurisdiction "KY"
  :client-type "FUND"
  :commercial-client-entity-id @head-office
  :as @cbu)

;; =============================================================================
;; LAYER 1: Trading Universe (5 markets, multiple currencies)
;; =============================================================================

;; US - NYSE (USD only)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

;; UK - London (GBP and USD cross-currency)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]
  :settlement-types ["DVP"])

;; Germany - Xetra (EUR and USD cross-currency)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XETR"
  :currencies ["EUR" "USD"]
  :settlement-types ["DVP"])

;; Japan - Tokyo (JPY and USD cross-currency)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XTKS"
  :currencies ["JPY" "USD"]
  :settlement-types ["DVP"])

;; Hong Kong (HKD and USD cross-currency)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XHKG"
  :currencies ["HKD" "USD"]
  :settlement-types ["DVP"])

;; =============================================================================
;; LAYER 2: Standing Settlement Instructions (one per market/currency)
;; =============================================================================

;; --- US Primary (USD) ---
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary"
  :type "SECURITIES"
  :safekeeping-account "GA-SAFE-US-001"
  :safekeeping-bic "BABOROCP"
  :safekeeping-name "Global Alpha US Safekeeping"
  :cash-account "GA-CASH-USD-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi-us)

;; --- UK GBP ---
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "UK GBP Primary"
  :type "SECURITIES"
  :safekeeping-account "GA-SAFE-UK-001"
  :safekeeping-bic "MIDLGB22"
  :safekeeping-name "Global Alpha UK Safekeeping"
  :cash-account "GA-CASH-GBP-001"
  :cash-bic "MIDLGB22"
  :cash-currency "GBP"
  :pset-bic "CABOROCP"
  :effective-date "2024-12-01"
  :as @ssi-uk-gbp)

;; --- Germany EUR ---
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "DE EUR Primary"
  :type "SECURITIES"
  :safekeeping-account "GA-SAFE-DE-001"
  :safekeeping-bic "COBADEFF"
  :safekeeping-name "Global Alpha DE Safekeeping"
  :cash-account "GA-CASH-EUR-001"
  :cash-bic "COBADEFF"
  :cash-currency "EUR"
  :pset-bic "DAKVDEFF"
  :effective-date "2024-12-01"
  :as @ssi-de-eur)

;; --- Japan JPY ---
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "JP JPY Primary"
  :type "SECURITIES"
  :safekeeping-account "GA-SAFE-JP-001"
  :safekeeping-bic "MABORJPJ"
  :safekeeping-name "Global Alpha JP Safekeeping"
  :cash-account "GA-CASH-JPY-001"
  :cash-bic "MABORJPJ"
  :cash-currency "JPY"
  :pset-bic "JAABORJP"
  :effective-date "2024-12-01"
  :as @ssi-jp-jpy)

;; --- Hong Kong HKD ---
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "HK HKD Primary"
  :type "SECURITIES"
  :safekeeping-account "GA-SAFE-HK-001"
  :safekeeping-bic "HABORHKH"
  :safekeeping-name "Global Alpha HK Safekeeping"
  :cash-account "GA-CASH-HKD-001"
  :cash-bic "HABORHKH"
  :cash-currency "HKD"
  :pset-bic "CCABORHK"
  :effective-date "2024-12-01"
  :as @ssi-hk-hkd)

;; Activate all SSIs
(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk-gbp)
(cbu-custody.activate-ssi :ssi-id @ssi-de-eur)
(cbu-custody.activate-ssi :ssi-id @ssi-jp-jpy)
(cbu-custody.activate-ssi :ssi-id @ssi-hk-hkd)

;; =============================================================================
;; Agent Overrides (intermediary chains for Japan)
;; =============================================================================

;; Japan requires intermediary agent chain
(cbu-custody.add-agent-override
  :ssi-id @ssi-jp-jpy
  :agent-role "INT1"
  :agent-bic "SABORJPJ"
  :agent-account "INT-JP-001"
  :agent-name "Sub-Custodian Japan"
  :sequence-order 1
  :reason "Local market requires Japanese sub-custodian")

;; =============================================================================
;; LAYER 3: Booking Rules (ALERT-style priority matching)
;; =============================================================================

;; --- Priority 10: Exact market + currency matches ---

;; US Equity USD
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "US Equity USD DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; UK Equity GBP
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-uk-gbp
  :name "UK Equity GBP DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; UK Equity USD (cross-currency settles via US)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "UK Equity USD DVP"
  :priority 11
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; DE Equity EUR
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-de-eur
  :name "DE Equity EUR DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XETR"
  :currency "EUR"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; DE Equity USD (cross-currency settles via US)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "DE Equity USD DVP"
  :priority 11
  :instrument-class "EQUITY"
  :market "XETR"
  :currency "USD"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; JP Equity JPY
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-jp-jpy
  :name "JP Equity JPY DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XTKS"
  :currency "JPY"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; JP Equity USD (cross-currency settles via US)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "JP Equity USD DVP"
  :priority 11
  :instrument-class "EQUITY"
  :market "XTKS"
  :currency "USD"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; HK Equity HKD
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-hk-hkd
  :name "HK Equity HKD DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XHKG"
  :currency "HKD"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; HK Equity USD (cross-currency settles via US)
(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "HK Equity USD DVP"
  :priority 11
  :instrument-class "EQUITY"
  :market "XHKG"
  :currency "USD"
  :settlement-type "DVP"
  :effective-date "2024-12-01")

;; --- Priority 50: Currency-based fallbacks ---

(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "USD Currency Fallback"
  :priority 50
  :currency "USD"
  :effective-date "2024-12-01")

(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-uk-gbp
  :name "GBP Currency Fallback"
  :priority 50
  :currency "GBP"
  :effective-date "2024-12-01")

(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-de-eur
  :name "EUR Currency Fallback"
  :priority 50
  :currency "EUR"
  :effective-date "2024-12-01")

;; --- Priority 100: Ultimate fallback ---

(cbu-custody.add-booking-rule
  :cbu-id @cbu
  :ssi-id @ssi-us
  :name "Ultimate Fallback"
  :priority 100
  :effective-date "2024-12-01")

;; =============================================================================
;; Validation
;; =============================================================================

;; Validate complete booking coverage
(cbu-custody.validate-booking-coverage :cbu-id @cbu)

;; Derive coverage requirements
(cbu-custody.derive-required-coverage :cbu-id @cbu)

;; =============================================================================
;; Test SSI Lookups (simulating trade routing)
;; =============================================================================

;; US trade - should match US Primary
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP")

;; UK GBP trade - should match UK GBP Primary
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP")

;; UK USD trade (cross-currency) - should match US Primary
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP")

;; Japan JPY trade - should match JP JPY Primary
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XTKS"
  :currency "JPY"
  :settlement-type "DVP")

;; Hong Kong HKD trade - should match HK HKD Primary
(cbu-custody.lookup-ssi
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XHKG"
  :currency "HKD"
  :settlement-type "DVP")
