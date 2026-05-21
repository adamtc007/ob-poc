(decision-pack cascading-decision
  :version "1.0.0"
  :description "Single decision task classifies; exclusive gateway routes to N classification-specific paths. Uses for-each for variable-arity path list."
  :domain-scope [cbu kyc deal]
  :parameters [
    {:name primary-eval-name  :type symbol      :required true}
    {:name primary-gate-name  :type symbol      :required true}
    {:name primary-decision   :type decision-ref :required true}
    {:name output-field       :type string      :required true
     :description "Instance data location where the primary classification is written"}
    {:name paths              :type list-of-map :required true
     :description "List of {value, path} maps. Each entry has :value (string) and :path (node-ref). The last entry receives :default true automatically."}
  ]
  :template [
    (flow $pre-node -> ,primary-eval-name)
    (flow ,primary-eval-name -> ,primary-gate-name)
    (for-each :var p :in paths
      (flow ,primary-gate-name -> ,p.path))
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
    :path-count        variable
    :first-output-drives-second true
  }
  :governance-ref cascading-decision-v1-status)

(governance-status cascading-decision-v1-status
  :atom cascading-decision
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
