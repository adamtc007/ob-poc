;; ==============================================================================
;; DOCUMENT-CENTRIC TRADING PROFILE TEST
;; ==============================================================================
;;
;; This test validates the document-centric trading profile workflow:
;;   Phase 1-3: Document construction verbs (add-instrument-class, add-market, etc.)
;;   Phase 4: Sync verbs (sync-to-operational)
;;   Phase 5: Validation verbs (validate-go-live-ready, validate-coverage)
;;   Phase 6: Lifecycle verbs (submit, approve)
;;
;; The workflow is:
;;   1. Create draft profile document
;;   2. Add trading configuration to the document
;;   3. Validate the document
;;   4. Submit for review
;;   5. Approve (syncs to operational tables automatically)
;;   6. Verify Trading Matrix API returns the data
;; ==============================================================================

;; ==============================================================================
;; SECTION 0: REFERENCE DATA PREREQUISITES
;; These must exist before creating the trading profile
;; ==============================================================================

;; --- Instrument Classes ---
(instrument-class.ensure
  :code "EQUITY"
  :name "Equity Securities"
  :settlement-cycle "T+2"
  :swift-family "MT54x"
  :cfi-category "E"
  :smpg-group "EQUITIES")

(instrument-class.ensure
  :code "GOVT_BOND"
  :name "Government Bonds"
  :settlement-cycle "T+1"
  :swift-family "MT54x"
  :cfi-category "D"
  :smpg-group "FIXED_INCOME")

;; --- Markets ---
(market.ensure :mic "XETR" :name "Deutsche Boerse Xetra" :country-code "DE" :primary-currency "EUR" :csd-bic "DAABORDC" :timezone "Europe/Berlin")
(market.ensure :mic "XLON" :name "London Stock Exchange" :country-code "GB" :primary-currency "GBP" :csd-bic "CRESTGB2" :timezone "Europe/London")
(market.ensure :mic "XNYS" :name "New York Stock Exchange" :country-code "US" :primary-currency "USD" :csd-bic "DTCYUS33" :timezone "America/New_York")

;; ==============================================================================
;; SECTION 1: CREATE CBU AND DRAFT PROFILE
;; ==============================================================================

