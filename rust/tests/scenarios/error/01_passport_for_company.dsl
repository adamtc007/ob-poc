
(cbu.create
    :name "Invalid Doc Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Cannot Have Passport Ltd"
    :company-number "UK000001"
    :jurisdiction "GB"
    :as @company)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "PASSPORT")
