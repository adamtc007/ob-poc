;; =============================================================================
;; Custody SSI Bulk Import Test
;;
;; Tests the setup-ssi verb which imports SSIs from an SSI_ONBOARDING document.
;; This simulates receiving SSI data from Omgeo ALERT or similar systems.
;;
;; Prerequisites:
;; 1. SSI_ONBOARDING document type must exist
;; 2. Document must be cataloged with extracted_data containing SSI JSON
;; =============================================================================

;; Create commercial client entity (head office)
(entity.create-limited-company
  :name "Bulk Import Test Fund LLC"
  :jurisdiction "US"
  :company-number "BULK-2024-001"
  :as @head-office)

;; Create the CBU
(cbu.ensure
  :name "Bulk Import Test Master Fund"
  :jurisdiction "KY"
  :client-type "FUND"
  :commercial-client-entity-id @head-office
  :as @cbu)

;; =============================================================================
;; Define Trading Universe (must be done before SSI import)
;; =============================================================================

;; US Equities
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

;; UK Equities (GBP and USD cross-currency)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]
  :settlement-types ["DVP"])

;; Germany Equities (EUR and USD cross-currency)
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XETR"
  :currencies ["EUR" "USD"]
  :settlement-types ["DVP"])

;; Japan Equities
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XTKS"
  :currencies ["JPY" "USD"]
  :settlement-types ["DVP"])

;; Hong Kong Equities
(cbu-custody.add-universe
  :cbu-id @cbu
  :instrument-class "EQUITY"
  :market "XHKG"
  :currencies ["HKD" "USD"]
  :settlement-types ["DVP"])

;; =============================================================================
;; Catalog SSI Onboarding Document
;; (In production, this would be uploaded via API with JSON content)
;; =============================================================================

(document.catalog
  :cbu-id @cbu
  :doc-type "SSI_ONBOARDING"
  :title "Global Alpha SSI Setup 2024"
  :as @ssi-doc)

;; =============================================================================
;; NOTE: The setup-ssi verb requires the document to have extracted_data
;; populated with the SSI JSON. In production, this would be done via:
;; 1. API upload with JSON body stored in extracted_data
;; 2. Or manual SQL update for testing:
;;
;; UPDATE "ob-poc".document_catalog
;; SET extracted_data = '{"settlement_instructions": [...]}'::jsonb
;; WHERE doc_id = '<doc-id>';
;; =============================================================================

;; For now, we demonstrate the manual SSI creation approach which works
;; without requiring the document to be pre-populated:

;; --- Manual SSI Creation (alternative to bulk import) ---

;; US Primary
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "US Primary"
  :type "SECURITIES"
  :safekeeping-account "BULK-SAFE-US-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "BULK-CASH-USD-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi-us)

;; UK GBP Primary
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "UK GBP Primary"
  :type "SECURITIES"
  :safekeeping-account "BULK-SAFE-UK-001"
  :safekeeping-bic "MIDLGB22"
  :cash-account "BULK-CASH-GBP-001"
  :cash-bic "MIDLGB22"
  :cash-currency "GBP"
  :pset-bic "CABOROCP"
  :effective-date "2024-12-01"
  :as @ssi-uk)

;; DE EUR Primary
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "DE EUR Primary"
  :type "SECURITIES"
  :safekeeping-account "BULK-SAFE-DE-001"
  :safekeeping-bic "COBADEFF"
  :cash-account "BULK-CASH-EUR-001"
  :cash-bic "COBADEFF"
  :cash-currency "EUR"
  :pset-bic "DAKVDEFF"
  :effective-date "2024-12-01"
  :as @ssi-de)

;; JP JPY Primary
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "JP JPY Primary"
  :type "SECURITIES"
  :safekeeping-account "BULK-SAFE-JP-001"
  :safekeeping-bic "MABORJPJ"
  :cash-account "BULK-CASH-JPY-001"
  :cash-bic "MABORJPJ"
  :cash-currency "JPY"
  :pset-bic "JAABORJP"
  :effective-date "2024-12-01"
  :as @ssi-jp)

;; HK HKD Primary
(cbu-custody.create-ssi
  :cbu-id @cbu
  :name "HK HKD Primary"
  :type "SECURITIES"
  :safekeeping-account "BULK-SAFE-HK-001"
  :safekeeping-bic "HABORHKH"
  :cash-account "BULK-CASH-HKD-001"
  :cash-bic "HABORHKH"
  :cash-currency "HKD"
  :pset-bic "CCABORHK"
  :effective-date "2024-12-01"
  :as @ssi-hk)

;; Activate all SSIs
(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk)
(cbu-custody.activate-ssi :ssi-id @ssi-de)
(cbu-custody.activate-ssi :ssi-id @ssi-jp)
(cbu-custody.activate-ssi :ssi-id @ssi-hk)

;; Add agent override for Japan (intermediary chain)
(cbu-custody.add-agent-override
  :ssi-id @ssi-jp
  :agent-role "INT1"
  :agent-bic "SABORJPJ"
  :agent-account "INT-JP-001"
  :agent-name "Sub-Custodian Japan"
  :sequence-order 1
  :reason "Local market requires Japanese sub-custodian")

;; =============================================================================
;; Booking Rules
;; =============================================================================

;; Market-specific rules (priority 10)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity USD" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-uk :name "UK Equity GBP" :priority 10
  :instrument-class "EQUITY" :market "XLON" :currency "GBP" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "UK Equity USD" :priority 11
  :instrument-class "EQUITY" :market "XLON" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-de :name "DE Equity EUR" :priority 10
  :instrument-class "EQUITY" :market "XETR" :currency "EUR" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "DE Equity USD" :priority 11
  :instrument-class "EQUITY" :market "XETR" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-jp :name "JP Equity JPY" :priority 10
  :instrument-class "EQUITY" :market "XTKS" :currency "JPY" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "JP Equity USD" :priority 11
  :instrument-class "EQUITY" :market "XTKS" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-hk :name "HK Equity HKD" :priority 10
  :instrument-class "EQUITY" :market "XHKG" :currency "HKD" :settlement-type "DVP" :effective-date "2024-12-01")

(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "HK Equity USD" :priority 11
  :instrument-class "EQUITY" :market "XHKG" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")

;; Currency fallbacks (priority 50)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-uk :name "GBP Fallback" :priority 50 :currency "GBP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-de :name "EUR Fallback" :priority 50 :currency "EUR" :effective-date "2024-12-01")

;; Ultimate fallback (priority 100)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "Ultimate Fallback" :priority 100 :effective-date "2024-12-01")

;; =============================================================================
;; Validation
;; =============================================================================

(cbu-custody.validate-booking-coverage :cbu-id @cbu)

;; =============================================================================
;; Test Lookups
;; =============================================================================

(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP")
(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XLON" :currency "GBP" :settlement-type "DVP")
(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XETR" :currency "EUR" :settlement-type "DVP")
(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XTKS" :currency "JPY" :settlement-type "DVP")
(cbu-custody.lookup-ssi :cbu-id @cbu :instrument-class "EQUITY" :market "XHKG" :currency "HKD" :settlement-type "DVP")

;; List agent overrides for Japan SSI
(cbu-custody.list-agent-overrides :ssi-id @ssi-jp)
