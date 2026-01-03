;; Tollgate Workflow Test
;; Tests end-to-end KYC with tollgate evaluation
;; Phase D.5 of KYC Control Enhancement

(cbu.create
    :name "Tollgate Test Client"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

;; Create corporate structure
(entity.create-limited-company
    :cbu-id @cbu
    :name "Tollgate Test Ltd"
    :company-number "UK777666"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Test"
    :last-name "UBO"
    :date-of-birth "1970-01-01"
    :nationality "GB"
    :as @ubo)

;; Create KYC case
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

;; Create workstreams for each entity
(entity-workstream.create
    :case-id @case
    :entity-id @company
    :as @ws-company)

(entity-workstream.create
    :case-id @case
    :entity-id @ubo
    :discovery-reason "BENEFICIAL_OWNER"
    :ownership-percentage 100
    :is-ubo true
    :as @ws-ubo)

;; Catalog required documents
(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :doc-type "CERTIFICATE_OF_INCORPORATION"
    :title "Cert of Inc - Tollgate Test Ltd"
    :as @cert)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :doc-type "REGISTER_OF_SHAREHOLDERS"
    :title "Share Register - Tollgate Test Ltd"
    :as @register)

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :doc-type "PASSPORT"
    :title "Passport - Test UBO"
    :as @passport)

;; Define capital structure for reconciliation
(capital.define-share-class
    :cbu-id @cbu
    :issuer-entity-id @company
    :name "Ordinary Shares"
    :share-type "ORDINARY"
    :issued-shares 100
    :voting-rights-per-share 1.0
    :as @shares)

(capital.allocate
    :share-class-id @shares
    :shareholder-entity-id @ubo
    :units 100)

;; Run screenings on UBO
(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "PEP"
    :as @pep)

(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "SANCTIONS"
    :as @sanctions)

;; Complete screenings with clear results
(case-screening.complete
    :screening-id @pep
    :status "CLEAR"
    :result-summary "No PEP matches found")

(case-screening.complete
    :screening-id @sanctions
    :status "CLEAR"
    :result-summary "No sanctions matches found")

;; Get current metrics (before evidence is complete)
(tollgate.get-metrics
    :cbu-id @cbu
    :as @metrics-before)

;; First tollgate evaluation - may not pass yet
(tollgate.evaluate
    :case-id @case
    :evaluation-type "DISCOVERY_COMPLETE"
    :evaluated-by "system"
    :as @eval-discovery)

;; Reconcile capital to verify ownership
(capital.reconcile
    :entity-id @company
    :as @capital-recon)

;; Second tollgate - evidence complete
(tollgate.evaluate
    :case-id @case
    :evaluation-type "EVIDENCE_COMPLETE"
    :evaluated-by "system"
    :as @eval-evidence)

;; Get decision readiness
(tollgate.get-decision-readiness
    :case-id @case
    :as @readiness)

;; Unified control analysis - identify all UBOs
(control.identify-ubos
    :cbu-id @cbu
    :as @ubo-analysis)

;; Build full control graph
(control.build-graph
    :cbu-id @cbu
    :depth 5
    :as @control-graph)

;; Final tollgate - decision ready
(tollgate.evaluate
    :case-id @case
    :evaluation-type "DECISION_READY"
    :evaluated-by "analyst@example.com"
    :as @eval-decision)

;; List all evaluations for audit trail
(tollgate.list-evaluations
    :case-id @case
    :as @all-evaluations)
