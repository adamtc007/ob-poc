(decision-pack disjunctive-gate
  :version "1.0.0"
  :description "Any one of N conditions routes to escalation path; standard path if none hold."
  :domain-scope [cbu kyc screening onboarding]
  :parameters [
    {:name conditions      :type list-of-condition-expr :required true
     :description "Conditions; any one being true routes to the escalation path"}
    {:name gate-name       :type symbol   :required true}
    {:name escalation-path :type node-ref :required true
     :description "Target node when any condition holds"}
    {:name standard-path   :type node-ref :required true
     :description "Target node (default) when no condition holds"}
  ]
  :template [
    (flow $pre-node -> ,gate-name)
    (flow ,gate-name -> ,escalation-path :default false)
    (flow ,gate-name -> ,standard-path :default true)
  ]
  :example-utterances [
    "if any red flag is present, escalate"
    "any one of these conditions triggers enhanced review"
    "escalate if KYC rejected OR sanctions hit OR PEP positive"
    "if any risk indicator fires, route to compliance"
    "any of these conditions -> heightened scrutiny"
  ]
  :structural-signature {
    :conditions-composition or
    :gateway-kind           exclusive
    :outcomes               2
  }
  :governance-ref disjunctive-gate-v1-status)

(governance-status disjunctive-gate-v1-status
  :atom disjunctive-gate
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
