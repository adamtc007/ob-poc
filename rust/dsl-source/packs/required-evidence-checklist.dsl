(decision-pack required-evidence-checklist
  :version "1.0.0"
  :description "N sequential evidence tasks; final gateway evaluates aggregate. Uses for-each for the task list. NOTE: sequential task chaining (task N → task N+1) is Sage's responsibility — for-each declares the tasks; Sage connects them in order."
  :domain-scope [cbu kyc onboarding]
  :parameters [
    ; TODO (v0.3): replace tasks with list-of-map when the sequential-chaining
    ; insertion-point pattern is implemented in the Sage instantiation engine.
    {:name tasks              :type list-of-map  :required true
     :description "List of {name} maps; each entry has :name (node-ref). Sequential chaining (task N → task N+1 → checklist gate) is Sage-managed."}
    {:name checklist-gate-name :type symbol       :required true}
    {:name approval-path       :type node-ref     :required true}
    {:name rejection-path      :type node-ref     :required true}
    {:name aggregate-condition :type condition-expr :required true
     :description "Boolean over evidence task outputs; must hold for approval-path"}
  ]
  :template [
    (flow $pre-node -> first-task-placeholder)
    (for-each :var task :in tasks
      (flow ,task.name -> next-task-or-gate-placeholder))
    (flow ,checklist-gate-name -> ,approval-path)
    (flow ,checklist-gate-name -> ,rejection-path)
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
