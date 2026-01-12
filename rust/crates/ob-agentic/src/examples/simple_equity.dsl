;; Simple Equity Pattern - Single market, single currency
(entity.create-limited-company :name "Simple Fund LLC" :jurisdiction "US" :as @head-office)
(cbu.ensure :name "Simple Equity Fund" :jurisdiction "KY" :client-type "FUND" :commercial-client-entity-id @head-office :as @cbu)

;; Create trading profile
(trading-profile.create-draft :cbu-id @cbu :base-currency "USD" :as @profile)

;; Add trading universe
(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY")
(trading-profile.add-market :profile-id @profile :market-code "XNYS" :currencies ["USD"])

;; Add SSI
(trading-profile.add-standing-instruction :profile-id @profile :ssi-name "US Primary" :ssi-type "SECURITIES"
  :safekeeping-account "SAFE-US-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01" :as @ssi-us)

;; Add booking rule
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "US Equity DVP"
  :priority 10 :instrument-class "EQUITY" :market "XNYS" :currency "USD")

;; Validate and submit
(trading-profile.validate-go-live-ready :profile-id @profile)
(trading-profile.submit :profile-id @profile)
