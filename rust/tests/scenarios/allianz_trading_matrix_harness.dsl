;; ==============================================================================
;; ALLIANZ TRADING MATRIX TEST HARNESS
;; ==============================================================================
;;
;; This comprehensive test harness validates the full Trading Instrument Matrix
;; implementation across all 4 phases:
;;   Phase 1: Universe, SSIs, Booking Rules (cbu-custody.*)
;;   Phase 2: Instruction Profile, Gateway Routing, Corporate Actions
;;   Phase 3: Settlement Chains, Tax Configuration
;;   Phase 4: Full Matrix Export and Validation
;;
;; Based on: config/seed/trading_profiles/allianzgi_complete.yaml
;; ==============================================================================

;; ==============================================================================
;; SECTION 1: REFERENCE DATA SETUP
;; These must exist before CBU-specific configuration
;; ==============================================================================

;; --- Instrument Classes (CFI-based taxonomy) ---
(instrument-class.ensure
  :code "EQUITY"
  :name "Equity Securities"
  :settlement-cycle "T+2"
  :swift-family "MT54x"
  :cfi-category "E"
  :smpg-group "EQUITIES"
  :as @ic-equity)

(instrument-class.ensure
  :code "GOVT_BOND"
  :name "Government Bonds"
  :settlement-cycle "T+1"
  :swift-family "MT54x"
  :cfi-category "D"
  :cfi-group "B"
  :smpg-group "FIXED_INCOME"
  :as @ic-govt-bond)

(instrument-class.ensure
  :code "CORP_BOND"
  :name "Corporate Bonds"
  :settlement-cycle "T+2"
  :swift-family "MT54x"
  :cfi-category "D"
  :cfi-group "C"
  :smpg-group "FIXED_INCOME"
  :as @ic-corp-bond)

(instrument-class.ensure
  :code "ETF"
  :name "Exchange Traded Funds"
  :settlement-cycle "T+2"
  :swift-family "MT54x"
  :cfi-category "C"
  :smpg-group "FUNDS"
  :as @ic-etf)

(instrument-class.ensure
  :code "OTC_IRS"
  :name "OTC Interest Rate Swaps"
  :settlement-cycle "T+0"
  :isda-asset-class "RATES"
  :requires-isda true
  :as @ic-otc-irs)

(instrument-class.ensure
  :code "OTC_FX"
  :name "OTC FX Derivatives"
  :settlement-cycle "T+2"
  :isda-asset-class "FX"
  :requires-isda true
  :as @ic-otc-fx)

;; --- Markets (ISO 10383 MIC codes) ---
(market.ensure :mic "XETR" :name "Deutsche Boerse Xetra" :country-code "DE" :primary-currency "EUR" :csd-bic "DAABORDC" :timezone "Europe/Berlin" :as @mkt-xetr)
(market.ensure :mic "XLON" :name "London Stock Exchange" :country-code "GB" :primary-currency "GBP" :csd-bic "CRESTGB2" :timezone "Europe/London" :as @mkt-xlon)
(market.ensure :mic "XNYS" :name "New York Stock Exchange" :country-code "US" :primary-currency "USD" :csd-bic "DTCYUS33" :timezone "America/New_York" :as @mkt-xnys)
(market.ensure :mic "XNAS" :name "NASDAQ" :country-code "US" :primary-currency "USD" :csd-bic "DTCYUS33" :timezone "America/New_York" :as @mkt-xnas)
(market.ensure :mic "XHKG" :name "Hong Kong Stock Exchange" :country-code "HK" :primary-currency "HKD" :csd-bic "CCASCHKX" :timezone "Asia/Hong_Kong" :as @mkt-xhkg)
(market.ensure :mic "XTKS" :name "Tokyo Stock Exchange" :country-code "JP" :primary-currency "JPY" :csd-bic "JASDECJP" :timezone "Asia/Tokyo" :as @mkt-xtks)
(market.ensure :mic "XSWX" :name "SIX Swiss Exchange" :country-code "CH" :primary-currency "CHF" :csd-bic "SABORDC1" :timezone "Europe/Zurich" :as @mkt-xswx)
(market.ensure :mic "XPAR" :name "Euronext Paris" :country-code "FR" :primary-currency "EUR" :csd-bic "SICVFR2P" :timezone "Europe/Paris" :as @mkt-xpar)

