
(cbu.create
    :name "Multi-Level Structure"
    :client-type "corporate"
    :jurisdiction "NL"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "OpCo Trading BV"
    :company-number "NL123456"
    :jurisdiction "NL"
    :as @opco)

(entity.create-limited-company
    :cbu-id @cbu
    :name "HoldCo Investments BV"
    :company-number "NL789012"
    :jurisdiction "NL"
    :as @holdco)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @holdco
    :role "SHAREHOLDER"
    :target-entity-id @opco
    :ownership-percentage 100)

(entity.create-limited-company
    :cbu-id @cbu
    :name "TopCo Holdings Ltd"
    :company-number "UK456789"
    :jurisdiction "GB"
    :as @topco)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @topco
    :role "SHAREHOLDER"
    :target-entity-id @holdco
    :ownership-percentage 100)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Victoria"
    :last-name "Windsor"
    :nationality "GB"
    :as @ubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @ubo
    :role "BENEFICIAL_OWNER"
    :target-entity-id @topco
    :ownership-percentage 100)

(document.catalog :cbu-id @cbu :entity-id @opco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @opco :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @holdco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @holdco :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @topco :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @topco :document-type "REGISTER_OF_SHAREHOLDERS")

(document.catalog :cbu-id @cbu :entity-id @ubo :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @ubo :document-type "PROOF_OF_ADDRESS")

(screening.sanctions :entity-id @opco)
(screening.sanctions :entity-id @holdco)
(screening.sanctions :entity-id @topco)
(screening.pep :entity-id @ubo)
(screening.sanctions :entity-id @ubo)
