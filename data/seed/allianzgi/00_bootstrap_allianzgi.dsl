;; ======================================================================
;; AllianzGI Bootstrap Script
;; Pattern: Entities FIRST, then CBU, then role assignments
;; ======================================================================

;; Step 1: Create the Management Company (ManCo)
(entity.create-limited-company 
  :name "Allianz Global Investors Luxembourg S.A." 
  :jurisdiction "LU" 
  :registration-number "B 27856"
  :as @manco)

;; Step 2: Create the Umbrella Fund (SICAV)
(fund.create-umbrella 
  :name "Allianz Global Investors Fund" 
  :jurisdiction "LU" 
  :fund-structure-type "SICAV" 
  :regulatory-status "UCITS"
  :as @umbrella)

;; Step 3: Create CBU (LAST - after all entities exist)
(cbu.create 
  :name "Allianz Global Investors Group" 
  :jurisdiction "LU" 
  :client-type "FUND"
  :commercial-client-entity-id @umbrella
  :as @cbu)

;; Step 4: Assign roles
(cbu.assign-role :cbu-id @cbu :entity-id @manco :role "MANAGEMENT_COMPANY")
(cbu.assign-role :cbu-id @cbu :entity-id @umbrella :role "ASSET_OWNER")
