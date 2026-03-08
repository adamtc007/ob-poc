
(cbu.create
    :name "Missing Arg Test"
    :client-type "individual"
    :jurisdiction "GB"
    :as @cbu)

(entity.create :entity-type "proper-person"
    :cbu-id @cbu
    :nationality "GB"
    :as @person)
