# Unified DSL Atom Model and Verb Catalogues for ob-poc
## Design Document v0.1 — Session 1 of 3 (Regeneration)
### Unified DSL Atom Model · Verb Catalogues

> **Session series**: This is Session 1 of 3.
> Session 2: Compiler architecture + runtime design.
> Session 3: Regression strategy + decision pack catalogue + worked examples.
>
> Sessions 2 and 3 reference this document. All atom kinds, slot names, and signature shapes are citable by section and appendix reference. Sections are numbered to support cross-session citation.
>
> **Regeneration note**: This replaces the prior v0.1 Session 1. Two gaps are addressed: (1) `decision-pack` added as a structural atom kind in §3.3; (2) template substitution syntax (`,name`, `,@name`) added to §3.5. These are required for Session 3 to declare the 12 seed packs as DSL artifacts and to specify their template bodies. All other content from the original session is retained and refined.

---

## 1. Executive Summary

This document is the language foundation for one engineering phase of the ob-poc platform. The phase unifies SemOS and bpmn-lite under a single DSL family, refactors the compiler to a four-pass pipeline, builds a journey-persisted runtime, and catalogues decision shape patterns as governed artifacts.

Session 1 specifies two interlocking foundations:

**Deliverable A — Unified DSL atom model.** A single s-expression language where every source artifact is an unordered bag of typed atoms. Atoms are either structural (compiled into the executable form) or declarative (governance metadata, never compiled). Structural atom kinds are enumerated and closed within a language version. A new `decision-pack` structural atom kind captures parameterisable templates for decision shape patterns, using Lisp-style `,name`/`,@name` substitution in template bodies — the only macro-like construct in the language.

**Deliverable B — Verb catalogues.** The SemOS verb model (~1,098 verbs) reshaped against the unified atom model, with a mapping table and four reshape patterns. The bpmn-lite verb catalogue invented from scratch by reconciling against full BPMN 2.0 (OMG `formal/2013-12-09`) with Camunda 8's executable subset annotated within the reconciliation.

**Two "pack" concepts** coexist and must not be conflated:
- `graph-pack` — SemOS DAG taxonomy pack. Declares lifecycle state machines and workspace constraints for a domain. Present in the prior atom-kind catalogue.
- `decision-pack` — governed template for a decision shape pattern. Used by Sage at author time to instantiate decision-making sections of bpmn-lite processes. **New in this regeneration.**

Both are structural atoms. Different roles, different slot shapes, different purposes.

---

## 2. Architectural Commitments

The following ten decisions are locked for this phase. This section restates each with its rationale.

**Commitment 1: DSL is canonical executable source. S-expression syntax.**
Both SemOS and bpmn-lite use the same DSL family. BPMN/DMN XML is a migration input format only — not a primary authoring surface. S-expressions are greppable, diffable, composable with standard text tools, and have no impedance mismatch with the SemOS REPL.

**Commitment 2: DSL source is order-independent.**
Each source artifact is a bag of typed atoms. Atoms reference other atoms by name. Forward references are handled identically to backward references — the assembly pass constructs structure from the name-ref graph regardless of declaration order. Requiring canonical ordering would impose a false structural constraint on a system whose fundamental data structure is a DAG.

**Commitment 3: Atoms are structural or declarative.**
Structural atoms participate in compilation and appear in the executable form. Declarative atoms are governance metadata (provenance, lifecycle state, review annotations, jurisdiction tags). The compiler reads declarative atoms but does not emit them into the executable form. The catalogue of structural kinds is closed within a language version; the catalogue of declarative kinds is open (new declarative kinds can be introduced without compiler changes).

**Commitment 4: Nodes and edges are independent atom kinds in bpmn-lite.**
A `(node ...)` atom declares identity and kind. A `(flow ...)` atom declares source, target, and optional condition. Gateway fan-out is expressed through flow atoms, not embedded in the gateway atom body. This enables non-linear process descriptions and eliminates the embedded-fan-out awkwardness.

**Commitment 5: Verbs declare context dependencies via `@`-placeholder slots.**
`@node`, `@process`, `@token`, `@decision`, `@subprocess`, `@parent`. Verbs do not carry graph-position refs directly; they declare slot requirements that compilation passes bind from authoring context.

**Commitment 6: Boolean composition vocabulary is unified.**
The same atoms (`and`, `or`, `not`, `xor`, `implies`, `iff`, `if-then-else`) are used in flow conditions, decision rule bodies, predicate bodies, and parallel-join merge conditions. One expression sub-language across the entire DSL.

**Commitment 7: Cleanest verb sets.**
Backwards compatibility is not a constraint. SemOS verbs are reshaped against the unified atom model with documented mappings. bpmn-lite verbs are invented for full BPMN 2.0 coverage with Camunda 8 annotated within the reconciliation.

**Commitment 8: Provenance is preserved in source.**
A `(provenance ...)` declarative atom records the originating decision pack (or hand-authorship), pack version, authoring metadata, and the set of structural atoms it covers. Written at author time alongside the expanded structural atoms. The DSL source is therefore directly executable without pack expansion at compile time.

**Commitment 9: Decision packs are declared in the unified DSL.**
A `(decision-pack ...)` structural atom captures the pack's name, version, parameters, template body, example utterances, and structural signature. Packs are DSL artifacts — parseable, queryable, governable through the same machinery as verbs and graph-packs. The compiler validates template bodies (parameter name references, type compatibility) but does not expand them; expansion is Sage's concern at author time.

**Commitment 10: Template substitution is explicit and Lisp-style.**
Pack template bodies use `,name` for scalar substitution and `,@name` for splice substitution. These forms are **only** valid inside the `:template` slot of a `(decision-pack ...)` atom. Outside that scope they are static parse errors. Pack templates are the **only** macro-like construct in the unified DSL. The DSL is otherwise non-macro — no general `defmacro`, no syntax extension, no compile-time computation. This is a deliberate constraint.

---

## 3. Unified DSL Atom Model

### 3.1 Atom syntax and EBNF

Every atom has the form:

```
(atom-kind [name] :slot value ...)
```

`atom-kind` identifies the kind. `name` is an optional unquoted symbol used for inter-atom references. The body is a sequence of `:keyword value` pairs.

#### 3.1.1 Formal grammar (ISO/IEC 14977 EBNF)

```ebnf
source-file       = atom* ;

atom              = '(' atom-kind [ name ] slot* ')' ;
atom-kind         = symbol ;
name              = symbol ;

slot              = ':' keyword value ;
keyword           = symbol ;

value             = literal
                  | name-ref
                  | slot-ref
                  | template-subst       (* only valid in decision-pack :template *)
                  | template-splice      (* only valid in decision-pack :template *)
                  | atom
                  | list-value
                  | map-value
                  ;

literal           = string-literal
                  | integer-literal
                  | decimal-literal
                  | boolean-literal
                  | null-literal
                  | uuid-literal
                  ;

name-ref          = symbol                  (* unqualified: resolves in current scope *)
                  | qualified-name          (* 'pack-name/atom-name' *)
                  ;

slot-ref          = '@' symbol ;            (* context-injected slot reference *)

template-subst    = ',' symbol ;            (* scalar substitution — see §3.5.x *)
template-splice   = ',@' symbol ;           (* list splice substitution — see §3.5.x *)

list-value        = '[' value* ']' ;
map-value         = '{' ( ':' keyword value )* '}' ;

qualified-name    = symbol '/' symbol ;

string-literal    = '"' character* '"' ;
integer-literal   = ['-'] digit+ ;
decimal-literal   = ['-'] digit+ '.' digit+ ;
boolean-literal   = 'true' | 'false' ;
null-literal      = 'null' ;
uuid-literal      = uuid-string ;
symbol            = ( letter | '-' | '_' | '.' ) ( letter | digit | '-' | '_' | '.' )* ;
```

`template-subst` and `template-splice` are valid grammar productions but are **static errors** when they appear outside the `:template` slot of a `(decision-pack ...)` atom. The parser accepts them uniformly; the assembly pass enforces the scope restriction.

#### 3.1.2 `->` syntactic sugar

The `->` token (two characters, no space) is syntactic sugar for the positional source→target pair in `(flow ...)` atoms:

```lisp
(flow create-cbu-task -> type-gateway)
```
is syntactically equivalent to:
```lisp
(flow create-cbu-task type-gateway)
```

The lexer emits `Arrow` for `->`. The parser reorders: `(flow Arrow-lhs Arrow-rhs rest...)` → `(flow lhs rhs rest...)`.

---

### 3.2 Structural/declarative dichotomy

**Classification is per atom kind, not per instance.** Every atom kind is statically classified as structural or declarative in the atom kind registry (§3.2.1). There is no wrapper token. The parser tags each parsed atom with `kind_class: Structural | Declarative` based solely on its kind name.

#### 3.2.1 Atom kind classification

| Class | Kind names |
|---|---|
| Structural | `verb`, `invoke`, `node`, `gateway`, `flow`, `boundary-attachment`, `parallel-join`, `entity`, `relationship`, `predicate`, `decision`, `data-type`, `message-definition`, `timer-definition`, `error-definition`, `graph-pack`, `utterance-binding`, `constellation-root`, `workspace-constraint`, **`decision-pack`** |
| Declarative | `provenance`, `governance-status`, `review-annotation`, `jurisdiction-tag` |

Unknown kind in the structural namespace: parse error (`UnknownAtomKind`).
Unknown kind in the declarative namespace: warning (`UnknownDeclarativeKind`) — the declarative catalogue is open.
Unknown kind in neither: parse error (treated as structural namespace by default).

