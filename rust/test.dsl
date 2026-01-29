;; Test DSL file for LSP validation

;; Load a client book
(session.load-cluster :client <Allianz> :jurisdiction "LU")

;; Create a CBU
(cbu.create :name "Test Fund Alpha" :type FUND :jurisdiction "LU" :as @fund)

;; Assign a role
(cbu.assign-role :cbu-id @fund :entity-id <Allianz SE> :role ASSET_OWNER)

;; List session
(session.list :limit 10)

;; View operations
(view.cbu :cbu-id @fund :mode trading)
