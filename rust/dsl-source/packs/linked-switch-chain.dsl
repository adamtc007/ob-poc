(decision-pack linked-switch-chain
  :version "1.0.0"
  :description "Sequential exclusive gateways; each check may fast-exit or proceed to next. Representative template for N=2 checks; N>=3 follows the same structural pattern."
  :domain-scope [cbu kyc onboarding]
  :parameters [
    {:name gate-1-name    :type symbol       :required true}
    {:name gate-2-name    :type symbol       :required true}
    {:name condition-1    :type condition-expr :required true
     :description "First check: if this FAILS, take exit-path-1"}
    {:name condition-2    :type condition-expr :required true
     :description "Second check: if this FAILS, take exit-path-2"}
    {:name exit-path-1    :type node-ref     :required true
     :description "Fast-exit when condition-1 fails"}
    {:name exit-path-2    :type node-ref     :required true
     :description "Fast-exit when condition-2 fails"}
    {:name final-path     :type node-ref     :required true
     :description "Destination when both checks pass"}
  ]
  :template [
    (flow $pre-node -> ,gate-1-name)
    (flow ,gate-1-name -> ,exit-path-1 :default false)
    (flow ,gate-1-name -> ,gate-2-name :default true)
    (flow ,gate-2-name -> ,exit-path-2 :default false)
    (flow ,gate-2-name -> ,final-path :default true)
  ]
  :example-utterances [
    "first verify identity, then check sanctions - exit early on any failure"
    "sequential checks with early exit on failure"
    "step-by-step eligibility: verify each requirement in order"
    "chain of compliance checks, each with a rejection path"
    "waterfall decision: each gate can reject before the next"
  ]
  :structural-signature {
    :evaluation-order sequential
    :gateway-kind     exclusive
    :early-exit       true
    :fixed-checks     2
  }
  :governance-ref linked-switch-chain-v1-status)

(governance-status linked-switch-chain-v1-status
  :atom linked-switch-chain
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
