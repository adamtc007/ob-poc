(decision-pack cascading-decision
  :version "1.0.0"
  :description "Two-stage decision: first decision classifies; second decision applies the appropriate ruleset for the classification."
  :domain-scope [cbu kyc deal]
  :parameters [
    {:name primary-eval-name  :type symbol      :required true}
    {:name primary-gate-name  :type symbol      :required true}
    {:name primary-decision   :type decision-ref :required true}
    {:name output-field       :type string      :required true
     :description "The instance data location where the primary classification is written"}
    {:name class-a-value      :type string      :required true
     :description "The classification value that routes to path-a"}
    {:name path-a             :type node-ref    :required true}
    {:name path-b             :type node-ref    :required true
     :description "Default path for all other classifications"}
  ]
  :template [
    (flow $pre-node -> ,primary-eval-name)
    (flow ,primary-eval-name -> ,primary-gate-name)
    (flow ,primary-gate-name -> ,path-a :default false)
    (flow ,primary-gate-name -> ,path-b :default true)
  ]
  :example-utterances [
    "first classify by entity type, then apply the appropriate rules for that type"
    "two-stage decision: entity type determines which ruleset applies"
    "primary classification feeds secondary decision"
    "the first check determines which second check to run"
    "cascading rules: output of step 1 selects step 2"
  ]
  :structural-signature {
    :stages            2
    :evaluation-order  sequential
    :gateway-kind      exclusive
    :first-output-drives-second true
  }
  :governance-ref cascading-decision-v1-status)

(governance-status cascading-decision-v1-status
  :atom cascading-decision
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
