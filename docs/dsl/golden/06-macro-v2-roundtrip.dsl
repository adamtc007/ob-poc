;; ============================================================================
;; Macro V2 Roundtrip
;; ============================================================================
;; intent: Demonstrate macro expansion from operator vocabulary to DSL
;;
;; Macros map business-friendly operator terms to technical DSL verbs.
;; This file shows the operator input and the expanded output.

;; ============================================================================
;; Example 1: Structure Setup
;; ============================================================================

;; OPERATOR INPUT:
;; intent: Set up a PE fund structure
;; macro: structure.setup
;;
;; UI shows: "Set up Structure"
;; Operator selects: type=PE, name="Blackstone Growth Fund"

;; EXPANDED DSL:
(cbu.create
  :name "Blackstone Growth Fund"
  :type "FUND"
  :kind "private-equity"
  :jurisdiction "LU"
  :as @structure)

;; ============================================================================
;; Example 2: Party Creation with Role
;; ============================================================================

;; OPERATOR INPUT:
;; intent: Add a General Partner to the structure
;; macro: party.add-to-structure
;;
;; UI shows: "Add Party to Structure"
;; Operator provides: name="BX GP LLC", role=GP

;; EXPANDED DSL (two statements):
(entity.create
  :name "BX GP LLC"
  :type "LEGAL"
  :jurisdiction "DE"
  :as @party)

(cbu-role.assign
  :cbu-id @structure
  :entity-id @party
  :role "GENERAL_PARTNER"
  :effective-date "2024-01-01")

;; ============================================================================
;; Example 3: Case Management
;; ============================================================================

;; OPERATOR INPUT:
;; intent: Open a KYC case for the new investor
;; macro: case.open
;;
;; UI shows: "Open Case"
;; Operator provides: type=investor_onboarding, subject=@party

;; EXPANDED DSL:
(kyc-case.create
  :name "BX GP LLC - Investor Onboarding"
  :type "INVESTOR_ONBOARDING"
  :cbu-id @structure
  :as @case)

(kyc-case.add-subject
  :case-id @case
  :entity-id @party
  :role "PRIMARY")

;; ============================================================================
;; Example 4: Mandate Setup
;; ============================================================================

;; OPERATOR INPUT:
;; intent: Create an investment mandate for the fund
;; macro: mandate.setup
;;
;; UI shows: "Set up Mandate"
;; Operator provides: strategy=growth, instruments=[equity, fixed_income]

;; EXPANDED DSL:
(trading-profile.create
  :cbu-id @structure
  :name "Growth Mandate"
  :strategy "GROWTH"
  :as @mandate)

(trading-profile.add-instrument :profile-id @mandate :instrument "EQUITY")
(trading-profile.add-instrument :profile-id @mandate :instrument "FIXED_INCOME")

;; ============================================================================
;; Example 5: Research and Bulk Onboard
;; ============================================================================

;; OPERATOR INPUT:
;; intent: Research Allianz group and onboard their Luxembourg funds
;; macro: onboarding.research-group
;;
;; UI shows: "Research & Onboard Group"
;; Operator provides: root-entity=<Allianz SE>, jurisdiction=LU

;; EXPANDED DSL:
(gleif.import-tree
  :entity-id "550e8400-e29b-41d4-a716-446655440000"
  :direction "BOTH"
  :depth 3
  :as @import)

(cbu.create-from-client-group
  :group-id "550e8400-e29b-41d4-a716-446655440001"
  :gleif-category "FUND"
  :jurisdiction-filter "LU"
  :as @created_structures)

;; ============================================================================
;; Vocabulary Translation Reference
;; ============================================================================
;;
;; | Operator Term | Internal Term        | Notes                    |
;; |---------------|----------------------|--------------------------|
;; | structure     | cbu                  | Fund/vehicle             |
;; | party         | entity               | Person or organization   |
;; | case          | kyc-case             | Compliance case          |
;; | mandate       | trading-profile      | Investment mandate       |
;; | GP            | GENERAL_PARTNER      | PE fund role             |
;; | IM            | INVESTMENT_MANAGER   | Asset manager role       |
;; | ManCo         | MANAGEMENT_COMPANY   | Fund manager role        |
;;
;; The macro system ensures operators never see internal implementation
;; details like "cbu", "entity_ref", or "trading-profile".
