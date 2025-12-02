;; Full KYC Workflow using the new Case Model
;;
;; Flow: CBU → Entities → Roles → Documents → KYC Case → Workstreams → Screenings

(cbu.create
    :name "Full KYC Workflow Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "KYC Complete Ltd"
    :company-number "UK111222"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Complete"
    :last-name "Review"
    :date-of-birth "1970-01-01"
    :nationality "GB"
    :as @ubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo
    :role "BENEFICIAL_OWNER"
    :target-entity-id @company
    :ownership-percentage 100)

;; Document cataloging
(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "CERTIFICATE_OF_INCORPORATION"
    :as @cert)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "ARTICLES_OF_ASSOCIATION"
    :as @articles)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "REGISTER_OF_SHAREHOLDERS"
    :as @shareholders)

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT"
    :as @passport)

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PROOF_OF_ADDRESS"
    :as @poa)

;; Create KYC case for the onboarding
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

;; Create workstreams for entities requiring KYC
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

;; Run screenings via workstreams
(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "PEP"
    :as @pep)

(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "SANCTIONS"
    :as @ubo-sanctions)

(case-screening.run
    :workstream-id @ws-company
    :screening-type "SANCTIONS"
    :as @company-sanctions)

(case-screening.run
    :workstream-id @ws-ubo
    :screening-type "ADVERSE_MEDIA"
    :as @adverse)

;; Complete screenings with clear results
(case-screening.complete
    :screening-id @pep
    :status "CLEAR"
    :result-summary "No PEP matches found")

(case-screening.complete
    :screening-id @ubo-sanctions
    :status "CLEAR"
    :result-summary "No sanctions matches found")

(case-screening.complete
    :screening-id @company-sanctions
    :status "CLEAR"
    :result-summary "No sanctions matches found")

(case-screening.complete
    :screening-id @adverse
    :status "CLEAR"
    :result-summary "No adverse media found")

;; Update workstream statuses
(entity-workstream.update-status
    :workstream-id @ws-ubo
    :status "COMPLETE")

(entity-workstream.update-status
    :workstream-id @ws-company
    :status "COMPLETE")

;; Complete the case
(kyc-case.update-status
    :case-id @case
    :status "APPROVED"
    :notes "All checks passed - standard corporate client")
