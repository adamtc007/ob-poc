;; =============================================================================
;; SCENARIO 01: Simple Single-IM Equity Fund
;; A straightforward US equity fund with one investment manager
;; Tests basic trading profile setup with custody SSIs
;; =============================================================================

;; --- Setup: Create the CBU ---
(cbu.ensure
  :name "Simple US Equity Fund"
  :jurisdiction "US"
  :client-type "fund"
  :as @fund)

;; --- Create Fund Entity ---
(entity.ensure-limited-company
  :name "Simple US Equity Fund LLC"
  :jurisdiction "US"
  :as @fund-entity)

;; --- Assign Fund Entity as Asset Owner ---
(cbu.assign-role
  :cbu-id @fund
  :entity-id @fund-entity
  :role "ASSET_OWNER")

;; --- Create Investment Manager Entity ---
(entity.ensure-limited-company
  :name "Vanguard Asset Management Inc"
  :jurisdiction "US"
  :as @im-entity)

;; --- Assign IM Role ---
(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-entity
  :role "INVESTMENT_MANAGER")

;; --- Define Trading Universe: US Equities Only ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-nyse)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNAS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-nasdaq)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "ETF"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-etf)

;; --- Create Standing Settlement Instructions ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "US Equity Primary SSI"
  :type "SECURITIES"
  :safekeeping-account "SAFE-US-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "CASH-US-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2025-01-01"
  :as @ssi-us)

(cbu-custody.activate-ssi :ssi-id @ssi-us)

;; --- Booking Rules ---
;; Primary rule: US Equities DVP
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "US Equity DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-nyse)

;; NASDAQ equities
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "NASDAQ DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNAS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-nasdaq)

;; ETF rule
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "ETF DVP"
  :priority 10
  :instrument-class "ETF"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-etf)

;; Fallback rule for any USD
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "USD Fallback"
  :priority 100
  :currency "USD"
  :as @rule-fallback)

;; --- Verify Configuration ---
(cbu-custody.list-universe :cbu-id @fund)
(cbu-custody.list-ssis :cbu-id @fund)
(cbu-custody.list-booking-rules :cbu-id @fund)

;; --- Test SSI Lookup ---
;; NOTE: lookup-ssi and validate-booking-coverage are query operations
;; that should be run interactively after the setup is complete.
;; They are commented out here because DAG execution may reorder them
;; before the booking rules are created.
;;
;; To test manually after running this scenario:
;; (cbu-custody.lookup-ssi :cbu-id @fund :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
;; (cbu-custody.validate-booking-coverage :cbu-id @fund)

;; =============================================================================
;; EXPECTED RESULTS:
;; - 1 CBU created
;; - 2 entities (fund + IM) with roles assigned
;; - 3 universe entries (NYSE equity, NASDAQ equity, NYSE ETF)
;; - 1 SSI (active)
;; - 4 booking rules (3 specific + 1 fallback)
;; - SSI lookup returns @ssi-us for NYSE equity trade
;; - Coverage validation passes
;; =============================================================================
