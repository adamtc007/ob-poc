(cbu.ensure :name "Settlement Tax Test Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)

(settlement-chain.define-location
  :code "EUROCLEAR_TEST"
  :name "Euroclear Test"
  :location-type "ICSD"
  :country-code "BE"
  :bic "MGTCBEBEECL"
  :as @location)

(settlement-chain.create-chain
  :cbu-id @fund
  :name "EU Equities Chain"
  :currency "EUR"
  :is-default true
  :as @chain)

(settlement-chain.add-hop
  :chain-id @chain
  :sequence 1
  :role "CUSTODIAN"
  :intermediary-bic "DEUTDEFF"
  :intermediary-name "Deutsche Bank"
  :as @hop1)

(settlement-chain.add-hop
  :chain-id @chain
  :sequence 2
  :role "ICSD"
  :intermediary-bic "MGTCBEBEECL"
  :intermediary-name "Euroclear"
  :as @hop2)

(settlement-chain.set-location-preference
  :cbu-id @fund
  :location-id @location
  :priority 10
  :as @loc-pref)

(tax-config.define-jurisdiction
  :code "TEST_US"
  :name "United States Test"
  :country-code "US"
  :default-rate 30.0
  :reclaim-available true
  :reclaim-deadline-days 1095
  :as @us-jur)

(tax-config.define-jurisdiction
  :code "TEST_LU"
  :name "Luxembourg Test"
  :country-code "LU"
  :default-rate 15.0
  :as @lu-jur)

(tax-config.set-treaty-rate
  :source-jurisdiction "TEST_US"
  :investor-jurisdiction "TEST_LU"
  :income-type "DIVIDEND"
  :standard-rate 30.0
  :treaty-rate 15.0
  :effective-date "2020-01-01"
  :as @treaty)

(tax-config.set-tax-status
  :cbu-id @fund
  :jurisdiction "TEST_LU"
  :investor-type "FUND"
  :documentation-status "VALIDATED"
  :as @tax-status)

(tax-config.set-reclaim-config
  :cbu-id @fund
  :source-jurisdiction "TEST_US"
  :reclaim-method "AUTOMATIC"
  :batch-frequency "MONTHLY"
  :as @reclaim)

(tax-config.set-reporting
  :cbu-id @fund
  :regime "CRS"
  :jurisdiction "TEST_LU"
  :status "PARTICIPATING"
  :as @reporting)

(settlement-chain.validate-settlement-config :cbu-id @fund)
(tax-config.validate-tax-config :cbu-id @fund)
