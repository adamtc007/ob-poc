(decision-pack multi-jurisdiction-overlay
  :version "1.0.0"
  :description "Jurisdiction-conditional routing to jurisdiction-specific processes. Uses for-each for variable-arity jurisdiction list; a fixed default-path handles all other jurisdictions."
  :domain-scope [cbu kyc deal compliance]
  :parameters [
    {:name jur-gate-name        :type symbol      :required true}
    {:name jurisdiction-field   :type string      :required true
     :description "Data location holding the ISO jurisdiction code"}
    {:name jurisdiction-paths   :type list-of-map :required true
     :description "List of {code, path} maps. Each entry has :code (string) and :path (node-ref)."}
    {:name default-path         :type node-ref    :required true
     :description "Path for all other jurisdictions"}
  ]
  :template [
    (flow $pre-node -> ,jur-gate-name)
    (for-each :var jp :in jurisdiction-paths
      (flow ,jur-gate-name -> ,jp.path))
    (flow ,jur-gate-name -> ,default-path)
  ]
  :example-utterances [
    "apply UK rules for UK clients, EU rules for EU clients, otherwise global standard"
    "jurisdiction-specific compliance routing"
    "different process per domicile"
    "route by jurisdiction: each country has its own requirements"
    "apply the relevant regulatory regime based on jurisdiction"
  ]
  :structural-signature {
    :routing-key   jurisdiction-string
    :gateway-kind  exclusive
    :outcomes      variable
  }
  :governance-ref multi-jurisdiction-overlay-v1-status)

(governance-status multi-jurisdiction-overlay-v1-status
  :atom multi-jurisdiction-overlay
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
