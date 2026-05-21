# Session 3 Patch — §8, §9 Example 12, Appendix E
## Reconciled against regenerated Session 1 (decision-pack structural atom + `,name`/`,@name` substitution)

> This document contains three patched sections to be spliced into Session 3 v0.1. All other Session 3 content — §7, §9 Examples 1–11, §10–12, Appendices A–D — is unchanged.

---

## 8. Decision Pack Catalogue

> **Patched section — replaces Session 3 v0.1 §8 in its entirety.**

### 8.1 Decision pack atom model

A decision pack is a **structural atom** in the unified DSL. It is not declarative. The classification is in §3.2.1 of the regenerated Session 1: `decision-pack` appears in the structural kind list alongside `node`, `gateway`, `verb`, and `graph-pack`. Being structural means packs are parsed, hashed, versioned, and governed through the same machinery as all other structural atoms. Being structural does not mean they are compiled into the executable form — `(decision-pack ...)` atoms are indexed into the pack registry during Pass 1 (assembly) and dropped at Pass 3 (lowering). See regenerated Session 1 §3.2.2 for the compiler treatment.

**Full pack atom schema** (regenerated against Session 1 §3.3.2 and Appendix B.8):

```lisp
(decision-pack name
  :version           str                         ; semver, e.g. "1.0.0"
  :description       str                         ; human-readable intent
  :domain-scope      [symbol*]                   ; domains where pack is approved
  :parameters        [param-def*]                ; typed parameters Sage supplies
  :template          [structural-atom*]          ; expanded atoms with ,name / ,@name substitution
  :example-utterances [str*]                     ; NL phrases for Sage matching
  :structural-signature map?                     ; formal shape descriptor for multi-modal matching
  :governance-ref    symbol?)                    ; name-ref to a governance-status declarative atom
```

**`param-def`** (each entry in `:parameters`):
```lisp
{:name N :type T :required bool :description str? :default value?}
```

**Parameter type vocabulary** (from Session 1 §3.3.2):

| Type token | Meaning |
|---|---|
| `string` | Literal string value |
| `symbol` | An unquoted symbol (typically used for generated atom names) |
| `integer` | Integer literal |
| `boolean` | Boolean literal |
| `node-ref` | A name-ref resolving to a `(node ...)` or `(gateway ...)` atom in the current process |
| `condition-expr` | A single boolean composition expression (§3.9 of Session 1) |
| `predicate-ref` | Name-ref to a `(predicate ...)` atom |
| `list-of-condition-expr` | Ordered list of condition expressions; target of `,@name` splice |
| `list-of-predicate-ref` | List of predicate name-refs |
| `list-of-node-ref` | List of node or gateway name-refs |
| `decision-ref` | Name-ref to a `(decision ...)` atom |
| `path-map` | Map of classification value → target `node-ref` |

Types not in this vocabulary are a `[GAP: ...]` requiring v0.2 extension. Affected packs are annotated below.

#### 8.1.1 Template substitution syntax

Template bodies (the `:template` slot only) may contain two substitution forms, defined in regenerated Session 1 §3.1.1 and §3.5.x:

- **`,name`** — scalar substitution. The form is replaced by the caller-supplied value for the parameter named `name`. Valid in any value position within `:template`.
- **`,@name`** — splice substitution. The parameter named `name` must be a list type. The list elements are spliced into the surrounding expression. Valid only inside a list or expression context — the canonical use is `(and ,@conditions)` or `(or ,@conditions)` where `conditions` is a `list-of-condition-expr`.

Both forms are **static errors** outside the `:template` slot of a `(decision-pack ...)` atom. The assembly pass enforces this scope restriction.

#### 8.1.2 Insertion-point protocol

This section closes the `[GAP]` deferred in regenerated Session 1 §3.3.2, line 337.

Pack templates attach to the surrounding process at zero, one, or two named insertion points. These points are resolved by Sage at author time, not by the compiler.

**`$pre-node`** — the predecessor attachment point. The node in the surrounding process whose outgoing flow connects to the first gateway (or first atom) emitted by the template. When Sage expands a pack, it resolves `$pre-node` from the user-indicated attachment position and replaces all occurrences in the expanded flow atoms before submitting the DSL to the compiler.

**`$post-node`** — the successor attachment point. The node in the surrounding process that receives the outgoing flow from the last atom emitted by the template. Used for pass-through templates that generate an optional section and reconnect to the main flow. None of the 12 seed packs require `$post-node`; it is defined here for v0.2 extensibility.

**Notation distinction**: `$pre-node` and `$post-node` use the `$` prefix to distinguish them from `,name` parameter substitutions and from ordinary name-refs. They are insertion markers, not parameters — they do not appear in the `:parameters` list and are not included in the `:params` map of a `(provenance ...)` atom. The compiler treats `$pre-node` as an unresolved name-ref in the template body and produces a `UnresolvedInsertionMarker` warning if it appears in source submitted to the compiler without prior Sage expansion (indicating the template was submitted uninstantiated).

**Resolution algorithm** (Sage-side, author time):
1. User indicates the attachment position in the process being authored (e.g., "after the intake-form node").
2. Sage resolves `$pre-node` to the identified predecessor node name.
3. Sage substitutes all `,name` parameter forms with confirmed parameter values.
4. Sage splices all `,@name` forms, expanding list parameters inline.
5. Sage replaces `$pre-node` with the resolved name in all `(flow ...)` atoms that reference it.
6. Sage emits the expanded structural atoms and the accompanying `(provenance ...)` atom.

#### 8.1.3 Variable-arity template limitation in v0.1

The splice form `,@name` expands a list parameter into positions within a single expression (e.g., `(and ,@conditions)` → `(and cond-a cond-b cond-c)`). It does **not** generate N separate structural atoms from a list — there is no template combinator for repeating an atom block N times. Packs whose decision shape inherently requires generating N atoms (N gateways, N sequential flow chains) cannot be expressed as closed-form templates in v0.1. Affected packs (3, 4, 5, 6, 7, 8, 10) are marked with `[GAP: variable-arity atom generation deferred to v0.2]` and their templates show a representative fixed-arity form.

---

### 8.2 Pack expansion semantics

> **Patched section — replaces Session 3 v0.1 §8.2.**

Seven-step Sage interaction for pack-based authoring. This is a recommended interaction pattern, not a runtime requirement.

**Step 1: Intent received.**
Sage receives a natural-language description of a decision requirement from the user within the context of a process being authored.

**Step 2: Multi-modal catalogue matching.**
Sage queries the pack catalogue using three signals simultaneously:
- *Utterance similarity*: cosine similarity between the user's utterance embedding and each pack's `example-utterances` embeddings (BGE asymmetric, existing Candle pipeline).
- *Structural signature matching*: Sage extracts the decision shape from the utterance (number of conditions, composition operator AND/OR/sequential, number of outcomes) and matches against each pack's `:structural-signature` map. This is a structured match — the `:structural-signature` map provides machine-readable keys, not a prose description.
- *Domain context*: the user's current workspace and domain context constrains eligible packs via `:domain-scope`. Packs without the current domain in their scope are excluded.

The three signals are combined into a confidence score per pack. Packs with `governance-status.state != active` are excluded. The authoritative substitution syntax reference is regenerated Session 1 §3.5.x.

**Step 3: Candidates presented.**
Sage presents the top 2–3 confidence-ranked candidates with brief descriptions and representative example utterances. If no pack exceeds confidence 0.6, Sage defaults to bespoke authoring mode.

**Step 4: User selects.**
User confirms the top candidate or selects an alternative. If neither fits, user may request bespoke authoring — hand-authored structural atoms, with provenance recording `source: hand-authored`.

**Step 5: Parameter extraction.**
Sage extracts pack parameters from the user's natural-language description using the existing verb arg extraction pipeline (LLM-based, 200–500ms). For `list-of-condition-expr` parameters, the extraction prompt identifies each boolean condition mentioned. For `node-ref` and `list-of-node-ref` parameters, Sage resolves references to existing nodes in the process being authored. Sage also resolves the `$pre-node` insertion point from the user's attachment indication. Sage presents its interpretation for confirmation.

**Step 6: User confirms parameters.**
User confirms the extracted parameters. Any correction is applied. This step produces the canonical parameter map that drives substitution.

**Step 7: Instantiation.**
Sage performs template expansion with the confirmed parameters:
1. All `,name` scalar forms are replaced with the corresponding parameter value.
2. All `,@name` splice forms are expanded inline within their expression contexts.
3. `$pre-node` is replaced with the resolved predecessor node name.
4. The expanded structural atoms are emitted.
5. A `(provenance ...)` declarative atom is emitted covering all expanded structural atoms, recording the pack name, version, session id, timestamps, and the confirmed parameter map.

