;; =============================================================================
;; EQUITY + OTC IRS WITH ISDA/CSA
;; =============================================================================

;; --- Entities (lookup existing counterparties) ---
(entity.read :name "Morgan Stanley" :as @ms)

;; --- CBU ---
(cbu.ensure :name "Pacific Fund" :jurisdiction "US" :client-type "FUND" :as @cbu)

;; --- Layer 1: Universe ---
;; Cash instruments
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])
;; OTC (counterparty-specific)
(cbu-custody.add-universe :cbu-id @cbu :instrument-class "OTC_IRS" :currencies ["USD"] :counterparty @ms :settlement-types ["DVP"])

;; --- Layer 2: SSIs ---
(cbu-custody.create-ssi :cbu-id @cbu :name "US Primary" :type "SECURITIES"
  :safekeeping-account "SAFE-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)

(cbu-custody.create-ssi :cbu-id @cbu :name "Collateral" :type "COLLATERAL"
  :safekeeping-account "COLL-001" :safekeeping-bic "BABOROCP"
  :cash-account "COLL-CASH-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-collateral)

(cbu-custody.activate-ssi :ssi-id @ssi-us)
(cbu-custody.activate-ssi :ssi-id @ssi-collateral)

;; --- Layer 3: Booking Rules ---
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "US Equity DVP" :priority 10
  :instrument-class "EQUITY" :market "XNYS" :currency "USD" :settlement-type "DVP" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "OTC IRS MS" :priority 20
  :instrument-class "OTC_IRS" :currency "USD" :counterparty @ms :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "USD Fallback" :priority 50 :currency "USD" :effective-date "2024-12-01")
(cbu-custody.add-booking-rule :cbu-id @cbu :ssi-id @ssi-us :name "Ultimate Fallback" :priority 100 :effective-date "2024-12-01")

;; --- ISDA ---
(isda.create :cbu-id @cbu :counterparty @ms :agreement-date "2024-12-01"
  :governing-law "NY" :effective-date "2024-12-01" :as @isda-ms)
(isda.add-coverage :isda-id @isda-ms :instrument-class "OTC_IRS")
(isda.add-csa :isda-id @isda-ms :csa-type "VM" :threshold 250000
  :threshold-currency "USD" :collateral-ssi @ssi-collateral :effective-date "2024-12-01" :as @csa-ms)

;; --- Validation ---
(cbu-custody.validate-booking-coverage :cbu-id @cbu)