;; Create test CBU for the trading profile
(cbu.ensure
  :name "Document Test Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :as @test-cbu)

;; Create a draft trading profile document
(trading-profile.create-draft
  :cbu-id @test-cbu
  :base-currency "EUR"
  :notes "Document-centric workflow test"
  :as @profile)

;; ==============================================================================
;; SECTION 2: ADD TRADING UNIVERSE TO DOCUMENT
;; These verbs modify the JSONB document, not operational tables
;; ==============================================================================

;; Add equity instrument class
(trading-profile.add-instrument-class
  :profile-id @profile
  :class-code "EQUITY"
  :is-held true
  :is-traded true)

;; Add German equities market
(trading-profile.add-market
  :profile-id @profile
  :mic "XETR"
  :currencies ["EUR"]
  :settlement-types ["DVP"])

;; Add UK equities with cross-currency
(trading-profile.add-market
  :profile-id @profile
  :mic "XLON"
  :currencies ["GBP" "EUR"]
  :settlement-types ["DVP"])

;; Add US equities
(trading-profile.add-market
  :profile-id @profile
  :mic "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

;; Add government bonds instrument class
(trading-profile.add-instrument-class
  :profile-id @profile
  :class-code "GOVT_BOND"
  :is-held true
  :is-traded true)

;; ==============================================================================
;; SECTION 3: ADD SSIs TO DOCUMENT
;; Standing Settlement Instructions for each market
;; ==============================================================================

;; German SSI
(trading-profile.add-standing-instruction
  :profile-id @profile
  :category "SECURITIES"
  :name "DE Equity SSI"
  :mic "XETR"
  :currency "EUR"
  :custody-account "DE-SAFE-001"
  :custody-bic "DAABORDC"
  :cash-account "DE-CASH-001"
  :cash-bic "COBADEFF"
  :settlement-model "DVP")

;; UK SSI
(trading-profile.add-standing-instruction
  :profile-id @profile
  :category "SECURITIES"
  :name "UK Equity SSI"
  :mic "XLON"
  :currency "GBP"
  :custody-account "UK-SAFE-001"
  :custody-bic "CRSTGB22"
  :cash-account "UK-CASH-001"
  :cash-bic "BABOROCP"
  :settlement-model "DVP")

;; US SSI
(trading-profile.add-standing-instruction
  :profile-id @profile
  :category "SECURITIES"
  :name "US Equity SSI"
  :mic "XNYS"
  :currency "USD"
  :custody-account "US-SAFE-001"
  :custody-bic "DTCYUS33"
  :cash-account "US-CASH-001"
  :cash-bic "CITIUS33"
  :settlement-model "DVP")

;; ==============================================================================
;; SECTION 4: ADD BOOKING RULES TO DOCUMENT
;; ALERT-style routing: trade characteristics -> SSI
;; ==============================================================================

;; German equity booking rule
(trading-profile.add-booking-rule
  :profile-id @profile
  :name "DE Equity DVP"
  :ssi-ref "DE Equity SSI"
  :priority 10
  :match-instrument-class "EQUITY"
  :match-mic "XETR"
  :match-currency "EUR"
  :match-settlement-type "DVP")

;; UK equity booking rules (two currencies)
(trading-profile.add-booking-rule
  :profile-id @profile
  :name "UK Equity GBP DVP"
  :ssi-ref "UK Equity SSI"
  :priority 10
  :match-instrument-class "EQUITY"
  :match-mic "XLON"
  :match-currency "GBP"
  :match-settlement-type "DVP")

(trading-profile.add-booking-rule
  :profile-id @profile
  :name "UK Equity EUR DVP"
  :ssi-ref "UK Equity SSI"
  :priority 10
  :match-instrument-class "EQUITY"
  :match-mic "XLON"
  :match-currency "EUR"
  :match-settlement-type "DVP")

;; US equity booking rule
(trading-profile.add-booking-rule
  :profile-id @profile
  :name "US Equity DVP"
  :ssi-ref "US Equity SSI"
  :priority 10
  :match-instrument-class "EQUITY"
  :match-mic "XNYS"
  :match-currency "USD"
  :match-settlement-type "DVP")

;; Govt bond booking rule
(trading-profile.add-booking-rule
  :profile-id @profile
  :name "DE Govt Bond DVP"
  :ssi-ref "DE Equity SSI"
  :priority 10
  :match-instrument-class "GOVT_BOND"
  :match-mic "XETR"
  :match-currency "EUR"
  :match-settlement-type "DVP")

;; Fallback rules (lower priority, less specific)
(trading-profile.add-booking-rule
  :profile-id @profile
  :name "EUR Fallback"
  :ssi-ref "DE Equity SSI"
  :priority 100
  :match-currency "EUR")

(trading-profile.add-booking-rule
  :profile-id @profile
  :name "GBP Fallback"
  :ssi-ref "UK Equity SSI"
  :priority 100
  :match-currency "GBP")

(trading-profile.add-booking-rule
  :profile-id @profile
  :name "USD Fallback"
  :ssi-ref "US Equity SSI"
  :priority 100
  :match-currency "USD")

;; ==============================================================================
;; SECTION 5: VALIDATE THE DOCUMENT
;; Check completeness before submission
;; ==============================================================================

;; Validate coverage - ensure all universe entries have booking rules
(trading-profile.validate-universe-coverage
  :profile-id @profile)

;; Validate go-live readiness
(trading-profile.validate-go-live-ready
  :profile-id @profile)

;; ==============================================================================
;; SECTION 6: DOCUMENT LIFECYCLE
;; Submit for review and approve
;; ==============================================================================

;; Submit the profile for review
(trading-profile.submit
  :profile-id @profile
  :submitted-by "test-harness"
  :notes "Automated test submission")

;; Approve the profile (this also syncs to operational tables)
(trading-profile.approve
  :profile-id @profile
  :approved-by "test-approver"
  :notes "Automated test approval")

;; ==============================================================================
;; SECTION 7: VERIFY OPERATIONAL DATA
;; Check that sync created the expected records
;; ==============================================================================

;; List the universe to verify sync worked
(cbu-custody.list-universe :cbu-id @test-cbu)

;; List SSIs
(cbu-custody.list-ssis :cbu-id @test-cbu)

;; List booking rules
(cbu-custody.list-booking-rules :cbu-id @test-cbu)

;; Validate booking coverage in operational tables
(cbu-custody.validate-booking-coverage :cbu-id @test-cbu)

;; ==============================================================================
;; TEST COMPLETE
;; If we get here without errors, the document-centric workflow is functional
;; ==============================================================================
