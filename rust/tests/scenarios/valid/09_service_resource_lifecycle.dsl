;; Test scenario: Service Resource Type and Instance Lifecycle
;; Tests: service-resource.ensure, service-resource.provision, service-resource.set-attr, service-resource.activate

;; Create a CBU first
(cbu.ensure :name "Service Resource Test Fund" :jurisdiction "US" :client-type "FUND" :as @fund)

;; Create service resource types (platforms/applications that deliver services)
(service-resource.ensure :name "Test Custody Platform" :resource-code "TEST_CUSTODY" :owner "Custody Operations" :resource-type "platform" :description "Test custody account management platform" :as @custody-type)

(service-resource.ensure :name "Test Reconciliation System" :resource-code "TEST_RECON" :owner "Operations" :resource-type "application" :description "Test reconciliation application" :as @recon-type)

;; Provision instances of service resources for the CBU
(service-resource.provision :cbu-id @fund :resource-type "TEST_CUSTODY" :instance-url "https://custody.test.com/accounts/test-fund-001" :instance-name "Test Fund Custody Account" :as @custody-instance)

;; Set attributes on the instance
(service-resource.set-attr :instance-id @custody-instance :attr "account_number" :value "CUST-TEST-001" :state "confirmed")
(service-resource.set-attr :instance-id @custody-instance :attr "base_currency" :value "USD" :state "confirmed")

;; Activate the instance
(service-resource.activate :instance-id @custody-instance)

;; Provision another service resource instance
(service-resource.provision :cbu-id @fund :resource-type "TEST_RECON" :instance-url "https://recon.test.com/accounts/test-fund-001" :instance-name "Test Fund Reconciliation" :as @recon-instance)

(service-resource.set-attr :instance-id @recon-instance :attr "schedule" :value "daily" :state "proposed")

;; Activate recon instance
(service-resource.activate :instance-id @recon-instance)