;; --- Tax Jurisdictions ---
(tax-config.define-jurisdiction :code "DE" :name "Germany" :country-code "DE" :default-rate 26.375 :reclaim-available true :reclaim-deadline-days 365 :as @tax-de)
(tax-config.define-jurisdiction :code "GB" :name "United Kingdom" :country-code "GB" :default-rate 0 :reclaim-available false :as @tax-gb)
(tax-config.define-jurisdiction :code "US" :name "United States" :country-code "US" :default-rate 30 :reclaim-available true :reclaim-deadline-days 730 :as @tax-us)
(tax-config.define-jurisdiction :code "JP" :name "Japan" :country-code "JP" :default-rate 15.315 :reclaim-available true :reclaim-deadline-days 365 :as @tax-jp)
(tax-config.define-jurisdiction :code "HK" :name "Hong Kong" :country-code "HK" :default-rate 0 :reclaim-available false :as @tax-hk)
(tax-config.define-jurisdiction :code "LU" :name "Luxembourg" :country-code "LU" :default-rate 15 :reclaim-available true :as @tax-lu)
(tax-config.define-jurisdiction :code "CH" :name "Switzerland" :country-code "CH" :default-rate 35 :reclaim-available true :reclaim-deadline-days 1095 :as @tax-ch)

;; --- Settlement Locations (CSDs and ICSDs) ---
(settlement-chain.define-location :code "CLEARSTREAM" :name "Clearstream Banking" :location-type "ICSD" :country-code "LU" :bic "CABORDC1" :as @loc-clearstream)
(settlement-chain.define-location :code "EUROCLEAR" :name "Euroclear Bank" :location-type "ICSD" :country-code "BE" :bic "MABORDC1" :as @loc-euroclear)
(settlement-chain.define-location :code "DTC" :name "Depository Trust Company" :location-type "CSD" :country-code "US" :bic "DTCYUS33" :as @loc-dtc)
(settlement-chain.define-location :code "CREST" :name "Euroclear UK & Ireland" :location-type "CSD" :country-code "GB" :bic "CRSTGB22" :as @loc-crest)
(settlement-chain.define-location :code "CBF" :name "Clearstream Banking Frankfurt" :location-type "CSD" :country-code "DE" :bic "DAABORDC" :as @loc-cbf)

;; ==============================================================================
;; SECTION 2: CBU SETUP - Create Allianz Test Fund
;; ==============================================================================

(cbu.ensure
  :name "AllianzGI Global Multi-Asset Test Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :as @allianz-fund)

;; ==============================================================================
;; SECTION 3: TRADING UNIVERSE POPULATION
;; Define what the fund can trade (Phase 1)
;; ==============================================================================

;; --- European Equities ---
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XETR" :currencies ["EUR"] :settlement-types ["DVP" "FOP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XLON" :currencies ["GBP"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XPAR" :currencies ["EUR"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XSWX" :currencies ["CHF"] :settlement-types ["DVP"])

;; --- US Equities ---
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XNAS" :currencies ["USD"] :settlement-types ["DVP"])

;; --- Asia-Pacific Equities ---
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XHKG" :currencies ["HKD"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "EQUITY" :market "XTKS" :currencies ["JPY"] :settlement-types ["DVP"])

;; --- Fixed Income ---
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "GOVT_BOND" :currencies ["EUR" "USD" "GBP"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "CORP_BOND" :currencies ["EUR" "USD"] :settlement-types ["DVP"])

;; --- ETFs ---
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "ETF" :market "XETR" :currencies ["EUR"] :settlement-types ["DVP"])
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "ETF" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])

;; --- OTC Derivatives (requires ISDA) ---
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "OTC_IRS" :currencies ["EUR" "USD" "GBP"] :is-held false :is-traded true)
(cbu-custody.add-universe :cbu-id @allianz-fund :instrument-class "OTC_FX" :currencies ["EUR" "USD" "GBP" "CHF" "JPY"] :is-held false :is-traded true)

