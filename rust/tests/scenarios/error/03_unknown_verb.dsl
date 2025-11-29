
(cbu.create
    :name "Verb Test"
    :client-type "individual"
    :jurisdiction "GB"
    :as @cbu)

(invalid.nonexistent-verb
    :cbu-id @cbu
    :some-arg "value")
