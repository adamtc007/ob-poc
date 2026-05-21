(decision-pack threshold-band-routing
  :version "1.0.0"
  :description "Numeric value partitioned into 3 bands; each band routes to a distinct path. Representative for 3 bands."
  :domain-scope [cbu kyc ubo]
  :parameters [
    {:name band-gate-name  :type symbol  :required true}
    {:name input-field     :type string  :required true
     :description "Data location of the numeric value to classify"}
    {:name threshold-low   :type integer :required true
     :description "Upper bound of the low band (inclusive)"}
    {:name threshold-mid   :type integer :required true
     :description "Upper bound of the medium band (inclusive)"}
    {:name path-low        :type node-ref :required true}
    {:name path-mid        :type node-ref :required true}
    {:name path-high       :type node-ref :required true
     :description "Path for values above threshold-mid (default)"}
  ]
  :template [
    (flow $pre-node -> ,band-gate-name)
    (flow ,band-gate-name -> ,path-low :default false)
    (flow ,band-gate-name -> ,path-mid :default false)
    (flow ,band-gate-name -> ,path-high :default true)
  ]
  :example-utterances [
    "route by ownership percentage: below 10% is minor, 10-25% is significant, above 25% is controlling"
    "tiered risk scoring: low/medium/high bands"
    "threshold-based routing on credit limit"
    "bands: 0-25% standard, 25-50% enhanced, 50%+ controlling"
    "ownership tier routing"
  ]
  :structural-signature {
    :input-kind    numeric
    :gateway-kind  exclusive
    :band-count    3
    :band-semantics ordered-threshold
  }
  :governance-ref threshold-band-routing-v1-status)

(governance-status threshold-band-routing-v1-status
  :atom threshold-band-routing
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