;; ==============================================================================
;; SECTION 4: STANDING SETTLEMENT INSTRUCTIONS (SSIs)
;; Phase 1: Pure account data for settlement
;; ==============================================================================

;; --- German Equities SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "DE_EQUITY_SSI"
  :type "SECURITIES"
  :safekeeping-account "DE-DEPOT-ALLIANZ-001"
  :safekeeping-bic "DEUTDEFF"
  :cash-account "DE-CASH-ALLIANZ-001"
  :cash-bic "DEUTDEFF"
  :cash-currency "EUR"
  :pset-bic "DAABORDC"
  :effective-date "2024-01-01"
  :as @ssi-de-equity)

;; --- UK Equities SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "UK_EQUITY_SSI"
  :type "SECURITIES"
  :safekeeping-account "GB-DEPOT-ALLIANZ-001"
  :safekeeping-bic "MIDLGB22"
  :cash-account "GB-CASH-ALLIANZ-001"
  :cash-bic "MIDLGB22"
  :cash-currency "GBP"
  :pset-bic "CRESTGB2"
  :effective-date "2024-01-01"
  :as @ssi-uk-equity)

;; --- US Equities SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "US_EQUITY_SSI"
  :type "SECURITIES"
  :safekeeping-account "US-DEPOT-ALLIANZ-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "US-CASH-ALLIANZ-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-01-01"
  :as @ssi-us-equity)

;; --- Hong Kong SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "HK_SSI"
  :type "SECURITIES"
  :safekeeping-account "HK-DEPOT-ALLIANZ-001"
  :safekeeping-bic "HSBCHKHH"
  :cash-account "HK-CASH-ALLIANZ-001"
  :cash-bic "HSBCHKHH"
  :cash-currency "HKD"
  :pset-bic "CCASCHKX"
  :effective-date "2024-01-01"
  :as @ssi-hk)

;; --- Japan SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "JP_SSI"
  :type "SECURITIES"
  :safekeeping-account "JP-DEPOT-ALLIANZ-001"
  :safekeeping-bic "MABORJPJ"
  :cash-account "JP-CASH-ALLIANZ-001"
  :cash-bic "MABORJPJ"
  :cash-currency "JPY"
  :pset-bic "JASDECJP"
  :effective-date "2024-01-01"
  :as @ssi-jp)

;; --- EUR Bond SSI (via Clearstream) ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "EUR_BOND_SSI"
  :type "SECURITIES"
  :safekeeping-account "EU-BOND-DEPOT-001"
  :safekeeping-bic "CABORDC1"
  :cash-account "EU-BOND-CASH-001"
  :cash-bic "CABORDC1"
  :cash-currency "EUR"
  :pset-bic "CABORDC1"
  :effective-date "2024-01-01"
  :as @ssi-eur-bond)

;; --- USD Bond SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "USD_BOND_SSI"
  :type "SECURITIES"
  :safekeeping-account "US-BOND-DEPOT-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "US-BOND-CASH-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-01-01"
  :as @ssi-usd-bond)

;; --- Default/Fallback SSI ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "DEFAULT_SSI"
  :type "SECURITIES"
  :safekeeping-account "DEFAULT-DEPOT-001"
  :safekeeping-bic "CABORDC1"
  :cash-account "DEFAULT-CASH-001"
  :cash-bic "CABORDC1"
  :cash-currency "EUR"
  :pset-bic "CABORDC1"
  :effective-date "2024-01-01"
  :as @ssi-default)

;; --- OTC Collateral SSIs (for CSA) ---
(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "GS_COLLATERAL_SSI"
  :type "COLLATERAL"
  :safekeeping-account "COLL-GS-EUR-001"
  :safekeeping-bic "CABORDC1"
  :cash-account "CASH-GS-EUR-001"
  :cash-bic "CABORDC1"
  :cash-currency "USD"
  :effective-date "2024-01-01"
  :as @ssi-gs-collateral)

(cbu-custody.create-ssi
  :cbu-id @allianz-fund
  :name "JPM_COLLATERAL_SSI"
  :type "COLLATERAL"
  :safekeeping-account "COLL-JPM-USD-001"
  :safekeeping-bic "IRVTUS3N"
  :cash-account "CASH-JPM-USD-001"
  :cash-bic "IRVTUS3N"
  :cash-currency "USD"
  :effective-date "2024-01-01"
  :as @ssi-jpm-collateral)

