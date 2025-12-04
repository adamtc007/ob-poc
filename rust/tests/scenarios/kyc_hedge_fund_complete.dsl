;; =============================================================================
;; KYC Complete Flow Test - Hedge Fund with Custody, Alternatives, Collateral
;;
;; This scenario demonstrates the full KYC lifecycle:
;; 1. CBU Discovery & Initial Allegation
;; 2. KYC Case Creation
;; 3. UBO Discovery (minimal ownership, no natural persons initially)
;; 4. Threshold-Based Document Requirements
;; 5. Evidence Collection & Verification
;; 6. UBO Proof Iterations (discover -> assert -> prove)
;; 7. CBU Changes During Case (new UBO discovered)
;; 8. Red-Flag Aggregation & Decision
;; 9. Case Closure
;;
;; Hedge Fund: "Atlas Global Macro Fund" (Cayman)
;; Products: Custody, Alternatives, Collateral Management
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

;; CBU starts in DISCOVERED status - set to VALIDATION_PENDING
(cbu.set-status :cbu-id @fund :status "VALIDATION_PENDING")

;; Create the fund legal entity (issuer of shares)
(entity.create-limited-company
  :name "Atlas Global Macro Fund LP"
  :jurisdiction "KY"
  :as @fund-entity)

;; Assign the fund entity as PRINCIPAL
(cbu.assign-role :cbu-id @fund :entity-id @fund-entity :role "PRINCIPAL")

;; =============================================================================
;; PART 2: INITIAL ALLEGATIONS (Client Claims - Unverified)
;; =============================================================================

;; Create KYC case first (needed for allegation tracking)
(kyc-case.create
  :cbu-id @fund
  :case-type "NEW_CLIENT"
  :notes "New hedge fund onboarding - Custody + Alternatives + Collateral"
  :as @case)

;; Record initial client allegation about ownership
;; "Client says: Fund is owned 60% by Atlas Capital Holdings Ltd"
(allegation.record
  :cbu-id @fund
  :entity-id @fund-entity
  :attribute-id "attr.ownership.parent_company"
  :value {"owner_name": "Atlas Capital Holdings Ltd", "percentage": 60}
  :display-value "60% owned by Atlas Capital Holdings Ltd"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @alleg-ownership)

;; "Client says: Remaining 40% held by institutional investors"
(allegation.record
  :cbu-id @fund
  :entity-id @fund-entity
  :attribute-id "attr.ownership.other_shareholders"
  :value {"description": "Institutional investors", "percentage": 40}
  :display-value "40% institutional investors"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @alleg-other)

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
;; PART 4: THRESHOLD EVALUATION & RFI GENERATION
;; =============================================================================

;; Derive risk band for the holding company (60% owner)
(threshold.derive :cbu-id @fund :entity-id @holding-company :as @risk-holding)

;; Derive risk band for the fund entity
(threshold.derive :cbu-id @fund :entity-id @fund-entity :as @risk-fund)

;; Generate document requests based on threshold requirements
;; This creates doc_request records in kyc.doc_requests
(rfi.generate :case-id @case :risk-band "MEDIUM" :as @rfi-batch)

;; List what was generated
(rfi.list-by-case :case-id @case)

;; =============================================================================
;; PART 5: DOCUMENT COLLECTION & VERIFICATION
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

;; Simulate: Documents received
(doc-request.receive :request-id @doc-req-cert :document-id @doc-cert-placeholder)
(doc-request.receive :request-id @doc-req-shareholders :document-id @doc-shareholders-placeholder)

;; Catalog the documents
(document.catalog
  :cbu-id @fund
  :doc-type "CERT_OF_INCORPORATION"
  :title "Atlas Capital Holdings - Certificate of Incorporation"
  :as @doc-cert)

(document.catalog
  :cbu-id @fund
  :doc-type "REGISTER_OF_SHAREHOLDERS"
  :title "Atlas Capital Holdings - Register of Shareholders"
  :as @doc-shareholders)

;; Verify the document requests
(doc-request.verify :request-id @doc-req-cert :verification-notes "Certificate valid, company active")
(doc-request.verify :request-id @doc-req-shareholders :verification-notes "Shareholder register complete")

;; Update workstream status
(entity-workstream.update-status :workstream-id @ws-holding :status "VERIFY")

;; =============================================================================
;; PART 6: UBO DISCOVERY - FIRST ITERATION
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
  :evidence-doc-id @doc-shareholders
  :as @own-oceanic)

