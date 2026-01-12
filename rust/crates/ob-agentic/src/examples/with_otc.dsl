;; OTC Pattern - Includes ISDA/CSA for derivatives
(entity.create-limited-company :name "Derivatives Alpha LLC" :jurisdiction "US" :as @head-office)
(entity.create-limited-company :name "Morgan Stanley International" :jurisdiction "GB" :as @ms)
(cbu.ensure :name "Derivatives Alpha Master Fund" :jurisdiction "KY" :client-type "FUND" :commercial-client-entity-id @head-office :as @cbu)

;; Create trading profile
(trading-profile.create-draft :cbu-id @cbu :base-currency "USD" :as @profile)

;; Add trading universe - equities + OTC
(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY")
(trading-profile.add-instrument-class :profile-id @profile :class-code "OTC_IRS")
(trading-profile.add-market :profile-id @profile :market-code "XNYS" :currencies ["USD"])

;; Add SSIs
(trading-profile.add-standing-instruction :profile-id @profile :ssi-name "US Primary" :ssi-type "SECURITIES"
  :safekeeping-account "SAFE-US-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01")

(trading-profile.add-standing-instruction :profile-id @profile :ssi-name "Collateral" :ssi-type "COLLATERAL"
  :cash-account "COLL-USD-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :effective-date "2024-12-01")

;; Add booking rules
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "US Equity DVP" :priority 10 :instrument-class "EQUITY" :market "XNYS" :currency "USD")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "OTC IRS" :priority 20 :instrument-class "OTC_IRS" :currency "USD")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "USD Fallback" :priority 50 :currency "USD")

;; Add ISDA config for Morgan Stanley
(trading-profile.add-isda-config :profile-id @profile :counterparty-name "Morgan Stanley International" :agreement-date "2024-01-15")
(trading-profile.add-isda-coverage :profile-id @profile :counterparty-ref "Morgan Stanley International" :asset-class "RATES" :base-products ["IRS" "XCCY"])

;; Add CSA for collateral
(trading-profile.add-csa-config :profile-id @profile :counterparty-ref "Morgan Stanley International"
  :threshold-amount 10000000 :threshold-currency "USD" :minimum-transfer 500000)
(trading-profile.add-csa-collateral :profile-id @profile :counterparty-ref "Morgan Stanley International"
  :collateral-type "CASH" :currencies ["USD" "EUR" "GBP"])
(trading-profile.link-csa-ssi :profile-id @profile :counterparty-ref "Morgan Stanley International" :ssi-ref "Collateral")

;; Validate and submit
(trading-profile.validate-go-live-ready :profile-id @profile)
(trading-profile.submit :profile-id @profile)
