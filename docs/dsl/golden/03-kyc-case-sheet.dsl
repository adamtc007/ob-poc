;; ============================================================================
;; KYC Case Sheet
;; ============================================================================
;; intent: Create a KYC case with entity list for onboarding review
;;
;; KYC cases track the compliance review of entities. Each case contains
;; subjects (entities being reviewed) and requirements (documents needed).

;; ----------------------------------------------------------------------------
;; Step 1: Create the Entities to Review
;; ----------------------------------------------------------------------------

;; intent: Create the fund being onboarded
(cbu.create
  :name "Nordic Growth Fund"
  :type "FUND"
  :jurisdiction "LU"
  :as @fund)

;; intent: Create the investor entity
;; macro: party.create
(entity.create
  :name "Swedish Pension Authority"
  :type "LEGAL"
  :jurisdiction "SE"
  :as @investor)

;; intent: Create the investor's authorized signatory
;; macro: party.create
(entity.create-proper-person
  :first-name "Erik"
  :last-name "Johansson"
  :nationality "SE"
  :as @signatory)

;; intent: Create the investor's UBO
;; macro: party.create
(entity.create-proper-person
  :first-name "Anna"
  :last-name "Lindgren"
  :nationality "SE"
  :as @ubo)

;; ----------------------------------------------------------------------------
;; Step 2: Create the KYC Case
;; ----------------------------------------------------------------------------

;; intent: Create KYC case for investor onboarding
;; macro: case.open
(kyc-case.create
  :name "Swedish Pension Authority - Onboarding"
  :type "INVESTOR_ONBOARDING"
  :cbu-id @fund
  :as @case)

;; ----------------------------------------------------------------------------
;; Step 3: Add Subjects to the Case
;; ----------------------------------------------------------------------------

;; intent: Add the investor as primary subject
(kyc-case.add-subject
  :case-id @case
  :entity-id @investor
  :role "PRIMARY")

;; intent: Add authorized signatory as related subject
(kyc-case.add-subject
  :case-id @case
  :entity-id @signatory
  :role "AUTHORIZED_SIGNATORY")

;; intent: Add UBO as related subject
(kyc-case.add-subject
  :case-id @case
  :entity-id @ubo
  :role "UBO")

;; ----------------------------------------------------------------------------
;; Step 4: Define Document Requirements
;; ----------------------------------------------------------------------------

;; intent: Require certificate of incorporation
(requirement.create
  :case-id @case
  :entity-id @investor
  :doc-type "CERTIFICATE_OF_INCORPORATION"
  :required true)

;; intent: Require articles of association
(requirement.create
  :case-id @case
  :entity-id @investor
  :doc-type "ARTICLES_OF_ASSOCIATION"
  :required true)

;; intent: Require signatory passport
(requirement.create
  :case-id @case
  :entity-id @signatory
  :doc-type "PASSPORT"
  :required true)

;; intent: Require signatory proof of address
(requirement.create
  :case-id @case
  :entity-id @signatory
  :doc-type "PROOF_OF_ADDRESS"
  :required true
  :max-age-days 90)

;; intent: Require UBO passport
(requirement.create
  :case-id @case
  :entity-id @ubo
  :doc-type "PASSPORT"
  :required true)

;; ----------------------------------------------------------------------------
;; Step 5: View Case Status
;; ----------------------------------------------------------------------------

;; intent: Get case summary with all subjects and requirements
(kyc-case.get :id @case)

;; intent: List pending requirements
(kyc-case.list-requirements :case-id @case :status "PENDING")
