
(cbu.create
    :name "Acme Ltd Corporate"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Acme Holdings Ltd"
    :company-number "12345678"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Jane"
    :last-name "Doe"
    :date-of-birth "1975-08-22"
    :nationality "GB"
    :as @ubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo
    :role "BENEFICIAL_OWNER"
    :target-entity-id @company
    :ownership-percentage 100)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "CERTIFICATE_OF_INCORPORATION")

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "ARTICLES_OF_ASSOCIATION")

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT")

(screening.pep :entity-id @ubo)
(screening.sanctions :entity-id @company)
