;; =============================================================================
;; SCENARIO 02: Multi-Manager Global Fund
;; Luxembourg fund with 3 investment managers covering different regions
;; Tests IM scoping, multi-currency SSIs, and complex booking rules
;; =============================================================================

;; --- Setup: Create the CBU ---
(cbu.ensure
  :name "Multi-Manager Global Fund"
  :jurisdiction "LU"
  :client-type "fund"
  :as @fund)

;; --- Create Fund Entity (SICAV) ---
(entity.ensure-limited-company
  :name "Multi-Manager Global SICAV"
  :jurisdiction "LU"
  :as @fund-entity)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @fund-entity
  :role "ASSET_OWNER")

;; --- Create Management Company ---
(entity.ensure-limited-company
  :name "Luxembourg Fund Management S.a r.l."
  :jurisdiction "LU"
  :as @manco)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @manco
  :role "MANAGEMENT_COMPANY")

;; --- Investment Manager 1: European Equities ---
(entity.ensure-limited-company
  :name "European Equity Partners GmbH"
  :jurisdiction "DE"
  :as @im-europe)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-europe
  :role "INVESTMENT_MANAGER")

;; --- Investment Manager 2: US Equities ---
(entity.ensure-limited-company
  :name "US Growth Capital LLC"
  :jurisdiction "US"
  :as @im-us)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-us
  :role "INVESTMENT_MANAGER")

;; --- Investment Manager 3: Fixed Income ---
(entity.ensure-limited-company
  :name "Global Fixed Income Partners Ltd"
  :jurisdiction "GB"
  :as @im-fi)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-fi
  :role "INVESTMENT_MANAGER")

;; --- Trading Universe: European Markets ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XETR"
  :currencies ["EUR"]
  :settlement-types ["DVP"]
  :as @universe-de)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XPAR"
  :currencies ["EUR"]
  :settlement-types ["DVP"]
  :as @universe-fr)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]
  :settlement-types ["DVP"]
  :as @universe-uk)

;; --- Trading Universe: US Markets ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"]
  :as @universe-us)

;; --- Trading Universe: Fixed Income ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "GOVT_BOND"
  :currencies ["EUR" "USD" "GBP"]
  :settlement-types ["DVP"]
  :as @universe-govtbond)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "CORP_BOND"
  :currencies ["EUR" "USD"]
  :settlement-types ["DVP"]
  :as @universe-corpbond)

;; --- SSIs: European Region ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "Europe EUR Primary"
  :type "SECURITIES"
  :safekeeping-account "SAFE-EU-001"
  :safekeeping-bic "DEUTDEFF"
  :cash-account "CASH-EU-001"
  :cash-bic "DEUTDEFF"
  :cash-currency "EUR"
  :pset-bic "CABOROEX"
  :effective-date "2025-01-01"
  :as @ssi-eu-eur)

(cbu-custody.activate-ssi :ssi-id @ssi-eu-eur)

;; --- SSIs: UK Region ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "UK GBP Primary"
  :type "SECURITIES"
  :safekeeping-account "SAFE-UK-001"
  :safekeeping-bic "LOYDGB2L"
  :cash-account "CASH-UK-001"
  :cash-bic "LOYDGB2L"
  :cash-currency "GBP"
  :pset-bic "CABOROEX"
  :effective-date "2025-01-01"
  :as @ssi-uk-gbp)

(cbu-custody.activate-ssi :ssi-id @ssi-uk-gbp)

(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "UK USD Cross"
  :type "SECURITIES"
  :safekeeping-account "SAFE-UK-002"
  :safekeeping-bic "LOYDGB2L"
  :cash-account "CASH-UK-002"
  :cash-bic "LOYDGB2L"
  :cash-currency "USD"
  :pset-bic "CABOROEX"
  :effective-date "2025-01-01"
  :as @ssi-uk-usd)

(cbu-custody.activate-ssi :ssi-id @ssi-uk-usd)

;; --- SSIs: US Region ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "US USD Primary"
  :type "SECURITIES"
  :safekeeping-account "SAFE-US-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "CASH-US-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2025-01-01"
  :as @ssi-us-usd)

(cbu-custody.activate-ssi :ssi-id @ssi-us-usd)

;; --- Booking Rules: European Equities ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-eu-eur
  :name "DE Equity EUR"
  :priority 10
  :instrument-class "EQUITY"
  :market "XETR"
  :currency "EUR"
  :settlement-type "DVP"
  :as @rule-de)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-eu-eur
  :name "FR Equity EUR"
  :priority 10
  :instrument-class "EQUITY"
  :market "XPAR"
  :currency "EUR"
  :settlement-type "DVP"
  :as @rule-fr)

;; --- Booking Rules: UK Equities ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk-gbp
  :name "UK Equity GBP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :settlement-type "DVP"
  :as @rule-uk-gbp)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk-usd
  :name "UK Equity USD"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-uk-usd)

;; --- Booking Rules: US Equities ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us-usd
  :name "US Equity USD"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP"
  :as @rule-us)

;; --- Booking Rules: Fixed Income ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-eu-eur
  :name "EUR Bond"
  :priority 20
  :instrument-class "GOVT_BOND"
  :currency "EUR"
  :as @rule-bond-eur)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us-usd
  :name "USD Bond"
  :priority 20
  :instrument-class "GOVT_BOND"
  :currency "USD"
  :as @rule-bond-usd)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk-gbp
  :name "GBP Bond"
  :priority 20
  :instrument-class "GOVT_BOND"
  :currency "GBP"
  :as @rule-bond-gbp)

;; --- Fallback Rules ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-eu-eur
  :name "EUR Fallback"
  :priority 100
  :currency "EUR"
  :as @rule-eur-fallback)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us-usd
  :name "USD Fallback"
  :priority 100
  :currency "USD"
  :as @rule-usd-fallback)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk-gbp
  :name "GBP Fallback"
  :priority 100
  :currency "GBP"
  :as @rule-gbp-fallback)

;; --- List Created Resources ---
(cbu-custody.list-universe :cbu-id @fund)
(cbu-custody.list-ssis :cbu-id @fund)
(cbu-custody.list-booking-rules :cbu-id @fund)

;; =============================================================================
;; EXPECTED RESULTS:
;; - 1 CBU with 5 entities (fund, manco, 3 IMs)
;; - 6 universe entries across markets and asset classes
;; - 4 SSIs (EU EUR, UK GBP, UK USD, US USD)
;; - 11 booking rules (5 specific + 3 bond + 3 fallback)
;; =============================================================================
