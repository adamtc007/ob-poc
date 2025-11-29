
(cbu.create
    :name "Symbol Test"
    :client-type "individual"
    :jurisdiction "GB"
    :as @cbu)

(document.catalog
    :cbu-id @cbu
    :entity-id @nonexistent
    :document-type "PASSPORT")
