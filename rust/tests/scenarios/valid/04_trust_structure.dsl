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

(screening.pep :entity-id @settlor)
(screening.pep :entity-id @beneficiary1)
(screening.pep :entity-id @beneficiary2)
(screening.sanctions :entity-id @trust)
