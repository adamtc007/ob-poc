;; Limited Partnership with KYC Case Model

(cbu.create
    :name "Investment Partnership"
    :client-type "fund"
    :jurisdiction "KY"
    :as @cbu)

(entity.create-partnership-limited
    :cbu-id @cbu
    :name "Alpha Investment LP"
    :jurisdiction "KY"
    :as @partnership)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Alpha GP Ltd"
    :jurisdiction "KY"
    :as @gp)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @gp
    :role "GENERAL_PARTNER"
    :target-entity-id @partnership)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Michael"
    :last-name "Chen"
    :nationality "SG"
    :as @gpubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @gpubo
    :role "BENEFICIAL_OWNER"
    :target-entity-id @gp
    :ownership-percentage 100)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Pension Fund A"
    :jurisdiction "US"
    :as @lp1)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @lp1
    :role "LIMITED_PARTNER"
    :target-entity-id @partnership
    :ownership-percentage 60)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Endowment Fund B"
    :jurisdiction "US"
    :as @lp2)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @lp2
    :role "LIMITED_PARTNER"
    :target-entity-id @partnership
    :ownership-percentage 40)

(document.catalog :cbu-id @cbu :entity-id @partnership :document-type "PARTNERSHIP_AGREEMENT")
(document.catalog :cbu-id @cbu :entity-id @gp :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @gpubo :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @lp1 :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @lp2 :document-type "CERTIFICATE_OF_INCORPORATION")

;; Create KYC case and workstreams
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

(entity-workstream.create
    :case-id @case
    :entity-id @partnership
    :as @ws-partnership)

(entity-workstream.create
    :case-id @case
    :entity-id @gp
    :discovery-reason "GENERAL_PARTNER"
    :as @ws-gp)

(entity-workstream.create
    :case-id @case
    :entity-id @gpubo
    :discovery-reason "BENEFICIAL_OWNER"
    :ownership-percentage 100
    :is-ubo true
    :as @ws-gpubo)

(entity-workstream.create
    :case-id @case
    :entity-id @lp1
    :discovery-reason "LIMITED_PARTNER"
    :ownership-percentage 60
    :as @ws-lp1)

(entity-workstream.create
    :case-id @case
    :entity-id @lp2
    :discovery-reason "LIMITED_PARTNER"
    :ownership-percentage 40
    :as @ws-lp2)

;; Run screenings via workstreams
(case-screening.run :workstream-id @ws-gpubo :screening-type "PEP")
(case-screening.run :workstream-id @ws-partnership :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-gp :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-lp1 :screening-type "SANCTIONS")
(case-screening.run :workstream-id @ws-lp2 :screening-type "SANCTIONS")
