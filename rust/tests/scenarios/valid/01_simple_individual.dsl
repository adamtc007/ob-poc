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
    :document-type "PROOF_OF_ADDRESS"
    :as @poa)

(screening.pep
    :entity-id @person
    :as @pepscreen)

(screening.sanctions
    :entity-id @person
    :as @sanctionsscreen)
