(cbu.create
    :name "Full KYC Workflow Test"
    :client-type "corporate"
    :jurisdiction "GB"
    :as @cbu)

(entity.create-limited-company
    :cbu-id @cbu
    :name "KYC Complete Ltd"
    :company-number "UK111222"
    :jurisdiction "GB"
    :as @company)

(entity.create-proper-person
    :cbu-id @cbu
    :first-name "Complete"
    :last-name "Review"
    :date-of-birth "1970-01-01"
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
    :document-type "CERTIFICATE_OF_INCORPORATION"
    :as @cert)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "ARTICLES_OF_ASSOCIATION"
    :as @articles)

(document.catalog
    :cbu-id @cbu
    :entity-id @company
    :document-type "REGISTER_OF_SHAREHOLDERS"
    :as @shareholders)

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PASSPORT"
    :as @passport)

(document.catalog
    :cbu-id @cbu
    :entity-id @ubo
    :document-type "PROOF_OF_ADDRESS"
    :as @poa)

(screening.pep
    :entity-id @ubo
    :as @pep)

(screening.sanctions
    :entity-id @ubo
    :as @ubosanctions)

(screening.sanctions
    :entity-id @company
    :as @companysanctions)

(screening.adverse-media
    :entity-id @ubo
    :as @adverse)

(kyc.initiate
    :cbu-id @cbu
    :investigation-type "ONBOARDING"
    :risk-rating "MEDIUM"
    :as @investigation)

(kyc.decide
    :investigation-id @investigation
    :decision "APPROVE"
    :rationale "All checks passed - standard corporate client"
    :decided-by "compliance_officer"
    :as @decision)
