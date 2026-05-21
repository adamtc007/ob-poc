(decision-pack parallel-evaluation-with-veto
  :version "1.0.0"
  :description "Two parallel evaluation tasks; any single veto at join blocks the application. Representative template for N=2; N>=3 follows the same structural pattern."
  :domain-scope [cbu kyc screening]
  :parameters [
    {:name fork-name        :type symbol   :required true}
    {:name join-name        :type symbol   :required true}
    {:name post-join-gate   :type symbol   :required true}
    {:name eval-task-1      :type node-ref :required true
     :description "First existing evaluation task node"}
    {:name eval-task-2      :type node-ref :required true
     :description "Second existing evaluation task node"}
    {:name veto-field       :type string   :required false :default "veto-result"}
    {:name vetoed-path      :type node-ref :required true}
    {:name approved-path    :type node-ref :required true}
  ]
  :template [
    (flow $pre-node -> ,fork-name)
    (flow ,fork-name -> ,eval-task-1)
    (flow ,fork-name -> ,eval-task-2)
    (flow ,eval-task-1 -> ,join-name)
    (flow ,eval-task-2 -> ,join-name)
    (flow ,join-name -> ,post-join-gate)
    (flow ,post-join-gate -> ,vetoed-path :default false)
    (flow ,post-join-gate -> ,approved-path :default true)
  ]
  :example-utterances [
    "run all checks in parallel; if any rejects, the whole application is rejected"
    "parallel screening: a single hit blocks the process"
    "concurrent evaluation with veto semantics"
    "all these checks happen simultaneously; any failure fails the whole thing"
    "parallel due diligence; one veto is enough to reject"
  ]
  :structural-signature {
    :evaluation-order  parallel
    :join-kind         parallel
    :veto-semantics    union-any
    :post-join-gateway exclusive
    :outcomes          2
  }
  :governance-ref parallel-evaluation-with-veto-v1-status)

(governance-status parallel-evaluation-with-veto-v1-status
  :atom parallel-evaluation-with-veto
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
