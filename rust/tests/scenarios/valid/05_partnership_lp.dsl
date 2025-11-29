
(cbu.create
    :name "Investment Partnership"
    :client-type "fund"
    :jurisdiction "KY"
    :as @cbu)

(entity.create-partnership
    :cbu-id @cbu
    :name "Alpha Investment LP"
    :partnership-type "LIMITED_PARTNERSHIP"
    :jurisdiction "KY"
    :as @partnership)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Alpha GP Ltd"
    :jurisdiction "KY"
    :as @gp)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @gp
    :role "GENERAL_PARTNER"
    :target-entity-id @partnership)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Michael"
    :last-name "Chen"
    :nationality "SG"
    :as @gpubo)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @gpubo
    :role "BENEFICIAL_OWNER"
    :target-entity-id @gp
    :ownership-percentage 100)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Pension Fund A"
    :jurisdiction "US"
    :as @lp1)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @lp1
    :role "LIMITED_PARTNER"
    :target-entity-id @partnership
    :ownership-percentage 60)

(entity.create-limited-company
    :cbu-id @cbu
    :name "Endowment Fund B"
    :jurisdiction "US"
    :as @lp2)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @lp2
    :role "LIMITED_PARTNER"
    :target-entity-id @partnership
    :ownership-percentage 40)

(document.catalog :cbu-id @cbu :entity-id @partnership :document-type "PARTNERSHIP_AGREEMENT")
(document.catalog :cbu-id @cbu :entity-id @gp :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @gpubo :document-type "PASSPORT")
(document.catalog :cbu-id @cbu :entity-id @lp1 :document-type "CERTIFICATE_OF_INCORPORATION")
(document.catalog :cbu-id @cbu :entity-id @lp2 :document-type "CERTIFICATE_OF_INCORPORATION")

(screening.pep :entity-id @gpubo)
(screening.sanctions :entity-id @partnership)
(screening.sanctions :entity-id @gp)
(screening.sanctions :entity-id @lp1)
(screening.sanctions :entity-id @lp2)
