(decision-pack linked-switch-chain
  :version "1.0.0"
  :description "Sequential exclusive gateways; each check may fast-exit or proceed to next. Uses for-each for the check list. NOTE: sequential chaining between N gateways (each gate's default continuing to the next) is Sage's responsibility during instantiation — for-each emits gate declarations; Sage connects them in order."
  :domain-scope [cbu kyc onboarding]
  :parameters [
    ; TODO (v0.3): replace with list-of-map when the sequential-chaining
    ; insertion-point pattern is implemented in the Sage instantiation engine.
    ; Until then, gateways are declared via for-each and Sage chains them.
    {:name gateway-names :type list-of-map :required true
     :description "List of {name, condition, exit-path} maps; each entry is one check gate. Sequential chaining (default of gate N → gate N+1) is Sage-managed."}
    {:name final-path    :type node-ref    :required true
     :description "Destination when all checks pass"}
  ]
  :template [
    (flow $pre-node -> first-gateway-placeholder)
    (for-each :var check :in gateway-names
      (flow ,check.name -> ,check.exit-path))
    (flow last-gateway-placeholder -> ,final-path)
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
    :fixed-checks     variable
  }
  :governance-ref linked-switch-chain-v1-status)

(governance-status linked-switch-chain-v1-status
  :atom linked-switch-chain
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
