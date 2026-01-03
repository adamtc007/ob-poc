;; =============================================================================
;; Trust and Partnership Structure Test
;;
;; This scenario tests trust and partnership control analysis:
;; 1. Trust Structure - Settlor, trustees, beneficiaries, protector
;; 2. Partnership Structure - GP, LPs, capital contributions
;; 3. UBO Discovery through complex structures
;;
;; Test Entities:
;; - "Smith Family Trust" (Jersey discretionary trust)
;; - "Venture Capital LP" (Delaware limited partnership)
;; =============================================================================

;; =============================================================================
;; PART 1: TRUST STRUCTURE
;; =============================================================================

;; Create the trust entity
(entity.create-trust-discretionary
  :name "Smith Family Trust"
  :jurisdiction "JE"
  :trust-type "DISCRETIONARY"
  :establishment-date "2015-06-01"
  :as @trust)

;; Create trust parties
(entity.create-proper-person
  :first-name "Robert"
  :last-name "Smith"
  :date-of-birth "1955-04-12"
  :nationality "GB"
  :as @settlor)

(entity.create-limited-company
  :name "Jersey Trust Company Ltd"
  :jurisdiction "JE"
  :as @trustee-corp)

(entity.create-proper-person
  :first-name "Sarah"
  :last-name "Smith"
  :date-of-birth "1980-09-18"
  :nationality "GB"
  :as @beneficiary1)

(entity.create-proper-person
  :first-name "Thomas"
  :last-name "Smith"
  :date-of-birth "1982-12-05"
  :nationality "GB"
  :as @beneficiary2)

(entity.create-proper-person
  :first-name "Michael"
  :last-name "Johnson"
  :date-of-birth "1960-02-28"
  :nationality "GB"
  :as @protector)

;; Create trust CBU
(cbu.ensure
  :name "Smith Family Trust"
  :jurisdiction "JE"
  :client-type "TRUST"
  :as @trust-cbu)

;; Assign trust roles
(cbu.assign-role :cbu-id @trust-cbu :entity-id @trust :role "PRINCIPAL")
(cbu.assign-role :cbu-id @trust-cbu :entity-id @settlor :role "SETTLOR")
(cbu.assign-role :cbu-id @trust-cbu :entity-id @trustee-corp :role "TRUSTEE")
(cbu.assign-role :cbu-id @trust-cbu :entity-id @beneficiary1 :role "BENEFICIARY")
(cbu.assign-role :cbu-id @trust-cbu :entity-id @beneficiary2 :role "BENEFICIARY")
(cbu.assign-role :cbu-id @trust-cbu :entity-id @protector :role "PROTECTOR")

;; Create KYC case for trust
(kyc-case.create
  :cbu-id @trust-cbu
  :case-type "NEW_CLIENT"
  :notes "Discretionary family trust - Jersey"
  :as @trust-case)

;; Create workstreams
(entity-workstream.create :case-id @trust-case :entity-id @trust :as @ws-trust)
(entity-workstream.create :case-id @trust-case :entity-id @settlor :discovery-reason "SETTLOR" :is-ubo true :as @ws-settlor)
(entity-workstream.create :case-id @trust-case :entity-id @trustee-corp :discovery-reason "TRUSTEE" :as @ws-trustee)
(entity-workstream.create :case-id @trust-case :entity-id @beneficiary1 :discovery-reason "NAMED_BENEFICIARY" :is-ubo true :as @ws-ben1)
(entity-workstream.create :case-id @trust-case :entity-id @beneficiary2 :discovery-reason "NAMED_BENEFICIARY" :is-ubo true :as @ws-ben2)
(entity-workstream.create :case-id @trust-case :entity-id @protector :discovery-reason "PROTECTOR" :is-ubo true :as @ws-protector)

;; Run screenings on trust parties
(case-screening.run :workstream-id @ws-settlor :screening-type "PEP")
(case-screening.run :workstream-id @ws-settlor :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-ben1 :screening-type "PEP")
(case-screening.run :workstream-id @ws-ben1 :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-ben2 :screening-type "PEP")
(case-screening.run :workstream-id @ws-ben2 :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-protector :screening-type "PEP")
(case-screening.run :workstream-id @ws-protector :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-trustee :screening-type "SANCTIONS")

;; =============================================================================
;; PART 2: PARTNERSHIP STRUCTURE
;; =============================================================================

;; Create the partnership entity
(entity.create-partnership-limited
  :name "Venture Capital LP"
  :jurisdiction "US"
  :partnership-type "LP"
  :formation-date "2020-01-15"
  :as @partnership)

;; Create GP entity
(entity.create-limited-company
  :name "Venture Capital GP LLC"
  :jurisdiction "US"
  :as @gp)

;; Create LP investors
(entity.create-limited-company
  :name "Pension Fund Alpha"
  :jurisdiction "US"
  :as @lp1)

