(decision-pack conjunctive-gate
  :version "1.0.0"
  :description "All N conditions must be satisfied; single gateway routes to enhanced or standard path."
  :domain-scope [cbu kyc onboarding screening]
  :parameters [
    {:name conditions      :type list-of-condition-expr :required true
     :description "Conditions that must ALL be true for the enhanced path"}
    {:name gate-name       :type symbol   :required true
     :description "Name for the generated gateway atom"}
    {:name enhanced-path   :type node-ref :required true
     :description "Target node when all conditions hold"}
    {:name standard-path   :type node-ref :required true
     :description "Target node (default) when any condition fails"}
  ]
  :template [
    (flow $pre-node -> ,gate-name)
    (flow ,gate-name -> ,enhanced-path :default false)
    (flow ,gate-name -> ,standard-path :default true)
  ]
  :example-utterances [
    "all checks must pass before activation"
    "only proceed if KYC, screening, and UBO are all approved"
    "all conditions satisfied -> enhanced path, otherwise standard"
    "when every requirement is met, route to fast track"
    "all of these must be true before we can activate"
  ]
  :structural-signature {
    :conditions-composition and
    :gateway-kind           exclusive
    :outcomes               2
  }
  :governance-ref conjunctive-gate-v1-status)

(governance-status conjunctive-gate-v1-status
  :atom conjunctive-gate
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
