(decision-pack periodic-refresh-trigger
  :version "1.0.0"
  :description "Exclusive gateway: if timestamp field age exceeds threshold months, route to refresh; otherwise continue."
  :domain-scope [cbu kyc periodic-review]
  :parameters [
    {:name age-gate-name     :type symbol  :required true}
    {:name timestamp-field   :type string  :required true
     :description "Data location of the last-refreshed timestamp"}
    {:name threshold-months  :type integer :required true}
    {:name refresh-path      :type node-ref :required true}
    {:name current-path      :type node-ref :required true
     :description "Path taken when the record is within the threshold (default)"}
  ]
  :template [
    (flow $pre-node -> ,age-gate-name)
    (flow ,age-gate-name -> ,refresh-path :default false)
    (flow ,age-gate-name -> ,current-path :default true)
  ]
  :example-utterances [
    "if KYC was last refreshed more than 12 months ago, trigger a refresh"
    "periodic KYC refresh: escalate if stale"
    "check if last review is older than the configured period"
    "time-based trigger: refresh if over threshold age"
    "annual review: if more than 12 months, re-verify"
  ]
  :structural-signature {
    :input-kind    timestamp
    :check-kind    age
    :gateway-kind  exclusive
    :outcomes      2
  }
  :governance-ref periodic-refresh-trigger-v1-status)

(governance-status periodic-refresh-trigger-v1-status
  :atom periodic-refresh-trigger
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