#### 3.2.2 Compiler treatment

**Pass 0 (parse)**: every atom tagged structural or declarative.

**Pass 1 (assembly)**: only structural atoms are processed. The assembly pass builds the process graph (bpmn-lite) or dependency DAG (SemOS) from structural atoms. Declarative atoms are carried in the AST bag but ignored by the structural algorithms. **Exception**: `(decision-pack ...)` atoms are read during assembly to validate that their template bodies reference only known parameter names and that substitution forms are used in type-compatible positions; the pack atom itself is indexed into the pack registry rather than incorporated into the process graph or dependency DAG.

**Pass 2 (resolution)**: resolves name-refs and @-slot bindings in structural atoms. Validates declarative atoms separately: checks that atom-refs in declarative atom slots resolve to structural atoms in the current source (or are known by FQN in the registry). Declarative validation produces warnings, not errors — a governance annotation may reference an atom defined in a separate source artifact loaded later.

**Pass 3 (lowering)**: produces the executable form. Declarative atoms are dropped. The executable form contains no declarative content. `(decision-pack ...)` atoms are also dropped from the executable form — they are not executable; they are templates.

**Runtime**: declarative atoms are never present in the runtime's view of the world. The runtime processes only the JourneySpec (bpmn-lite) or ExecutionPlan (SemOS) produced by lowering.

#### 3.2.3 Consequences

- Declarative atoms can be added, edited, or removed from a source artifact without changing the executable form's hash.
- A new declarative kind (e.g., `risk-annotation`) can be introduced by registering it as declarative — no compiler pass changes needed.
- The structural kind catalogue is closed, preventing uncontrolled language extension. Adding a new structural kind is a language version increment.
- `(decision-pack ...)` atoms are structural (and therefore versioned, hashed, governable) but are not compiled into the executable form. They live in the pack registry. This makes them first-class language citizens subject to governance without polluting the runtime.

---

### 3.3 Structural atom kinds

Full slot signatures for all structural atom kinds. See Appendix B for the consolidated reference.

#### 3.3.1 Language-shared structural atoms

---

**`(verb name :inputs I :outputs O :@-slots S :effects E :errors R)`** — §B.1

Declares a verb. Verbs are the unit of executable behaviour in both SemOS and bpmn-lite.

| Slot | Type | Required | Description |
|---|---|---|---|
| `:inputs` | list of input-def | yes | `{:name N :type T :required bool :default V?}` |
| `:outputs` | list of output-def | yes | `{:name N :type T}` |
| `:@-slots` | list of slot-ref | no | Context slots required at invocation site |
| `:effects` | list of effect-decl | no | `{:kind read|write|side-effect :target T :mode local|external}` |
| `:errors` | list of error-def | no | `{:code C :description D?}` |

---

**`(invoke verb-ref :@-bindings B :args A)`** — §B.2

Invokes a declared verb at an authoring site (a bpmn-lite node, or a SemOS runbook step).

| Slot | Type | Required | Description |
|---|---|---|---|
| `:@-bindings` | list of `{:slot @ref :value expr}` | no | Explicit @-slot overrides; normally injected by assembly pass |
| `:args` | map of name → value | yes | Argument values matching verb `:inputs` declarations |

---

**`(entity name :type T :attributes A)`** — §B.3

| Slot | Type | Required |
|---|---|---|
| `:type` | symbol (`entity`, `event`, `value-object`) | yes |
| `:attributes` | list of `{:name N :type T :required bool :default V?}` | no |

---

**`(relationship name :from E :to E :cardinality C :via attr?)`** — §B.4

| Slot | Type | Required |
|---|---|---|
| `:from` | name-ref (entity) | yes |
| `:to` | name-ref (entity) | yes |
| `:cardinality` | symbol (`one-to-one`, `one-to-many`, `many-to-many`) | yes |
| `:via` | symbol (junction attribute) | no |

---

**`(predicate name :inputs I :body expr)`** — §B.5

Named reusable boolean predicate.

| Slot | Type | Required |
|---|---|---|
| `:inputs` | list of `{:name N :type T}` | yes |
| `:body` | boolean composition expression | yes |

---

**`(decision name :inputs I :outputs O :hit-policy H :rules R)`** — §B.6

Inline decision table.

| Slot | Type | Required |
|---|---|---|
| `:inputs` | list of `{:name N :type T}` | yes |
| `:outputs` | list of `{:name N :type T}` | yes |
| `:hit-policy` | symbol (`first`, `unique`) | yes |
| `:rules` | list of `{:id ID :when expr :then {name value*}}` | yes |

---

**`(data-type name :base T :constraints C)`** — §B.7

| Slot | Type | Required |
|---|---|---|
| `:base` | symbol (`string`, `integer`, `decimal`, `uuid`, `date`, `timestamp`, `duration`, `boolean`) | yes |
| `:constraints` | list of `{:kind min|max|regex|enum :value V}` | no |

---

#### 3.3.2 The `decision-pack` structural atom — §B.8

`(decision-pack name :version V :description D :domain-scope S :parameters P :template T :example-utterances U :structural-signature sig? :governance-ref G?)`

This is the **new** structural atom kind added in this regeneration. It captures a parameterisable template for a decision shape pattern. Packs are first-class DSL artifacts: parsed, indexed, governed, and versioned. They are not compiled into the executable form; they live in the pack registry.

Full slot specification:

| Slot | Type | Required | Description |
|---|---|---|---|
| `:version` | string (semver) | yes | e.g. `"1.0.0"`. New versions create new pack entries; old versions remain queryable by provenance references. |
| `:description` | string | yes | Human-readable description of when this pack applies and what pattern it represents. |
| `:domain-scope` | list of symbols | yes | Domains where this pack is approved. e.g. `[kyc-onboarding screening]`. Sage uses this for context filtering. |
| `:parameters` | list of param-def | yes | Typed parameters Sage supplies at instantiation. Each param-def: `{:name N :type T :required bool :description D :default V?}`. Parameter types: `string`, `symbol`, `integer`, `boolean`, `node-ref`, `condition-expr`, `predicate-ref`, `list-of-predicate-ref`, `list-of-node-ref`, `decision-ref`, `path-map`. |
| `:template` | list of structural atoms | yes | The atoms emitted on instantiation, with `,name` and `,@name` substitution forms referencing parameter names. Template atoms are **not** compiled; they are expanded by Sage at author time. |
| `:example-utterances` | list of strings | yes | Natural language phrases Sage matches against for intent-to-pack mapping. |
| `:structural-signature` | map | no | Formal extraction shape for multi-modal matching: e.g. `{:predicates 3 :gateway-kind exclusive :outcomes 2}`. |
| `:governance-ref` | symbol | no | Name of a `(governance-status ...)` declarative atom tracking this pack's FSM lifecycle. If absent, pack is implicitly in `draft` state. |

**Assembly pass treatment of `(decision-pack ...)`:**
1. Parse template body as if it were normal atom content, with the addition that `template-subst` and `template-splice` productions (§3.1.1) are permitted inside `:template`.
2. Validate each substitution form: the symbol following `,` or `,@` must name a declared parameter in `:parameters`. Unknown parameter name → `UnknownTemplateParameter` error.
3. Type-check substitution use sites (§3.5.x).
4. Index the pack into the pack registry keyed by `(name, version)`.
5. Do **not** add the pack atom to the process graph or dependency DAG — it is not a node in either.

**The `:template` slot is a list of atoms, not a single atom.** Template bodies typically emit multiple structural atoms (a gateway node, multiple flow atoms, etc.). The splice form `,@conditions` in a template can expand a `list-of-predicate-ref` parameter into multiple condition expressions.

**Example** (see §3.11, Example 5 for the full worked illustration):

```lisp
(decision-pack conjunctive-gate
  :version "1.0.0"
  :description "All N conditions must hold to proceed to enhanced path; default routes to standard path."
  :domain-scope [kyc-onboarding screening cbu]
  :parameters [
    {:name conditions    :type list-of-condition-expr :required true
     :description "Conditions that must ALL be true for enhanced routing"}
    {:name gate-name     :type symbol :required true
     :description "Name for the generated gateway atom"}
    {:name enhanced-path :type node-ref :required true}
    {:name standard-path :type node-ref :required true}
  ]
  :template [
    (gateway ,gate-name :kind exclusive)
    (flow $pre-node -> ,gate-name)        ; $pre-node is the insertion point
    (flow ,gate-name -> ,enhanced-path
      :condition (and ,@conditions))
    (flow ,gate-name -> ,standard-path
      :default true)
  ]
  :example-utterances [
    "all checks must pass before activation"
    "only proceed if KYC, screening, and UBO are all approved"
    "all conditions satisfied → enhanced path"
    "when every requirement is met, proceed to fast track"
  ]
  :structural-signature {:conditions-composition and :gateway-kind exclusive :outcomes 2}
  :governance-ref conjunctive-gate-v1-status)
```

Note: `$pre-node` in the template above is a special insertion-point marker (not a parameter) indicating where the template attaches to the surrounding process. The exact attachment mechanism is part of the Sage/author-time expansion protocol and is not compiled by the language processor. The compiler validates that `gate-name`, `enhanced-path`, `standard-path`, and `conditions` are declared parameters; `$pre-node` is treated as an unresolved name-ref and left for the expansion step.

