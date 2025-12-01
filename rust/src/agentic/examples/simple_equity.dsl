;; =============================================================================
;; SIMPLE US EQUITY SETUP
;; =============================================================================

;; --- CBU ---
(cbu.ensure :name "Apex Capital" :jurisdiction "US" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])

;; --- Layer 2: SSI ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-us)

;; --- Layer 3: Booking Rules ---
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity DVP" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD" :effective-date "2024-12-01")

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
