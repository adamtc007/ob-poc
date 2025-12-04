;; Test scenario: Service Resource Instance Lifecycle
;; Tests: service-resource.provision, service-resource.set-attr, service-resource.activate, service-resource.suspend

;; Create a CBU first
(cbu.ensure :name "Service Resource Test Fund" :jurisdiction "US" :client-type "FUND" :as @fund)

;; Service resource types are reference data (read-only)
;; Use existing types: CUSTODY_ACCT, SETTLE_ACCT, etc.

;; Provision a custody account instance for the CBU
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.test.com/accounts/test-fund-001"
  :instance-name "Test Fund Custody Account"
  :as @custody-instance)

;; Set attributes on the instance
(service-resource.set-attr :instance-id @custody-instance :attr "account_number" :value "CUST-TEST-001")
(service-resource.set-attr :instance-id @custody-instance :attr "base_currency" :value "USD")

;; Activate the instance
(service-resource.activate :instance-id @custody-instance)

;; Provision a settlement account instance
(service-resource.provision
  :cbu-id @fund
  :resource-type "SETTLE_ACCT"
  :instance-url "https://settle.test.com/accounts/test-fund-001"
  :instance-name "Test Fund Settlement Account"
  :as @settle-instance)

(service-resource.set-attr :instance-id @settle-instance :attr "account_number" :value "SETT-TEST-001")

;; Activate settlement instance
(service-resource.activate :instance-id @settle-instance)

;; Suspend custody account (e.g., for maintenance)
(service-resource.suspend :instance-id @custody-instance)
