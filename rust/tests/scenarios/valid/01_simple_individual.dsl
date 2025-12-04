;; Simple Individual Onboarding with KYC Case

(cbu.create
    :name "John Smith Individual"
    :client-type "individual"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "John"
    :last-name "Smith"
    :date-of-birth "1985-03-15"
    :nationality "GB"
    :as @person)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT"
    :as @passport)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "UTILITY_BILL"
    :title "Gas Bill - March 2024"
    :as @poa)

;; Create KYC case and workstream
(kyc-case.create
    :cbu-id @cbu
    :case-type "NEW_CLIENT"
    :as @case)

(entity-workstream.create
    :case-id @case
    :entity-id @person
    :as @ws)

;; Run screenings via workstream
(case-screening.run
    :workstream-id @ws
    :screening-type "PEP"
    :as @pepscreen)

(case-screening.run
    :workstream-id @ws
    :screening-type "SANCTIONS"
    :as @sanctionsscreen)
