;; =============================================================================
;; Trading Profile Lifecycle Test
;;
;; Tests the document-centric trading profile workflow:
;; 1. Create/ensure Allianz CBU
;; 2. Import trading profile from YAML (single source of truth)
;; 3. Validate the profile
;; 4. Activate the profile
;; 5. Materialize to operational tables (SSIs, booking rules, universe)
;; 6. Verify materialization results
;;
;; Uses DSL verbs only - no direct database access
;; =============================================================================

;; =============================================================================
;; STEP 1: Create the CBU
;; =============================================================================

;; Ensure the Allianz fund CBU exists
(cbu.ensure
  :name "AllianzGI Global Multi-Asset Fund"
  :jurisdiction "LU"
  :client-type "FUND"
  :as @allianz-cbu)

;; =============================================================================
;; STEP 2: Import Trading Profile from YAML
;;
;; The trading profile YAML is the SINGLE SOURCE OF TRUTH containing:
;; - Universe (what can be traded: markets, currencies, instrument classes)
;; - Investment Managers (who trades what, with priority and scope)
;; - ISDA Agreements (OTC derivatives framework with counterparties)
;; - Settlement Config (matching platforms, subcustodian network)
;; - Booking Rules (ALERT-style SSI selection logic)
;; - Standing Instructions (the actual SSI account data)
;; - Pricing Matrix (valuation sources by instrument type)
;; =============================================================================

(trading-profile.import
  :cbu-id @allianz-cbu
  :file-path "config/seed/trading_profiles/allianzgi_complete.yaml"
  :version 1
  :status "DRAFT"
  :notes "Initial import from seed data"
  :as @profile)

;; =============================================================================
;; STEP 3: Validate the Profile (optional but recommended)
;;
;; Validates:
;; - SSI references in booking rules exist in standing_instructions
;; - Collateral SSI refs in CSA exist in OTC_COLLATERAL section
;; - Entity refs (LEIs) can be resolved
;; - Required fields are present
;; =============================================================================

(trading-profile.validate
  :file-path "config/seed/trading_profiles/allianzgi_complete.yaml")

;; =============================================================================
;; STEP 4: Activate the Profile
;;
;; - Sets status from DRAFT to ACTIVE
;; - Supersedes any previous active profile
;; - Records activated_by and activated_at
;; =============================================================================

(trading-profile.activate
  :profile-id @profile
  :activated-by "test-harness")

;; =============================================================================
;; STEP 5: Materialize to Operational Tables
;;
;; Converts the YAML document to operational database records:
;; - custody.cbu_instrument_universe (from universe section)
;; - custody.cbu_ssi (from standing_instructions section)
;; - custody.ssi_booking_rules (from booking_rules section)
;; - custody.isda_agreements + csa_agreements (from isda_agreements section)
;;
;; This is idempotent - running twice produces the same result
;; =============================================================================

;; First, do a dry run to see what would be created
(trading-profile.materialize
  :profile-id @profile
  :dry-run true)

;; Now actually materialize all sections
(trading-profile.materialize
  :profile-id @profile
  :force false)

;; =============================================================================
;; STEP 6: Verify using read operations
;; =============================================================================

;; Read back the active profile
(trading-profile.get-active
  :cbu-id @allianz-cbu)

;; List universe entries for the CBU
(cbu-custody.list-universe
  :cbu-id @allianz-cbu)

;; List SSIs for the CBU
(cbu-custody.list-ssis
  :cbu-id @allianz-cbu)

;; List booking rules for the CBU
(cbu-custody.list-booking-rules
  :cbu-id @allianz-cbu)

;; Validate that booking rules cover the universe
(cbu-custody.validate-booking-coverage
  :cbu-id @allianz-cbu)

;; Test SSI lookup for a specific trade
(cbu-custody.lookup-ssi
  :cbu-id @allianz-cbu
  :instrument-class "EQUITY"
  :market "XETR"
  :currency "EUR"
  :settlement-type "DVP")

;; Test SSI lookup for US equities
(cbu-custody.lookup-ssi
  :cbu-id @allianz-cbu
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP")

;; Test SSI lookup for government bonds
(cbu-custody.lookup-ssi
  :cbu-id @allianz-cbu
  :instrument-class "GOVT_BOND"
  :currency "EUR"
  :settlement-type "DVP")

;; =============================================================================
;; STEP 7: Test version management
;; =============================================================================

;; List all profile versions
(trading-profile.list-versions
  :cbu-id @allianz-cbu)

;; Export the profile back to YAML (verify round-trip)
(trading-profile.export
  :profile-id @profile)
