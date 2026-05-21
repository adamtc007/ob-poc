(decision-pack threshold-band-routing
  :version "1.0.0"
  :description "Numeric value partitioned into N bands; each band routes to a distinct path. Uses for-each for variable arity."
  :domain-scope [cbu kyc ubo]
  :parameters [
    {:name band-gate-name :type symbol      :required true
     :description "Name of the exclusive gateway that routes by band"}
    {:name input-field    :type string      :required true
     :description "Data location of the numeric value to classify"}
    {:name bands          :type list-of-map :required true
     :description "Ordered list of band maps; each entry has :upper (integer) and :path (node-ref). The last entry receives :default true automatically."}
  ]
  :template [
    (flow $pre-node -> ,band-gate-name)
    (for-each :var band :in bands
      (flow ,band-gate-name -> ,band.path))
  ]
  :example-utterances [
    "route by ownership percentage: below 10% is minor, 10-25% is significant, above 25% is controlling"
    "tiered risk scoring: low/medium/high bands"
    "threshold-based routing on credit limit"
    "bands: 0-25% standard, 25-50% enhanced, 50%+ controlling"
    "ownership tier routing"
  ]
  :structural-signature {
    :input-kind        numeric
    :gateway-kind      exclusive
    :band-count        variable
    :band-semantics    ordered-threshold
  }
  :governance-ref threshold-band-routing-v1-status)

(governance-status threshold-band-routing-v1-status
  :atom threshold-band-routing
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
