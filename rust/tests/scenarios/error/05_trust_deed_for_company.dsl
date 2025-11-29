
(cbu.create
    :name "Trust Deed Error Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Not A Trust Ltd"
    :company-number "UK000002"
    :jurisdiction "GB"
    :as @company)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "TRUST_DEED")
