(cbu.create
    :name "Document Catalog Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Incomplete Docs Ltd"
    :company-number "UK999888"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Pending"
    :last-name "Documentation"
    :nationality "GB"
    :as @person)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @person
    :role "DIRECTOR"
    :target-entity-id @company)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "CERTIFICATE_OF_INCORPORATION"
    :as @doc1)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "ARTICLES_OF_ASSOCIATION"
    :as @doc2)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "REGISTER_OF_DIRECTORS"
    :as @doc3)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PASSPORT"
    :as @doc4)

(document.catalog
    :cbu-id @cbu
    :entity-id @person
    :document-type "PROOF_OF_ADDRESS"
    :as @doc5)
