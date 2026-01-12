;; Test DAG ordering for trade-gateway verbs
;; This test verifies that:
;; 1. Gateway definitions come before routing rules (gateway is referenced)
;; 2. Connectivity comes before routing (connectivity establishes CBU-gateway link)
;;
;; Note: instruction-profile write verbs (assign-template, add-field-override) have been
;; removed as part of the matrix-first pivot. Use trading-profile verbs for authoring.

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

;; === INSTRUCTION PROFILE DOMAIN (Reference Data Only) ===
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

;; === VALIDATION VERBS (read operations) ===
;; These should come AFTER all the writes they read from

;; List operations to verify data was written
(trade-gateway.list-gateways)
(trade-gateway.list-cbu-gateways :cbu-id @fund)
(trade-gateway.list-routing-rules :cbu-id @fund)
(instruction-profile.list-message-types)
(instruction-profile.list-templates)
