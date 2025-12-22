;; =============================================================================
;; SCENARIO 05: Sub-Advised UCITS Fund
;; Irish UCITS with ManCo delegating to multiple sub-advisors
;; Tests delegation chains and global market coverage
;; =============================================================================

;; --- Setup: Create the CBU ---
(cbu.ensure
  :name "Emerald Growth UCITS"
  :jurisdiction "IE"
  :client-type "fund"
  :as @fund)

;; --- Create Fund Entity ---
(entity.ensure-limited-company
  :name "Emerald Growth ICAV"
  :jurisdiction "IE"
  :as @fund-entity)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @fund-entity
  :role "ASSET_OWNER")

;; --- Create Management Company ---
(entity.ensure-limited-company
  :name "Dublin Fund Services Ltd"
  :jurisdiction "IE"
  :as @manco)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @manco
  :role "MANAGEMENT_COMPANY")

;; --- Sub-Advisors (delegated IM role) ---
(entity.ensure-limited-company
  :name "Tokyo Asset Management"
  :jurisdiction "JP"
  :as @im-japan)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-japan
  :role "INVESTMENT_MANAGER")

(entity.ensure-limited-company
  :name "Singapore Growth Partners"
  :jurisdiction "SG"
  :as @im-apac)

(cbu.assign-role
  :cbu-id @fund
  :entity-id @im-apac
  :role "INVESTMENT_MANAGER")

;; --- Trading Universe: Asia Pacific ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XTKS"
  :currencies ["JPY"]
  :settlement-types ["DVP"]
  :as @universe-jp)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XSES"
  :currencies ["SGD" "USD"]
  :settlement-types ["DVP"]
  :as @universe-sg)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XHKG"
  :currencies ["HKD" "USD"]
  :settlement-types ["DVP"]
  :as @universe-hk)

;; --- Trading Universe: Europe ---
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP"]
  :settlement-types ["DVP"]
  :as @universe-uk)

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XETR"
  :currencies ["EUR"]
  :settlement-types ["DVP"]
  :as @universe-de)

;; --- SSIs: Japan ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "Japan JPY"
  :type "SECURITIES"
  :safekeeping-account "SAFE-JP-001"
  :safekeeping-bic "MABOROJP"
  :cash-account "CASH-JP-001"
  :cash-bic "MABOROJP"
  :cash-currency "JPY"
  :pset-bic "JSDCOROJ"
  :effective-date "2025-01-01"
  :as @ssi-jp)

(cbu-custody.activate-ssi :ssi-id @ssi-jp)

;; --- SSIs: Singapore ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "Singapore SGD"
  :type "SECURITIES"
  :safekeeping-account "SAFE-SG-001"
  :safekeeping-bic "DBSSSGSG"
  :cash-account "CASH-SG-001"
  :cash-bic "DBSSSGSG"
  :cash-currency "SGD"
  :pset-bic "CDPXSGSG"
  :effective-date "2025-01-01"
  :as @ssi-sg)

(cbu-custody.activate-ssi :ssi-id @ssi-sg)

;; --- SSIs: Hong Kong ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "Hong Kong HKD"
  :type "SECURITIES"
  :safekeeping-account "SAFE-HK-001"
  :safekeeping-bic "HABOROHK"
  :cash-account "CASH-HK-001"
  :cash-bic "HABOROHK"
  :cash-currency "HKD"
  :pset-bic "CCASHKHH"
  :effective-date "2025-01-01"
  :as @ssi-hk)

(cbu-custody.activate-ssi :ssi-id @ssi-hk)

;; --- SSIs: Europe ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "Europe EUR"
  :type "SECURITIES"
  :safekeeping-account "SAFE-EU-001"
  :safekeeping-bic "DEUTDEFF"
  :cash-account "CASH-EU-001"
  :cash-bic "DEUTDEFF"
  :cash-currency "EUR"
  :pset-bic "CABOROEX"
  :effective-date "2025-01-01"
  :as @ssi-eu)

(cbu-custody.activate-ssi :ssi-id @ssi-eu)

(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "UK GBP"
  :type "SECURITIES"
  :safekeeping-account "SAFE-UK-001"
  :safekeeping-bic "LOYDGB2L"
  :cash-account "CASH-UK-001"
  :cash-bic "LOYDGB2L"
  :cash-currency "GBP"
  :pset-bic "CABOROEX"
  :effective-date "2025-01-01"
  :as @ssi-uk)

(cbu-custody.activate-ssi :ssi-id @ssi-uk)

;; --- SSIs: USD for cross-currency ---
(cbu-custody.ensure-ssi
  :cbu-id @fund
  :name "Global USD"
  :type "SECURITIES"
  :safekeeping-account "SAFE-USD-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "CASH-USD-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2025-01-01"
  :as @ssi-usd)

(cbu-custody.activate-ssi :ssi-id @ssi-usd)

;; --- Booking Rules: APAC ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-jp
  :name "Japan Equity"
  :priority 10
  :instrument-class "EQUITY"
  :market "XTKS"
  :currency "JPY"
  :as @rule-jp)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-sg
  :name "Singapore Equity SGD"
  :priority 10
  :instrument-class "EQUITY"
  :market "XSES"
  :currency "SGD"
  :as @rule-sg)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-usd
  :name "Singapore Equity USD"
  :priority 10
  :instrument-class "EQUITY"
  :market "XSES"
  :currency "USD"
  :as @rule-sg-usd)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-hk
  :name "HK Equity HKD"
  :priority 10
  :instrument-class "EQUITY"
  :market "XHKG"
  :currency "HKD"
  :as @rule-hk)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-usd
  :name "HK Equity USD"
  :priority 10
  :instrument-class "EQUITY"
  :market "XHKG"
  :currency "USD"
  :as @rule-hk-usd)

;; --- Booking Rules: Europe ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-uk
  :name "UK Equity"
  :priority 10
  :instrument-class "EQUITY"
  :market "XLON"
  :currency "GBP"
  :as @rule-uk)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-eu
  :name "Germany Equity"
  :priority 10
  :instrument-class "EQUITY"
  :market "XETR"
  :currency "EUR"
  :as @rule-de)

;; --- Fallback Rules ---
(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-jp
  :name "JPY Fallback"
  :priority 100
  :currency "JPY"
  :as @rule-jpy-fb)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-usd
  :name "USD Fallback"
  :priority 100
  :currency "USD"
  :as @rule-usd-fb)

(cbu-custody.ensure-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-eu
  :name "EUR Fallback"
  :priority 100
  :currency "EUR"
  :as @rule-eur-fb)

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
;; - 1 CBU with 4 entities (fund, manco, 2 sub-advisors)
;; - 5 universe entries (JP, SG, HK, UK, DE)
;; - 6 SSIs (JP, SG, HK, EU, UK, USD)
;; - 11 booking rules (7 specific + 4 fallback)
;; =============================================================================
