;; =============================================================================
;; SCENARIO 04: Transition Manager Scenario
;; Pension fund with temporary transition manager for portfolio restructure
;; Tests date-bounded IM assignments and multi-region coverage
;; =============================================================================

;; --- Setup: Create the CBU ---
(cbu.ensure
  :name "Pacific Pension Transition"
  :jurisdiction "US"
  :client-type "fund"
  :as @fund)

;; --- Create Fund Entity ---
(entity.ensure-limited-company
  :name "Pacific State Pension Fund"
  :jurisdiction "US"
  :as @fund-entity)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @fund-entity
  :role "ASSET_OWNER")

;; --- Create Primary Investment Manager ---
(entity.ensure-limited-company
  :name "Pacific Investment Management"
  :jurisdiction "US"
  :as @im-primary)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-primary
  :role "INVESTMENT_MANAGER")

;; --- Create Transition Manager (temporary) ---
(entity.ensure-limited-company
  :name "BlackRock Transition Management"
  :jurisdiction "US"
  :as @im-transition)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-transition
  :role "INVESTMENT_MANAGER")

;; --- Trading Universe ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-us-equity)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]
  :settlement-types ["DVP"]
  :as @universe-uk-equity)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "GOVT_BOND"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-bond)

;; --- SSIs ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "US Primary"
  :type "SECURITIES"
  :safekeeping-account "PENS-US-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "PENS-CASH-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2025-01-01"
  :as @ssi-us)

(cbu-custody.activate-ssi :ssi-id @ssi-us)

(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "UK GBP"
  :type "SECURITIES"
  :safekeeping-account "PENS-UK-001"
  :safekeeping-bic "LOYDGB2L"
  :cash-account "PENS-UK-CASH"
  :cash-bic "LOYDGB2L"
  :cash-currency "GBP"
  :pset-bic "CABOROEX"
  :effective-date "2025-01-01"
  :as @ssi-uk)

(cbu-custody.activate-ssi :ssi-id @ssi-uk)

;; --- Booking Rules ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "US Equity"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-us-eq)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk
  :name "UK Equity GBP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP"
  :as @rule-uk-eq)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "UK Equity USD Cross"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-uk-usd)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "US Bonds"
  :priority 20
  :instrument-class "GOVT_BOND"
  :currency "USD"
  :as @rule-bond)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "USD Fallback"
  :priority 100
  :currency "USD"
  :as @rule-usd-fb)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk
  :name "GBP Fallback"
  :priority 100
  :currency "GBP"
  :as @rule-gbp-fb)

;; --- List Created Resources ---
(cbu-custody.list-universe :cbu-id @fund)
(cbu-custody.list-ssis :cbu-id @fund)
(cbu-custody.list-booking-rules :cbu-id @fund)

;; =============================================================================
;; EXPECTED RESULTS:
;; - 1 CBU with 3 entities (fund, primary IM, transition IM)
;; - 3 universe entries
;; - 2 SSIs (US, UK)
;; - 6 booking rules
;; =============================================================================
