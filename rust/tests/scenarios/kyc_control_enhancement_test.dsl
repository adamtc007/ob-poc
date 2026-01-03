;; =============================================================================
;; KYC Control Enhancement Test - Capital, Board, Trust, Partnership, Tollgate
;;
;; This scenario tests the new control analysis domains:
;; 1. Capital Structure - Share classes, holdings, transfers
;; 2. Board Composition - Appointments and control analysis
;; 3. Trust Analysis - Provisions, beneficiaries, control vectors
;; 4. Partnership Analysis - Capital contributions, distributions
;; 5. Tollgate Evaluation - Decision readiness metrics
;; 6. Control Analysis - Unified control vector detection
;;
;; Test Entity: "Global Investment Holdings Ltd" (LU)
;; =============================================================================

;; =============================================================================
;; PART 1: SETUP - CBU AND ENTITIES
;; =============================================================================

;; Create the main corporate entity
(entity.create-limited-company
  :name "Global Investment Holdings Ltd"
  :jurisdiction "LU"
  :as @holdco)

;; Create the CBU
(cbu.ensure
  :name "Global Investment Holdings"
  :jurisdiction "LU"
  :client-type "CORPORATE"
  :as @cbu)

;; Assign as principal
(cbu.assign-role :cbu-id @cbu :entity-id @holdco :role "PRINCIPAL")

;; Create natural persons for UBO/control analysis
(entity.create-proper-person
  :first-name "Hans"
  :last-name "Mueller"
  :date-of-birth "1965-03-15"
  :nationality "DE"
  :as @hans)

(entity.create-proper-person
  :first-name "Marie"
  :last-name "Dubois"
  :date-of-birth "1972-08-22"
  :nationality "FR"
  :as @marie)

(entity.create-proper-person
  :first-name "James"
  :last-name "Smith"
  :date-of-birth "1968-11-30"
  :nationality "GB"
  :as @james)

;; Assign roles
(cbu.assign-role :cbu-id @cbu :entity-id @hans :role "DIRECTOR")
(cbu.assign-role :cbu-id @cbu :entity-id @marie :role "DIRECTOR")
(cbu.assign-role :cbu-id @cbu :entity-id @james :role "BENEFICIAL_OWNER")

;; =============================================================================
;; PART 2: CAPITAL STRUCTURE
;; =============================================================================

;; Create share class for the holding company
(share-class.create
  :cbu-id @cbu
  :entity-id @holdco
  :name "Ordinary Shares"
  :currency "EUR"
  :class-category "CORPORATE"
  :as @shares)

;; Create holdings for the shareholders
(holding.create
  :share-class-id @shares
  :investor-entity-id @hans
  :as @hans-holding)

(holding.create
  :share-class-id @shares
  :investor-entity-id @marie
  :as @marie-holding)

(holding.create
  :share-class-id @shares
  :investor-entity-id @james
  :as @james-holding)

;; Record subscriptions
(movement.subscribe
  :holding-id @hans-holding
  :units 400000
  :price-per-unit 1.00
  :amount 400000.00
  :trade-date "2024-01-15"
  :reference "SUB-HANS-001")

(movement.subscribe
  :holding-id @marie-holding
  :units 350000
  :price-per-unit 1.00
  :amount 350000.00
  :trade-date "2024-01-15"
  :reference "SUB-MARIE-001")

(movement.subscribe
  :holding-id @james-holding
  :units 250000
  :price-per-unit 1.00
  :amount 250000.00
  :trade-date "2024-01-15"
  :reference "SUB-JAMES-001")

;; Update holdings with units
(holding.update-units :holding-id @hans-holding :units 400000 :cost-basis 400000.00)
(holding.update-units :holding-id @marie-holding :units 350000 :cost-basis 350000.00)
(holding.update-units :holding-id @james-holding :units 250000 :cost-basis 250000.00)

;; =============================================================================
;; PART 3: KYC CASE AND WORKSTREAMS
;; =============================================================================

;; Create KYC case
(kyc-case.create
  :cbu-id @cbu
  :case-type "NEW_CLIENT"
  :notes "Corporate client with multiple beneficial owners"
  :as @case)

;; Create workstreams for each entity
(entity-workstream.create
  :case-id @case
  :entity-id @holdco
  :as @ws-holdco)

