;; Trust Structure with KYC Case Model

(cbu.create
    :name "Family Trust Structure"
    :client-type "trust"
    :jurisdiction "JE"
    :as @cbu)

(entity.create-trust-discretionary
    :cbu-id @cbu
    :name "Smith Family Trust"
    :jurisdiction "JE"
    :as @trust)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Robert"
    :last-name "Smith"
    :nationality "GB"
    :as @settlor)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @settlor
    :role "SETTLOR"
    :target-entity-id @trust)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Jersey Trustees Ltd"
    :jurisdiction "JE"
    :as @trustee)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @trustee
    :role "TRUSTEE"
    :target-entity-id @trust)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Emma"
    :last-name "Smith"
    :nationality "GB"
    :as @beneficiary1)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @beneficiary1
    :role "BENEFICIARY"
    :target-entity-id @trust)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "James"
    :last-name "Smith"
    :nationality "GB"
    :as @beneficiary2)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @beneficiary2
    :role "BENEFICIARY"
    :target-entity-id @trust)

(document.catalog :cbu-id @cbu :entity-id @trust :document-type "TRUST_DEED")
(document.catalog :cbu-id @cbu :entity-id @settlor :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @beneficiary1 :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @beneficiary2 :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @trustee :document-type "CERTIFICATE_OF_INCORPORATION")

;; Create KYC case and workstreams
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

(entity-workstream.create
    :case-id @case
    :entity-id @trust
    :as @ws-trust)

(entity-workstream.create
    :case-id @case
    :entity-id @settlor
    :discovery-reason "SETTLOR"
    :as @ws-settlor)

(entity-workstream.create
    :case-id @case
    :entity-id @beneficiary1
    :discovery-reason "BENEFICIARY"
    :as @ws-ben1)

(entity-workstream.create
    :case-id @case
    :entity-id @beneficiary2
    :discovery-reason "BENEFICIARY"
    :as @ws-ben2)

;; Run screenings via workstreams
(case-screening.run :workstream-id @ws-settlor :screening-type "PEP")
(case-screening.run :workstream-id @ws-ben1 :screening-type "PEP")
(case-screening.run :workstream-id @ws-ben2 :screening-type "PEP")
(case-screening.run :workstream-id @ws-trust :screening-type "SANCTIONS")