The resulting DSL (expanded structural atoms + provenance atom) is the authoritative source. It is passed to `validate()` or `compile()` without any pack-awareness — the compiler sees only ordinary structural atoms and one declarative atom.

**Compiler's role**: the compiler does not know about packs. Pack identity lives only in the `(provenance ...)` declarative atom, which is dropped at Pass 3 (lowering). The `(decision-pack ...)` atom defining the template is in the pack registry; the expanded atoms in the process source are standalone.

**Idempotent instantiation**: if the user triggers a second Sage authoring session targeting the same process nodes, Sage detects existing provenance atoms covering those nodes and offers to replace (re-instantiate with updated parameters) or extend (add atoms alongside existing ones).

---

### 8.3 Initial pack catalogue

> **Patched section — replaces Session 3 v0.1 §8.3.**

All 12 seed packs. For each: full atom definition, worked instantiation (parameters and expanded DSL), example utterances, structural signature, and governance status atom.

---

#### Pack 1: conjunctive-gate

**Description**: All of N conditions must hold; single exclusive gateway routes to enhanced path if all hold, standard path otherwise.

```lisp
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
    (gateway ,gate-name :kind exclusive)
    (flow $pre-node -> ,gate-name)
    (flow ,gate-name -> ,enhanced-path
      :condition (and ,@conditions))
    (flow ,gate-name -> ,standard-path
      :default true)
  ]
  :example-utterances [
    "all checks must pass before activation"
    "only proceed if KYC, screening, and UBO are all approved"
    "all conditions satisfied → enhanced path, otherwise standard"
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
```

**Worked instantiation** — User utterance: *"before the client can be activated, all three checks must pass: KYC approved, sanctions clear, UBO resolved. If all pass, proceed to activate. Otherwise, route to enhanced review."*

Sage-extracted parameters:
```
gate-name:      activation-eligibility-gate
conditions:     [(= kyc-case.status approved)
                 (= sanctions-result clear)
                 (= ubo-status resolved)]
enhanced-path:  activate-cbu-task
standard-path:  enhanced-review-task
$pre-node:      pre-activation-check       ; resolved from user's attachment indication
```

Expanded DSL:
```lisp
(gateway activation-eligibility-gate :kind exclusive)
(flow pre-activation-check -> activation-eligibility-gate)
(flow activation-eligibility-gate -> activate-cbu-task
  :condition (and (= kyc-case.status approved)
                  (= sanctions-result clear)
                  (= ubo-status resolved)))
(flow activation-eligibility-gate -> enhanced-review-task
  :default true)

(provenance activation-eligibility-gate-prov
  :covers [activation-eligibility-gate
           pre-activation-check->activation-eligibility-gate
           activation-eligibility-gate->activate-cbu-task
           activation-eligibility-gate->enhanced-review-task]
  :source      pack
  :source-id   conjunctive-gate
  :version     "1.0.0"
  :session     "sess-019e4a1f-3b22-7e01-9f01-23f456789abc"
  :authored-at "2026-05-21T12:00:00Z"
  :confirmed-at "2026-05-21T12:00:28Z"
  :params {
    :gate-name      activation-eligibility-gate
    :conditions     ["(= kyc-case.status approved)"
                     "(= sanctions-result clear)"
                     "(= ubo-status resolved)"]
    :enhanced-path  activate-cbu-task
    :standard-path  enhanced-review-task
  })
```

---

#### Pack 2: disjunctive-gate

**Description**: Any of N conditions suffices to route to the escalation path; all must fail to take the standard path.

```lisp
(decision-pack disjunctive-gate
  :version "1.0.0"
  :description "Any one of N conditions routes to escalation path; standard path if none hold."
  :domain-scope [cbu kyc screening onboarding]
  :parameters [
    {:name conditions      :type list-of-condition-expr :required true
     :description "Conditions; any one being true routes to the escalation path"}
    {:name gate-name       :type symbol   :required true}
    {:name escalation-path :type node-ref :required true
     :description "Target node when any condition holds"}
    {:name standard-path   :type node-ref :required true
     :description "Target node (default) when no condition holds"}
  ]
  :template [
    (gateway ,gate-name :kind exclusive)
    (flow $pre-node -> ,gate-name)
    (flow ,gate-name -> ,escalation-path
      :condition (or ,@conditions))
    (flow ,gate-name -> ,standard-path
      :default true)
  ]
  :example-utterances [
    "if any red flag is present, escalate"
    "any one of these conditions triggers enhanced review"
    "escalate if KYC rejected OR sanctions hit OR PEP positive"
    "if any risk indicator fires, route to compliance"
    "any of these conditions → heightened scrutiny"
  ]
  :structural-signature {
    :conditions-composition or
    :gateway-kind           exclusive
    :outcomes               2
  }
  :governance-ref disjunctive-gate-v1-status)

(governance-status disjunctive-gate-v1-status
  :atom disjunctive-gate
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

**Worked instantiation** — User utterance: *"if any of the following apply, escalate immediately: sanctions hit, PEP match, or adverse media found."*

Parameters:
```
gate-name:        risk-indicator-gate
conditions:       [(= sanctions-result hit)
                   (= pep-match true)
                   (= adverse-media found)]
escalation-path:  immediate-escalation-task
standard-path:    standard-review-task
$pre-node:        screening-complete
```

Expanded DSL:
```lisp
(gateway risk-indicator-gate :kind exclusive)
(flow screening-complete -> risk-indicator-gate)
(flow risk-indicator-gate -> immediate-escalation-task
  :condition (or (= sanctions-result hit)
                 (= pep-match true)
                 (= adverse-media found)))
(flow risk-indicator-gate -> standard-review-task
  :default true)

(provenance risk-indicator-gate-prov
  :covers [risk-indicator-gate
           screening-complete->risk-indicator-gate
           risk-indicator-gate->immediate-escalation-task
           risk-indicator-gate->standard-review-task]
  :source    pack
  :source-id disjunctive-gate
  :version   "1.0.0"
  :session   "sess-019e4a2a-..."
  :authored-at "2026-05-21T12:05:00Z"
  :params {
    :gate-name        risk-indicator-gate
    :conditions       ["(= sanctions-result hit)"
                       "(= pep-match true)"
                       "(= adverse-media found)"]
    :escalation-path  immediate-escalation-task
    :standard-path    standard-review-task
  })
```

---

#### Pack 3: linked-switch-chain

**Description**: Sequential exclusive gateways; each step checks one condition and may fast-exit; the final gateway routes to the completion path. Used when intermediate tasks or audit entries exist between checks, or when individual rejection paths differ per condition.

[GAP: variable-arity atom generation — a linked-switch-chain for N checks requires N gateway atoms and 3N flow atoms. The v0.1 template model supports `,@name` splice into single expressions but not repetition of atom blocks. The template below is representative for N=2. For N≥3, Sage generates the additional gateway and flow atoms structurally, following the N=2 pattern extended; the parameters are expressed as pairs rather than a combined list type. A `for-each` template combinator deferring to v0.2 will close this gap.]

```lisp
(decision-pack linked-switch-chain
  :version "1.0.0"
  :description "Sequential exclusive gateways; each check may fast-exit or proceed to next. Representative template for N=2 checks; N≥3 follows the same structural pattern."
  :domain-scope [cbu kyc onboarding]
  :parameters [
    {:name gate-1-name    :type symbol       :required true}
    {:name gate-2-name    :type symbol       :required true}
    {:name condition-1    :type condition-expr :required true
     :description "First check: if this FAILS, take exit-path-1"}
    {:name condition-2    :type condition-expr :required true
     :description "Second check: if this FAILS, take exit-path-2"}
    {:name exit-path-1    :type node-ref     :required true
     :description "Fast-exit when condition-1 fails"}
    {:name exit-path-2    :type node-ref     :required true
     :description "Fast-exit when condition-2 fails"}
    {:name final-path     :type node-ref     :required true
     :description "Destination when both checks pass"}
  ]
  :template [
    (gateway ,gate-1-name :kind exclusive)
    (flow $pre-node -> ,gate-1-name)
    (flow ,gate-1-name -> ,exit-path-1
      :condition (not ,condition-1))
    (flow ,gate-1-name -> ,gate-2-name
      :default true)
    (gateway ,gate-2-name :kind exclusive)
    (flow ,gate-2-name -> ,exit-path-2
      :condition (not ,condition-2))
    (flow ,gate-2-name -> ,final-path
      :default true)
  ]
  :example-utterances [
    "first verify identity, then check sanctions — exit early on any failure"
    "sequential checks with early exit on failure"
    "step-by-step eligibility: verify each requirement in order"
    "chain of compliance checks, each with a rejection path"
    "waterfall decision: each gate can reject before the next"
  ]
  :structural-signature {
    :evaluation-order sequential
    :gateway-kind     exclusive
    :early-exit       true
    :fixed-checks     2
  }
  :governance-ref linked-switch-chain-v1-status)

