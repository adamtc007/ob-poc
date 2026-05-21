(decision-pack manual-override-checkpoint
  :version "1.0.0"
  :description "Automated decision presented to human for confirmation or override; final routing on human decision."
  :domain-scope [cbu kyc compliance governance]
  :parameters [
    {:name auto-eval-name    :type symbol       :required true}
    {:name review-task-name  :type symbol       :required true}
    {:name override-gate-name :type symbol      :required true}
    {:name auto-decision     :type decision-ref :required true}
    {:name reviewer-role     :type string       :required true
     :description "Role authorised to review and override"}
    {:name auto-result-field :type string       :required true
     :description "Data location where the auto-decision result is written"}
    {:name confirmed-path    :type node-ref     :required true
     :description "Path when human confirms the auto-decision"}
    {:name override-path     :type node-ref     :required true
     :description "Path when human overrides the auto-decision"}
  ]
  :template [
    (flow $pre-node -> ,auto-eval-name)
    (flow ,auto-eval-name -> ,review-task-name)
    (flow ,review-task-name -> ,override-gate-name)
    (flow ,override-gate-name -> ,override-path :default false)
    (flow ,override-gate-name -> ,confirmed-path :default true)
  ]
  :example-utterances [
    "automatically assess risk but allow a compliance officer to override"
    "system recommendation with human approval checkpoint"
    "automated decision with manual override capability"
    "present the auto-assessment to the reviewer for sign-off or correction"
    "4-eyes check: algorithm recommends, human confirms"
  ]
  :structural-signature {
    :automation-level  hybrid
    :human-in-loop     true
    :gateway-kind      exclusive
    :outcomes          2
  }
  :governance-ref manual-override-checkpoint-v1-status)

(governance-status manual-override-checkpoint-v1-status
  :atom manual-override-checkpoint
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