;; --- Activate all SSIs ---
(cbu-custody.activate-ssi :ssi-id @ssi-de-equity)
(cbu-custody.activate-ssi :ssi-id @ssi-uk-equity)
(cbu-custody.activate-ssi :ssi-id @ssi-us-equity)
(cbu-custody.activate-ssi :ssi-id @ssi-hk)
(cbu-custody.activate-ssi :ssi-id @ssi-jp)
(cbu-custody.activate-ssi :ssi-id @ssi-eur-bond)
(cbu-custody.activate-ssi :ssi-id @ssi-usd-bond)
(cbu-custody.activate-ssi :ssi-id @ssi-default)
(cbu-custody.activate-ssi :ssi-id @ssi-gs-collateral)
(cbu-custody.activate-ssi :ssi-id @ssi-jpm-collateral)

;; ==============================================================================
;; SECTION 5: BOOKING RULES (ALERT-Style SSI Selection)
;; Phase 1: Priority-based routing from trade -> SSI
;; ==============================================================================

;; --- Market-Specific Rules (High Priority) ---
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-de-equity :name "German Equities via CBF" :priority 10 :instrument-class "EQUITY" :market "XETR")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-uk-equity :name "UK Equities via CREST" :priority 10 :instrument-class "EQUITY" :market "XLON")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-us-equity :name "US Equities NYSE via DTC" :priority 10 :instrument-class "EQUITY" :market "XNYS")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-us-equity :name "US Equities NASDAQ via DTC" :priority 10 :instrument-class "EQUITY" :market "XNAS")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-hk :name "Hong Kong via CCASS" :priority 10 :market "XHKG")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-jp :name "Japan via JASDEC" :priority 10 :market "XTKS")

;; --- Fixed Income Rules (Medium Priority) ---
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-eur-bond :name "EUR Government Bonds" :priority 20 :instrument-class "GOVT_BOND" :currency "EUR")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-usd-bond :name "USD Government Bonds" :priority 20 :instrument-class "GOVT_BOND" :currency "USD")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-eur-bond :name "EUR Corporate Bonds" :priority 25 :instrument-class "CORP_BOND" :currency "EUR")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-usd-bond :name "USD Corporate Bonds" :priority 25 :instrument-class "CORP_BOND" :currency "USD")

;; --- ETF Rules ---
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-de-equity :name "EUR ETFs via Xetra" :priority 15 :instrument-class "ETF" :market "XETR")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-us-equity :name "US ETFs via DTC" :priority 15 :instrument-class "ETF" :market "XNYS")

;; --- OTC Derivative Rules (with ISDA asset class) ---
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-gs-collateral :name "GS IRS Collateral" :priority 5 :instrument-class "OTC_IRS" :isda-asset-class "RATES")
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-gs-collateral :name "GS FX Collateral" :priority 5 :instrument-class "OTC_FX" :isda-asset-class "FX")

;; --- Fallback Rule (Lowest Priority) ---
(cbu-custody.add-booking-rule :cbu-id @allianz-fund :ssi-id @ssi-default :name "Default Fallback" :priority 100)

;; ==============================================================================
;; SECTION 6: SETTLEMENT CHAIN CONFIGURATION
;; Phase 3: Multi-hop settlement paths
;; ==============================================================================

;; --- German Equities via Deutsche Bank -> CBF ---
(settlement-chain.create-chain
  :cbu-id @allianz-fund
  :name "DE-EQUITY-CHAIN"
  :market "XETR"
  :instrument-class "EQUITY"
  :currency "EUR"
  :is-default true
  :as @chain-de-equity)

(settlement-chain.add-hop :chain-id @chain-de-equity :sequence 1 :role "CUSTODIAN" :intermediary-bic "DEUTDEFF" :intermediary-name "Deutsche Bank" :account-number "DE-DEPOT-ALLIANZ-001")
(settlement-chain.add-hop :chain-id @chain-de-equity :sequence 2 :role "CSD" :intermediary-bic "DAABORDC" :intermediary-name "Clearstream Banking Frankfurt")

