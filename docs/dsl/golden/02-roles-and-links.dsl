;; ============================================================================
;; Roles and Links
;; ============================================================================
;; intent: Create entities and assign roles to a fund structure
;;
;; Funds require various service providers and governance roles.
;; This example creates the key parties and links them to a CBU.

;; ----------------------------------------------------------------------------
;; Step 1: Create the Fund Structure
;; ----------------------------------------------------------------------------

;; intent: Create a PE fund structure
;; macro: structure.setup
(cbu.create
  :name "Alpine Private Equity III"
  :type "FUND"
  :jurisdiction "LU"
  :legal-form "SCSp"
  :as @fund)

;; ----------------------------------------------------------------------------
;; Step 2: Create Service Provider Entities
;; ----------------------------------------------------------------------------

;; intent: Create the management company
;; macro: party.create
(entity.create
  :name "Alpine Capital Management S.a r.l."
  :type "LEGAL"
  :jurisdiction "LU"
  :as @manco)

;; intent: Create the depositary bank
;; macro: party.create
(entity.create
  :name "State Street Bank Luxembourg"
  :type "LEGAL"
  :jurisdiction "LU"
  :as @depositary)

;; intent: Create the administrator
;; macro: party.create
(entity.create
  :name "Alter Domus Luxembourg"
  :type "LEGAL"
  :jurisdiction "LU"
  :as @admin)

;; intent: Create the auditor
;; macro: party.create
(entity.create
  :name "KPMG Luxembourg"
  :type "LEGAL"
  :jurisdiction "LU"
  :as @auditor)

;; ----------------------------------------------------------------------------
;; Step 3: Create Natural Person Directors
;; ----------------------------------------------------------------------------

;; intent: Create fund director (natural person)
;; macro: party.create
(entity.create-proper-person
  :first-name "Marie"
  :last-name "Dubois"
  :nationality "LU"
  :as @director1)

;; intent: Create second director
;; macro: party.create
(entity.create-proper-person
  :first-name "Hans"
  :last-name "Mueller"
  :nationality "DE"
  :as @director2)

;; ----------------------------------------------------------------------------
;; Step 4: Assign Roles
;; ----------------------------------------------------------------------------

;; intent: Assign management company role
;; macro: structure.assign-role
(cbu-role.assign
  :cbu-id @fund
  :entity-id @manco
  :role "MANAGEMENT_COMPANY"
  :effective-date "2024-01-01")

;; intent: Assign depositary role
;; macro: structure.assign-role
(cbu-role.assign
  :cbu-id @fund
  :entity-id @depositary
  :role "DEPOSITARY"
  :effective-date "2024-01-01")

;; intent: Assign administrator role
;; macro: structure.assign-role
(cbu-role.assign
  :cbu-id @fund
  :entity-id @admin
  :role "ADMINISTRATOR"
  :effective-date "2024-01-01")

;; intent: Assign auditor role
;; macro: structure.assign-role
(cbu-role.assign
  :cbu-id @fund
  :entity-id @auditor
  :role "AUDITOR"
  :effective-date "2024-01-01")

;; intent: Assign board directors
;; macro: structure.assign-role
(cbu-role.assign
  :cbu-id @fund
  :entity-id @director1
  :role "DIRECTOR"
  :effective-date "2024-01-01")

(cbu-role.assign
  :cbu-id @fund
  :entity-id @director2
  :role "DIRECTOR"
  :effective-date "2024-01-01")

;; ----------------------------------------------------------------------------
;; Step 5: Verify Role Assignments
;; ----------------------------------------------------------------------------

;; intent: List all roles for the fund
(cbu-role.list :cbu-id @fund)
