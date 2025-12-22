;; =============================================================================
;; SCENARIO 03: Hedge Fund with OTC Derivatives
;; Cayman hedge fund trading equities plus IRS/CDS requiring ISDA/CSA
;; Tests OTC instrument classes and counterparty-specific SSIs
;; =============================================================================

;; --- Setup: Create the CBU ---
(cbu.ensure
  :name "Phoenix Macro Hedge Fund"
  :jurisdiction "KY"
  :client-type "fund"
  :as @fund)

;; --- Create Fund Entity ---
(entity.ensure-limited-company
  :name "Phoenix Macro Master Fund Ltd"
  :jurisdiction "KY"
  :as @fund-entity)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @fund-entity
  :role "ASSET_OWNER")

;; --- Create Investment Manager ---
(entity.ensure-limited-company
  :name "Phoenix Capital Management LLC"
  :jurisdiction "US"
  :as @im)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im
  :role "INVESTMENT_MANAGER")

;; --- Create Counterparties for OTC ---
(entity.ensure-limited-company
  :name "Goldman Sachs International"
  :jurisdiction "GB"
  :as @gs)

(entity.ensure-limited-company
  :name "Morgan Stanley & Co"
  :jurisdiction "US"
  :as @ms)

;; --- Trading Universe: Equities ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-equity)

;; --- Trading Universe: OTC Derivatives ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "IRS"
  :currencies ["USD" "EUR"]
  :settlement-types ["DVP"]
  :as @universe-irs)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "CDS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-cds)

;; --- SSIs: Equity ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "US Equity Primary"
  :type "SECURITIES"
  :safekeeping-account "SAFE-US-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "CASH-US-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2025-01-01"
  :as @ssi-equity)

(cbu-custody.activate-ssi :ssi-id @ssi-equity)

;; --- SSIs: OTC Collateral ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "OTC Collateral USD"
  :type "COLLATERAL"
  :safekeeping-account "COLL-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "COLL-CASH-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :effective-date "2025-01-01"
  :as @ssi-collateral)

(cbu-custody.activate-ssi :ssi-id @ssi-collateral)

;; --- Booking Rules ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-equity
  :name "US Equity DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-equity)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-collateral
  :name "IRS USD"
  :priority 10
  :instrument-class "IRS"
  :currency "USD"
  :as @rule-irs-usd)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-collateral
  :name "CDS USD"
  :priority 10
  :instrument-class "CDS"
  :currency "USD"
  :as @rule-cds)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-collateral
  :name "USD Fallback"
  :priority 100
  :currency "USD"
  :as @rule-fallback)

;; --- List Created Resources ---
(cbu-custody.list-universe :cbu-id @fund)
(cbu-custody.list-ssis :cbu-id @fund)
(cbu-custody.list-booking-rules :cbu-id @fund)

;; =============================================================================
;; EXPECTED RESULTS:
;; - 1 CBU with 4 entities (fund, IM, 2 counterparties)
;; - 3 universe entries (equity, IRS, CDS)
;; - 2 SSIs (equity, collateral)
;; - 4 booking rules
;; =============================================================================