;; --- UK Equities via HSBC -> CREST ---
(settlement-chain.create-chain
  :cbu-id @allianz-fund
  :name "UK-EQUITY-CHAIN"
  :market "XLON"
  :instrument-class "EQUITY"
  :currency "GBP"
  :is-default true
  :as @chain-uk-equity)

(settlement-chain.add-hop :chain-id @chain-uk-equity :sequence 1 :role "CUSTODIAN" :intermediary-bic "MIDLGB22" :intermediary-name "HSBC UK" :account-number "GB-DEPOT-ALLIANZ-001")
(settlement-chain.add-hop :chain-id @chain-uk-equity :sequence 2 :role "CSD" :intermediary-bic "CRSTGB22" :intermediary-name "Euroclear UK (CREST)")

;; --- US Equities via BNY -> DTC ---
(settlement-chain.create-chain
  :cbu-id @allianz-fund
  :name "US-EQUITY-CHAIN"
  :market "XNYS"
  :instrument-class "EQUITY"
  :currency "USD"
  :is-default true
  :as @chain-us-equity)

(settlement-chain.add-hop :chain-id @chain-us-equity :sequence 1 :role "CUSTODIAN" :intermediary-bic "IRVTUS3N" :intermediary-name "BNY Mellon" :account-number "US-DEPOT-ALLIANZ-001")
(settlement-chain.add-hop :chain-id @chain-us-equity :sequence 2 :role "CSD" :intermediary-bic "DTCYUS33" :intermediary-name "Depository Trust Company")

;; --- Cross-Border: DE -> US (Bridge via Clearstream) ---
(settlement-chain.set-cross-border
  :cbu-id @allianz-fund
  :source-market "XETR"
  :target-market "XNYS"
  :settlement-method "VIA_ICSD"
  :bridge-location-id @loc-clearstream
  :preferred-currency "USD"
  :fx-timing "PRE_SETTLEMENT"
  :additional-days 1)

;; --- Cross-Border: UK -> US (Bridge via Euroclear) ---
(settlement-chain.set-cross-border
  :cbu-id @allianz-fund
  :source-market "XLON"
  :target-market "XNYS"
  :settlement-method "VIA_ICSD"
  :bridge-location-id @loc-euroclear
  :preferred-currency "USD"
  :fx-timing "ON_SETTLEMENT")

;; ==============================================================================
;; SECTION 7: TAX CONFIGURATION
;; Phase 3: Withholding, treaty rates, and reclaim setup
;; ==============================================================================

;; --- Treaty Rates (Luxembourg fund investing in various jurisdictions) ---
(tax-config.set-treaty-rate :source-jurisdiction "DE" :investor-jurisdiction "LU" :income-type "DIVIDEND" :standard-rate 26.375 :treaty-rate 15.0 :effective-date "2024-01-01" :treaty-reference "DE-LU-DTA-1958")
(tax-config.set-treaty-rate :source-jurisdiction "US" :investor-jurisdiction "LU" :income-type "DIVIDEND" :standard-rate 30.0 :treaty-rate 15.0 :effective-date "2024-01-01" :treaty-reference "US-LU-DTA-1996")
(tax-config.set-treaty-rate :source-jurisdiction "US" :investor-jurisdiction "LU" :income-type "INTEREST" :standard-rate 30.0 :treaty-rate 0.0 :effective-date "2024-01-01" :treaty-reference "US-LU-DTA-1996")
(tax-config.set-treaty-rate :source-jurisdiction "JP" :investor-jurisdiction "LU" :income-type "DIVIDEND" :standard-rate 15.315 :treaty-rate 10.0 :effective-date "2024-01-01" :treaty-reference "JP-LU-DTA-2010")
(tax-config.set-treaty-rate :source-jurisdiction "CH" :investor-jurisdiction "LU" :income-type "DIVIDEND" :standard-rate 35.0 :treaty-rate 15.0 :effective-date "2024-01-01" :treaty-reference "CH-LU-DTA-1993")