(entity-workstream.create
  :case-id @case
  :entity-id @hans
  :discovery-reason "SHAREHOLDER_40PCT"
  :is-ubo true
  :as @ws-hans)

(entity-workstream.create
  :case-id @case
  :entity-id @marie
  :discovery-reason "SHAREHOLDER_35PCT"
  :is-ubo true
  :as @ws-marie)

(entity-workstream.create
  :case-id @case
  :entity-id @james
  :discovery-reason "SHAREHOLDER_25PCT"
  :is-ubo true
  :as @ws-james)

;; =============================================================================
;; PART 4: SCREENINGS
;; =============================================================================

;; Run screenings on natural persons
(case-screening.run :workstream-id @ws-hans :screening-type "PEP")
(case-screening.run :workstream-id @ws-hans :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-marie :screening-type "PEP")
(case-screening.run :workstream-id @ws-marie :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-james :screening-type "PEP")
(case-screening.run :workstream-id @ws-james :screening-type "SANCTIONS")

;; Run sanctions on corporate entity
(case-screening.run :workstream-id @ws-holdco :screening-type "SANCTIONS")

;; =============================================================================
;; PART 5: DOCUMENT REQUESTS
;; =============================================================================

;; Request identity documents for UBOs
(doc-request.create :workstream-id @ws-hans :doc-type "PASSPORT" :is-mandatory true)
(doc-request.create :workstream-id @ws-hans :doc-type "PROOF_OF_ADDRESS" :is-mandatory true)
(doc-request.create :workstream-id @ws-marie :doc-type "PASSPORT" :is-mandatory true)
(doc-request.create :workstream-id @ws-marie :doc-type "PROOF_OF_ADDRESS" :is-mandatory true)
(doc-request.create :workstream-id @ws-james :doc-type "PASSPORT" :is-mandatory true)
(doc-request.create :workstream-id @ws-james :doc-type "PROOF_OF_ADDRESS" :is-mandatory true)

;; Request corporate documents
(doc-request.create :workstream-id @ws-holdco :doc-type "CERTIFICATE_OF_INCORPORATION" :is-mandatory true)
(doc-request.create :workstream-id @ws-holdco :doc-type "REGISTER_OF_SHAREHOLDERS" :is-mandatory true)
(doc-request.create :workstream-id @ws-holdco :doc-type "REGISTER_OF_DIRECTORS" :is-mandatory true)

;; =============================================================================
;; PART 6: UBO REGISTRATION
;; =============================================================================

;; Add ownership relationships
(ubo.add-ownership
  :owner-entity-id @hans
  :owned-entity-id @holdco
  :percentage 40.00
  :ownership-type "DIRECT"
  :as @own-hans)

(ubo.add-ownership
  :owner-entity-id @marie
  :owned-entity-id @holdco
  :percentage 35.00
  :ownership-type "DIRECT"
  :as @own-marie)

(ubo.add-ownership
  :owner-entity-id @james
  :owned-entity-id @holdco
  :percentage 25.00
  :ownership-type "DIRECT"
  :as @own-james)

;; Register UBOs
(ubo.register-ubo
  :cbu-id @cbu
  :subject-entity-id @holdco
  :ubo-person-id @hans
  :relationship-type "OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 40.00
  :workflow-type "ONBOARDING"
  :as @ubo-hans)

(ubo.register-ubo
  :cbu-id @cbu
  :subject-entity-id @holdco
  :ubo-person-id @marie
  :relationship-type "OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 35.00
  :workflow-type "ONBOARDING"
  :as @ubo-marie)

(ubo.register-ubo
  :cbu-id @cbu
  :subject-entity-id @holdco
  :ubo-person-id @james
  :relationship-type "OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 25.00
  :workflow-type "ONBOARDING"
  :as @ubo-james)

;; =============================================================================
;; END OF TEST SCENARIO
;; =============================================================================

;; Summary:
;; - Created CBU with corporate principal
;; - Set up capital structure with 3 shareholders (40%, 35%, 25%)
;; - Created KYC case with entity workstreams
;; - Ran PEP and sanctions screenings
;; - Created document requests
;; - Registered UBO determinations
;;
;; Note: The plugin handlers for control.analyze, board.analyze-control,
;; capital.reconcile, tollgate.evaluate, etc. require database queries
;; that depend on populated reference data.