(ubo.add-ownership
  :owner-entity-id @nordic-trust
  :owned-entity-id @holding-company
  :percentage 30
  :ownership-type "DIRECT"
  :evidence-doc-id @doc-shareholders
  :as @own-nordic)

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

;; Assert suspected UBOs (we don't yet know the natural persons)
;; These are corporate entities that WILL have UBOs behind them
(ubo.assert
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :ubo-person-id @oceanic
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 42
  :workflow-type "ONBOARDING"
  :case-id @case
  :workstream-id @ws-oceanic
  :discovery-method "DOCUMENT"
  :as @ubo-oceanic-suspected)

;; =============================================================================
;; PART 7: RUN SCREENINGS
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
;; PART 8: RAISE RED FLAGS FROM FINDINGS
;; =============================================================================

;; Raise red flag for adverse media hit
(red-flag.raise
  :case-id @case
  :workstream-id @ws-oceanic
  :flag-type "ADVERSE_MEDIA"
  :severity "SOFT"
  :description "Oceanic Investments received regulatory fine in 2019 for late filing"
  :source "SCREENING"
  :source-reference @screen-oceanic-media
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
;; PART 9: UBO DISCOVERY - SECOND ITERATION (Find Natural Persons)
;; =============================================================================

;; Request additional documents for Oceanic
(doc-request.create
  :workstream-id @ws-oceanic
  :doc-type "REGISTER_OF_SHAREHOLDERS"
  :is-mandatory true
  :as @doc-req-oceanic-shareholders)

;; Simulate document received and processed
;; Discovery: Oceanic is 100% owned by "Chen Wei" (Singapore citizen)
(entity.create-proper-person
  :first-name "Chen"
  :last-name "Wei"
  :as @chen-wei)

(ubo.add-ownership
  :owner-entity-id @chen-wei
  :owned-entity-id @oceanic
  :percentage 100
  :ownership-type "DIRECT"
  :as @own-chen)

;; Create workstream for discovered natural person
(entity-workstream.create
  :case-id @case
  :entity-id @chen-wei
  :discovery-source-id @ws-oceanic
  :discovery-reason "UBO_OWNER"
  :discovery-depth 3
  :is-ubo true
  :as @ws-chen)

;; Assert Chen Wei as UBO
(ubo.assert
  :cbu-id @fund
  :subject-entity-id @fund-entity
  :ubo-person-id @chen-wei
  :relationship-type "INDIRECT_OWNER"
  :qualifying-reason "OWNERSHIP_25PCT"
  :ownership-percentage 42
  :workflow-type "ONBOARDING"
  :case-id @case
  :workstream-id @ws-chen
  :discovery-method "DOCUMENT"
  :as @ubo-chen-suspected)

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
;; PART 10: UBO PROOF WITH EVIDENCE
;; =============================================================================

;; Catalog passport for Chen Wei
(document.catalog
  :cbu-id @fund
  :doc-type "PASSPORT"
  :title "Chen Wei - Singapore Passport"
  :as @doc-chen-passport)

;; Attach evidence to UBO record
(ubo.attach-evidence
  :ubo-id @ubo-chen-suspected
  :evidence-type "DOCUMENT"
  :evidence-role "IDENTITY_PROOF"
  :document-id @doc-chen-passport
  :description "Singapore passport for Chen Wei"
  :attached-by "analyst@bank.com"
  :as @evidence-chen-identity)

(ubo.attach-evidence
  :ubo-id @ubo-chen-suspected
  :evidence-type "DOCUMENT"
  :evidence-role "OWNERSHIP_PROOF"
  :document-id @doc-shareholders
  :description "Chain: Atlas Holdings -> Oceanic -> Chen Wei"
  :attached-by "analyst@bank.com"
  :as @evidence-chen-ownership)

;; Verify the evidence
(ubo.verify-evidence
  :evidence-id @evidence-chen-identity
  :verification-status "VERIFIED"
  :verified-by "senior.analyst@bank.com"
  :verification-notes "Passport valid until 2028")

(ubo.verify-evidence
  :evidence-id @evidence-chen-ownership
  :verification-status "VERIFIED"
  :verified-by "senior.analyst@bank.com"
  :verification-notes "Ownership chain verified through corporate docs")

;; Check if UBO can be proven
(ubo.can-prove :ubo-id @ubo-chen-suspected)

;; Prove the UBO
(ubo.prove
  :ubo-id @ubo-chen-suspected
  :proof-method "DOCUMENT"
  :evidence-doc-ids [@doc-chen-passport @doc-shareholders]
  :proof-notes "UBO proven via passport and ownership chain documentation")

;; =============================================================================
;; PART 11: CBU EVIDENCE & STATUS UPDATE
;; =============================================================================

;; Attach evidence to CBU
(cbu.attach-evidence
  :cbu-id @fund
  :evidence-type "DOCUMENT"
  :document-id @doc-cert
  :evidence-category "IDENTITY"
  :description "Certificate of Incorporation for holding company"
  :attached-by "analyst@bank.com"
  :as @cbu-evidence-cert)

(cbu.attach-evidence
  :cbu-id @fund
  :evidence-type "DOCUMENT"
  :document-id @doc-shareholders
  :evidence-category "OWNERSHIP"
  :description "Ownership structure documentation"
  :attached-by "analyst@bank.com"
  :as @cbu-evidence-ownership)

;; Verify CBU evidence
(cbu.verify-evidence
  :evidence-id @cbu-evidence-cert
  :verification-status "VERIFIED"
  :verified-by "senior.analyst@bank.com")

(cbu.verify-evidence
  :evidence-id @cbu-evidence-ownership
  :verification-status "VERIFIED"
  :verified-by "senior.analyst@bank.com")

;; Check CBU evidence completeness
(cbu.check-evidence-completeness :cbu-id @fund)

;; Log changes to audit trail
(cbu.log-change
  :cbu-id @fund
  :change-type "EVIDENCE_VERIFIED"
  :field-name "ownership_structure"
  :new-value {"verified": true, "ubos_identified": 1}
  :evidence-ids [@cbu-evidence-cert @cbu-evidence-ownership]
  :changed-by "senior.analyst@bank.com"
  :reason "Ownership structure verified through documentation"
  :case-id @case)

;; =============================================================================
;; PART 12: MITIGATE RED FLAGS
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
;; PART 13: RED-FLAG AGGREGATION & EVALUATION
;; =============================================================================

;; Update case to ASSESSMENT
(kyc-case.update-status :case-id @case :status "ASSESSMENT")

;; Complete workstreams
(entity-workstream.update-status :workstream-id @ws-holding :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-fund-entity :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-oceanic :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-chen :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-nordic :status "ASSESS")

;; Aggregate red-flag scores
(red-flag.aggregate :case-id @case)

;; Evaluate case for decision
(red-flag.evaluate :case-id @case :evaluated-by "compliance.manager@bank.com" :as @eval-snapshot)

;; List evaluations
(red-flag.list-evaluations :case-id @case)

;; =============================================================================
;; PART 14: UBO SNAPSHOT & COMPLETENESS CHECK
;; =============================================================================

;; Check UBO completeness
(ubo.check-completeness :cbu-id @fund :threshold 25.0)

;; Capture UBO snapshot
(ubo.snapshot-cbu
  :cbu-id @fund
  :case-id @case
  :snapshot-type "CASE_CLOSE"
  :reason "Pre-approval UBO snapshot"
  :as @ubo-snapshot)

;; Trace ownership chains
(ubo.trace-chains :cbu-id @fund :threshold 25.0)

;; =============================================================================
;; PART 15: FINAL DECISION & CASE CLOSURE
;; =============================================================================

;; Update case to REVIEW
(kyc-case.update-status :case-id @case :status "REVIEW")

;; Set risk rating
(kyc-case.set-risk-rating :case-id @case :risk-rating "MEDIUM")

;; Apply decision (based on evaluation)
(kyc-case.apply-decision
  :case-id @case
  :decision "APPROVE"
  :decided-by "compliance.director@bank.com"
  :notes "Approved - all UBOs identified and verified, red flags mitigated")

;; Update CBU status to VALIDATED
(cbu.set-status :cbu-id @fund :status "VALIDATED")

;; Close the case
(kyc-case.close
  :case-id @case
  :status "APPROVED"
  :notes "Hedge fund onboarding complete. Products: Custody, Alternatives, Collateral Management")

;; =============================================================================
;; SUMMARY: Test demonstrates:
;; - CBU lifecycle: DISCOVERED -> VALIDATION_PENDING -> VALIDATED
;; - Case lifecycle: INTAKE -> DISCOVERY -> ASSESSMENT -> REVIEW -> APPROVED
;; - UBO lifecycle: SUSPECTED -> PROVEN (with evidence)
;; - Workstream lifecycle: PENDING -> VERIFY -> ASSESS -> COMPLETE
;; - Red-flag lifecycle: OPEN -> MITIGATED
;; - Threshold-based RFI generation
;; - Multi-iteration UBO discovery
;; - Evidence attachment and verification
;; - Red-flag aggregation and evaluation
;; - Decision application with validation
;; =============================================================================