;; --- CBU Tax Status (Fund's status in each source jurisdiction) ---
(tax-config.set-tax-status :cbu-id @allianz-fund :jurisdiction "DE" :investor-type "FUND" :documentation-status "VALIDATED" :applicable-treaty-rate 15.0)
(tax-config.set-tax-status :cbu-id @allianz-fund :jurisdiction "US" :investor-type "FUND" :documentation-status "VALIDATED" :applicable-treaty-rate 15.0 :qualified-intermediary true)
(tax-config.set-tax-status :cbu-id @allianz-fund :jurisdiction "GB" :investor-type "FUND" :tax-exempt true :exempt-reason "No UK WHT on dividends")
(tax-config.set-tax-status :cbu-id @allianz-fund :jurisdiction "JP" :investor-type "FUND" :documentation-status "SUBMITTED" :applicable-treaty-rate 10.0)
(tax-config.set-tax-status :cbu-id @allianz-fund :jurisdiction "HK" :investor-type "FUND" :tax-exempt true :exempt-reason "No HK WHT")
(tax-config.set-tax-status :cbu-id @allianz-fund :jurisdiction "CH" :investor-type "FUND" :documentation-status "VALIDATED" :applicable-treaty-rate 15.0)

;; --- Tax Reclaim Configuration ---
(tax-config.set-reclaim-config :cbu-id @allianz-fund :source-jurisdiction "DE" :reclaim-method "AUTOMATIC" :minimum-amount 500 :minimum-currency "EUR" :batch-frequency "MONTHLY" :expected-recovery-days 180)
(tax-config.set-reclaim-config :cbu-id @allianz-fund :source-jurisdiction "CH" :reclaim-method "OUTSOURCED" :minimum-amount 1000 :minimum-currency "CHF" :batch-frequency "QUARTERLY" :expected-recovery-days 365)
(tax-config.set-reclaim-config :cbu-id @allianz-fund :source-jurisdiction "US" :reclaim-method "AUTOMATIC" :minimum-amount 100 :minimum-currency "USD" :batch-frequency "MONTHLY" :expected-recovery-days 90)

;; --- Tax Reporting Obligations ---
(tax-config.set-reporting :cbu-id @allianz-fund :regime "FATCA" :jurisdiction "US" :status "PARTICIPATING" :giin "ALLIANZGI.99999.SL.442")
(tax-config.set-reporting :cbu-id @allianz-fund :regime "CRS" :jurisdiction "LU" :status "PARTICIPATING" :registration-date "2017-01-01")

;; ==============================================================================
;; SECTION 8: VALIDATION AND EXPORT (Phase 4)
;; ==============================================================================

;; --- Validate booking rule coverage (IMPLEMENTED) ---
(cbu-custody.validate-booking-coverage :cbu-id @allianz-fund)

;; --- Validate settlement configuration (TODO: implement plugin) ---
;; (settlement-chain.validate-settlement-config :cbu-id @allianz-fund)

;; --- Validate tax configuration (TODO: implement plugin) ---
;; (tax-config.validate-tax-config :cbu-id @allianz-fund)

;; --- Validate complete trading matrix readiness (TODO: implement plugin) ---
;; (trading-profile.validate-matrix-completeness :cbu-id @allianz-fund :validation-level "STANDARD")

;; --- Export full trading matrix (TODO: implement plugin) ---
;; (trading-profile.export-full-matrix :cbu-id @allianz-fund :format "YAML" :include-gaps true :include-dependencies true)

;; ==============================================================================
;; SECTION 9: CLEANUP VERIFICATION QUERIES
;; List what was created for verification
;; ==============================================================================

(cbu-custody.list-universe :cbu-id @allianz-fund)
(cbu-custody.list-ssis :cbu-id @allianz-fund)
(cbu-custody.list-booking-rules :cbu-id @allianz-fund)
(settlement-chain.list-chains :cbu-id @allianz-fund)
(tax-config.list-tax-status :cbu-id @allianz-fund)
(tax-config.list-reclaim-configs :cbu-id @allianz-fund)
(tax-config.list-reporting :cbu-id @allianz-fund)

;; ==============================================================================
;; END OF ALLIANZ TRADING MATRIX TEST HARNESS
;; ==============================================================================
