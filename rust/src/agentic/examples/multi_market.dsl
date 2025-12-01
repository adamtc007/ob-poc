;; =============================================================================
;; MULTI-MARKET EQUITY WITH CROSS-CURRENCY
;; =============================================================================

;; --- CBU ---
(cbu.ensure :name "Global Fund" :jurisdiction "LU" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XLON" :currencies ["GBP" "USD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XFRA" :currencies ["EUR" "USD"] :settlement-types ["DVP"])

;; --- Layer 2: SSIs ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-US" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)

(cbu-custody.create-ssi :cbu-id @cbu :name "UK Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-UK" :safekeeping-bic "CABOROCP"
  :cash-account "CASH-GBP" :cash-bic "CABOROCP" :cash-currency "GBP"
  :pset-bic "CABOROCP" :effective-date "2024-12-01" :as @ssi-uk)

(cbu-custody.create-ssi :cbu-id @cbu :name "DE Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-DE" :safekeeping-bic "DAKVDEFF"
  :cash-account "CASH-EUR" :cash-bic "DAKVDEFF" :cash-currency "EUR"
  :pset-bic "DAKVDEFF" :effective-date "2024-12-01" :as @ssi-de)

(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-uk)
(cbu-custody.activate-ssi :ssi-id @ssi-de)

;; --- Layer 3: Booking Rules ---
;; Specific rules (high priority)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity USD" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-uk :name "UK Equity GBP" :priority 15
  :instrument-class "EQUITY" :market "XLON" :currency "GBP" :settlement-type "DVP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "UK Equity USD" :priority 16
  :instrument-class "EQUITY" :market "XLON" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-de :name "DE Equity EUR" :priority 20
  :instrument-class "EQUITY" :market "XFRA" :currency "EUR" :settlement-type "DVP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "DE Equity USD" :priority 21
  :instrument-class "EQUITY" :market "XFRA" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")

;; Currency fallbacks (medium priority)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-uk :name "GBP Fallback" :priority 51 :currency "GBP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-de :name "EUR Fallback" :priority 52 :currency "EUR" :effective-date "2024-12-01")

;; Ultimate fallback (low priority)
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "Ultimate Fallback" :priority 100 :effective-date "2024-12-01")

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
