;; =============================================================================
;; KYC Complete Flow Test - Hedge Fund with Multi-Step UBO Discovery
;;
;; This scenario demonstrates the full KYC lifecycle:
;; 1. CBU Discovery & Initial Structure
;; 2. KYC Case Creation
;; 3. Entity Workstreams
;; 4. Document Collection & Verification
;; 5. UBO Discovery - First Iteration (Corporate Owners)
;; 6. Screenings
;; 7. Red Flags
;; 8. UBO Discovery - Second Iteration (Natural Persons)
;; 9. UBO Verification
;; 10. Case Completion
;;
;; Hedge Fund: "Atlas Global Macro Fund" (Cayman)
;; =============================================================================

;; =============================================================================
;; PART 1: CBU DISCOVERY & INITIAL STRUCTURE
;; =============================================================================

;; Create the commercial client (head office) - a holding company
(entity.create-limited-company
  :name "Atlas Capital Holdings Ltd"
  :jurisdiction "GB"
  :as @holding-company)

;; Create the hedge fund CBU
(cbu.ensure
  :name "Atlas Global Macro Fund"
  :jurisdiction "KY"
  :client-type "FUND"
  :nature-purpose "Hedge fund pursuing global macro strategies"
  :commercial-client-entity-id @holding-company
  :as @fund)

;; Create the fund legal entity (issuer of shares)
(entity.create-limited-company
  :name "Atlas Global Macro Fund LP"
  :jurisdiction "KY"
  :as @fund-entity)

;; Assign the fund entity as PRINCIPAL
(cbu.assign-role :cbu-id @fund :entity-id @fund-entity :role "PRINCIPAL")

;; Assign the holding company as COMMERCIAL_CLIENT (head office)
(cbu.assign-role :cbu-id @fund :entity-id @holding-company :role "COMMERCIAL_CLIENT")

;; =============================================================================
;; PART 2: KYC CASE CREATION
;; =============================================================================

;; Create KYC case
(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "New hedge fund onboarding - Custody + Alternatives + Collateral"
  :as @case)

;; =============================================================================
;; PART 3: CREATE ENTITY WORKSTREAMS
;; =============================================================================

;; Workstream for the holding company
(entity-workstream.create
  :case-id @case
  :entity-id @holding-company
  :discovery-reason "COMMERCIAL_CLIENT"
  :as @ws-holding)

;; Workstream for the fund entity
(entity-workstream.create
  :case-id @case
  :entity-id @fund-entity
  :discovery-reason "PRINCIPAL"
  :as @ws-fund-entity)

;; Update case to DISCOVERY phase
(kyc-case.update-status :case-id @case :status "DISCOVERY")

;; =============================================================================
;; PART 4: DOCUMENT COLLECTION & VERIFICATION
;; =============================================================================

;; Create document requests for holding company
(doc-request.create
  :workstream-id @ws-holding
  :doc-type "CERT_OF_INCORPORATION"
  :is-mandatory true
  :priority "HIGH"
  :as @doc-req-cert)

(doc-request.create
  :workstream-id @ws-holding
  :doc-type "REGISTER_OF_SHAREHOLDERS"
  :is-mandatory true
  :priority "HIGH"
  :as @doc-req-shareholders)

;; Catalog the documents first
(document.catalog
  :cbu-id @fund
  :entity-id @holding-company
  :document-type "CERT_OF_INCORPORATION"
  :as @doc-cert)

(document.catalog
  :cbu-id @fund
  :entity-id @holding-company
  :document-type "REGISTER_OF_SHAREHOLDERS"
  :as @doc-shareholders)

;; Mark documents as received
(doc-request.receive :request-id @doc-req-cert :document-id @doc-cert)
(doc-request.receive :request-id @doc-req-shareholders :document-id @doc-shareholders)

;; Verify the document requests
(doc-request.verify :request-id @doc-req-cert :verification-notes "Certificate valid, company active")
(doc-request.verify :request-id @doc-req-shareholders :verification-notes "Shareholder register complete")

;; Update workstream status
(entity-workstream.update-status :workstream-id @ws-holding :status "VERIFY")

;; =============================================================================
;; PART 5: UBO DISCOVERY - FIRST ITERATION (Corporate Owners)
;; =============================================================================

