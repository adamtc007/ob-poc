;; Multi-Market Pattern - Multiple markets with cross-currency
(entity.create-limited-company :name "Global Alpha LLC" :jurisdiction "US" :as @head-office)
(cbu.ensure :name "Global Alpha Master Fund" :jurisdiction "KY" :client-type "FUND" :commercial-client-entity-id @head-office :as @cbu)

;; Create trading profile
(trading-profile.create-draft :cbu-id @cbu :base-currency "USD" :as @profile)

;; Add trading universe - multiple markets
(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY")
(trading-profile.add-market :profile-id @profile :market-code "XNYS" :currencies ["USD"])
(trading-profile.add-market :profile-id @profile :market-code "XLON" :currencies ["GBP" "USD"])
(trading-profile.add-market :profile-id @profile :market-code "XETR" :currencies ["EUR" "USD"])

;; Add SSIs for each market
(trading-profile.add-standing-instruction :profile-id @profile :ssi-name "US Primary" :ssi-type "SECURITIES"
  :safekeeping-account "SAFE-US-001" :safekeeping-bic "BABOROCP"
  :cash-account "CASH-USD-001" :cash-bic "BABOROCP" :cash-currency "USD"
  :pset-bic "DTCYUS33" :effective-date "2024-12-01")

(trading-profile.add-standing-instruction :profile-id @profile :ssi-name "UK Primary" :ssi-type "SECURITIES"
  :safekeeping-account "SAFE-UK-001" :safekeeping-bic "MIDLGB22"
  :cash-account "CASH-GBP-001" :cash-bic "MIDLGB22" :cash-currency "GBP"
  :pset-bic "CABOROCP" :effective-date "2024-12-01")

(trading-profile.add-standing-instruction :profile-id @profile :ssi-name "DE Primary" :ssi-type "SECURITIES"
  :safekeeping-account "SAFE-DE-001" :safekeeping-bic "COBADEFF"
  :cash-account "CASH-EUR-001" :cash-bic "COBADEFF" :cash-currency "EUR"
  :pset-bic "DAKVDEFF" :effective-date "2024-12-01")

;; Add booking rules - market specific
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "US Equity USD" :priority 10 :instrument-class "EQUITY" :market "XNYS" :currency "USD")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "UK Primary" :rule-name "UK Equity GBP" :priority 15 :instrument-class "EQUITY" :market "XLON" :currency "GBP")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "UK Equity USD" :priority 16 :instrument-class "EQUITY" :market "XLON" :currency "USD")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "DE Primary" :rule-name "DE Equity EUR" :priority 20 :instrument-class "EQUITY" :market "XETR" :currency "EUR")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "DE Equity USD" :priority 21 :instrument-class "EQUITY" :market "XETR" :currency "USD")

;; Currency fallbacks
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "US Primary" :rule-name "USD Fallback" :priority 50 :currency "USD")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "UK Primary" :rule-name "GBP Fallback" :priority 51 :currency "GBP")
(trading-profile.add-booking-rule :profile-id @profile :ssi-ref "DE Primary" :rule-name "EUR Fallback" :priority 52 :currency "EUR")

;; Validate and submit
(trading-profile.validate-go-live-ready :profile-id @profile)
(trading-profile.submit :profile-id @profile)
