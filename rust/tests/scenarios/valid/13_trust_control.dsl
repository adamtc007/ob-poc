;; Trust Control Test
;; Tests trust provisions and UBO identification
;; Phase D.3 of KYC Control Enhancement

(cbu.create
    :name "Trust Structure Test"
    :client-type "corporate"
    :jurisdiction "JE"
    :as @cbu)

;; Create the trust
(entity.create-trust-discretionary
    :cbu-id @cbu
    :name "Smith Family Trust"
    :jurisdiction "JE"
    :as @trust)

;; Create trust parties
(entity.create-proper-person
    :cbu-id @cbu
    :first-name "William"
    :last-name "Smith"
    :date-of-birth "1950-06-15"
    :nationality "GB"
    :as @settlor)

(entity.create-limited-company
    :cbu-id @cbu
    :name "ABC Trustees Ltd"
    :company-number "JE98765"
    :jurisdiction "JE"
    :as @trustee-co)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "James"
    :last-name "Smith"
    :date-of-birth "1975-09-20"
    :nationality "GB"
    :as @beneficiary-1)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Emma"
    :last-name "Smith"
    :date-of-birth "1978-04-10"
    :nationality "GB"
    :as @beneficiary-2)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Richard"
    :last-name "Protector"
    :date-of-birth "1960-11-30"
    :nationality "GB"
    :as @protector)

;; Record trust provisions - beneficiaries with absolute discretion
(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "DISCRETIONARY_BENEFICIARY"
    :holder-entity-id @beneficiary-1
    :discretion-level "ABSOLUTE")

(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "DISCRETIONARY_BENEFICIARY"
    :holder-entity-id @beneficiary-2
    :discretion-level "ABSOLUTE")

;; Record protector powers
(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "PROTECTOR_POWER"
    :holder-entity-id @protector)

(trust.record-provision
    :cbu-id @cbu
    :trust-entity-id @trust
    :provision-type "TRUSTEE_REMOVAL"
    :holder-entity-id @protector)

;; Assign trust-specific roles
(cbu.assign-role
    :cbu-id @cbu
    :entity-id @settlor
    :role "SETTLOR"
    :target-entity-id @trust)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @trustee-co
    :role "TRUSTEE"
    :target-entity-id @trust)

(cbu.assign-role
    :cbu-id @cbu
    :entity-id @protector
    :role "PROTECTOR"
    :target-entity-id @trust)

;; Analyze trust control - who has control powers?
(trust.analyze-control
    :trust-entity-id @trust
    :as @trust-control)

;; Identify UBOs from trust structure
(trust.identify-ubos
    :trust-entity-id @trust
    :as @trust-ubos)

;; List all provisions for the trust
(trust.list-provisions
    :trust-entity-id @trust
    :as @provisions)

;; Classify the trust type
(trust.classify
    :trust-entity-id @trust
    :as @trust-classification)