;; From shareholder register, we discover the holding company is owned by:
;; - 70% by "Oceanic Investments Pte Ltd" (Singapore)
;; - 30% by "Nordic Trust Services" (Jersey)

;; Create the discovered entities
(entity.create-limited-company
  :name "Oceanic Investments Pte Ltd"
  :jurisdiction "SG"
  :as @oceanic)

(entity.create-limited-company
  :name "Nordic Trust Services"
  :jurisdiction "JE"
  :as @nordic-trust)

;; Add ownership relationships
(ubo.add-ownership
  :owner-entity-id @oceanic
  :owned-entity-id @holding-company
  :percentage 70
  :ownership-type "DIRECT"
  :evidence-doc-id @doc-shareholders)

(ubo.add-ownership
  :owner-entity-id @nordic-trust
  :owned-entity-id @holding-company
  :percentage 30
  :ownership-type "DIRECT"
  :evidence-doc-id @doc-shareholders)

;; Link discovered entities to CBU with SHAREHOLDER role
(cbu.assign-role :cbu-id @fund :entity-id @oceanic :role "SHAREHOLDER")
(cbu.assign-role :cbu-id @fund :entity-id @nordic-trust :role "SHAREHOLDER")

;; Create workstreams for newly discovered entities
(entity-workstream.create
  :case-id @case
  :entity-id @oceanic
  :discovery-source-id @ws-holding
  :discovery-reason "SHAREHOLDER_70PCT"
  :discovery-depth 2
  :as @ws-oceanic)

(entity-workstream.create
  :case-id @case
  :entity-id @nordic-trust
  :discovery-source-id @ws-holding
  :discovery-reason "SHAREHOLDER_30PCT"
  :discovery-depth 2
  :as @ws-nordic)

;; =============================================================================
;; PART 6: RUN SCREENINGS
;; =============================================================================

;; Screen the discovered entities
(case-screening.run
  :workstream-id @ws-oceanic
  :screening-type "SANCTIONS"
  :as @screen-oceanic-sanctions)

(case-screening.run
  :workstream-id @ws-oceanic
  :screening-type "ADVERSE_MEDIA"
  :as @screen-oceanic-media)

(case-screening.run
  :workstream-id @ws-nordic
  :screening-type "SANCTIONS"
  :as @screen-nordic-sanctions)

;; Complete screenings with results
(case-screening.complete
  :screening-id @screen-oceanic-sanctions
  :status "CLEAR"
  :result-summary "No sanctions matches")

(case-screening.complete
  :screening-id @screen-oceanic-media
  :status "HIT_PENDING_REVIEW"
  :result-summary "Minor media hit - regulatory fine 2019")

(case-screening.complete
  :screening-id @screen-nordic-sanctions
  :status "CLEAR"
  :result-summary "No sanctions matches")

;; =============================================================================
;; PART 7: RAISE RED FLAGS FROM FINDINGS
;; =============================================================================

;; Raise red flag for adverse media hit
(red-flag.raise
  :case-id @case
  :workstream-id @ws-oceanic
  :flag-type "ADVERSE_MEDIA"
  :severity "SOFT"
  :description "Oceanic Investments received regulatory fine in 2019 for late filing"
  :source "SCREENING"
  :as @flag-media)

;; Raise red flag for complex structure (Jersey trust involved)
(red-flag.raise
  :case-id @case
  :workstream-id @ws-nordic
  :flag-type "COMPLEX_STRUCTURE"
  :severity "SOFT"
  :description "Ownership through Jersey trust - requires enhanced documentation"
  :source "ANALYST"
  :as @flag-structure)

;; =============================================================================
;; PART 8: UBO DISCOVERY - SECOND ITERATION (Find Natural Persons)
;; =============================================================================

;; Request additional documents for Oceanic
(doc-request.create
  :workstream-id @ws-oceanic
  :doc-type "REGISTER_OF_SHAREHOLDERS"
  :is-mandatory true)

;; Discovery: Oceanic is 100% owned by "Chen Wei" (Singapore citizen)
(entity.create-proper-person
  :first-name "Chen"
  :last-name "Wei"
  :as @chen-wei)

