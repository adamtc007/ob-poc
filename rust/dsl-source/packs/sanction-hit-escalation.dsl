(decision-pack sanction-hit-escalation
  :version "1.0.0"
  :description "Sanctions check service task; hard-block exclusive gateway: any hit value escalates immediately."
  :domain-scope [cbu kyc screening compliance]
  :parameters [
    {:name sanctions-check-name :type symbol   :required true
     :description "Name for the generated sanctions check service task node"}
    {:name sanctions-gate-name  :type symbol   :required true}
    {:name sanctions-field      :type string   :required true
     :description "Data location where the sanctions check writes its result"}
    {:name hit-value            :type string   :required false :default "hit"
     :description "The result value that constitutes a hit"}
    {:name escalation-path      :type node-ref :required true}
    {:name clear-path           :type node-ref :required true}
  ]
  :template [
    (flow $pre-node -> ,sanctions-check-name)
    (flow ,sanctions-check-name -> ,sanctions-gate-name)
    (flow ,sanctions-gate-name -> ,escalation-path :default false)
    (flow ,sanctions-gate-name -> ,clear-path :default true)
  ]
  :example-utterances [
    "if there's a sanctions match, immediately escalate to compliance"
    "sanctions hit -> hard block, route to compliance officer"
    "screening: positive sanctions result overrides everything"
    "any sanctions hit must go to manual review regardless"
    "hard block on sanctions: escalate immediately"
  ]
  :structural-signature {
    :check-kind    sanctions-lookup
    :gateway-kind  exclusive
    :hard-block    true
    :outcomes      2
  }
  :governance-ref sanction-hit-escalation-v1-status)

(governance-status sanction-hit-escalation-v1-status
  :atom sanction-hit-escalation
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