[GAP: the exact insertion-point protocol for template attachment to surrounding process context is deferred to the Session 3 pack catalogue specification. The compiler's job is parameter validation only.]

---

#### 3.3.3 bpmn-lite-specific structural atoms

---

**`(node name :kind K :verb V? :event-def E? :loop L? :multi-instance MI? :compensation-handler H?)`** — §B.9

| Slot | Type | Required |
|---|---|---|
| `:kind` | symbol — see node kind table §3.3.3.1 | yes |
| `:verb` | name-ref to an `(invoke ...)` atom | conditional (required for task kinds) |
| `:event-def` | name-ref to a message/timer/error-definition | conditional (required for event kinds with definitions) |
| `:loop` | `{:condition expr :max-count int?}` | no |
| `:multi-instance` | `{:cardinality sequential|parallel :data-input name?}` | no |
| `:compensation-handler` | name-ref to handler node | no |

##### 3.3.3.1 Node kind taxonomy

| Kind | Category | Camunda 8 |
|---|---|---|
| `start-event` | Event | ✓ |
| `start-event-message` | Event | ✓ |
| `start-event-timer` | Event | ✓ |
| `start-event-signal` | Event | ✓ |
| `start-event-error` | Event (event subprocess) | ✓ |
| `start-event-escalation` | Event | modeller |
| `start-event-compensation` | Event (transaction) | modeller |
| `end-event` | Event | ✓ |
| `end-event-message` | Event | ✓ |
| `end-event-error` | Event | ✓ |
| `end-event-signal` | Event | ✓ |
| `end-event-terminate` | Event | ✓ |
| `end-event-escalation` | Event | ✓ |
| `end-event-compensation` | Event | ✓ |
| `end-event-cancel` | Event (transaction) | ✓ |
| `intermediate-catch-message` | Event | ✓ |
| `intermediate-catch-timer` | Event | ✓ |
| `intermediate-catch-signal` | Event | ✓ |
| `intermediate-catch-link` | Event | ✓ |
| `intermediate-throw-message` | Event | ✓ |
| `intermediate-throw-signal` | Event | ✓ |
| `intermediate-throw-link` | Event | ✓ |
| `intermediate-throw-escalation` | Event | ✓ |
| `intermediate-throw-compensation` | Event | ✓ |
| `service-task` | Activity | ✓ |
| `user-task` | Activity | ✓ |
| `send-task` | Activity | ✓ |
| `receive-task` | Activity | ✓ |
| `manual-task` | Activity | ✓ (modeller) |
| `business-rule-task` | Activity | ✓ |
| `script-task` | Activity | ✓ |
| `subprocess` | Activity | ✓ |
| `event-subprocess` | Activity | ✓ |
| `transaction-subprocess` | Activity | ✓ |
| `call-activity` | Activity | ✓ |

---

**`(gateway name :kind G)`** — §B.10

| `:kind` value | BPMN 2.0 | Camunda 8 | bpmn-lite |
|---|---|---|---|
| `exclusive` | §13.3 | ✓ | ✓ |
| `inclusive` | §13.4 | ✓ | ✓ |
| `parallel` | §13.5 | ✓ | ✓ |
| `event-based` | §13.6 | ✓ | ✓ |
| `parallel-event-based` | §13.6.6 | modeller | ✓ |
| `complex` | §13.7 | not supported | ✗ rejected |

---

**`(flow source target :condition expr? :default bool?)`** — §B.11

Positional: `source` and `target` are name-refs to node or gateway atoms.

| Slot | Type | Required |
|---|---|---|
| `:condition` | boolean composition expression | no (absent = unconditional) |
| `:default` | bool | no (true = default flow for gateway) |

---

**`(boundary-attachment node event-kind :interrupting bool :event-def E? :compensation-handler H?)`** — §B.12

First positional argument is the host node name-ref.

| Slot | Type | Required |
|---|---|---|
| `:event-kind` | symbol (`error`, `timer`, `message`, `signal`, `escalation`, `compensation`, `cancel`) | yes |
| `:interrupting` | bool | yes |
| `:event-def` | name-ref to message/timer/error definition | conditional |
| `:compensation-handler` | name-ref to handler node | conditional (compensation events only) |

---

**`(parallel-join name :expects E :merge M?)`** — §B.13

Explicit parallel join with declared merge semantics.

| Slot | Type | Required |
|---|---|---|
| `:expects` | list of fork gateway name-refs | yes |
| `:merge` | list of `{:location data-ref :operator op :custom verb-ref?}` | no |

Merge operators: `max`, `min`, `union`, `concat`, `sum`, `latest`, `earliest`, `custom`.

---

**`(message-definition name :correlation-key expr :payload-schema S?)`** — §B.14

**`(timer-definition name :type duration|cycle|date :expression expr)`** — §B.15

**`(error-definition name :code str :description str?)`** — §B.16

---

#### 3.3.4 SemOS-specific structural atoms

---

**`(graph-pack name :domain D :slots S :constraints C?)`** — §B.17

SemOS DAG taxonomy pack. **Distinct from `decision-pack`.** Declares lifecycle state machines and cross-workspace constraints for a domain. Not a decision shape template; not used by Sage for process authoring.

| Slot | Type | Required |
|---|---|---|
| `:domain` | symbol | yes |
| `:slots` | list of slot-def: `{:name N :states [S*] :transitions [T*] :category-gated bool?}` | yes |
| `:constraints` | list of constraint-def: `{:mode A|B|C :condition expr :effect block|tollgate|cascade :description str?}` | no |

---

**`(utterance-binding name :phrases P :verb V :domain D?)`** — §B.18

NLCI verb binding.

| Slot | Type | Required |
|---|---|---|
| `:phrases` | list of strings | yes |
| `:verb` | name-ref | yes |
| `:domain` | symbol | no |

---

**`(constellation-root name :dag-ref R :workspace W)`** — §B.19

Maps a workspace kind to its governing graph-pack.

---

**`(workspace-constraint name :mode A|B|C :condition expr :effect E :description str?)`** — §B.20

Cross-workspace constraint.

| Slot | Type | Required |
|---|---|---|
| `:mode` | symbol (`A` = blocking gate, `B` = derived aggregate, `C` = cascade) | yes |
| `:condition` | boolean expression | yes |
| `:effect` | symbol (`block`, `tollgate`, `cascade`) | yes |

---

### 3.4 Declarative atom kinds

Declarative atoms carry governance metadata. They are parsed and validated but never compiled into the executable form or seen by the runtime.

---

**`(provenance name :covers C :source src :source-id S :version V :session sess :authored-at ts :confirmed-at ts? :params P?)`** — §B.21

Records authoring provenance of a set of structural atoms.

| Slot | Type | Required | Description |
|---|---|---|---|
| `:covers` | list of name-refs | yes | Structural atoms this provenance annotation covers |
| `:source` | symbol (`pack`, `hand-authored`, `migration-tool`) | yes | |
| `:source-id` | string | yes | Pack FQN for `:source pack`; user id for hand-authored; tool id for migration |
| `:version` | string | yes | Pack version (semver) or `"bespoke"` |
| `:session` | uuid | yes | Authoring session identifier |
| `:authored-at` | timestamp | yes | When Sage generated the expansion |
| `:confirmed-at` | timestamp | no | When the user confirmed |
| `:params` | map | no | Pack parameter values used |

See §3.10 for full provenance semantics.

---

**`(governance-status name :atom A :state S :approver apr? :approved-at ts? :retires-at ts? ...metadata)`** — §B.22

FSM lifecycle state. Valid `:state` values: `draft`, `active`, `deprecated`, `retired`. Additional metadata slots: `:flavour`, `:tier`, `:noun`, `:state-effect`, `:consequence-baseline`, `:consequence-escalation`, `:role-guard`, `:tags`, `:phase-tags`, `:source-of-truth`.

---

**`(review-annotation name :atom A :reviewer R :status S :reviewed-at ts :notes str?)`** — §B.23

`:status` values: `pending`, `passed`, `failed`, `waived`.

---

**`(jurisdiction-tag name :atom A :jurisdictions J :note str?)`** — §B.24

Restricts an atom's applicability to a set of jurisdiction codes.

---

**Extensibility**: new declarative kinds are introduced by registering the kind name as declarative. No compiler changes needed. Unknown declarative kinds produce `UnknownDeclarativeKind` warnings (not errors). The structural/declarative split makes governance tooling extensibility safe.

---

### 3.5 Reference model

**Atom names** are unique within a DSL source artifact. Duplicate names within a source are assembly errors.

**Qualified names**: `pack-name/atom-name`. Used for inter-artifact references.

**Forward references**: the assembly pass processes all atoms in the bag after building a complete name index. Unresolved refs at end of assembly are carried forward as `UnresolvedRef` nodes for the resolution pass.

#### 3.5.1 The @-slot family

| Slot | Resolved by | Description |
|---|---|---|
| `@node` | Assembly | The process node this `(invoke ...)` is bound to (bpmn-lite) |
| `@process` | Runtime | The enclosing process instance (instance_id) |
| `@token` | Runtime | The current execution token |
| `@decision` | Assembly | The decision context when invoked from a rule body |
| `@subprocess` | Assembly | The enclosing subprocess node |
| `@parent` | Runtime | The parent process instance (call activities) |

**Binding rules:**
- Assembly-resolved slots (`@node`, `@decision`, `@subprocess`): the assembly pass injects these from the structural context of the invocation site. A `@node` is always satisfied by any enclosing `(node ...)` context. Missing required assembly-resolved slots → `MissingAtSlotBinding` error, attributed to the `(invoke ...)` atom.
- Runtime-resolved slots (`@process`, `@token`, `@parent`): marked `RuntimeBound`; no assembly action. The runtime injects at execution time.
- Type compatibility is checked at the resolution pass against the verb's `@-slots` declaration.

#### 3.5.x Template substitution syntax

Pack templates in `(decision-pack :template [...])` use two substitution forms:

**Scalar substitution** — `,name`

Replaces the form with the parameter value as a single value. Valid in any slot value position within the template body.

```lisp
; Pack parameter: {:name threshold :type integer :required true}
; Template usage:
(predicate ownership-controlling
  :inputs [{:name pct :type decimal}]
  :body (>= pct ,threshold))

; After instantiation with threshold = 25:
(predicate ownership-controlling
  :inputs [{:name pct :type decimal}]
  :body (>= pct 25))
```

**List splice substitution** — `,@name`

Replaces the form by splicing the parameter value's elements into the surrounding list. The parameter must be a list-typed parameter. Used for variable-length argument sequences — most commonly when a pack accepts N conditions and splices them into an `and` or `or` expression.

```lisp
; Pack parameter: {:name conditions :type list-of-condition-expr :required true}
; Template usage:
(flow ,gate-name -> ,enhanced-path
  :condition (and ,@conditions))

; After instantiation with conditions = [(= kyc-approved true) (= sanctions-clear true) (= ubo-resolved true)]:
(flow approval-gate -> fast-track-path
  :condition (and (= kyc-approved true) (= sanctions-clear true) (= ubo-resolved true)))
```

**Scope restriction**: substitution forms are only valid inside the `:template` slot of a `(decision-pack ...)` atom. The assembly pass enforces this:
- `,name` or `,@name` outside a `(decision-pack :template ...)` body → `TemplateSubstOutsidePackTemplate` static error.

**Static type checking of substitutions** (performed by the assembly pass on the `(decision-pack ...)` atom):
- `,name` must reference a declared `:parameter` name. Unknown name → `UnknownTemplateParameter` error.
- `,@name` must reference a list-typed parameter (`list-of-predicate-ref`, `list-of-condition-expr`, `list-of-node-ref`, etc.). Using `,@name` on a scalar parameter → `SpliceOnScalarParameter` error.
- `,name` (scalar) must be used where a single value is expected. Using `,name` on a list parameter in a position expecting a single value → `ScalarUseOfListParameter` error.

**When substitution happens**: at author time, when Sage instantiates a pack. The substitution is performed outside the compiler; the compiler never sees unresolved substitution forms in committed source. The resulting expanded structural atoms are written to source along with a `(provenance ...)` atom recording the instantiation. The compiler then compiles the expanded atoms as ordinary structural atoms.

**Pack templates are the only macro-like construct.** The DSL has no general `defmacro`, no syntax extension, no compile-time computation beyond template substitution. This is a deliberate constraint. Template substitution is a constrained, declarative parameterisation form — it produces a fixed set of structural atoms per instantiation; it cannot be used to alter the language's syntax or introduce new atom kinds.

---

### 3.6 Verb signature surface

The `(verb ...)` declaration (§3.3.1) applies to both SemOS verbs and bpmn-lite verbs. The key properties:

**Portability**: a verb declared once is invocable from multiple authoring sites. The `@`-slot mechanism provides structural context without embedding graph position into the verb declaration.

**Effects are declared**: every data location a verb reads or writes is declared in `:effects`. This is the basis for the compiler's dependency analysis (SemOS) and the runtime's conflict detection at parallel joins (bpmn-lite).

**Errors are declared**: every error kind a verb can raise is listed in `:errors`. This enables compile-time validation that boundary events reference declared error codes.

**No embedded routing**: verbs declare no routing logic. Gateway decisions are the switch adaptor's responsibility (Session 2). Verbs declare what they do; the process graph declares how control flows.

---

### 3.7 Effect model

Effect declarations on verbs:

```lisp
:effects [(kind target mode) ...]
```

| Field | Values | Description |
|---|---|---|
| `kind` | `read`, `write`, `side-effect` | What the verb does to the target |
| `target` | data-location-ref (entity/attribute path, token write log slot, external endpoint) | Named data location |
| `mode` | `local`, `external` | `local` = within transaction; `external` = crosses process boundary |

The effect model is used by:
- **Compiler (SemOS)**: dependency analysis — verbs that write data required by other verbs are ordered before them in the dependency DAG.
- **Runtime (bpmn-lite)**: conflict detection at parallel join — the `token-write-log` target kind names the slot in each token's write log that the merge protocol inspects.
- **Audit**: `external`-mode effects are recorded in the audit log for regulatory review.

---

### 3.8 Data model integration

**Type system position: structural nominal typing.**

Atom slots declare type names as symbols. The resolution pass resolves type names against the entity/data-type registry. Two atoms with the same type name are the same type. No structural equivalence checking; no inheritance hierarchy in v0.1.

Rationale: untyped-with-runtime-checks is insufficient for regulated finance — errors surface too late. Full structural or dependent typing adds complexity disproportionate to the domain's needs. Nominal typing provides adequate authoring-time safety.

**Primitive types**: `string`, `integer`, `decimal`, `boolean`, `uuid`, `date`, `timestamp`, `duration`.

**Domain types**: declared via `(data-type ...)` atoms or imported from the entity registry. The entity registry is authoritative; DSL refers to domain types by name.

[GAP: full type lattice and subtyping rules deferred to v0.2.]

---

### 3.9 Boolean composition sub-language

Used uniformly in: flow `:condition` expressions, `(predicate :body)`, `(decision :rules :when)`, `(parallel-join :merge :condition)`.

| Atom | Arity | Semantics |
|---|---|---|
| `(and e1 e2 ...)` | N | True iff all arguments true; left-to-right short-circuit |
| `(or e1 e2 ...)` | N | True iff any argument true; left-to-right short-circuit |
| `(not e)` | 1 | Logical negation |
| `(xor e1 e2)` | 2 | True iff exactly one argument true |
| `(implies e1 e2)` | 2 | `(or (not e1) e2)` |
| `(iff e1 e2)` | 2 | `(and (implies e1 e2) (implies e2 e1))` |
| `(if-then-else c t f)` | 3 | `c` true → `t`; else `f` |
| `(= a b)` | 2 | Equality |
| `(!= a b)` | 2 | Inequality |
| `(< a b)`, `(> a b)`, `(<= a b)`, `(>= a b)` | 2 | Numeric comparison |
| `(in x [v1 v2 ...])` | 2 | Set membership |
| `(not-in x [...])` | 2 | Set non-membership |

**Two-valued logic**: null on either side of a comparison is a runtime error with a diagnostic — not null-propagation, not false. This matches existing dmn-lite semantics and is simpler than SQL three-valued logic for the closed-domain custody banking context.

---

### 3.10 Provenance atom specification

A `(provenance ...)` atom records which structural atoms were generated from a decision pack instantiation (or hand-authored). Full schema in §3.4.

**The `(covers ...)` mechanism**: `:covers` is a list of name-refs to structural atoms in the current source. This is an explicit, queryable association enabling governance queries: "which atoms were generated from pack X version Y?"

**Multiple provenance atoms per source**: a source file may contain one provenance atom per pack instantiation and one per hand-authored section. Structural atoms with no covering provenance are "bespoke" — implicitly hand-authored, no annotation required.

**Overlapping coverage**: if two provenance atoms cover the same structural atom, the assembly pass emits a `DuplicateProvenanceCoverage` warning (not error). May indicate intentional dual-sourcing.

**Relationship between provenance references and `(decision-pack ...)` atoms**:

A `(provenance :source-id "conjunctive-gate" :version "1.0.0" ...)` reference points to a pack by FQN and version string. The referenced `(decision-pack conjunctive-gate :version "1.0.0" ...)` atom does **not** need to be present in the same source artifact. The compiler resolves pack references against the pack registry, which is populated from separately-loaded pack source artifacts. If the pack is unknown to the registry (not yet loaded), the compiler emits `UnknownPackReference` warning — not an error — because the pack may be defined in a separately-deployed artifact. The structural atoms covered by the provenance atom are compiled and executable regardless of whether the pack is resolved.

**Pack version change behaviour**:

| Pack governance-status state | Effect on provenance refs |
|---|---|
| `active` | No effect; reference is valid |
| `deprecated` | `DeprecatedPackVersion` warning at resolution pass |
| `retired` | `RetiredPackVersion` error at resolution pass |

On `RetiredPackVersion`: the structural atoms covered by the provenance atom remain executable. Only the provenance annotation is invalid. Migration: delete the provenance atom (process continues to run without pack traceability) or re-author the covered nodes with the active pack version (Sage produces new provenance atoms).

---

### 3.11 Worked examples

#### Example 1: SemOS verb invocation

```lisp
(verb cbu.create
  :inputs  [{:name client-name :type string  :required true}
             {:name client-type :type cbu-type :required true}
             {:name jurisdiction :type string  :required false :default "GB"}]
  :outputs [{:name cbu-id :type uuid}]
  :@-slots [@process]
  :effects [(write cbu-record local)
             (side-effect audit-log external)]
  :errors  [{:code cbu-already-exists}
             {:code invalid-client-type}])

(invoke cbu.create
  :args {:client-name "Allianz Asset Management AG"
         :client-type "FUND_MANDATE"
         :jurisdiction "DE"})
```

#### Example 2: bpmn-lite railway fragment

```lisp
(node start            :kind start-event)
(node create-cbu       :kind service-task
  :verb (invoke cbu.create :args {:client-name @input-name :client-type @input-type}))
(gateway type-gateway  :kind exclusive)
(node setup-fund       :kind service-task
  :verb (invoke cbu.add-fund-mandate :args {:cbu-id @cbu}))
(node setup-corp       :kind service-task
  :verb (invoke cbu.add-corporate :args {:cbu-id @cbu}))
(node onboard-end      :kind end-event)

(flow start       -> create-cbu)
(flow create-cbu  -> type-gateway)
(flow type-gateway -> setup-fund  :condition (= @cbu-type "fund"))
(flow type-gateway -> setup-corp  :default true)
(flow setup-fund  -> onboard-end)
(flow setup-corp  -> onboard-end)
```

#### Example 3: Verb using @node slot

```lisp
(verb workflow.record-step
  :inputs  [{:name note :type string :required false}]
  :outputs []
  :@-slots [@node @process @token]
  :effects [(write audit-log external)])
```

The assembly pass injects `@node` from the enclosing `(node ...)` context. The runtime injects `@process` (instance_id) and `@token` (token_id) at execution time.

#### Example 4: Parallel fork with merge clause

```lisp
(node kyc-fork :kind parallel)
(parallel-join kyc-deal-im-join
  :expects [kyc-fork]
  :merge [
    {:location kyc-outcome  :operator latest}
    {:location deal-id      :operator latest}
    {:location im-config-id :operator latest}
  ])

(flow kyc-fork      -> kyc-task)
(flow kyc-fork      -> deal-task)
(flow kyc-fork      -> im-task)
(flow kyc-task      -> kyc-deal-im-join)
(flow deal-task     -> kyc-deal-im-join)
(flow im-task       -> kyc-deal-im-join)
(flow kyc-deal-im-join -> final-review)
```

#### Example 5: Decision pack definition and instantiation (side-by-side)

This example shows the complete `conjunctive-gate` pack atom, then the pre-substitution template body, then the result of instantiation with concrete parameters, and finally the provenance atom.

**Pre-instantiation — pack atom definition:**

```lisp
(decision-pack conjunctive-gate
  :version "1.0.0"
  :description "All N conditions AND-composed; single exclusive gateway; enhanced or standard path."
  :domain-scope [kyc-onboarding screening cbu]
  :parameters [
    {:name gate-name     :type symbol               :required true
     :description "Name for the generated gateway atom"}
    {:name conditions    :type list-of-condition-expr :required true
     :description "Conditions that must ALL be true for the enhanced path"}
    {:name enhanced-path :type node-ref              :required true}
    {:name standard-path :type node-ref              :required true}
  ]
  :template [
    (gateway ,gate-name :kind exclusive)
    (flow ,gate-name -> ,enhanced-path
      :condition (and ,@conditions))
    (flow ,gate-name -> ,standard-path
      :default true)
  ]
  :example-utterances [
    "all checks must pass before activation"
    "only proceed if KYC, screening, and UBO are all approved"
    "every requirement must be satisfied to proceed"
    "all conditions satisfied → enhanced path; otherwise standard"
  ]
  :structural-signature {:conditions-composition and :gateway-kind exclusive :outcomes 2}
  :governance-ref conjunctive-gate-v1-status)
```

**Template body (pre-substitution), extracted for clarity:**

```lisp
; ,gate-name   — scalar substitution of a symbol parameter
; ,enhanced-path — scalar substitution of a node-ref parameter
; ,standard-path — scalar substitution of a node-ref parameter
; ,@conditions   — SPLICE substitution of a list-of-condition-expr parameter
;                  expands the list into the (and ...) argument positions

(gateway ,gate-name :kind exclusive)
(flow ,gate-name -> ,enhanced-path
  :condition (and ,@conditions))     ; <-- splice expands here
(flow ,gate-name -> ,standard-path
  :default true)
```

**Post-instantiation — expanded structural atoms (concrete parameters):**

Sage instantiates with:
```
gate-name     = activation-eligibility-gate
conditions    = [(= kyc-case.status approved)
                  (= sanctions-result clear)
                  (= ubo-status resolved)]
enhanced-path = activate-cbu-task
standard-path = compliance-review-task
```

Expanded structural atoms:
```lisp
(gateway activation-eligibility-gate :kind exclusive)
(flow activation-eligibility-gate -> activate-cbu-task
  :condition (and (= kyc-case.status approved)
                  (= sanctions-result clear)
                  (= ubo-status resolved)))
(flow activation-eligibility-gate -> compliance-review-task
  :default true)
```

Note: `,gate-name` → `activation-eligibility-gate` (scalar substitution of symbol). `,@conditions` → the three condition expressions spliced directly into the `(and ...)` argument list (list splice substitution).

**Provenance atom (emitted alongside expanded atoms):**

```lisp
(provenance activation-eligibility-gate-prov
  :covers [activation-eligibility-gate
           activation-eligibility-gate->activate-cbu-task
           activation-eligibility-gate->compliance-review-task]
  :source pack
  :source-id "conjunctive-gate"
  :version "1.0.0"
  :session "sess-019e4a1f-3b22-7e01-9f01-23f456789abc"
  :authored-at "2026-05-21T12:00:00Z"
  :confirmed-at "2026-05-21T12:00:28Z"
  :params {
    :gate-name "activation-eligibility-gate"
    :conditions ["(= kyc-case.status approved)"
                  "(= sanctions-result clear)"
                  "(= ubo-status resolved)"]
    :enhanced-path "activate-cbu-task"
    :standard-path "compliance-review-task"
  })
```

The provenance atom is declarative — it is dropped at lowering. The expanded structural atoms are the only thing the compiler processes. The JourneySpec output is identical to what would result from hand-authoring the three structural atoms directly.

#### Example 6: Fragment embedding the instantiation in a larger process

```lisp
; Pre-existing nodes in the process (not generated by the pack)
(node pre-check-task :kind user-task
  :verb (invoke kyc.run-final-checks :args {:case-id @case-id}))
(node activate-cbu-task :kind service-task
  :verb (invoke cbu.activate :args {:cbu-id @cbu-id}))
(node compliance-review-task :kind user-task
  :verb (invoke kyc.initiate-enhanced-review :args {:case-id @case-id}))
(node process-end :kind end-event)

; Pack-generated atoms (from conjunctive-gate instantiation above)
(gateway activation-eligibility-gate :kind exclusive)
(flow activation-eligibility-gate -> activate-cbu-task
  :condition (and (= kyc-case.status approved)
                  (= sanctions-result clear)
                  (= ubo-status resolved)))
(flow activation-eligibility-gate -> compliance-review-task
  :default true)

; Connecting flows (hand-authored)
(flow pre-check-task -> activation-eligibility-gate)
(flow activate-cbu-task -> process-end)
(flow compliance-review-task -> process-end)

; Provenance atom (declarative — covers only the pack-generated atoms)
(provenance activation-eligibility-gate-prov
  :covers [activation-eligibility-gate
           activation-eligibility-gate->activate-cbu-task
           activation-eligibility-gate->compliance-review-task]
  :source pack
  :source-id "conjunctive-gate"
  :version "1.0.0"
  :session "sess-019e4a1f-..."
  :authored-at "2026-05-21T12:00:00Z"
  :confirmed-at "2026-05-21T12:00:28Z"
  :params {:gate-name "activation-eligibility-gate"
           :conditions [...]
           :enhanced-path "activate-cbu-task"
           :standard-path "compliance-review-task"})
```

The connecting flows (`pre-check-task -> activation-eligibility-gate`) are hand-authored and have no provenance coverage — they are implicitly bespoke. This is correct and expected.

---

## 4. Verb Catalogues

### 4.1 SemOS verb reshape

#### 4.1.1 Mapping table: VerbConfig → unified atom model

The current SemOS verb model is defined by `VerbConfig` in `rust/crates/dsl-core/src/config/types.rs`. The reshape maps each field to the appropriate location in the unified model.

| VerbConfig field | New location | Notes |
|---|---|---|
| `description` | `(governance-status :description str)` or doc comment | Governance metadata, not a structural slot |
| `behavior` (`crud` \| `plugin`) | Verb `:effects` + implementation registry | Behavior is an implementation concern |
| `args` (name/type/required/maps_to/lookup/default) | Verb `:inputs` list | `maps_to` → removed (SQL mapping is implementation detail); `lookup` → entity registry resolution at resolution pass |
| `returns` (type/fields/capture) | Verb `:outputs` list | `capture` → expressed through `@process` write-back |
| `effect_class` | Verb `:effects` list | `read_snapshot` → `(read ... local)`; `read_modify_write` → `(write ... local)`; `append_fact` → `(write ... local)`; `admin_override` → `(write ... local)` + governance annotation |
| `metadata.tier` | `(governance-status :tier tier)` | Declarative |
| `metadata.source_of_truth` | `(governance-status :source-of-truth str)` | Declarative |
| `metadata.noun` | `(utterance-binding :noun noun)` | Part of NLCI binding |
| `metadata.tags`, `phase_tags` | `(governance-status :tags [...] :phase-tags [...])` | Declarative |
| `metadata.side_effects` | Verb `:effects` list | `state_write` → `(write entity-state local)`; `emitting` → `(side-effect outbox external)` |
| `lifecycle.entity_arg` | Verb `:inputs` with `:type entity-ref` | Explicit typing |
| `lifecycle.requires_states` | `(workspace-constraint :mode A ...)` in `(graph-pack ...)` | State requirement lives in the DAG pack, not the verb |
| `lifecycle.precondition_checks` | Named `(predicate ...)` atoms referenced from `(workspace-constraint)` | Explicit named predicates |
| `three_axis.state_effect` | `(governance-status :state-effect symbol)` | Declarative |
| `three_axis.external_effects` | Verb `:effects` with `mode external` | Structural |
| `three_axis.consequence` | `(governance-status :consequence-baseline symbol :consequence-escalation symbol)` | Declarative |
| `flavour` | `(governance-status :flavour symbol)` | Declarative |
| `role_guard` | `(governance-status :role-guard {...})` | Declarative; ABAC enforcement at runtime |
| `transition_args` | Verb `@-slots` + `(workspace-constraint :mode C ...)` | Transition target from @node context |
| `invocation_phrases` | `(utterance-binding :phrases [...] :verb verb-ref)` | Separate atom; decouples NLCI from verb |

#### 4.1.2 Reshape pattern categories

| Pattern | Prevalence | Verbs | Description | Mechanical effort |
|---|---|---|---|---|
| A — Direct slot mapping | ~85% | ~933 | Typed args, simple returns, straightforward effect class. `args/returns` map directly; governance metadata moves to `(governance-status ...)`; phrases move to `(utterance-binding ...)`. | 5–10 min/verb with automated tooling |
| B — Lifecycle precondition binding | ~8% | ~88 | `lifecycle.requires_states` checks. State requirements move to `(workspace-constraint :mode A ...)` in the governing `(graph-pack ...)`. Verb itself only declares its functional contract. | 20–30 min/verb (requires DAG pack annotation) |
| C — Three-axis governance | ~5% | ~55 | Non-trivial `three_axis.consequence` levels (requires-confirmation or requires-explicit-authorisation). Consequence level becomes declarative `(governance-status ...)` annotation. | 15 min/verb |
| D — Transition target injection | ~2% | ~22 | `transition_args` specifying exact state machine transition. [GAP: full mapping deferred to v0.2; v0.1 treats as Pattern A with governance annotation.] | 30 min/verb |

**Blockers identified**: none. The `maps_to` SQL column mapping is an implementation detail handled by the `SemOsVerbOp` layer. The `lookup` mechanism (entity ref resolution) maps to entity registry lookup at the resolution pass.

#### 4.1.3 Representative verbs in full new shape (10 verbs)

```lisp
; --- Pattern A: instance_adding, read_modify_write ---

; 1. cbu.create
(verb cbu.create
  :inputs  [{:name client-name :type string   :required true}
             {:name client-type :type cbu-type :required true}
             {:name jurisdiction :type string  :required false :default "GB"}]
  :outputs [{:name cbu-id :type uuid}]
  :@-slots [@process]
  :effects [(write cbu-record local) (side-effect audit-log external)]
  :errors  [{:code cbu-already-exists} {:code invalid-client-type}])

(utterance-binding cbu.create-phrases
  :phrases ["create a CBU" "set up a new client business unit"
             "onboard client" "create new CBU"]
  :verb cbu.create :domain cbu)

(governance-status cbu.create-governance
  :atom cbu.create :state active :flavour instance-adding
  :tier intent :noun cbu :state-effect transition
  :consequence-baseline reviewable)

; 2. entity.create
(verb entity.create
  :inputs  [{:name entity-type :type entity-kind    :required true}
             {:name legal-name :type string          :required true}
             {:name lei         :type lei-code        :required false}
             {:name jurisdiction :type iso-3166-alpha2 :required true}]
  :outputs [{:name entity-id :type uuid}]
  :@-slots [@process]
  :effects [(write entity-record local) (side-effect audit-log external)]
  :errors  [{:code entity-already-exists}])

; 3. document.request (append_fact)
(verb document.request
  :inputs  [{:name cbu-id       :type uuid          :required true}
             {:name document-type :type document-kind :required true}
             {:name required-by  :type date          :required false}]
  :outputs [{:name request-id :type uuid}]
  :@-slots [@process]
  :effects [(write document-request-record local)
             (side-effect notification-service external)]
  :errors  [{:code document-already-requested}])

; 4. session.load-cbu (read_snapshot, navigation)
(verb session.load-cbu
  :inputs  [{:name cbu-id :type uuid :required true}]
  :outputs [{:name session-scope :type session-scope-update}]
  :@-slots [@process]
  :effects [(read cbu-record local)]
  :errors  [{:code cbu-not-found}])

; 5. trading-profile.set-mandate-type (attribute_mutating)
(verb trading-profile.set-mandate-type
  :inputs  [{:name cbu-id       :type uuid              :required true}
             {:name mandate-type :type mandate-type-enum :required true}]
  :outputs []
  :@-slots [@process]
  :effects [(write trading-profile-record local)]
  :errors  [{:code invalid-mandate-type}])

; --- Pattern B: lifecycle precondition ---

; 6. kyc.approve (requires case in-progress state — requirement in graph-pack, not here)
(verb kyc.approve
  :inputs  [{:name case-id       :type uuid   :required true}
             {:name approver-notes :type string :required false}]
  :outputs [{:name approval-id :type uuid}]
  :@-slots [@process]
  :effects [(write kyc-case-record local) (side-effect audit-log external)]
  :errors  [{:code case-not-in-review} {:code insufficient-evidence}])

; 7. cbu.submit-for-validation (requires cbu in evidenced state)
(verb cbu.submit-for-validation
  :inputs  [{:name cbu-id :type uuid :required true}]
  :outputs []
  :@-slots [@process]
  :effects [(write cbu-record local)]
  :errors  [{:code missing-required-evidence} {:code cbu-not-in-evidenced-state}])

; --- Pattern C: governance consequence ---

; 8. governance.retire-pack (requires-explicit-authorisation)
(verb governance.retire-pack
  :inputs  [{:name pack-id      :type string :required true}
             {:name version      :type string :required true}
             {:name reason       :type string :required true}
             {:name effective-date :type date :required true}]
  :outputs [{:name retirement-id :type uuid}]
  :@-slots [@process]
  :effects [(write pack-governance-record local)
             (side-effect audit-log external)
             (side-effect notification-service external)]
  :errors  [{:code pack-not-active} {:code active-processes-using-pack}])

(governance-status governance.retire-pack-governance
  :atom governance.retire-pack :state active
  :flavour tollgate :consequence-baseline requires-explicit-authorisation
  :consequence-escalation requires-explicit-authorisation)

; 9. attribute.define (external-visibility, full ceremony)
(verb attribute.define
  :inputs  [{:name fqn              :type string             :required true}
             {:name category        :type attribute-category :required true}
             {:name type            :type attribute-type     :required true}
             {:name validation-rules :type json              :required false}]
  :outputs [{:name attribute-id :type uuid}]
  :@-slots [@process]
  :effects [(write attribute-registry-record local)
             (side-effect sem-os-snapshot external)]
  :errors  [{:code attribute-already-exists} {:code invalid-category}])

; 10. gleif.import-hierarchy (external side-effect, Pattern A)
(verb gleif.import-hierarchy
  :inputs  [{:name lei   :type lei-code :required true}
             {:name depth :type integer  :required false :default 3}]
  :outputs [{:name imported-count :type integer}
             {:name entity-ids    :type list-of-uuid}]
  :@-slots [@process]
  :effects [(write entity-record local)
             (write relationship-record local)
             (side-effect gleif-api external)]
  :errors  [{:code lei-not-found} {:code api-unavailable}])
```

---

### 4.2 bpmn-lite verb catalogue

Reconciliation against BPMN 2.0 (OMG `formal/2013-12-09`) with Camunda 8 status per https://docs.camunda.io/docs/components/modeler/bpmn/bpmn-coverage/.

Legend: ✓ covered | D deferred | ✗ rejected

#### 4.2.1 Events — start events

| Element | §OMG | Camunda 8 | bpmn-lite | Atom / Notes |
|---|---|---|---|---|
| None start | §10.4 | ✓ | ✓ | `(node N :kind start-event)` |
| Message start | §10.4.3 | ✓ | ✓ | `(node N :kind start-event-message :event-def msg-ref)` |
| Timer start | §10.4.4 | ✓ | ✓ | `(node N :kind start-event-timer :event-def timer-ref)` |
| Signal start | §10.4.5 | ✓ | ✓ | `(node N :kind start-event-signal :event-def sig-ref)` |
| Error start | §10.4.6 | ✓ | ✓ | `(node N :kind start-event-error :event-def err-ref)` — only valid in event-subprocess scope |
| Escalation start | §10.4.6 | modeller | ✓ | `(node N :kind start-event-escalation)` |
| Compensation start | §10.4.6 | modeller | ✓ | `(node N :kind start-event-compensation)` — only valid in transaction-subprocess |
| Conditional start | §10.4.7 | not supported | D | Requires external condition monitoring [GAP] |
| Multiple start | §10.4.8 | not supported | ✗ | No Camunda support; no identified use case |
| Parallel multiple start | §10.4.8 | not supported | ✗ | Same |

#### 4.2.2 Events — intermediate catching events

| Element | §OMG | Camunda 8 | bpmn-lite | Atom / Notes |
|---|---|---|---|---|
| Message catch | §10.4.3 | ✓ | ✓ | `(node N :kind intermediate-catch-message :event-def msg-ref)` |
| Timer catch | §10.4.4 | ✓ | ✓ | `(node N :kind intermediate-catch-timer :event-def timer-ref)` |
| Signal catch | §10.4.5 | ✓ | ✓ | `(node N :kind intermediate-catch-signal :event-def sig-ref)` |
| Link catch | §10.4.9 | ✓ | ✓ | `(node N :kind intermediate-catch-link :link-ref name)` |
| Conditional catch | §10.4.7 | not supported | D | [GAP] |
| Escalation catch | §10.4.6 | modeller | D | Limited custody banking use [GAP] |
| Multiple catch | §10.4.8 | not supported | ✗ | Use event-based gateway instead |

#### 4.2.3 Events — intermediate throwing events

| Element | §OMG | Camunda 8 | bpmn-lite | Atom / Notes |
|---|---|---|---|---|
| Message throw | §10.4.3 | ✓ | ✓ | `(node N :kind intermediate-throw-message :event-def msg-ref)` |
| Signal throw | §10.4.5 | ✓ | ✓ | `(node N :kind intermediate-throw-signal :event-def sig-ref)` |
| Escalation throw | §10.4.6 | ✓ | ✓ | `(node N :kind intermediate-throw-escalation :event-def esc-ref)` |
| Link throw | §10.4.9 | ✓ | ✓ | `(node N :kind intermediate-throw-link :link-ref name)` |
| Compensation throw | §10.4.10 | ✓ | ✓ | `(node N :kind intermediate-throw-compensation)` |
| Multiple throw | §10.4.8 | not supported | ✗ | Rejected |

#### 4.2.4 Events — end events

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| None end | §10.4 | ✓ | ✓ | `(node N :kind end-event)` |
| Message end | §10.4.3 | ✓ | ✓ | `(node N :kind end-event-message)` |
| Error end | §10.4.6 | ✓ | ✓ | `(node N :kind end-event-error :event-def err-ref)` |
| Signal end | §10.4.5 | ✓ | ✓ | `(node N :kind end-event-signal)` |
| Terminate end | §10.4.11 | ✓ | ✓ | `(node N :kind end-event-terminate)` — kills all tokens in scope |
| Escalation end | §10.4.6 | ✓ | ✓ | `(node N :kind end-event-escalation)` |
| Compensation end | §10.4.10 | ✓ | ✓ | `(node N :kind end-event-compensation)` |
| Cancel end | §10.4.12 | ✓ | ✓ | `(node N :kind end-event-cancel)` — transaction-subprocess only |
| Multiple end | §10.4.8 | not supported | ✗ | Rejected |

#### 4.2.5 Events — boundary events

| Element | §OMG | Camunda 8 | bpmn-lite | Atom / Notes |
|---|---|---|---|---|
| Error boundary (interrupting) | §10.4.6 | ✓ | ✓ | `(boundary-attachment node error :interrupting true)` |
| Timer boundary (interrupting) | §10.4.4 | ✓ | ✓ | `(boundary-attachment node timer :interrupting true)` |
| Timer boundary (non-interrupting) | §10.4.4 | ✓ | ✓ | `(boundary-attachment node timer :interrupting false)` — spawns parallel token |
| Message boundary (interrupting) | §10.4.3 | ✓ | ✓ | `(boundary-attachment node message :interrupting true)` |
| Message boundary (non-interrupting) | §10.4.3 | ✓ | ✓ | `(boundary-attachment node message :interrupting false)` |
| Signal boundary (int.) | §10.4.5 | ✓ | ✓ | `(boundary-attachment node signal :interrupting true)` |
| Signal boundary (non-int.) | §10.4.5 | ✓ | ✓ | `(boundary-attachment node signal :interrupting false)` |
| Escalation boundary (int.) | §10.4.6 | ✓ | ✓ | `(boundary-attachment node escalation :interrupting true)` |
| Escalation boundary (non-int.) | §10.4.6 | ✓ | ✓ | `(boundary-attachment node escalation :interrupting false)` |
| Compensation boundary | §10.4.10 | ✓ | ✓ | `(boundary-attachment node compensation :interrupting true :compensation-handler H)` |
| Cancel boundary | §10.4.12 | ✓ | ✓ | `(boundary-attachment node cancel :interrupting true)` — transaction-subprocess only |
| Conditional boundary | §10.4.7 | not supported | D | [GAP] |

**Non-interrupting boundary events**: modelled by the runtime spawning a parallel token at the boundary's outgoing flow while the host node's original token continues. The host token and the boundary-spawned token are independent and do not share a join.

#### 4.2.6 Activities — tasks

| Element | §OMG | Camunda 8 | bpmn-lite | Atom / Notes |
|---|---|---|---|---|
| Abstract task | §10.2.2 | modeller | ✓ | `(node N :kind service-task)` without `:verb` — placeholder |
| Service task | §10.3.3 | ✓ | ✓ | `(node N :kind service-task :verb invoke-ref)` |
| User task | §10.3.2 | ✓ | ✓ | `(node N :kind user-task :verb invoke-ref)` — creates human task record, waits for completion |
| Send task | §10.3.4 | ✓ | ✓ | `(node N :kind send-task :event-def msg-ref :verb invoke-ref?)` |
| Receive task | §10.3.5 | ✓ | ✓ | `(node N :kind receive-task :event-def msg-ref)` |
| Manual task | §10.3.6 | modeller | ✓ | `(node N :kind manual-task)` |
| Business rule task | §10.3.7 | ✓ | ✓ | `(node N :kind business-rule-task :decision decision-ref)` — switch adaptor |
| Script task | §10.3.8 | ✓ | ✓ | `(node N :kind script-task :script expr)` |

#### 4.2.7 Activities — subprocesses and scopes

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| Embedded subprocess | §10.2.5 | ✓ | ✓ | `(node N :kind subprocess)` — inline, shares parent instance |
| Event subprocess | §10.2.6 | ✓ | ✓ | `(node N :kind event-subprocess)` |
| Transaction subprocess | §10.4.12 | ✓ | ✓ | `(node N :kind transaction-subprocess)` — compensation and cancel boundary |
| Call activity | §10.2.3 | ✓ | ✓ | `(node N :kind call-activity :called-process ref :input-mapping [...] :output-mapping [...])` |
| Ad-hoc subprocess | §10.2.7 | not supported | ✗ | No Camunda support; no custody banking need. Rejected. |

**Multi-instance and loop markers** (slot modifiers on any task kind):

| Marker | §OMG | Camunda 8 | bpmn-lite |
|---|---|---|---|
| Sequential loop | §10.2.4.1 | ✓ | ✓ — `:loop {:condition expr :max-count int?}` |
| Sequential multi-instance | §10.2.4.2 | ✓ | ✓ — `:multi-instance {:cardinality sequential :data-input collection-ref}` |
| Parallel multi-instance | §10.2.4.2 | ✓ | ✓ (static count) / D (dynamic count) — `:multi-instance {:cardinality parallel :data-input collection-ref}` with static expected count; dynamic expected count [GAP v0.2] |
| Compensation handler | §10.4.10 | ✓ | ✓ — `:compensation-handler handler-node-ref` |

#### 4.2.8 Gateways

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| Exclusive (XOR) | §13.3 | ✓ | ✓ | `(gateway N :kind exclusive)` — switch adaptor selects one outgoing flow |
| Inclusive (OR) | §13.4 | ✓ | ✓ | `(gateway N :kind inclusive)` — switch adaptor selects 1..N; join via `(parallel-join :expects [N])` with dynamic expected set |
| Parallel (AND) | §13.5 | ✓ | ✓ | `(gateway N :kind parallel)` — all flows; join via `(parallel-join :expects [N])` |
| Event-based | §13.6 | ✓ | ✓ | `(gateway N :kind event-based)` — race; first event wins; others cancelled |
| Parallel event-based | §13.6.6 | modeller | ✓ | `(gateway N :kind parallel-event-based)` |
| Complex | §13.7 | not supported | ✗ | Expressible through inclusive + predicate. Rejected. |

**Inclusive gateway (§13.4) fan-in semantics**: at fork time, the runtime records which branch tokens were emitted into the corresponding `(parallel-join :expects [...])` atom's expected-arrivals set. The join fires when all emitted tokens arrive. Token death (Commitment 12) removes the dead token from the expected-arrivals set. This gives correct BPMN 2.0 §13.4 semantics without static path reachability analysis at join-fire time.

**Event-based gateway (§13.6) race semantics**: the runtime registers N pending waits (one per outgoing catching event). The first event to fire wins; the runtime advances the token to that catching event node and cancels all other pending waits.

#### 4.2.9 Data

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| Data object | §10.5 | modeller | ✓ (by convention) | Named data locations referenced via @-slot names and token write log. Not a separate atom kind — data locations are declared in verb `:effects`. |
| Data store | §10.5.4 | not supported | ✗ | Not executable in Camunda; persistent external store is outside process scope. Rejected. |
| Data input / output | §10.5.1–2 | modeller | ✓ (via verb slots) | Expressed in verb `:inputs`/`:outputs` with @-slot bindings |
| Data association | §10.5.3 | implicit | ✓ (via invoke :args) | Expressed in `(invoke :args {...})` maps |

#### 4.2.10 Flows

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| Sequence flow | §12.2 | ✓ | ✓ | `(flow source target)` |
| Conditional sequence flow | §12.2 | ✓ | ✓ | `(flow source target :condition expr)` |
| Default flow | §12.2 | ✓ | ✓ | `(flow source target :default true)` |
| Message flow | §12.3 | modeller (non-executable) | ✗ | Cross-pool visualisation; messages are first-class events in bpmn-lite, not flows. Rejected as a flow atom kind. |

#### 4.2.11 Artifacts

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| Group | §10.6.3 | modeller | D | No execution semantics. [GAP — deferred as declarative annotation kind] |
| Text annotation | §10.6.4 | modeller | ✓ | Expressed via `(review-annotation ...)` declarative atom |

#### 4.2.12 Swimlanes

| Element | §OMG | Camunda 8 | bpmn-lite | Notes |
|---|---|---|---|---|
| Pool | §10.6.2 | modeller | D | Process boundary visualisation; bpmn-lite processes are inherently single-pool. [GAP — deferred as declarative annotation] |
| Lane | §10.6.1 | modeller | D | Responsibility annotation; maps to role guards on user tasks. [GAP — deferred as declarative annotation] |

---

### 4.3 Migration coverage statement

**Covered (bpmn-lite atom directly available)**: service, user, send, receive, manual, business-rule, and script tasks; embedded, event, and transaction subprocesses; call activities; sequential loops and sequential/parallel multi-instance (static count); all start/end event types (message, timer, signal, error, terminate, escalation, compensation, cancel); all boundary event types (error, timer, message, signal, escalation, compensation, cancel — interrupting and non-interrupting); intermediate catch/throw events (message, timer, signal, link, escalation, compensation); sequence flows (unconditional, conditional, default); exclusive, inclusive, parallel, event-based, and parallel event-based gateways.

**Deferred (not v0.1)**: conditional events (start, intermediate, boundary); parallel multi-instance with dynamic expected count; escalation intermediate catching event; group artifact; pool/lane swimlane annotations; out-of-transaction-scope compensation.

**Rejected**: ad-hoc subprocess; complex gateway; multiple start/catch/throw/end events; data stores; message flows (as flow atoms); parallel multiple start.

**Estimate**: >90% of typical Camunda 8 BPMN models expressible in bpmn-lite v0.1 without redesign. The remaining fraction involves conditional events (Camunda-unsupported), parallel multi-instance dynamic count, and swimlane visualisation annotations — none of which are executable in Camunda either.

**Migration tooling input** (later phase): an XML import tool must handle element mapping per the reconciliation; rejection/deferral reporting; data object → @-slot name extraction; message correlation key derivation; timer expression normalisation; and compensation handler wiring.

---

## 5. Open Questions for Sessions 2 and 3

The following questions emerged during Session 1 design and require resolution in downstream sessions.

**For Session 2 (compiler + runtime):**

1. **Template insertion point protocol** (§3.3.2 GAP): How does the compiler validate and Sage connect a pack template to the surrounding process? The `$pre-node` marker used in the worked example is informally specified. Session 2 must define the precise attachment mechanism — whether as a named splice point in the template, a before/after annotation, or a separate connection step in the Sage interaction protocol.

2. **`(decision-pack ...)` in the compiler pipeline**: The assembly pass indexes packs into the pack registry. Session 2 must specify the pack registry's interface, how it is populated (pre-loaded from separately deployed source artifacts, lazy-loaded from the registry DB, or embedded in the compilation unit), and how the resolution pass looks up packs referenced in provenance atoms.

3. **`(parallel-join :expects [...])` dynamic expected count for inclusive gateways**: The assembly pass must validate that inclusive gateways are matched with a corresponding `(parallel-join ...)` atom. Session 2 must specify: (a) whether the expected-set is statically declared or computed at runtime from the fork's adaptor reply; (b) the assembly validation rule for orphaned inclusive gateways.

4. **`business-rule-task :decision` slot type**: Currently typed as `decision-ref`. Session 2 must specify whether this is a reference to an inline `(decision ...)` atom in the same source, a reference to an external dmn-lite decision by FQN, or both. The switch adaptor protocol binding depends on this.

**For Session 3 (regression + packs):**

5. **Pack template arity validation**: Session 3 specifies 12 seed packs. Each pack's `:template` must be validated against its `:parameters`. Session 3 should include a table of validation rules per pack confirming that all substitution forms match declared parameters and are used in type-compatible positions.

6. **Provenance atom naming convention**: Worked examples use ad-hoc names for provenance atoms (e.g., `activation-eligibility-gate-prov`). Session 3 should define a naming convention to avoid collisions when multiple packs are instantiated in the same source.

7. **Pack `:structural-signature` formal schema**: §3.3.2 specifies `:structural-signature` as a `map` with informal content. Session 3 must define the canonical keys and value types for the structural signature, since Sage uses these for multi-modal matching.

---

## Appendix A: Glossary

**Atom**: the unit of DSL source — one s-expression `(kind [name] :slot value ...)`. Either structural or declarative.

**Assembly pass**: compiler Pass 1. Builds the typed process graph (bpmn-lite) or dependency DAG (SemOS) from the structural atom bag.

**`decision-pack`**: a structural atom kind declaring a parameterisable template for a decision shape pattern. Used by Sage at author time; not compiled into the executable form; indexed in the pack registry.

**Declarative atom**: atom whose kind is classified as declarative. Carries governance metadata. Never compiled into the executable form.

**Executable form**: the compiler output consumed by the runtime — `JourneySpec` (bpmn-lite) or `ExecutionPlan` (SemOS). Contains no declarative atoms; contains no `(decision-pack ...)` atoms.

**`graph-pack`**: a structural atom kind declaring lifecycle state machines and workspace constraints for a domain (SemOS DAG taxonomy pack). **Distinct from `decision-pack`.**

**Pack registry**: the store of indexed `(decision-pack ...)` atoms, keyed by `(name, version)`. Populated during compilation; queried by Sage for intent matching and by the resolution pass for provenance validation.

**Provenance atom**: a declarative `(provenance ...)` atom recording which structural atoms were generated by a decision pack instantiation or hand-authoring session.

**Railway**: the `RailwayGraph` produced by the bpmn-lite assembly pass — directed graph of typed nodes connected by sequence flows. Synonym: process graph.

**Structural atom**: atom whose kind is classified as structural. Participates in compilation and appears in the executable form (except `decision-pack` atoms, which are structural but not compiled into the executable form — they are indexed into the pack registry instead).

**Switch adaptor**: a pluggable component that evaluates gateway decision requests and returns chosen branches. Decouples gateway logic from the runtime.

**Template substitution**: the mechanism by which `,name` (scalar) and `,@name` (splice) forms in a `(decision-pack :template [...])` are replaced with concrete values at Sage-driven author time. The compiler validates but does not perform substitution.

**Token**: a unit of control flow in the journey-persisted runtime. Represents one active path of execution within a process instance.

**@-slot**: a context-injected reference declared by a verb (e.g., `@node`, `@process`). Bound at assembly time (structural context) or runtime (instance context).

---

## Appendix B: Atom Kind Reference

Complete atom kind table with full slot signatures. All atom kinds with their §3 reference.

### B.1–B.7: Language-shared structural atoms
See §3.3.1. Kinds: `verb` (B.1), `invoke` (B.2), `entity` (B.3), `relationship` (B.4), `predicate` (B.5), `decision` (B.6), `data-type` (B.7).

### B.8: `decision-pack` (structural) — **new**

Full slot specification in §3.3.2. Summary:

| Slot | Type | Req |
|---|---|---|
| `:version` | string (semver) | yes |
| `:description` | string | yes |
| `:domain-scope` | list of symbols | yes |
| `:parameters` | list of param-def | yes |
| `:template` | list of atoms (with `,name`/`,@name`) | yes |
| `:example-utterances` | list of strings | yes |
| `:structural-signature` | map | no |
| `:governance-ref` | symbol | no |

param-def: `{:name N :type T :required bool :description D :default V?}`

### B.9–B.16: bpmn-lite-specific structural atoms
See §3.3.3. Kinds: `node` (B.9), `gateway` (B.10), `flow` (B.11), `boundary-attachment` (B.12), `parallel-join` (B.13), `message-definition` (B.14), `timer-definition` (B.15), `error-definition` (B.16).

### B.17–B.20: SemOS-specific structural atoms
See §3.3.4. Kinds: `graph-pack` (B.17), `utterance-binding` (B.18), `constellation-root` (B.19), `workspace-constraint` (B.20).

### B.21–B.24: Declarative atom kinds
See §3.4. Kinds: `provenance` (B.21), `governance-status` (B.22), `review-annotation` (B.23), `jurisdiction-tag` (B.24).

**Total structural kinds: 20** (7 language-shared including `decision-pack` + 8 bpmn-lite + 4 SemOS + 1 new `decision-pack`).
**Total declarative kinds: 4** (extensible catalogue).

---

## Appendix C: BPMN 2.0 Reconciliation Table

Full element-by-element table. Summary by category:

| Category | Elements | ✓ Covered | D Deferred | ✗ Rejected |
|---|---|---|---|---|
| Start events | 10 | 8 | 1 | 1 |
| Intermediate catch | 7 | 4 | 2 | 1 |
| Intermediate throw | 6 | 5 | 0 | 1 |
| End events | 9 | 8 | 0 | 1 |
| Boundary events | 11 | 10 | 1 | 0 |
| Tasks | 8 | 8 | 0 | 0 |
| Subprocesses + Call | 5 | 4 | 0 | 1 |
| Multi-instance / Loop | 3 | 2 | 1 | 0 |
| Gateways | 6 | 5 | 0 | 1 |
| Data | 4 | 2 | 0 | 2 |
| Flows | 4 | 3 | 0 | 1 |
| Artifacts | 2 | 1 | 1 | 0 |
| Swimlanes | 2 | 0 | 2 | 0 |
| **Total** | **77** | **60 (78%)** | **8 (10%)** | **9 (12%)** |

Coverage against *Camunda 8 executable* elements only (excluding modeller-only and unsupported): **>90%**.

Full per-element rows with OMG section citations are in §4.2.1–4.2.12 above.

---

*Session 1 complete. Sessions 2 and 3 may cite this document by section (e.g., §3.3.2 for `decision-pack` slots, §3.5.x for template substitution syntax, Appendix B.8 for the atom kind reference, §4.2.x for BPMN element handling decisions).*
