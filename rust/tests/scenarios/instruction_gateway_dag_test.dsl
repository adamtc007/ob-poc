;; Test DAG ordering for instruction-profile and trade-gateway verbs
;; This test verifies that:
;; 1. Gateway definitions come before routing rules (gateway is referenced)
;; 2. Template definitions come before assignments (template is referenced)
;; 3. Assignments come before field overrides (assignment is referenced)

;; CBU setup
(cbu.ensure :name "DAG Test Fund" :jurisdiction "LU" :client-type "fund" :as @fund)

;; === GATEWAY DOMAIN ===
;; Define gateway first (reference data) - should be ordered BEFORE routing rules
(trade-gateway.define-gateway
  :code "TEST_SWIFT"
  :name "Test SWIFT Gateway"
  :gateway-type "SWIFT_FIN"
  :protocol "MT"
  :supported-events ["SETTLEMENT_INSTRUCTION" "CONFIRMATION"]
  :as @gateway)

;; Enable gateway for CBU - depends on gateway definition
(trade-gateway.enable-gateway
  :cbu-id @fund
  :gateway-id @gateway
  :status "ACTIVE"
  :as @connectivity)

;; Add routing rule - depends on both CBU and gateway
(trade-gateway.add-routing-rule
  :cbu-id @fund
  :gateway-id @gateway
  :lifecycle-event "SETTLEMENT_INSTRUCTION"
  :priority 10
  :as @routing)

;; Set fallback - depends on gateways
(trade-gateway.set-fallback
  :cbu-id @fund
  :primary-gateway-id @gateway
  :fallback-gateway-id @gateway
  :trigger-conditions ["TIMEOUT" "ERROR"]
  :priority 1
  :as @fallback)

;; === INSTRUCTION PROFILE DOMAIN ===
;; Define message type (reference data)
(instruction-profile.define-message-type
  :lifecycle-event "SETTLEMENT_INSTRUCTION"
  :message-standard "MT"
  :message-type "MT543"
  :direction "SEND"
  :description "Test settlement instruction"
  :as @msg-type)

;; Create template - depends on message type
(instruction-profile.create-template
  :code "TEST_MT543_TEMPLATE"
  :name "Test MT543 Template"
  :message-type-id @msg-type
  :base-template "{}"
  :as @template)

;; Assign template to CBU - depends on both CBU and template
(instruction-profile.assign-template
  :cbu-id @fund
  :template-id @template
  :lifecycle-event "SETTLEMENT_INSTRUCTION"
  :priority 10
  :as @assignment)

;; Add field override - depends on assignment
(instruction-profile.add-field-override
  :assignment-id @assignment
  :field-path "95P/REAG/BIC"
  :override-type "STATIC"
  :override-value "TESTBIC1"
  :reason "Test override"
  :as @override)

;; === VALIDATION VERBS (read operations) ===
;; These should come AFTER all the writes they read from

;; List operations to verify data was written
(trade-gateway.list-gateways)
(trade-gateway.list-cbu-gateways :cbu-id @fund)
(trade-gateway.list-routing-rules :cbu-id @fund)
(instruction-profile.list-message-types)
(instruction-profile.list-templates)
(instruction-profile.list-assignments :cbu-id @fund)