(ubo.add-ownership
  :owner-entity-id @chen-wei
  :owned-entity-id @oceanic
  :percentage 100
  :ownership-type "DIRECT")

;; Link Chen Wei to CBU as BENEFICIAL_OWNER
(cbu.assign-role :cbu-id @fund :entity-id @chen-wei :role "BENEFICIAL_OWNER")

;; Create workstream for discovered natural person
(entity-workstream.create
  :case-id @case
  :entity-id @chen-wei
  :discovery-source-id @ws-oceanic
  :discovery-reason "UBO_OWNER"
  :discovery-depth 3
  :is-ubo true
  :as @ws-chen)

;; Register Chen Wei as UBO (42% indirect = 70% of 60%)
(ubo.register-ubo
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :ubo-person-id @chen-wei
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 42
  :workflow-type "ONBOARDING"
  :as @ubo-chen)

;; Screen the UBO
(case-screening.run
  :workstream-id @ws-chen
  :screening-type "PEP"
  :as @screen-chen-pep)

(case-screening.run
  :workstream-id @ws-chen
  :screening-type "SANCTIONS"
  :as @screen-chen-sanctions)

;; Screenings clear
(case-screening.complete
  :screening-id @screen-chen-pep
  :status "CLEAR"
  :result-summary "Not a PEP")

(case-screening.complete
  :screening-id @screen-chen-sanctions
  :status "CLEAR"
  :result-summary "No sanctions matches")

;; =============================================================================
;; PART 9: UBO VERIFICATION
;; =============================================================================

;; Catalog passport for Chen Wei
(document.catalog
  :cbu-id @fund
  :entity-id @chen-wei
  :document-type "PASSPORT"
  :as @doc-chen-passport)

;; Verify UBO
(ubo.verify-ubo
  :ubo-id @ubo-chen
  :verification-status "VERIFIED"
  :risk-rating "LOW")

;; =============================================================================
;; PART 10: MITIGATE RED FLAGS
;; =============================================================================

;; Mitigate the adverse media flag
(red-flag.mitigate
  :red-flag-id @flag-media
  :notes "Fine was for minor administrative issue (late filing), company is in good standing")

;; Mitigate the complex structure flag
(red-flag.mitigate
  :red-flag-id @flag-structure
  :notes "Full ownership chain documented, natural person UBO identified and verified")

;; =============================================================================
;; PART 11: CASE ASSESSMENT & COMPLETION
;; =============================================================================

;; Update case to ASSESSMENT
(kyc-case.update-status :case-id @case :status "ASSESSMENT")

;; Complete workstreams
(entity-workstream.update-status :workstream-id @ws-holding :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-fund-entity :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-oceanic :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-chen :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-nordic :status "COMPLETE")

;; =============================================================================
;; PART 12: FINAL DECISION & CASE CLOSURE
;; =============================================================================

;; Update case to REVIEW
(kyc-case.update-status :case-id @case :status "REVIEW")

;; Set risk rating
(kyc-case.set-risk-rating :case-id @case :risk-rating "MEDIUM")

;; Close the case as approved
(kyc-case.close
  :case-id @case
  :status "APPROVED"
  :notes "Hedge fund onboarding complete. All UBOs identified and verified.")

;; =============================================================================
;; List UBOs for verification
;; =============================================================================
(ubo.list-ubos :cbu-id @fund)

;; =============================================================================
;; SUMMARY:
;; CBU Created: "Atlas Global Macro Fund" (Cayman hedge fund)
;; Entities:
;;   - Atlas Capital Holdings Ltd (GB) - commercial client
;;   - Atlas Global Macro Fund LP (KY) - fund entity/principal
;;   - Oceanic Investments Pte Ltd (SG) - 70% owner of Holdings
;;   - Nordic Trust Services (JE) - 30% owner of Holdings
;;   - Chen Wei - natural person, 100% owner of Oceanic
;; UBO: Chen Wei (42% indirect ownership via Oceanic -> Holdings -> Fund)
;; Case: NEW_CLIENT -> DISCOVERY -> ASSESSMENT -> REVIEW -> APPROVED
;; Red Flags: 2 raised (ADVERSE_MEDIA, COMPLEX_STRUCTURE), both mitigated
;; =============================================================================
