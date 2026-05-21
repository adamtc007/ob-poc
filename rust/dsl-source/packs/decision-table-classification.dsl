(decision-pack decision-table-classification
  :version "1.0.0"
  :description "Single business-rule-task evaluating a named decision table; output routes to N classification-specific paths. Uses for-each for variable-arity path list."
  :domain-scope [cbu kyc deal im]
  :parameters [
    {:name classify-name    :type symbol       :required true}
    {:name route-gate-name  :type symbol       :required true}
    {:name decision         :type decision-ref :required true}
    {:name output-field     :type string       :required true}
    {:name paths            :type list-of-map  :required true
     :description "List of {value, path} maps. Each entry has :value (string) and :path (node-ref). The last entry receives :default true automatically."}
  ]
  :template [
    (flow $pre-node -> ,classify-name)
    (flow ,classify-name -> ,route-gate-name)
    (for-each :var p :in paths
      (flow ,route-gate-name -> ,p.path))
  ]
  :example-utterances [
    "classify the investor type and route accordingly"
    "use the risk classification table to determine next steps"
    "apply the CBU category ruleset and branch on result"
    "run the eligibility decision table"
    "DMN classification -> routing"
  ]
  :structural-signature {
    :gateway-kind       exclusive
    :classification     true
    :hit-policy         dmn-compatible
    :outcomes           variable
  }
  :governance-ref decision-table-classification-v1-status)

(governance-status decision-table-classification-v1-status
  :atom decision-table-classification
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
