(decision-pack decision-table-classification
  :version "1.0.0"
  :description "Single business-rule-task evaluating a named decision table; output routes to classification-specific paths. Representative for 2 explicit paths plus default."
  :domain-scope [cbu kyc deal im]
  :parameters [
    {:name classify-name    :type symbol       :required true}
    {:name route-gate-name  :type symbol       :required true}
    {:name decision         :type decision-ref :required true}
    {:name output-field     :type string       :required true}
    {:name class-a-value    :type string       :required true}
    {:name path-a           :type node-ref     :required true}
    {:name default-path     :type node-ref     :required true
     :description "Path for all classifications not explicitly listed"}
  ]
  :template [
    (flow $pre-node -> ,classify-name)
    (flow ,classify-name -> ,route-gate-name)
    (flow ,route-gate-name -> ,path-a :default false)
    (flow ,route-gate-name -> ,default-path :default true)
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
