(decision-pack multi-jurisdiction-overlay
  :version "1.0.0"
  :description "Jurisdiction-conditional routing to jurisdiction-specific processes. Representative for 2 explicit jurisdictions plus default."
  :domain-scope [cbu kyc deal compliance]
  :parameters [
    {:name jur-gate-name        :type symbol   :required true}
    {:name jurisdiction-field   :type string   :required true
     :description "Data location holding the ISO jurisdiction code"}
    {:name jurisdiction-a       :type string   :required true
     :description "Jurisdiction code for the first explicit path"}
    {:name path-a               :type node-ref :required true}
    {:name jurisdiction-b       :type string   :required true}
    {:name path-b               :type node-ref :required true}
    {:name default-path         :type node-ref :required true
     :description "Path for all other jurisdictions"}
  ]
  :template [
    (flow $pre-node -> ,jur-gate-name)
    (flow ,jur-gate-name -> ,path-a :default false)
    (flow ,jur-gate-name -> ,path-b :default false)
    (flow ,jur-gate-name -> ,default-path :default true)
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
