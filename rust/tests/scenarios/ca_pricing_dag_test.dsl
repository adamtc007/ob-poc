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

(pricing-config.set-valuation-schedule
  :cbu-id @fund
  :frequency "EOD"
  :valuation-time "16:00"
  :timezone "America/New_York"
  :as @val-schedule)

(pricing-config.set-fallback-chain
  :cbu-id @fund
  :fallback-sources ["BLOOMBERG" "REUTERS" "MARKIT"]
  :fallback-trigger "STALE"
  :as @fallback)

(pricing-config.set-stale-policy
  :cbu-id @fund
  :max-age-hours 24
  :stale-action "USE_FALLBACK"
  :as @stale-policy)

(pricing-config.set-nav-threshold
  :cbu-id @fund
  :threshold-pct 5.0
  :action "ALERT"
  :as @nav-threshold)

(corporate-action.validate-ca-config :cbu-id @fund)
(pricing-config.validate-pricing-config :cbu-id @fund)
