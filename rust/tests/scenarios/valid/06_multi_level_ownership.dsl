;; Multi-Level Ownership Structure with KYC Case Model

(cbu.create
    :name "Multi-Level Structure"
    :client-type "corporate"
    :jurisdiction "NL"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "OpCo Trading BV"
    :company-number "NL123456"
    :jurisdiction "NL"
    :as @opco)

(entity.create-limited-company
    :cbu-id @cbu
    :name "HoldCo Investments BV"
    :company-number "NL789012"
    :jurisdiction "NL"
    :as @holdco)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @holdco
    :role "SHAREHOLDER"
    :target-entity-id @opco
    :ownership-percentage 100)

(entity.create-limited-company
    :cbu-id @cbu
    :name "TopCo Holdings Ltd"
    :company-number "UK456789"
    :jurisdiction "GB"
    :as @topco)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @topco
    :role "SHAREHOLDER"
    :target-entity-id @holdco
    :ownership-percentage 100)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Victoria"
    :last-name "Windsor"
    :nationality "GB"
    :as @ubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo
    :role "BENEFICIAL_OWNER"
    :target-entity-id @topco
    :ownership-percentage 100)

(document.catalog :cbu-id @cbu :entity-id @opco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @opco :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @holdco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @holdco :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @topco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @topco :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @ubo :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @ubo :document-type "PROOF_OF_ADDRESS")

;; Create KYC case and workstreams for multi-level structure
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

(entity-workstream.create
    :case-id @case
    :entity-id @opco
    :as @ws-opco)

(entity-workstream.create
    :case-id @case
    :entity-id @holdco
    :discovery-reason "SHAREHOLDER"
    :discovery-depth 1
    :as @ws-holdco)

(entity-workstream.create
    :case-id @case
    :entity-id @topco
    :discovery-reason "SHAREHOLDER"
    :discovery-depth 2
    :as @ws-topco)

(entity-workstream.create
    :case-id @case
    :entity-id @ubo
    :discovery-reason "BENEFICIAL_OWNER"
    :discovery-depth 3
    :ownership-percentage 100
    :is-ubo true
    :as @ws-ubo)

;; Run screenings via workstreams
(case-screening.run :workstream-id @ws-opco :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-holdco :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-topco :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-ubo :screening-type "PEP")
(case-screening.run :workstream-id @ws-ubo :screening-type "SANCTIONS")
