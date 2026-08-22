(cbu.ensure :name "CA Pricing Test Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)

(corporate-action.set-preferences
  :cbu-id @fund
  :event-type "CASH_DIV"
  :processing-mode "AUTO_INSTRUCT"
  :as @ca-pref)

(corporate-action.set-instruction-window
  :cbu-id @fund
  :event-type "CASH_DIV"
  :cutoff-days-before 3
  :as @ca-window)

(cbu-custody.create-ssi
  :cbu-id @fund
  :ssi-name "CA Settlement SSI"
  :ssi-type "SECURITIES"
  :safekeeping-account "CA-SAFE-001"
  :safekeeping-bic "DEUTDEFF"
  :effective-date "2025-01-01"
  :as @ca-ssi)

(corporate-action.link-ca-ssi
  :cbu-id @fund
  :event-type "CASH_DIV"
  :currency "EUR"
  :ssi-id @ca-ssi
  :as @ca-ssi-link)

(corporate-action.validate-ca-config :cbu-id @fund)
