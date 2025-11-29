
(cbu.create
    :name "Multi-Owner Corp"
    :client-type "corporate"
    :jurisdiction "LU"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Luxembourg Holdings SARL"
    :company-number "B123456"
    :jurisdiction "LU"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Alice"
    :last-name "Johnson"
    :nationality "US"
    :as @ubo1)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo1
    :role "BENEFICIAL_OWNER"
    :target-entity-id @company
    :ownership-percentage 45)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Bob"
    :last-name "Williams"
    :nationality "GB"
    :as @ubo2)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo2
    :role "BENEFICIAL_OWNER"
    :target-entity-id @company
    :ownership-percentage 35)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Carol"
    :last-name "Davis"
    :nationality "DE"
    :as @ubo3)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo3
    :role "BENEFICIAL_OWNER"
    :target-entity-id @company
    :ownership-percentage 20)

(document.catalog :cbu-id @cbu :entity-id @company :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @company :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @ubo1 :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @ubo2 :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @ubo3 :document-type "PASSPORT")

(screening.pep :entity-id @ubo1)
(screening.pep :entity-id @ubo2)
(screening.pep :entity-id @ubo3)
(screening.sanctions :entity-id @company)
