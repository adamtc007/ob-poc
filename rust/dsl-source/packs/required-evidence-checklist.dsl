(decision-pack required-evidence-checklist
  :version "1.0.0"
  :description "Three sequential evidence tasks; final gateway evaluates aggregate. Representative for N=3 tasks."
  :domain-scope [cbu kyc onboarding]
  :parameters [
    {:name task-1              :type node-ref      :required true
     :description "First existing evidence task node"}
    {:name task-2              :type node-ref      :required true}
    {:name task-3              :type node-ref      :required true}
    {:name checklist-gate-name :type symbol        :required true}
    {:name approval-path       :type node-ref      :required true}
    {:name rejection-path      :type node-ref      :required true}
    {:name aggregate-condition :type condition-expr :required true
     :description "Boolean over evidence task outputs; must hold for approval-path"}
  ]
  :template [
    (flow $pre-node -> ,task-1)
    (flow ,task-1 -> ,task-2)
    (flow ,task-2 -> ,task-3)
    (flow ,task-3 -> ,checklist-gate-name)
    (flow ,checklist-gate-name -> ,approval-path :default false)
    (flow ,checklist-gate-name -> ,rejection-path :default true)
  ]
  :example-utterances [
    "collect and verify all required documents before making a decision"
    "sequential evidence checklist: ID, address, source of wealth"
    "each piece of evidence must be verified in order"
    "step-by-step document verification before final approval"
    "checklist: all evidence collected and verified -> proceed"
  ]
  :structural-signature {
    :evaluation-order    sequential
    :evidence-collection true
    :final-gateway       exclusive
    :outcomes            2
  }
  :governance-ref required-evidence-checklist-v1-status)

(governance-status required-evidence-checklist-v1-status
  :atom required-evidence-checklist
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
