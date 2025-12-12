;; Test DSL file for LSP validation
;; Open this in Zed to test completions, hover, diagnostics

;; Create a CBU (Client Business Unit)
(cbu.create  :name "Aviva test sicav" :jurisdiction LU :client-type FUND :as @fund)
;;(cbu.add-product :cbu-id @fund :product "Fund accounting")
;; Create entities
;;(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)
;;(entity.create-limited-company :name "Acme Holdings Ltd" :jurisdiction "GB" :as @holdings)

;; Assign roles
;;(cbu.add-product :cbu-id @fund :product FUND_ACCOUNTING)
;;(cbu.add-product :cbu-id @fund :product ALTS)
;;(cbu.add-product :cbu-id @fund :product "Custody")

;;(cbu.assign-role :cbu-id @fund :entity-id @holdings :role PRINCIPAL)
;;(cbu.assign-role :cbu-id @fund :entity-id @"Maria Rodriguez (PERSON)" :role DIRECTOR)
;;(cbu.assign-role :cbu-id @fund :entity-id @john-doe :role MANAGER)
;;(cbu.assign-role :cbu-id @fund :entity-id @adam-tc :role OWNER)
;; Add product to existing CBU
;;(cbu.add-product :cbu-id @fund :product ALTS)
;;(cbu.add-product :cbu-id @fund :product "Custody")

;; This should show an error - undefined symbol
;; (cbu.assign-role :cbu-id @undefined :entity-id @john :role "DIRECTOR")

;; This should show an error - unknown verb
;; (cbu.unknown-verb :name "Test")