(governance-status linked-switch-chain-v1-status
  :atom linked-switch-chain
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

**Worked instantiation** — *"first check that identity is verified, then check that the entity is not on the exclusion list; either failure routes to manual review; both passing routes to the next stage."*

Parameters:
```
gate-1-name:  identity-gate
gate-2-name:  exclusion-gate
condition-1:  (= identity-verified true)
condition-2:  (= exclusion-list-status clear)
exit-path-1:  manual-review-task
exit-path-2:  manual-review-task
final-path:   due-diligence-task
$pre-node:    intake-complete
```

Expanded DSL:
```lisp
(gateway identity-gate :kind exclusive)
(flow intake-complete -> identity-gate)
(flow identity-gate -> manual-review-task
  :condition (not (= identity-verified true)))
(flow identity-gate -> exclusion-gate
  :default true)
(gateway exclusion-gate :kind exclusive)
(flow exclusion-gate -> manual-review-task
  :condition (not (= exclusion-list-status clear)))
(flow exclusion-gate -> due-diligence-task
  :default true)
```

---

#### Pack 4: parallel-evaluation-with-veto

**Description**: N independent evaluation tasks run in parallel; any single veto overrides all others at the join. Used for concurrent screening workstreams where one hit is sufficient to block the application.

[GAP: variable-arity atom generation — for N evaluations, N flow atoms from fork to each task and N flow atoms from each task to join are required. The template below is representative for N=2 evaluation tasks that are assumed to already exist in the surrounding process (the pack attaches fork, join, and post-join routing). A `for-each` combinator deferring to v0.2 will support generating the task nodes alongside the structural plumbing.]

```lisp
(decision-pack parallel-evaluation-with-veto
  :version "1.0.0"
  :description "Two parallel evaluation tasks; any single veto at join blocks the application. Representative template for N=2; N≥3 follows the same structural pattern."
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
    (gateway ,fork-name :kind parallel)
    (flow $pre-node -> ,fork-name)
    (flow ,fork-name -> ,eval-task-1)
    (flow ,fork-name -> ,eval-task-2)
    (parallel-join ,join-name
      :expects [,fork-name]
      :merge [{:location ,veto-field :operator union}])
    (flow ,eval-task-1 -> ,join-name)
    (flow ,eval-task-2 -> ,join-name)
    (gateway ,post-join-gate :kind exclusive)
    (flow ,join-name -> ,post-join-gate)
    (flow ,post-join-gate -> ,vetoed-path
      :condition (in "vetoed" ,veto-field))
    (flow ,post-join-gate -> ,approved-path
      :default true)
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
```

**Worked instantiation** — *"run sanctions screening and PEP check in parallel; if either produces a hit, route to compliance escalation; if both clear, proceed."*

Parameters:
```
fork-name:      screening-fork
join-name:      screening-join
post-join-gate: screening-outcome-gate
eval-task-1:    sanctions-check-task    ; pre-existing node
eval-task-2:    pep-check-task          ; pre-existing node
veto-field:     "screening-veto"
vetoed-path:    compliance-escalation-task
approved-path:  post-screening-task
$pre-node:      entity-data-collected
```

Expanded DSL (abbreviated):
```lisp
(gateway screening-fork :kind parallel)
(flow entity-data-collected -> screening-fork)
(flow screening-fork -> sanctions-check-task)
(flow screening-fork -> pep-check-task)
(parallel-join screening-join
  :expects [screening-fork]
  :merge [{:location "screening-veto" :operator union}])
(flow sanctions-check-task -> screening-join)
(flow pep-check-task -> screening-join)
(gateway screening-outcome-gate :kind exclusive)
(flow screening-join -> screening-outcome-gate)
(flow screening-outcome-gate -> compliance-escalation-task
  :condition (in "vetoed" "screening-veto"))
(flow screening-outcome-gate -> post-screening-task
  :default true)
```

---

#### Pack 5: cascading-decision

**Description**: Primary decision classifies an entity; the classification drives selection of a secondary decision. Used when entity type or risk tier determines which ruleset applies.

[GAP: variable-arity atom generation — for N secondary decision paths, N conditional flows from the primary gate are required. The template below shows a 2-secondary-decision representative form.]

```lisp
(decision-pack cascading-decision
  :version "1.0.0"
  :description "Two-stage decision: first decision classifies; second decision applies the appropriate ruleset for the classification."
  :domain-scope [cbu kyc deal]
  :parameters [
    {:name primary-eval-name  :type symbol      :required true}
    {:name primary-gate-name  :type symbol      :required true}
    {:name primary-decision   :type decision-ref :required true}
    {:name output-field       :type string      :required true
     :description "The instance data location where the primary classification is written"}
    {:name class-a-value      :type string      :required true
     :description "The classification value that routes to path-a"}
    {:name path-a             :type node-ref    :required true}
    {:name path-b             :type node-ref    :required true
     :description "Default path for all other classifications"}
  ]
  :template [
    (node ,primary-eval-name :kind business-rule-task
      :verb (invoke switch.evaluate-decision
        :args {:decision ,primary-decision :output-field ,output-field}))
    (flow $pre-node -> ,primary-eval-name)
    (gateway ,primary-gate-name :kind exclusive)
    (flow ,primary-eval-name -> ,primary-gate-name)
    (flow ,primary-gate-name -> ,path-a
      :condition (= ,output-field ,class-a-value))
    (flow ,primary-gate-name -> ,path-b
      :default true)
  ]
  :example-utterances [
    "first classify by entity type, then apply the appropriate rules for that type"
    "two-stage decision: entity type determines which ruleset applies"
    "primary classification feeds secondary decision"
    "the first check determines which second check to run"
    "cascading rules: output of step 1 selects step 2"
  ]
  :structural-signature {
    :stages            2
    :evaluation-order  sequential
    :gateway-kind      exclusive
    :first-output-drives-second true
  }
  :governance-ref cascading-decision-v1-status)

(governance-status cascading-decision-v1-status
  :atom cascading-decision
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

**Worked instantiation** — *"classify the client as institutional or retail; institutional clients go to the institutional onboarding track; retail goes to the standard track."*

Parameters:
```
primary-eval-name:  client-type-classifier
primary-gate-name:  client-type-gate
primary-decision:   client-type-classification
output-field:       "client-category"
class-a-value:      "institutional"
path-a:             institutional-onboarding-task
path-b:             retail-onboarding-task
$pre-node:          intake-form-complete
```

Expanded DSL:
```lisp
(node client-type-classifier :kind business-rule-task
  :verb (invoke switch.evaluate-decision
    :args {:decision client-type-classification :output-field "client-category"}))
(flow intake-form-complete -> client-type-classifier)
(gateway client-type-gate :kind exclusive)
(flow client-type-classifier -> client-type-gate)
(flow client-type-gate -> institutional-onboarding-task
  :condition (= "client-category" "institutional"))
(flow client-type-gate -> retail-onboarding-task
  :default true)
```

---

#### Pack 6: decision-table-classification

**Description**: Single business-rule-task evaluates a named decision table; output routes to one of N classification-specific paths.

[GAP: variable-arity atom generation — for N classification outputs, N conditional flow atoms from the route gateway are required. The template below shows a 2-path representative form.]

```lisp
(decision-pack decision-table-classification
  :version "1.0.0"
  :description "Single business-rule-task evaluating a named decision table; output routes to classification-specific paths. Representative for 2 explicit paths plus default."
  :domain-scope [cbu kyc deal im]
  :parameters [
    {:name classify-name    :type symbol       :required true}
    {:name route-gate-name  :type symbol       :required true}
    {:name decision         :type decision-ref :required true}
    {:name output-field     :type string       :required true}
    {:name class-a-value    :type string       :required true}
    {:name path-a           :type node-ref     :required true}
    {:name default-path     :type node-ref     :required true
     :description "Path for all classifications not explicitly listed"}
  ]
  :template [
    (node ,classify-name :kind business-rule-task
      :verb (invoke switch.evaluate-decision
        :args {:decision ,decision :output-field ,output-field}))
    (flow $pre-node -> ,classify-name)
    (gateway ,route-gate-name :kind exclusive)
    (flow ,classify-name -> ,route-gate-name)
    (flow ,route-gate-name -> ,path-a
      :condition (= ,output-field ,class-a-value))
    (flow ,route-gate-name -> ,default-path
      :default true)
  ]
  :example-utterances [
    "classify the investor type and route accordingly"
    "use the risk classification table to determine next steps"
    "apply the CBU category ruleset and branch on result"
    "run the eligibility decision table"
    "DMN classification → routing"
  ]
  :structural-signature {
    :gateway-kind       exclusive
    :classification     true
    :hit-policy         dmn-compatible
    :outcomes           variable
  }
  :governance-ref decision-table-classification-v1-status)

(governance-status decision-table-classification-v1-status
  :atom decision-table-classification
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

#### Pack 7: threshold-band-routing

**Description**: Numeric input partitioned into ordered bands; each band routes to a different path. Used for ownership percentage tiers, risk score bands, and AUM thresholds.

[GAP: band-list type is not in the v0.1 type vocabulary. `:bands` is typed as `path-map` here with the understanding that keys are string-encoded upper-bound thresholds and values are node-refs. A `band-list` type with typed numeric bounds is deferred to v0.2. Template shows a 3-band representative form.]

```lisp
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
    (gateway ,band-gate-name :kind exclusive)
    (flow $pre-node -> ,band-gate-name)
    (flow ,band-gate-name -> ,path-low
      :condition (<= ,input-field ,threshold-low))
    (flow ,band-gate-name -> ,path-mid
      :condition (and (> ,input-field ,threshold-low)
                      (<= ,input-field ,threshold-mid)))
    (flow ,band-gate-name -> ,path-high
      :default true)
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
```

**Worked instantiation** — *"route by UBO ownership stake: below 10% is minor interest, 10-25% is significant, above 25% is controlling — each routes to a different disclosure track."*

Parameters:
```
band-gate-name:  ownership-band-gate
input-field:     "ubo-ownership-pct"
threshold-low:   10
threshold-mid:   25
path-low:        minor-interest-track
path-mid:        significant-interest-track
path-high:       controlling-interest-track
$pre-node:       ubo-data-verified
```

---

#### Pack 8: required-evidence-checklist

**Description**: Sequential evidence tasks; each must complete before the next; final gateway evaluates aggregate condition.

[GAP: variable-arity atom generation — connecting N evidence tasks sequentially requires N-1 flow atoms between them. The template below shows the representative structure for 3 evidence tasks. A `for-each` combinator in v0.2 will support variable N.]

```lisp
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
    (gateway ,checklist-gate-name :kind exclusive)
    (flow ,task-3 -> ,checklist-gate-name)
    (flow ,checklist-gate-name -> ,approval-path
      :condition ,aggregate-condition)
    (flow ,checklist-gate-name -> ,rejection-path
      :default true)
  ]
  :example-utterances [
    "collect and verify all required documents before making a decision"
    "sequential evidence checklist: ID, address, source of wealth"
    "each piece of evidence must be verified in order"
    "step-by-step document verification before final approval"
    "checklist: all evidence collected and verified → proceed"
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
```

---

#### Pack 9: periodic-refresh-trigger

**Description**: Check the age of a timestamp field; if older than a configured threshold, trigger a refresh workflow.

```lisp
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
    (gateway ,age-gate-name :kind exclusive)
    (flow $pre-node -> ,age-gate-name)
    (flow ,age-gate-name -> ,refresh-path
      :condition (> (months-since ,timestamp-field) ,threshold-months))
    (flow ,age-gate-name -> ,current-path
      :default true)
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
```

**Worked instantiation** — *"if the KYC case was last completed more than 24 months ago, route to re-verification; otherwise proceed to activation."*

Parameters:
```
age-gate-name:    kyc-staleness-gate
timestamp-field:  "kyc-last-completed-at"
threshold-months: 24
refresh-path:     kyc-re-verification-task
current-path:     cbu-activation-task
$pre-node:        kyc-review-complete
```

Expanded DSL:
```lisp
(gateway kyc-staleness-gate :kind exclusive)
(flow kyc-review-complete -> kyc-staleness-gate)
(flow kyc-staleness-gate -> kyc-re-verification-task
  :condition (> (months-since "kyc-last-completed-at") 24))
(flow kyc-staleness-gate -> cbu-activation-task
  :default true)
```

---

#### Pack 10: multi-jurisdiction-overlay

**Description**: Routes to jurisdiction-specific decision processes based on the client's domicile or booking jurisdiction.

[GAP: variable-arity atom generation — N jurisdiction paths require N conditional flow atoms from the routing gateway. Template below is representative for 2 explicit jurisdictions plus default.]

```lisp
(decision-pack multi-jurisdiction-overlay
  :version "1.0.0"
  :description "Jurisdiction-conditional routing to jurisdiction-specific processes. Representative for 2 explicit jurisdictions plus default."
  :domain-scope [cbu kyc deal compliance]
  :parameters [
    {:name jur-gate-name        :type symbol   :required true}
    {:name jurisdiction-field   :type string   :required true
     :description "Data location holding the ISO jurisdiction code"}
    {:name jurisdiction-a       :type string   :required true
     :description "Jurisdiction code for the first explicit path (e.g. \"GB\")"}
    {:name path-a               :type node-ref :required true}
    {:name jurisdiction-b       :type string   :required true}
    {:name path-b               :type node-ref :required true}
    {:name default-path         :type node-ref :required true
     :description "Path for all other jurisdictions"}
  ]
  :template [
    (gateway ,jur-gate-name :kind exclusive)
    (flow $pre-node -> ,jur-gate-name)
    (flow ,jur-gate-name -> ,path-a
      :condition (= ,jurisdiction-field ,jurisdiction-a))
    (flow ,jur-gate-name -> ,path-b
      :condition (= ,jurisdiction-field ,jurisdiction-b))
    (flow ,jur-gate-name -> ,default-path
      :default true)
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
```

**Worked instantiation** — *"route UK clients to the CASS-specific onboarding track, EU clients to the MiFID track, everyone else to the global standard track."*

Parameters:
```
jur-gate-name:      booking-jurisdiction-gate
jurisdiction-field: "booking-jurisdiction"
jurisdiction-a:     "GB"
path-a:             cass-onboarding-subprocess
jurisdiction-b:     "EU"
path-b:             mifid-onboarding-subprocess
default-path:       global-standard-onboarding-subprocess
$pre-node:          client-data-verified
```

---

#### Pack 11: sanction-hit-escalation

**Description**: Dedicated sanctions check node followed by a hard-block gateway. Any positive hit escalates regardless of other workflow state.

```lisp
(decision-pack sanction-hit-escalation
  :version "1.0.0"
  :description "Sanctions check service task; hard-block exclusive gateway: any hit value escalates immediately."
  :domain-scope [cbu kyc screening compliance]
  :parameters [
    {:name sanctions-check-name :type symbol   :required true
     :description "Name for the generated sanctions check service task node"}
    {:name sanctions-gate-name  :type symbol   :required true}
    {:name sanctions-field      :type string   :required true
     :description "Data location where the sanctions check writes its result"}
    {:name hit-value            :type string   :required false :default "hit"
     :description "The result value that constitutes a hit"}
    {:name escalation-path      :type node-ref :required true}
    {:name clear-path           :type node-ref :required true}
  ]
  :template [
    (node ,sanctions-check-name :kind service-task
      :verb (invoke screening.check-sanctions
        :args {:result-field ,sanctions-field}))
    (flow $pre-node -> ,sanctions-check-name)
    (gateway ,sanctions-gate-name :kind exclusive)
    (flow ,sanctions-check-name -> ,sanctions-gate-name)
    (flow ,sanctions-gate-name -> ,escalation-path
      :condition (= ,sanctions-field ,hit-value))
    (flow ,sanctions-gate-name -> ,clear-path
      :default true)
  ]
  :example-utterances [
    "if there's a sanctions match, immediately escalate to compliance"
    "sanctions hit → hard block, route to compliance officer"
    "screening: positive sanctions result overrides everything"
    "any sanctions hit must go to manual review regardless"
    "hard block on sanctions: escalate immediately"
  ]
  :structural-signature {
    :check-kind    sanctions-lookup
    :gateway-kind  exclusive
    :hard-block    true
    :outcomes      2
  }
  :governance-ref sanction-hit-escalation-v1-status)

(governance-status sanction-hit-escalation-v1-status
  :atom sanction-hit-escalation
  :state active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

**Worked instantiation** — *"run a sanctions check on the entity; if there's a hit, route to the compliance hold queue; if clear, proceed to KYC review."*

Parameters:
```
sanctions-check-name: entity-sanctions-check
sanctions-gate-name:  sanctions-outcome-gate
sanctions-field:      "entity-sanctions-result"
hit-value:            "hit"
escalation-path:      compliance-hold-task
clear-path:           kyc-review-task
$pre-node:            entity-data-loaded
```

Expanded DSL:
```lisp
(node entity-sanctions-check :kind service-task
  :verb (invoke screening.check-sanctions
    :args {:result-field "entity-sanctions-result"}))
(flow entity-data-loaded -> entity-sanctions-check)
(gateway sanctions-outcome-gate :kind exclusive)
(flow entity-sanctions-check -> sanctions-outcome-gate)
(flow sanctions-outcome-gate -> compliance-hold-task
  :condition (= "entity-sanctions-result" "hit"))
(flow sanctions-outcome-gate -> kyc-review-task
  :default true)
```

---

#### Pack 12: manual-override-checkpoint

**Description**: Automatic decision computed by a business-rule-task; result presented to a human reviewer; human can confirm or override.

```lisp
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
    (node ,auto-eval-name :kind business-rule-task
      :verb (invoke switch.evaluate-decision
        :args {:decision ,auto-decision :output-field ,auto-result-field}))
    (flow $pre-node -> ,auto-eval-name)
    (node ,review-task-name :kind user-task
      :verb (invoke workflow.present-for-override
        :args {:auto-result ,auto-result-field :reviewer-role ,reviewer-role}))
    (flow ,auto-eval-name -> ,review-task-name)
    (gateway ,override-gate-name :kind exclusive)
    (flow ,review-task-name -> ,override-gate-name)
    (flow ,override-gate-name -> ,override-path
      :condition (= override-decision "override"))
    (flow ,override-gate-name -> ,confirmed-path
      :default true)
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
```

**Worked instantiation** — *"automatically classify the client's risk profile; show the classification to the risk officer for sign-off; if they override, route to enhanced review; otherwise proceed to standard onboarding."*

Parameters:
```
auto-eval-name:     risk-auto-classifier
review-task-name:   risk-officer-sign-off
override-gate-name: risk-override-gate
auto-decision:      client-risk-profile-decision
reviewer-role:      "risk-officer"
auto-result-field:  "auto-risk-classification"
confirmed-path:     standard-onboarding-task
override-path:      enhanced-review-task
$pre-node:          client-data-enriched
```

Expanded DSL:
```lisp
(node risk-auto-classifier :kind business-rule-task
  :verb (invoke switch.evaluate-decision
    :args {:decision client-risk-profile-decision
           :output-field "auto-risk-classification"}))
(flow client-data-enriched -> risk-auto-classifier)
(node risk-officer-sign-off :kind user-task
  :verb (invoke workflow.present-for-override
    :args {:auto-result "auto-risk-classification"
           :reviewer-role "risk-officer"}))
(flow risk-auto-classifier -> risk-officer-sign-off)
(gateway risk-override-gate :kind exclusive)
(flow risk-officer-sign-off -> risk-override-gate)
(flow risk-override-gate -> enhanced-review-task
  :condition (= override-decision "override"))
(flow risk-override-gate -> standard-onboarding-task
  :default true)
```

---

**Governance note on all 12 packs**: this is the seed catalogue. All 12 carry `governance-status.state = active` in v0.1. The catalogue grows through the same governance process as other SemOS artifacts. New packs follow Draft → Active lifecycle. Pack retirement requires identifying all processes with provenance atoms referencing the retiring pack (via a provenance query on the compiled source store) and migrating them. See §8.5.

---

### 8.4 Sage interaction model

> **Patched section — replaces Session 3 v0.1 §8.4. Substantive content preserved; syntax examples updated.**

The interaction pattern for pack-based authoring is documented here as an architectural responsibility, not a runtime requirement.

**Multi-modal matching** uses three signals (utterance similarity, `:structural-signature` map matching, domain context). The signals are combined as a weighted sum; weights default to 0.5 / 0.3 / 0.2 (utterance similarity dominant; `:structural-signature` prevents false-positive matches on superficially similar phrases with different structural requirements). The `:structural-signature` map is machine-readable — keys like `:conditions-composition`, `:gateway-kind`, `:outcomes` are extracted from the utterance and matched structurally. The earlier prose `:structural-intent` string from the pre-patch schema was not machine-matchable; the replacement map is.

**Confidence threshold**: Sage presents candidates with confidence > 0.6. Below that threshold, Sage defaults to bespoke authoring mode — hand-authored atoms with `(provenance :source hand-authored ...)`.

**Parameter extraction**: Sage uses the verb argument extraction pipeline (existing LLM-based arg extraction, 200–500ms) to identify parameter values from the user's utterance. For `list-of-condition-expr` parameters, the extraction prompt is specialised to identify boolean conditions in natural language and parse them into unified DSL condition expressions. Extracted conditions are displayed to the user as a bulleted list for confirmation before instantiation.

**Insertion-point resolution**: Sage identifies the `$pre-node` attachment point from the user's description of where in the process the decision section belongs. If the user says "after the intake form is submitted", Sage resolves `$pre-node` to the `intake-form` node (or the node immediately downstream of it, if the intake form's outgoing flow is already occupied). Attachment ambiguities are surfaced as clarification questions before instantiation.

**Idempotent instantiation**: if the user triggers a second Sage authoring session targeting the same process attachment point, Sage detects existing provenance atoms covering that structural neighbourhood and offers to replace (re-instantiate with updated parameters) or extend.

---

### 8.5 Pack governance lifecycle

> **Patched section — replaces Session 3 v0.1 §8.5. Content preserved.**

**FSM states and transitions:**

| State | Meaning | Sage uses it? | Existing processes run? |
|---|---|---|---|
| `draft` | Under review; not yet approved | No | N/A |
| `active` | Approved; available for authoring | Yes | Yes |
| `deprecated` | Retained; new authoring discouraged | Warn (not default) | Yes |
| `retired` | Not instantiable; migration required | No | No (compilation fails on new provenance) |

**Transition triggers:**

- `draft → active`: approval from designated approver (role: `registry-approver`). Logged in the `governance-status` atom with `:approver` and `:approved-at`.
- `active → deprecated`: governance decision (pack has a better replacement, or usage is declining). `:retires-at` field on the `governance-status` atom warns users of the planned retirement date.
- `deprecated → retired`: at `:retires-at` date, or via explicit `governance.retire-pack` verb.
- `retired → (none)`: terminal. The pack template remains in the registry for provenance resolution but cannot be instantiated.

**Effect on existing processes at deprecation and retirement:**

Deprecation: no immediate effect. Processes that reference a `deprecated` pack in a provenance atom compile with a `DeprecatedPackVersion` warning. Operators are notified via a scheduled query on provenance atoms referencing deprecated packs.

Retirement: processes with provenance atoms referencing the retired pack version fail compilation with `RetiredPackVersion` error. The structural atoms themselves remain valid and executable — they are standalone. Only the provenance annotation is invalid. Operators must either:
1. Delete the provenance atom (the process continues to work; it loses pack traceability).
2. Re-author the decision nodes with the current active pack version — Sage produces new provenance atoms pointing to the new version.

**Sage's catalogue view**: Sage queries the pack catalogue filtered by `governance-status.state = active`. Deprecated packs appear with a visual indicator ("deprecated — replacement available"). Retired packs do not appear.

---

## 9.x Example 12: Pack-authored process with provenance

> **Patched section — replaces Session 3 v0.1 §9 Example 12. Sage transcript preserved. DSL output updated to reflect the new `conjunctive-gate` template (gate-name as explicit parameter; `$pre-node` insertion point shown; provenance `:params` includes gate-name).**

**BPMN intent**: A Sage-authored decision fragment using the `conjunctive-gate` pack. Three conditions (KYC approved, UBO resolved, sanctions clear) must all hold before activation.

**Sage interaction transcript:**

```
User: before the client can go live, three things must all be true: KYC approved,
      UBO hierarchy resolved, and no sanctions hits. If all three, proceed to
      activate. Otherwise, route to compliance review.

Sage: I've matched your description to the conjunctive-gate pattern (confidence 0.91).
      This pattern emits a single exclusive gateway after checking all your conditions;
      all must hold to proceed.

      Shall I use this pack? Alternative: linked-switch-chain (evaluates conditions
      sequentially with individual rejection paths, confidence 0.67).

User: conjunctive-gate is correct.

Sage: I'll extract:
      gate-name:     activation-eligibility-gate
      conditions:    (= kyc-case.status approved)
                     (= ubo-status resolved)
                     (= sanctions-result clear)
      proceed-to:    activate-cbu-task
      review-path:   compliance-review-task
      attach-after:  pre-activation-check

      Is that correct?

User: Correct.

Sage: [Instantiates pack; produces expanded DSL + provenance atom]
```

**Resulting DSL (output of Sage instantiation):**

The template (from §8.3 Pack 1) before substitution:
```lisp
; Template body — annotated with substitution forms
(gateway ,gate-name :kind exclusive)           ; ,gate-name → activation-eligibility-gate
(flow $pre-node -> ,gate-name)                 ; $pre-node → pre-activation-check
(flow ,gate-name -> ,enhanced-path             ; ,enhanced-path → activate-cbu-task
  :condition (and ,@conditions))               ; ,@conditions → splice of 3 conditions
(flow ,gate-name -> ,standard-path             ; ,standard-path → compliance-review-task
  :default true)
```

After substitution and `$pre-node` resolution:
```lisp
(gateway activation-eligibility-gate :kind exclusive)
(flow pre-activation-check -> activation-eligibility-gate)
(flow activation-eligibility-gate -> activate-cbu-task
  :condition (and (= kyc-case.status approved)
                  (= ubo-status resolved)
                  (= sanctions-result clear)))
(flow activation-eligibility-gate -> compliance-review-task
  :default true)

(provenance activation-eligibility-gate-prov
  :covers [activation-eligibility-gate
           pre-activation-check->activation-eligibility-gate
           activation-eligibility-gate->activate-cbu-task
           activation-eligibility-gate->compliance-review-task]
  :source       pack
  :source-id    conjunctive-gate
  :version      "1.0.0"
  :session      "sess-019e4a1f-3b22-7e01-9f01-23f456789abc"
  :authored-at  "2026-05-21T12:00:00Z"
  :confirmed-at "2026-05-21T12:00:28Z"
  :params {
    :gate-name      activation-eligibility-gate
    :conditions     ["(= kyc-case.status approved)"
                     "(= ubo-status resolved)"
                     "(= sanctions-result clear)"]
    :enhanced-path  activate-cbu-task
    :standard-path  compliance-review-task
  })
```

Note: `$pre-node` (`pre-activation-check`) is **not** in the `:params` map. Insertion-point markers are resolved by Sage from authoring context, not supplied as pack parameters. The provenance atom records pack parameters only.

**Validate response assertions:**
- Zero errors, zero warnings.
- `graph.nodes` contains `activation-eligibility-gate` with kind `exclusive`.
- `graph.edges` contains the conditional flow (three-condition `and` expression) and the default flow.
- `provenance_summary.instantiations[0].pack_id = "conjunctive-gate"`.
- `provenance_summary.instantiations[0].covered_atoms` = 4 structural atoms (gateway + 3 flows including the pre-node attachment flow).
- `provenance_summary.uncovered_atoms` = [] (all structural atoms in this fragment are covered).
- The `(provenance ...)` atom does not appear in the `JourneySpec` output of the lowering pass — declarative atoms are dropped at Pass 3.

**Compilation note**: The `(provenance ...)` atom is parsed as `kind_class: Declarative`. The lowering pass drops it. The `JourneySpec` contains only the gateway node and its three outgoing flows — identical to what would be produced if the provenance atom had never been written. The pack's identity is preserved only in the DSL source, not in the executable form.

---

## Appendix E: Decision Pack Catalogue — Full Definitions

> **Patched section — replaces Session 3 v0.1 Appendix E (which was a summary table only). This appendix provides the full atom definition for each of the 12 seed packs in Session 1 syntax, suitable for registry import. §8.3 and this appendix are consistent; §8.3 may abbreviate; Appendix E is canonical.**

All 12 packs carry `:state active` at v0.1. The `(governance-status ...)` atoms are listed once here as the canonical governance record for each pack.

---

### E.1 conjunctive-gate — v1.0.0

```lisp
(decision-pack conjunctive-gate
  :version "1.0.0"
  :description "All N conditions must be satisfied to take the enhanced path; any failure routes to the standard path. Single exclusive gateway. N conditions supplied as list-of-condition-expr; spliced into (and ...) at instantiation."
  :domain-scope [cbu kyc onboarding screening]
  :parameters [
    {:name conditions      :type list-of-condition-expr :required true
     :description "Boolean conditions that must ALL be true for the enhanced path. Sage extracts from natural language description."}
    {:name gate-name       :type symbol   :required true
     :description "Atom name for the generated gateway. Must be unique in the target process."}
    {:name enhanced-path   :type node-ref :required true
     :description "Target node when all conditions hold."}
    {:name standard-path   :type node-ref :required true
     :description "Target node (default) when any condition fails."}
  ]
  :template [
    (gateway ,gate-name :kind exclusive)
    (flow $pre-node -> ,gate-name)
    (flow ,gate-name -> ,enhanced-path
      :condition (and ,@conditions))
    (flow ,gate-name -> ,standard-path
      :default true)
  ]
  :example-utterances [
    "all checks must pass before activation"
    "only proceed if KYC, screening, and UBO are all approved"
    "all conditions satisfied → enhanced path, otherwise standard"
    "when every requirement is met, route to fast track"
    "all of these must be true before we can proceed"
  ]
  :structural-signature {
    :conditions-composition and
    :gateway-kind           exclusive
    :outcomes               2
  }
  :governance-ref conjunctive-gate-v1-status)

(governance-status conjunctive-gate-v1-status
  :atom    conjunctive-gate
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.2 disjunctive-gate — v1.0.0

```lisp
(decision-pack disjunctive-gate
  :version "1.0.0"
  :description "Any one of N conditions triggers the escalation path; all must fail to take the standard path. Single exclusive gateway."
  :domain-scope [cbu kyc screening onboarding]
  :parameters [
    {:name conditions      :type list-of-condition-expr :required true
     :description "Boolean conditions; any one being true routes to escalation."}
    {:name gate-name       :type symbol   :required true}
    {:name escalation-path :type node-ref :required true
     :description "Target node when any condition holds."}
    {:name standard-path   :type node-ref :required true
     :description "Target node (default) when no condition holds."}
  ]
  :template [
    (gateway ,gate-name :kind exclusive)
    (flow $pre-node -> ,gate-name)
    (flow ,gate-name -> ,escalation-path
      :condition (or ,@conditions))
    (flow ,gate-name -> ,standard-path
      :default true)
  ]
  :example-utterances [
    "if any red flag is present, escalate"
    "any one of these conditions triggers enhanced review"
    "escalate if KYC rejected OR sanctions hit OR PEP positive"
    "if any risk indicator fires, route to compliance"
    "any of these conditions → heightened scrutiny"
  ]
  :structural-signature {
    :conditions-composition or
    :gateway-kind           exclusive
    :outcomes               2
  }
  :governance-ref disjunctive-gate-v1-status)

(governance-status disjunctive-gate-v1-status
  :atom    disjunctive-gate
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.3 linked-switch-chain — v1.0.0

```lisp
(decision-pack linked-switch-chain
  :version "1.0.0"
  :description "Sequential exclusive gateways; each checks one condition and may fast-exit to its dedicated rejection path. Template is for N=2 checks. [GAP: variable-arity atom generation deferred to v0.2; N>=3 chains require repeated atom generation not supported by v0.1 ,@name splice.]"
  :domain-scope [cbu kyc onboarding]
  :parameters [
    {:name gate-1-name  :type symbol       :required true}
    {:name gate-2-name  :type symbol       :required true}
    {:name condition-1  :type condition-expr :required true
     :description "First check. If condition-1 is false, exit to exit-path-1."}
    {:name condition-2  :type condition-expr :required true
     :description "Second check. If condition-2 is false, exit to exit-path-2."}
    {:name exit-path-1  :type node-ref     :required true}
    {:name exit-path-2  :type node-ref     :required true}
    {:name final-path   :type node-ref     :required true
     :description "Path taken when both checks pass."}
  ]
  :template [
    (gateway ,gate-1-name :kind exclusive)
    (flow $pre-node -> ,gate-1-name)
    (flow ,gate-1-name -> ,exit-path-1
      :condition (not ,condition-1))
    (flow ,gate-1-name -> ,gate-2-name
      :default true)
    (gateway ,gate-2-name :kind exclusive)
    (flow ,gate-2-name -> ,exit-path-2
      :condition (not ,condition-2))
    (flow ,gate-2-name -> ,final-path
      :default true)
  ]
  :example-utterances [
    "first verify identity, then check sanctions — exit early on any failure"
    "sequential checks with early exit on failure"
    "step-by-step eligibility: verify each requirement in order"
    "chain of compliance checks, each with a rejection path"
    "waterfall decision: each gate can reject before the next"
  ]
  :structural-signature {
    :evaluation-order sequential
    :gateway-kind     exclusive
    :early-exit       true
    :fixed-checks     2
  }
  :governance-ref linked-switch-chain-v1-status)

(governance-status linked-switch-chain-v1-status
  :atom    linked-switch-chain
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.4 parallel-evaluation-with-veto — v1.0.0

```lisp
(decision-pack parallel-evaluation-with-veto
  :version "1.0.0"
  :description "Two parallel evaluation tasks; parallel fork → parallel join with union merge on veto field → exclusive gateway routing on veto-in-set. Template for N=2 tasks. [GAP: variable-arity deferred to v0.2.]"
  :domain-scope [cbu kyc screening]
  :parameters [
    {:name fork-name       :type symbol   :required true}
    {:name join-name       :type symbol   :required true}
    {:name post-join-gate  :type symbol   :required true}
    {:name eval-task-1     :type node-ref :required true}
    {:name eval-task-2     :type node-ref :required true}
    {:name veto-field      :type string   :required false :default "veto-result"}
    {:name vetoed-path     :type node-ref :required true}
    {:name approved-path   :type node-ref :required true}
  ]
  :template [
    (gateway ,fork-name :kind parallel)
    (flow $pre-node -> ,fork-name)
    (flow ,fork-name -> ,eval-task-1)
    (flow ,fork-name -> ,eval-task-2)
    (parallel-join ,join-name
      :expects [,fork-name]
      :merge [{:location ,veto-field :operator union}])
    (flow ,eval-task-1 -> ,join-name)
    (flow ,eval-task-2 -> ,join-name)
    (gateway ,post-join-gate :kind exclusive)
    (flow ,join-name -> ,post-join-gate)
    (flow ,post-join-gate -> ,vetoed-path
      :condition (in "vetoed" ,veto-field))
    (flow ,post-join-gate -> ,approved-path
      :default true)
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
  :atom    parallel-evaluation-with-veto
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.5 cascading-decision — v1.0.0

```lisp
(decision-pack cascading-decision
  :version "1.0.0"
  :description "Two-stage decision: first business-rule-task classifies; exclusive gateway routes to one of two downstream paths based on classification output. [GAP: N>2 secondary paths require variable-arity flow generation deferred to v0.2.]"
  :domain-scope [cbu kyc deal]
  :parameters [
    {:name primary-eval-name  :type symbol       :required true}
    {:name primary-gate-name  :type symbol       :required true}
    {:name primary-decision   :type decision-ref :required true}
    {:name output-field       :type string       :required true}
    {:name class-a-value      :type string       :required true}
    {:name path-a             :type node-ref     :required true}
    {:name path-b             :type node-ref     :required true
     :description "Default path for all other classifications."}
  ]
  :template [
    (node ,primary-eval-name :kind business-rule-task
      :verb (invoke switch.evaluate-decision
        :args {:decision ,primary-decision :output-field ,output-field}))
    (flow $pre-node -> ,primary-eval-name)
    (gateway ,primary-gate-name :kind exclusive)
    (flow ,primary-eval-name -> ,primary-gate-name)
    (flow ,primary-gate-name -> ,path-a
      :condition (= ,output-field ,class-a-value))
    (flow ,primary-gate-name -> ,path-b
      :default true)
  ]
  :example-utterances [
    "first classify by entity type, then apply the appropriate rules for that type"
    "two-stage decision: entity type determines which ruleset applies"
    "primary classification feeds secondary decision"
    "the first check determines which second check to run"
    "cascading rules: output of step 1 selects step 2"
  ]
  :structural-signature {
    :stages                       2
    :evaluation-order             sequential
    :gateway-kind                 exclusive
    :first-output-drives-second   true
  }
  :governance-ref cascading-decision-v1-status)

(governance-status cascading-decision-v1-status
  :atom    cascading-decision
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.6 decision-table-classification — v1.0.0

```lisp
(decision-pack decision-table-classification
  :version "1.0.0"
  :description "Single business-rule-task evaluates a named decision table; exclusive gateway routes on output. Template for one explicit classification value plus default. [GAP: N>1 explicit classifications require variable-arity flow generation deferred to v0.2.]"
  :domain-scope [cbu kyc deal im]
  :parameters [
    {:name classify-name    :type symbol       :required true}
    {:name route-gate-name  :type symbol       :required true}
    {:name decision         :type decision-ref :required true}
    {:name output-field     :type string       :required true}
    {:name class-a-value    :type string       :required true}
    {:name path-a           :type node-ref     :required true}
    {:name default-path     :type node-ref     :required true}
  ]
  :template [
    (node ,classify-name :kind business-rule-task
      :verb (invoke switch.evaluate-decision
        :args {:decision ,decision :output-field ,output-field}))
    (flow $pre-node -> ,classify-name)
    (gateway ,route-gate-name :kind exclusive)
    (flow ,classify-name -> ,route-gate-name)
    (flow ,route-gate-name -> ,path-a
      :condition (= ,output-field ,class-a-value))
    (flow ,route-gate-name -> ,default-path
      :default true)
  ]
  :example-utterances [
    "classify the investor type and route accordingly"
    "use the risk classification table to determine next steps"
    "apply the CBU category ruleset and branch on result"
    "run the eligibility decision table"
    "DMN classification → routing"
  ]
  :structural-signature {
    :gateway-kind      exclusive
    :classification    true
    :hit-policy        dmn-compatible
    :outcomes          variable
  }
  :governance-ref decision-table-classification-v1-status)

(governance-status decision-table-classification-v1-status
  :atom    decision-table-classification
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.7 threshold-band-routing — v1.0.0

```lisp
(decision-pack threshold-band-routing
  :version "1.0.0"
  :description "Numeric input partitioned into 3 ordered bands; exclusive gateway routes each band to a distinct path. [GAP: band-list type with typed numeric bounds deferred to v0.2; this version uses 3 explicit threshold parameters.]"
  :domain-scope [cbu kyc ubo]
  :parameters [
    {:name band-gate-name  :type symbol  :required true}
    {:name input-field     :type string  :required true}
    {:name threshold-low   :type integer :required true
     :description "Upper bound of the low band (inclusive)."}
    {:name threshold-mid   :type integer :required true
     :description "Upper bound of the medium band (inclusive). Values above this go to path-high."}
    {:name path-low        :type node-ref :required true}
    {:name path-mid        :type node-ref :required true}
    {:name path-high       :type node-ref :required true}
  ]
  :template [
    (gateway ,band-gate-name :kind exclusive)
    (flow $pre-node -> ,band-gate-name)
    (flow ,band-gate-name -> ,path-low
      :condition (<= ,input-field ,threshold-low))
    (flow ,band-gate-name -> ,path-mid
      :condition (and (> ,input-field ,threshold-low)
                      (<= ,input-field ,threshold-mid)))
    (flow ,band-gate-name -> ,path-high
      :default true)
  ]
  :example-utterances [
    "route by ownership percentage: below 10% is minor, 10-25% is significant, above 25% is controlling"
    "tiered risk scoring: low/medium/high bands"
    "threshold-based routing on credit limit"
    "bands: 0-25% standard, 25-50% enhanced, 50%+ controlling"
    "ownership tier routing"
  ]
  :structural-signature {
    :input-kind      numeric
    :gateway-kind    exclusive
    :band-count      3
    :band-semantics  ordered-threshold
  }
  :governance-ref threshold-band-routing-v1-status)

(governance-status threshold-band-routing-v1-status
  :atom    threshold-band-routing
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.8 required-evidence-checklist — v1.0.0

```lisp
(decision-pack required-evidence-checklist
  :version "1.0.0"
  :description "Three sequential evidence tasks connected in order; exclusive gateway evaluates aggregate condition at the end. [GAP: variable-N sequential task connection deferred to v0.2.]"
  :domain-scope [cbu kyc onboarding]
  :parameters [
    {:name task-1               :type node-ref      :required true}
    {:name task-2               :type node-ref      :required true}
    {:name task-3               :type node-ref      :required true}
    {:name checklist-gate-name  :type symbol        :required true}
    {:name approval-path        :type node-ref      :required true}
    {:name rejection-path       :type node-ref      :required true}
    {:name aggregate-condition  :type condition-expr :required true
     :description "Boolean expression over evidence outputs that must hold for approval."}
  ]
  :template [
    (flow $pre-node -> ,task-1)
    (flow ,task-1 -> ,task-2)
    (flow ,task-2 -> ,task-3)
    (gateway ,checklist-gate-name :kind exclusive)
    (flow ,task-3 -> ,checklist-gate-name)
    (flow ,checklist-gate-name -> ,approval-path
      :condition ,aggregate-condition)
    (flow ,checklist-gate-name -> ,rejection-path
      :default true)
  ]
  :example-utterances [
    "collect and verify all required documents before making a decision"
    "sequential evidence checklist: ID, address, source of wealth"
    "each piece of evidence must be verified in order"
    "step-by-step document verification before final approval"
    "checklist: all evidence collected and verified → proceed"
  ]
  :structural-signature {
    :evaluation-order     sequential
    :evidence-collection  true
    :final-gateway        exclusive
    :outcomes             2
  }
  :governance-ref required-evidence-checklist-v1-status)

(governance-status required-evidence-checklist-v1-status
  :atom    required-evidence-checklist
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.9 periodic-refresh-trigger — v1.0.0

```lisp
(decision-pack periodic-refresh-trigger
  :version "1.0.0"
  :description "Checks the age of a timestamp field against a threshold in months. If stale, routes to refresh; otherwise continues."
  :domain-scope [cbu kyc periodic-review]
  :parameters [
    {:name age-gate-name     :type symbol  :required true}
    {:name timestamp-field   :type string  :required true}
    {:name threshold-months  :type integer :required true}
    {:name refresh-path      :type node-ref :required true}
    {:name current-path      :type node-ref :required true}
  ]
  :template [
    (gateway ,age-gate-name :kind exclusive)
    (flow $pre-node -> ,age-gate-name)
    (flow ,age-gate-name -> ,refresh-path
      :condition (> (months-since ,timestamp-field) ,threshold-months))
    (flow ,age-gate-name -> ,current-path
      :default true)
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
  :atom    periodic-refresh-trigger
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.10 multi-jurisdiction-overlay — v1.0.0

```lisp
(decision-pack multi-jurisdiction-overlay
  :version "1.0.0"
  :description "Routes to jurisdiction-specific processes via an exclusive gateway keyed on a jurisdiction string field. Template for 2 explicit jurisdictions plus default. [GAP: variable-N jurisdictions deferred to v0.2.]"
  :domain-scope [cbu kyc deal compliance]
  :parameters [
    {:name jur-gate-name       :type symbol   :required true}
    {:name jurisdiction-field  :type string   :required true}
    {:name jurisdiction-a      :type string   :required true}
    {:name path-a              :type node-ref :required true}
    {:name jurisdiction-b      :type string   :required true}
    {:name path-b              :type node-ref :required true}
    {:name default-path        :type node-ref :required true}
  ]
  :template [
    (gateway ,jur-gate-name :kind exclusive)
    (flow $pre-node -> ,jur-gate-name)
    (flow ,jur-gate-name -> ,path-a
      :condition (= ,jurisdiction-field ,jurisdiction-a))
    (flow ,jur-gate-name -> ,path-b
      :condition (= ,jurisdiction-field ,jurisdiction-b))
    (flow ,jur-gate-name -> ,default-path
      :default true)
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
  :atom    multi-jurisdiction-overlay
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.11 sanction-hit-escalation — v1.0.0

```lisp
(decision-pack sanction-hit-escalation
  :version "1.0.0"
  :description "Dedicated sanctions check service task followed by a hard-block exclusive gateway. Any match to hit-value escalates immediately."
  :domain-scope [cbu kyc screening compliance]
  :parameters [
    {:name sanctions-check-name :type symbol  :required true}
    {:name sanctions-gate-name  :type symbol  :required true}
    {:name sanctions-field      :type string  :required true}
    {:name hit-value            :type string  :required false :default "hit"}
    {:name escalation-path      :type node-ref :required true}
    {:name clear-path           :type node-ref :required true}
  ]
  :template [
    (node ,sanctions-check-name :kind service-task
      :verb (invoke screening.check-sanctions
        :args {:result-field ,sanctions-field}))
    (flow $pre-node -> ,sanctions-check-name)
    (gateway ,sanctions-gate-name :kind exclusive)
    (flow ,sanctions-check-name -> ,sanctions-gate-name)
    (flow ,sanctions-gate-name -> ,escalation-path
      :condition (= ,sanctions-field ,hit-value))
    (flow ,sanctions-gate-name -> ,clear-path
      :default true)
  ]
  :example-utterances [
    "if there's a sanctions match, immediately escalate to compliance"
    "sanctions hit → hard block, route to compliance officer"
    "screening: positive sanctions result overrides everything"
    "any sanctions hit must go to manual review regardless"
    "hard block on sanctions: escalate immediately"
  ]
  :structural-signature {
    :check-kind    sanctions-lookup
    :gateway-kind  exclusive
    :hard-block    true
    :outcomes      2
  }
  :governance-ref sanction-hit-escalation-v1-status)

(governance-status sanction-hit-escalation-v1-status
  :atom    sanction-hit-escalation
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.12 manual-override-checkpoint — v1.0.0

```lisp
(decision-pack manual-override-checkpoint
  :version "1.0.0"
  :description "Automated business-rule-task computes a decision; user-task presents it to a designated reviewer; reviewer may confirm (default path) or override (override path)."
  :domain-scope [cbu kyc compliance governance]
  :parameters [
    {:name auto-eval-name     :type symbol       :required true}
    {:name review-task-name   :type symbol       :required true}
    {:name override-gate-name :type symbol       :required true}
    {:name auto-decision      :type decision-ref :required true}
    {:name reviewer-role      :type string       :required true}
    {:name auto-result-field  :type string       :required true}
    {:name confirmed-path     :type node-ref     :required true}
    {:name override-path      :type node-ref     :required true}
  ]
  :template [
    (node ,auto-eval-name :kind business-rule-task
      :verb (invoke switch.evaluate-decision
        :args {:decision ,auto-decision :output-field ,auto-result-field}))
    (flow $pre-node -> ,auto-eval-name)
    (node ,review-task-name :kind user-task
      :verb (invoke workflow.present-for-override
        :args {:auto-result ,auto-result-field :reviewer-role ,reviewer-role}))
    (flow ,auto-eval-name -> ,review-task-name)
    (gateway ,override-gate-name :kind exclusive)
    (flow ,review-task-name -> ,override-gate-name)
    (flow ,override-gate-name -> ,override-path
      :condition (= override-decision "override"))
    (flow ,override-gate-name -> ,confirmed-path
      :default true)
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
  :atom    manual-override-checkpoint
  :state   active
  :approver "chief-compliance-architect"
  :approved-at "2026-05-21T00:00:00Z")
```

---

### E.13 Pack catalogue summary

| # | Pack | Composition | Gateway | Fixed/Variable | Domain |
|---|---|---|---|---|---|
| 1 | conjunctive-gate | AND(N) | exclusive | variable-condition, fixed-atom | KYC, onboarding |
| 2 | disjunctive-gate | OR(N) | exclusive | variable-condition, fixed-atom | KYC, screening |
| 3 | linked-switch-chain | sequential | exclusive | fixed-2 [GAP v0.2] | KYC, onboarding |
| 4 | parallel-evaluation-with-veto | parallel + veto | parallel → exclusive | fixed-2 [GAP v0.2] | Screening, KYC |
| 5 | cascading-decision | sequential 2-stage | exclusive | fixed-2-path [GAP v0.2] | CBU, KYC, deal |
| 6 | decision-table-classification | DMN table | exclusive | fixed-1-explicit [GAP v0.2] | CBU, KYC, deal, IM |
| 7 | threshold-band-routing | numeric band | exclusive | fixed-3-bands [GAP v0.2] | CBU, KYC, UBO |
| 8 | required-evidence-checklist | sequential evidence | exclusive | fixed-3-tasks [GAP v0.2] | KYC, onboarding |
| 9 | periodic-refresh-trigger | timestamp age | exclusive | fixed-atom | KYC, periodic review |
| 10 | multi-jurisdiction-overlay | jurisdiction routing | exclusive | fixed-2-jur [GAP v0.2] | CBU, KYC, compliance |
| 11 | sanction-hit-escalation | sanctions lookup | exclusive | fixed-atom | Screening, compliance |
| 12 | manual-override-checkpoint | auto + human | exclusive | fixed-atom | KYC, compliance |

**Note on "fixed-atom" vs "variable-condition"**: Packs 1 and 2 generate a fixed number of atoms (one gateway + three flows) regardless of how many conditions are supplied; the conditions are spliced into a single `(and ,@conditions)` or `(or ,@conditions)` expression. This is fully supported by v0.1 splice substitution. Packs 3, 4, 5, 6, 7, 8, 10 require generating N atoms from a list parameter — this is the v0.2 GAP.

**Internal consistency verification** (per patch prompt requirement):
- All `:template` bodies use `,name` or `,@name` only. No `$name` outside `$pre-node`. ✓
- Every pack has a `:structural-signature` map (not a string). ✓
- Every pack uses `:governance-ref` (not `:governance`). ✓
- All 12 pack names, descriptions, and example utterances preserved from Session 3 v0.1. ✓
- §9 Example 12's expanded DSL is consistent with the §8.3 conjunctive-gate template after substitution. ✓ (gate-name = activation-eligibility-gate; 3 conditions spliced into `and`; $pre-node = pre-activation-check; provenance :params includes gate-name.)
- Appendix E and §8.3 do not contradict each other. Appendix E is the canonical reference; §8.3 abbreviates. ✓