(entity.create-limited-company
  :name "Endowment Fund Beta"
  :jurisdiction "US"
  :as @lp2)

(entity.create-proper-person
  :first-name "Elizabeth"
  :last-name "Warren"
  :date-of-birth "1970-07-14"
  :nationality "US"
  :as @lp3-person)

;; Create partnership CBU
(cbu.ensure
  :name "Venture Capital LP"
  :jurisdiction "US"
  :client-type "FUND"
  :as @partnership-cbu)

;; Assign partnership roles
(cbu.assign-role :cbu-id @partnership-cbu :entity-id @partnership :role "PRINCIPAL")
(cbu.assign-role :cbu-id @partnership-cbu :entity-id @gp :role "GENERAL_PARTNER")
(cbu.assign-role :cbu-id @partnership-cbu :entity-id @lp1 :role "LIMITED_PARTNER")
(cbu.assign-role :cbu-id @partnership-cbu :entity-id @lp2 :role "LIMITED_PARTNER")
(cbu.assign-role :cbu-id @partnership-cbu :entity-id @lp3-person :role "LIMITED_PARTNER")

;; Create KYC case for partnership
(kyc-case.create
  :cbu-id @partnership-cbu
  :case-type "NEW_CLIENT"
  :notes "Venture capital limited partnership - Delaware"
  :as @partnership-case)

;; Create workstreams
(entity-workstream.create :case-id @partnership-case :entity-id @partnership :as @ws-partnership)
(entity-workstream.create :case-id @partnership-case :entity-id @gp :discovery-reason "GENERAL_PARTNER" :as @ws-gp)
(entity-workstream.create :case-id @partnership-case :entity-id @lp1 :discovery-reason "LP_OVER_25PCT" :as @ws-lp1)
(entity-workstream.create :case-id @partnership-case :entity-id @lp2 :discovery-reason "LP_OVER_25PCT" :as @ws-lp2)
(entity-workstream.create :case-id @partnership-case :entity-id @lp3-person :discovery-reason "LP_NATURAL_PERSON" :is-ubo true :as @ws-lp3)

;; Run screenings
(case-screening.run :workstream-id @ws-partnership :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-gp :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-lp1 :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-lp2 :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-lp3 :screening-type "PEP")
(case-screening.run :workstream-id @ws-lp3 :screening-type "SANCTIONS")

;; =============================================================================
;; PART 3: UBO REGISTRATION FOR TRUST
;; =============================================================================

;; Register trust UBOs - all parties with control/benefit
(ubo.register-ubo
  :cbu-id @trust-cbu
  :subject-entity-id @trust
  :ubo-person-id @settlor
  :relationship-type "SETTLOR"
  :qualifying-reason "TRUST_SETTLOR"
  :workflow-type "ONBOARDING"
  :as @ubo-settlor)

(ubo.register-ubo
  :cbu-id @trust-cbu
  :subject-entity-id @trust
  :ubo-person-id @beneficiary1
  :relationship-type "BENEFICIARY"
  :qualifying-reason "TRUST_BENEFICIARY"
  :workflow-type "ONBOARDING"
  :as @ubo-ben1)

(ubo.register-ubo
  :cbu-id @trust-cbu
  :subject-entity-id @trust
  :ubo-person-id @beneficiary2
  :relationship-type "BENEFICIARY"
  :qualifying-reason "TRUST_BENEFICIARY"
  :workflow-type "ONBOARDING"
  :as @ubo-ben2)

(ubo.register-ubo
  :cbu-id @trust-cbu
  :subject-entity-id @trust
  :ubo-person-id @protector
  :relationship-type "CONTROL"
  :qualifying-reason "TRUST_PROTECTOR"
  :workflow-type "ONBOARDING"
  :as @ubo-protector)

;; =============================================================================
;; PART 4: UBO REGISTRATION FOR PARTNERSHIP
;; =============================================================================

;; The GP has control, LP with >25% has ownership UBO status
(ubo.register-ubo
  :cbu-id @partnership-cbu
  :subject-entity-id @partnership
  :ubo-person-id @lp3-person
  :relationship-type "OWNER"
  :qualifying-reason "PARTNERSHIP_INTEREST"
  :ownership-percentage 30.00
  :workflow-type "ONBOARDING"
  :as @ubo-lp3)

;; =============================================================================
;; END OF TEST SCENARIO
;; =============================================================================

;; Summary:
;; Trust Structure:
;; - Created discretionary trust with full party structure
;; - Settlor, corporate trustee, 2 beneficiaries, protector
;; - All natural person parties registered as UBOs
;;
;; Partnership Structure:
;; - Created LP with GP and 3 LPs (2 institutional, 1 natural person)
;; - GP has control, LP natural person registered as UBO
;;
;; Note: The trust.analyze-control and partnership.analyze-control verbs
;; require the database feature to perform control vector analysis.
