# Unified DSL Atom Model, Compiler, Runtime, and Decision Pack Catalogue for ob-poc
## Design Document v0.1 — Session 2: Deliverables 3 and 4
### Compiler Architecture · Runtime Design

> Continuation of Session 1. Sections numbered sequentially from Session 1.
> Session 1 covered §1–4 (executive summary, commitments, atom model, verb catalogues).
> This session covers §5 (compiler) and §6 (runtime).
> Session 3 covers §7–12 plus appendices.

---

## 5. Compiler Architecture

### 5.1 High-level architecture

The unified compiler is a single binary with two frontends sharing infrastructure. The language-specific code is confined to the two assembly passes (§5.4). All other passes — parse, resolution, lowering — are shared.

**Crate decomposition:**

```
dsl-syntax          — shared lexer + parser + atom kind dispatch + untyped AST
dsl-semos-frontend  — SemOS assembly pass: utterance bag → dependency DAG
dsl-bpmn-frontend   — bpmn-lite assembly pass: node/flow bag → process graph (railway)
dsl-resolver        — shared resolution pass: verb lookup, @-slot binding,
                       declarative validation, cross-artifact ref resolution
dsl-lowering        — frontend-specific lowering to executable form.
                       SemOS: execution plan (CompileStep sequence + injection graph).
                       bpmn-lite: journey specification (node table + transition rules)
dsl-diagnostics     — typed error model, source location attribution,
                       diagnostic formatting
dsl-compiler        — orchestration crate; public API surface for REPL + deploy tool
```

**Dependency graph:**

```
dsl-compiler
  ├── dsl-syntax (always)
  ├── dsl-semos-frontend (feature = "semos")
  ├── dsl-bpmn-frontend  (feature = "bpmn")
  ├── dsl-resolver
  ├── dsl-lowering
  └── dsl-diagnostics

dsl-semos-frontend  depends on: dsl-syntax, dsl-diagnostics
dsl-bpmn-frontend   depends on: dsl-syntax, dsl-diagnostics
dsl-resolver        depends on: dsl-syntax, dsl-diagnostics
dsl-lowering        depends on: dsl-syntax, dsl-resolver, dsl-diagnostics
```

`dsl-syntax`, `dsl-resolver`, and `dsl-diagnostics` have no frontend dependency. The language-specific frontends do not depend on each other. This structure ensures the shared infrastructure is testable without the frontends and that adding a third frontend (e.g., a DMN XML import frontend) requires only a new crate.

**Four-pass pipeline:**

```
Source text
  │
  ▼  Pass 0: parse (dsl-syntax)
Untyped atom bag (AtomBag — Vec<UntypedAtom>)
  │
  ▼  Pass 1: assembly (dsl-semos-frontend or dsl-bpmn-frontend)
Typed AST with assembled structure (DagGraph | RailwayGraph) + UnresolvedRefs
  │
  ▼  Pass 2: resolution (dsl-resolver)
Fully resolved AST; all name-refs and @-slots bound; declarative atoms validated
  │
  ▼  Pass 3: lowering (dsl-lowering)
Executable form (ExecutionPlan | JourneySpec); declarative atoms dropped
```

Diagnostics are accumulated across all passes; the compiler continues through later passes even when earlier passes have errors (up to a configurable error limit), in order to surface as many errors as possible in one compilation run.

### 5.2 Parser

**Shared S-expression lexer** (`dsl-syntax/src/lexer.rs`):

Token kinds:
```rust
pub enum Token {
    LParen,
    RParen,
    Keyword(String),      // :keyword
    SlotRef(String),      // @name
    Symbol(String),       // unquoted symbol or atom-kind
    StringLit(String),
    IntLit(i64),
    DecimalLit(f64),
    BoolLit(bool),
    Null,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Arrow,                // ->  (syntactic sugar for flow source/target)
    Uuid(uuid::Uuid),
    Eof,
}
```

The `->` arrow token is a parsing convenience for flow atoms:
```lisp
(flow create-cbu-task -> type-gateway)
```
is equivalent to:
```lisp
(flow create-cbu-task type-gateway)
```

**Parser** (`dsl-syntax/src/parser.rs`):

Recursive-descent over a token stream. The top-level entry point is `parse_source_file() -> Result<AtomBag, DiagnosticSet>`.

```rust
fn parse_atom(tokens: &mut TokenStream) -> Result<UntypedAtom, Diagnostic> {
    tokens.expect(LParen)?;
    let kind_name = tokens.expect_symbol()?;
    let kind_class = ATOM_KIND_REGISTRY.classify(&kind_name);
    // kind_class = Structural | Declarative | Unknown
    let name = if tokens.peek_is_symbol() { Some(tokens.next_symbol()) } else { None };
    let mut slots = Vec::new();
    while !tokens.peek_is(RParen) {
        let key = tokens.expect_keyword()?;
        let value = parse_value(tokens)?;
        slots.push((key, value));
    }
    tokens.expect(RParen)?;
    Ok(UntypedAtom { kind: kind_name, kind_class, name, slots, span: tokens.current_span() })
}
```

Unknown structural kinds: `Diagnostic::UnknownAtomKind { kind, span, severity: Error }`.
Unknown declarative kinds: `Diagnostic::UnknownDeclarativeKind { kind, span, severity: Warn }`.

**Atom kind registry** (`dsl-syntax/src/registry.rs`):

A static lookup table mapping atom kind name strings to `Structural | Declarative`. Populated from the two constant arrays:

```rust
const STRUCTURAL_KINDS: &[&str] = &[
    "verb", "invoke", "node", "gateway", "flow",
    "boundary-attachment", "parallel-join",
    "entity", "relationship", "predicate", "decision", "data-type",
    "message-definition", "timer-definition", "error-definition",
    "graph-pack", "utterance-binding", "constellation-root", "workspace-constraint",
];
const DECLARATIVE_KINDS: &[&str] = &[
    "provenance", "governance-status", "review-annotation", "jurisdiction-tag",
];
```

The registry is not extensible at runtime in v0.1. New declarative kinds require a code change and registry update.

### 5.3 AST representation

The untyped AST produced by the parser is a flat bag of atoms. Each atom carries its kind, optional name, slots as key-value pairs, span (source location), and kind class.

```rust
// dsl-syntax/src/ast.rs

pub struct AtomBag {
    pub atoms: Vec<UntypedAtom>,
    pub source_id: SourceId,
}

pub struct UntypedAtom {
    pub kind: String,
    pub kind_class: KindClass,
    pub name: Option<String>,
    pub slots: Vec<(String, AtomValue)>,
    pub span: Span,
}

pub enum AtomValue {
    Literal(Literal),
    NameRef(NameRef),           // unresolved at parse time
    SlotRef(String),            // @name — unresolved at parse time
    Nested(Box<UntypedAtom>),
    List(Vec<AtomValue>),
    Map(Vec<(String, AtomValue)>),
}

pub enum NameRef {
    Unqualified(String),
    Qualified { pack: String, atom: String },
}

pub struct Literal {
    pub kind: LiteralKind,
    pub raw: String,
}

pub enum LiteralKind {
    String, Integer, Decimal, Boolean, Null, Uuid,
}

pub struct Span {
    pub source_id: SourceId,
    pub line: u32,
    pub col: u32,
    pub len: u32,
}
```

**Forward references**: `NameRef::Unqualified` atoms in the bag may reference other atoms that appear later in the source. The parser does not attempt to resolve them — it records the name as a string. The assembly pass uses the name as a key into the atom name index (built at the start of the assembly pass from the full atom bag).

**The assembly pass does not mutate the atom bag**: it reads from the bag and produces a typed graph structure (DagGraph or RailwayGraph) alongside a typed AST that replaces NameRef nodes with resolved references where possible at assembly time.

### 5.4 Frontend-specific assembly passes

#### 5.4.1 SemOS assembly pass (`dsl-semos-frontend`)

**Input**: AtomBag containing verb atoms, invoke atoms, utterance-binding atoms, predicate atoms, graph-pack atoms, constellation-root atoms, workspace-constraint atoms, and declarative atoms.

**Output**: 
- `DagGraph` — a directed acyclic graph of verbs linked by data dependencies (outputs of one verb feeding inputs of another)
- `TypedSemosAst` — the resolved atom bag with typed nodes for each atom kind
- `UnresolvedRefs` — name references not resolved in the assembly pass (forwarded to resolution)

**Algorithm:**

1. Build atom name index: `HashMap<String, &UntypedAtom>` from the bag.
2. Extract all `(verb ...)` atoms → `VerbDeclaration` nodes.
3. Extract all `(invoke ...)` atoms → `InvocationNode` nodes.
4. Extract all `(graph-pack ...)` atoms → `DagPackNode` nodes.
5. Build dependency edges: for each `(invoke ...)` atom, if its output slot (`:outputs`) provides a value that another `(invoke ...)` atom consumes as an `@`-binding or arg, draw a directed edge from producer to consumer.
6. Validate DAG: detect cycles. SemOS dependency DAGs must be acyclic at the static level (dynamic cycles via loops in packs are not modelled at compile time). Cycle detection uses DFS; cycle members reported as assembly errors.
7. Build `DagGraph`: nodes are `InvocationNode` instances; edges are typed dependency links.
8. Attach predicate atoms, constraint atoms, and utterance-binding atoms to the graph as annotations (not part of the execution topology, but needed by the resolver and runtime).

**Structural validation rules (SemOS):**

- Every `(invoke ...)` atom must name a verb that exists in the atom bag or in an imported source. Unresolved verb refs are `UnresolvedRef` entries forwarded to the resolution pass.
- Every named atom referenced by an edge (output/input binding) must exist. Unresolved binding refs are forwarded.
- A DAG with no start nodes (invocations with no incoming edges) is valid — it represents a free verb surface.
- Duplicate atom names are assembly errors.

---

#### 5.4.2 bpmn-lite assembly pass (`dsl-bpmn-frontend`)

**Input**: AtomBag containing node atoms, gateway atoms, flow atoms, boundary-attachment atoms, parallel-join atoms, message/timer/error-definition atoms, and declarative atoms.

**Output**:
- `RailwayGraph` — the typed process graph: nodes connected by typed edges, gateway fan-out encoded, boundary attachments resolved
- `TypedBpmnAst` — typed atom nodes
- `UnresolvedRefs` — forwarded to resolution

**Algorithm:**

1. Build atom name index.
2. Separate atoms by kind: nodes (including gateways and parallel-joins), flow atoms, boundary-attachment atoms, definition atoms.
3. Build node table: `HashMap<String, NodeEntry>` — every node/gateway/parallel-join atom becomes a `NodeEntry` with its kind, verb ref (if any), event-def ref (if any), and empty edge lists.
4. Thread flow edges: for each `(flow source target ...)` atom:
   a. Look up source and target in node table.
   b. If either is not found: `UnresolvedFlowEndpoint` diagnostic + continue.
   c. Add typed edge to source's outgoing list and target's incoming list.
   d. Carry `:condition` and `:default` attributes on the edge.
5. Thread boundary attachment edges: for each `(boundary-attachment :node N ...)` atom, create a synthetic node entry (kind = `boundary-<event-kind>`, interrupting flag set). Thread the synthetic node into the graph at position N (outgoing from the synthetic node; the assembly pass creates a synthetic edge from the parent node to the synthetic node for flow purposes).
6. Validate structural rules (see below).
7. Build `RailwayGraph`: nodes are typed `ProcessNode` variants; edges are `SequenceFlow` or `BoundaryEdge`.

**Structural validation rules (bpmn-lite):**

| Rule | Error kind | Severity |
|---|---|---|
| Every flow source must resolve to a known node or gateway atom name | `UnresolvedFlowEndpoint` | Error |
| Every flow target must resolve to a known node or gateway atom name | `UnresolvedFlowEndpoint` | Error |
| Every node must be reachable from at least one start-event node (via BFS from all start-events) | `UnreachableNode` | Error |
| Every terminal path must end at an end-event node or a parallel-join that feeds an end-event | `UnterminatedPath` | Error |
| Exclusive gateway: must have ≥ 1 outgoing flow with a condition; must have exactly 1 default flow (or exactly 0, in which case the last condition must be a catch-all) | `GatewayMissingDefault` | Warn |
| Exclusive gateway: exactly 1 incoming flow (split-only; convergence is through explicit join nodes) | `GatewayMultipleIncoming` | Error |
| Parallel gateway (fork): exactly 1 incoming flow; ≥ 2 outgoing flows | `GatewayInvalidFanout` | Error |
| Parallel gateway (join via parallel-join atom): must have a corresponding `(parallel-join :expects [...])` atom naming it | `MissingParallelJoin` | Error |
| Inclusive gateway: must have a corresponding `(parallel-join :expects [...])` naming it | `MissingParallelJoin` | Error |
| Boundary attachment: `:node` must resolve to an activity (task or subprocess), not a gateway or event | `InvalidBoundaryAttachmentTarget` | Error |
| Boundary attachment: at most 1 interrupting timer per host node (non-interrupting timers are unlimited) | `DuplicateInterruptingBoundary` | Error |
| Cycles: cycles are allowed only when passing through a loop-marked task (`:loop {:condition ...}`) or through an explicit loop-gateway pattern (exclusive gateway with back-edge). All other cycles are errors | `ForbiddenCycle` | Error |
| Duplicate atom names within the source | `DuplicateName` | Error |
| Forward references: all unresolved name-refs are forwarded; the assembly pass does not error on unresolved refs (the resolution pass handles them) | — | — |

**Forward references in bpmn-lite**: it is valid to define `(flow create-task -> validate-task)` before either `create-task` or `validate-task` appears in the source. The assembly pass builds a forward-reference map and resolves it after all atoms have been indexed. If either endpoint is still unresolved after full indexing, it becomes an `UnresolvedFlowEndpoint`.

### 5.5 Shared resolution pass

**Input**: Typed AST from either assembly pass + UnresolvedRefs.

**Resolution steps (in order):**

1. **Cross-artifact ref resolution**: qualified name-refs (`pack-name/atom-name`) are resolved against imported sources loaded into the compilation context. Unresolvable qualified refs are `UnresolvedQualifiedRef` errors.

2. **Verb lookup**: every `(invoke verb-ref ...)` node's `verb-ref` is looked up in:
   a. The current source's verb atom bag (already indexed by the assembly pass).
   b. The external verb registry (the SemOS registry, accessible via `VerbRegistryClient`).
   If not found in either: `UnresolvedVerbRef` error.

3. **@-slot binding from authoring context**:
   For each `(verb :@-slots [...])` declaration and each `(invoke ...)` invocation site:
   a. Assembly-resolvable slots (`@node`, `@decision`, `@subprocess`): inject from the enclosing structural context available in the typed AST (e.g., `@node` = the name of the enclosing `(node ...)` atom).
   b. Runtime-resolvable slots (`@process`, `@token`, `@parent`): mark as `RuntimeBound` — no resolution action needed; the runtime will inject.
   c. Required slots with no available injection site: `MissingAtSlotBinding` error (naming the verb, the slot, and the invocation site).
   d. Type compatibility: check that the structural context's type matches the @-slot's declared type. Type mismatch: `AtSlotTypeMismatch` error.

4. **Input type checking**: for each `(invoke :args {...})`, check that arg names match the verb's `:inputs` declarations and that provided values are type-compatible. Missing required inputs: `MissingRequiredInput`. Unknown input name: `UnknownInputName`. Type mismatch: `InputTypeMismatch`.

5. **Declarative atom validation**:
   a. `(provenance :covers [atom-refs ...])`: each atom-ref must resolve to a structural atom in the current source. Unresolvable provenance refs: `UnresolvedProvenanceRef` warn.
   b. `(governance-status :atom atom-ref)`: atom-ref must resolve. Unresolvable: `UnresolvedGovernanceRef` warn.
   c. `(review-annotation :atom atom-ref)`: same.
   d. Pack version status: if a `(provenance :source-id P :version V)` references a pack version whose `(governance-status ...)` records `state: deprecated`, emit `DeprecatedPackVersion` warn. If `state: retired`, emit `RetiredPackVersion` error.

6. **Error kind coverage** (bpmn-lite only): for each `end-event-error` node naming an error code, check that the error code is declared by at least one reachable verb's `:errors` list. Undeclared error code: `UndeclaredErrorCode` warn.

7. **Output**: fully resolved typed AST; all NameRef nodes replaced with resolved atom pointers or marked as `RuntimeBound`; DiagnosticSet with accumulated warnings and errors.

**External registry interface:**

```rust
pub trait VerbRegistryClient: Send + Sync {
    fn lookup_verb(&self, fqn: &str) -> Option<VerbDeclaration>;
    fn lookup_entity_type(&self, name: &str) -> Option<EntityTypeDeclaration>;
    fn lookup_data_type(&self, name: &str) -> Option<DataTypeDeclaration>;
}
```

The resolution pass receives a `&dyn VerbRegistryClient`. In production this calls the SemOS registry. In tests it accepts a mock. This is the only external dependency of the resolution pass.

### 5.6 Lowering pass

**Input**: Fully resolved typed AST from the resolution pass.

**Output**: frontend-specific executable form. Declarative atoms are dropped at this pass.

#### SemOS lowering (`dsl-lowering/src/semos.rs`)

Produces an `ExecutionPlan`:
```rust
pub struct ExecutionPlan {
    pub steps: Vec<CompileStep>,
    pub injection_graph: InjectionGraph,  // which step outputs feed which step inputs
    pub verb_registry_snapshot: VerbRegistrySnapshot,
}

pub struct CompileStep {
    pub index: usize,
    pub verb_fqn: String,
    pub resolved_args: HashMap<String, ResolvedValue>,
    pub at_slot_bindings: HashMap<String, AtSlotBinding>,
    pub source_span: Span,
}
```

`InjectionGraph` is a DAG of step indices mirroring the `DagGraph` from the assembly pass. The injection graph drives the sequencer's dispatch ordering and `@`-symbol binding.

This is largely a mapping from the resolved typed AST nodes to the existing `CompileStep` structures. The primary change from the current compiler is that the new lowering pass reads from the unified typed AST rather than from a VerbCall-to-CompileStep direct conversion.

#### bpmn-lite lowering (`dsl-lowering/src/bpmn.rs`)

Produces a `JourneySpec`:
```rust
pub struct JourneySpec {
    pub workflow_id: String,
    pub version_hash: [u8; 32],   // BLAKE3 of normalised source
    pub nodes: HashMap<String, JourneyNode>,
    pub start_nodes: Vec<String>,
}

pub struct JourneyNode {
    pub id: String,
    pub kind: JourneyNodeKind,
    pub outgoing: Vec<JourneyEdge>,
    pub boundary_events: Vec<BoundarySpec>,
    pub verb_binding: Option<ResolvedVerbBinding>,
    pub event_def: Option<ResolvedEventDef>,
    pub loop_spec: Option<LoopSpec>,
    pub multi_instance: Option<MultiInstanceSpec>,
    pub compensation_handler: Option<String>,  // node id
}

pub enum JourneyNodeKind {
    StartEvent(StartEventKind),
    EndEvent(EndEventKind),
    IntermediateCatch(IntermediateCatchKind),
    IntermediateThrow(IntermediateThrowKind),
    ServiceTask,
    UserTask,
    SendTask,
    ReceiveTask,
    ManualTask,
    BusinessRuleTask { decision_ref: String },
    ScriptTask { script: ResolvedExpr },
    Subprocess { inline: Box<JourneySpec> },
    EventSubprocess { trigger: ResolvedEventDef, inline: Box<JourneySpec> },
    TransactionSubprocess { inline: Box<JourneySpec> },
    CallActivity { called_workflow: String, input_mapping: Vec<DataMapping>, output_mapping: Vec<DataMapping> },
    ExclusiveGateway,
    InclusiveGateway,
    ParallelGateway,
    EventBasedGateway,
    ParallelEventBasedGateway,
    ParallelJoin { expected_forks: Vec<String>, merge_clauses: Vec<MergeClause> },
    BoundaryEvent(BoundaryEventKind, BoundaryInterrupting),
}

pub struct JourneyEdge {
    pub target: String,
    pub condition: Option<ResolvedExpr>,
    pub is_default: bool,
}

pub struct MergeClause {
    pub location: DataLocation,
    pub operator: MergeOperator,
    pub custom_verb: Option<String>,
}
```

`version_hash` is BLAKE3 of the normalised (whitespace-stripped, deterministically-sorted) source text. Identical source always produces the same hash; any change to structural content changes the hash. This is the stable identity used to match deployed JourneySpecs to persisted instances.

**The JourneySpec is the contract between the compiler and the runtime.** The runtime reads JourneySpecs from the deployed process store; it does not re-compile from source at runtime.

### 5.7 Diagnostic surface

**Error taxonomy:**

| Pass | Error kind | Severity |
|---|---|---|
| Parse (0) | `UnexpectedToken` | Error |
| Parse (0) | `UnmatchedParen` | Error |
| Parse (0) | `InvalidLiteral` | Error |
| Parse (0) | `UnknownAtomKind` (structural namespace) | Error |
| Parse (0) | `UnknownDeclarativeKind` | Warn |
| Assembly (1) | `DuplicateName` | Error |
| Assembly (1) | `UnresolvedFlowEndpoint` | Error |
| Assembly (1) | `UnreachableNode` | Error |
| Assembly (1) | `UnterminatedPath` | Error |
| Assembly (1) | `GatewayMissingDefault` | Warn |
| Assembly (1) | `GatewayMultipleIncoming` | Error |
| Assembly (1) | `GatewayInvalidFanout` | Error |
| Assembly (1) | `MissingParallelJoin` | Error |
| Assembly (1) | `InvalidBoundaryAttachmentTarget` | Error |
| Assembly (1) | `DuplicateInterruptingBoundary` | Error |
| Assembly (1) | `ForbiddenCycle` | Error |
| Assembly (1) | `DagCycle` (SemOS only) | Error |
| Resolution (2) | `UnresolvedVerbRef` | Error |
| Resolution (2) | `UnresolvedQualifiedRef` | Error |
| Resolution (2) | `MissingAtSlotBinding` | Error |
| Resolution (2) | `AtSlotTypeMismatch` | Error |
| Resolution (2) | `MissingRequiredInput` | Error |
| Resolution (2) | `UnknownInputName` | Error |
| Resolution (2) | `InputTypeMismatch` | Error |
| Resolution (2) | `UnresolvedProvenanceRef` | Warn |
| Resolution (2) | `UnresolvedGovernanceRef` | Warn |
| Resolution (2) | `DeprecatedPackVersion` | Warn |
| Resolution (2) | `RetiredPackVersion` | Error |
| Resolution (2) | `UndeclaredErrorCode` | Warn |

Every diagnostic carries:
```rust
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub severity: Severity,  // Error | Warn | Info
    pub span: Span,           // source location
    pub message: String,      // human-readable with context
    pub notes: Vec<String>,   // additional context (e.g., "verb declared here")
    pub pass: CompilerPass,
}
```

**Multi-pass accumulation**: the compiler accumulates diagnostics across passes and returns them together. A pass with only `Warn` diagnostics does not prevent the next pass from running. A pass with any `Error` diagnostics causes subsequent passes to run in degraded mode (they continue, but lowering is skipped — no executable form is produced when errors exist).

**Error limit**: configurable via `CompilerOptions::max_errors` (default 50). Once the limit is reached, compilation stops mid-pass with a `TooManyErrors` sentinel.

### 5.8 REPL integration contract

The compiler exposes three public operations through `dsl-compiler`:

---

**`validate(source: &str, options: CompilerOptions) -> ValidateResult`**

Runs passes 0–2. Does not run lowering. Returns:

```rust
pub struct ValidateResult {
    pub diagnostics: DiagnosticSet,
    pub graph: Option<AssembledGraph>,       // None if assembly errors
    pub resolution_summary: Option<ResolutionSummary>,
    pub provenance_summary: Option<ProvenanceSummary>,
}

pub enum AssembledGraph {
    Dag(DagGraphSummary),       // SemOS: verb nodes, dependency edges
    Railway(RailwayGraphSummary), // bpmn-lite: node list, edge list, gateway kinds
}

pub struct DagGraphSummary {
    pub nodes: Vec<DagNodeSummary>,    // verb name, inputs, outputs
    pub edges: Vec<(String, String)>,  // producer verb → consumer verb
}

pub struct RailwayGraphSummary {
    pub nodes: Vec<RailwayNodeSummary>,  // id, kind, verb_ref, reachable, has_errors
    pub edges: Vec<RailwayEdgeSummary>,  // source, target, condition_expr, is_default
    pub unreachable_nodes: Vec<String>,
    pub path_count: usize,               // number of distinct source-to-end paths
}

pub struct ResolutionSummary {
    pub resolved_verbs: Vec<String>,
    pub unresolved_refs: Vec<UnresolvedRef>,
    pub at_slot_bindings: Vec<AtSlotBindingSummary>,
}

pub struct ProvenanceSummary {
    pub instantiations: Vec<ProvenanceInstantiationSummary>,  // one per provenance atom
    pub uncovered_atoms: Vec<String>,  // structural atoms with no provenance coverage
}
```

The `AssembledGraph` is a normative compiler output surfaced to the REPL. The REPL displays the railway or DAG shape, highlights errors by node, and allows the user to inspect verb bindings per node. This is the primary authoring feedback loop.

---

**`compile(source: &str, options: CompilerOptions) -> CompileResult`**

Runs all four passes. Returns:

```rust
pub struct CompileResult {
    pub diagnostics: DiagnosticSet,
    pub executable: Option<ExecutableForm>,
    pub graph: Option<AssembledGraph>,
}

pub enum ExecutableForm {
    ExecutionPlan(ExecutionPlan),  // SemOS
    JourneySpec(JourneySpec),      // bpmn-lite
}
```

No executable form is produced if any Error diagnostics exist. The caller must check `diagnostics.has_errors()` before using `executable`.

---

**`deploy(name: &str, source: &str, options: CompilerOptions) -> DeployResult`**

Compiles and, on success, stores the executable form in the deployed process store.

```rust
pub struct DeployResult {
    pub diagnostics: DiagnosticSet,
    pub deployed_version: Option<DeployedVersion>,
}

pub struct DeployedVersion {
    pub name: String,
    pub version_hash: [u8; 32],
    pub deployed_at: DateTime<Utc>,
    pub deployed_by: String,
}
```

Deploy is idempotent on hash — if a JourneySpec with the same `version_hash` already exists in the store, `deploy` returns the existing `DeployedVersion` without re-inserting.

**Frontend selection**: `CompilerOptions::frontend` determines which assembly pass runs. Options: `SemOS`, `Bpmn`, `Auto` (infer from source content — if `(workflow ...)` or node/flow atoms are present, use bpmn-lite; otherwise use SemOS).

---

## 6. Runtime Design

### 6.1 Execution model overview

The runtime is a **journey-persisted hydrate/dehydrate event processor**. Every state transition is recorded as an entry in the append-only journey log in Postgres. The in-memory state of a running process instance is authoritative only for the duration of a single event-processing transaction; between events, all state lives in Postgres.

**The journey metaphor**: a process instance is a journey — an ordered sequence of state transitions. The journey log is the chronicle of that sequence. The current state is the fold of the log. The runtime reads the fold (hydration), applies one event, and appends new entries (dehydration). Between events, the instance holds no memory in the runtime process.

**Explicit contrast with in-memory engines:**

| In-memory engine (e.g. Zeebe, Camunda Engine) | Journey-persisted engine (bpmn-lite) |
|---|---|
| Process instance lives in RAM; persisted asynchronously or via Raft replication | Process instance state is the Postgres log; RAM is a per-event working buffer |
| Crash recovery requires log replay or snapshot restore | Crash recovery is trivial — restart the event loop; instances resume from last committed log entry |
| Long-lived waits (timers, message correlation) consume memory for the duration | Long-lived waits cost zero memory; the instance is fully dehydrated; a timer service polls the pending_wait table |
| Multi-node concurrency requires consensus or partitioning at the engine level | Concurrency is per-instance; different instances can be processed by different workers without coordination; serialisation is only needed within a single instance |
| Execution is synchronous within a fiber | Each event is an independent database transaction |

**Hydrate/dehydrate cycle:**

```
Event arrives (instance_id, event_kind, event_payload)
    │
    ▼
Hydration query: load active_token rows for this instance
                 load pending_wait row for this event (if event-driven)
                 load instance_data snapshot
    │
    ▼
Compute: determine which node(s) to advance;
         evaluate gateway conditions (via switch adaptor);
         invoke verb (in-process, synchronous);
         compute data writes and follow-on events
    │
    ▼
Dehydration (within single database transaction):
    - Append journey_log entries (token transitions, data deltas)
    - Update active_token rows (new positions, updated write logs)
    - Upsert instance_data
    - Delete or update pending_wait rows (if wait was satisfied)
    - Insert follow-on events into event queue (or emit directly)
    - Optionally insert audit_log entries
    │
    ▼
Commit transaction
    │
    ▼
Event processor dehydrated; instance holds no state in RAM
```

**Idempotency**: if the database transaction fails after computing but before committing, the event is re-processed on the next delivery. The computation must be idempotent. Verb invocations that have external side effects (sending messages, calling external APIs) are wrapped in idempotency envelopes keyed by `(instance_id, event_id, verb_fqn)`. If the side effect was already applied before the crash, the idempotency check returns the cached result without re-applying.

**Event ordering**: events for the same instance are serialised by the event loop (through either a per-instance queue or a database advisory lock). Events for different instances are independent and can be processed concurrently.

### 6.2 Instance state schema

Full PostgreSQL DDL for all runtime tables:

```sql
-- The deployed process store: compiled JourneySpecs
CREATE TABLE deployed_journey (
    workflow_id          TEXT         NOT NULL,
    version_hash         BYTEA        NOT NULL,  -- 32-byte BLAKE3
    spec_json            JSONB        NOT NULL,  -- serialised JourneySpec
    deployed_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    deployed_by          TEXT         NOT NULL,
    is_current           BOOLEAN      NOT NULL DEFAULT TRUE,
    CONSTRAINT pk_deployed_journey PRIMARY KEY (workflow_id, version_hash)
);
CREATE INDEX idx_deployed_journey_current ON deployed_journey (workflow_id) WHERE is_current;

-- Top-level process instance record
CREATE TABLE workflow_instance (
    instance_id          UUID         NOT NULL DEFAULT gen_random_uuid(),
    workflow_id          TEXT         NOT NULL,
    version_hash         BYTEA        NOT NULL,  -- which JourneySpec version
    status               TEXT         NOT NULL   -- running|waiting|completed|failed|cancelled
                         CHECK (status IN ('running','waiting','completed','failed','cancelled')),
    started_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    completed_at         TIMESTAMPTZ,
    tenant_id            TEXT         NOT NULL,
    initiated_by         TEXT,        -- actor who started the process
    correlation_id       TEXT,        -- external correlation (if started by message)
    CONSTRAINT pk_workflow_instance PRIMARY KEY (instance_id)
);
CREATE INDEX idx_workflow_instance_status  ON workflow_instance (tenant_id, status);
CREATE INDEX idx_workflow_instance_workflow ON workflow_instance (workflow_id, status);
CREATE INDEX idx_workflow_instance_correlation ON workflow_instance (correlation_id) WHERE correlation_id IS NOT NULL;

-- Append-only journey log: every state transition
CREATE TABLE journey_log (
    entry_id             BIGSERIAL    NOT NULL,
    instance_id          UUID         NOT NULL REFERENCES workflow_instance(instance_id),
    token_id             UUID         NOT NULL,
    sequence             BIGINT       NOT NULL,  -- monotonic within instance
    event_kind           TEXT         NOT NULL,
    from_node            TEXT,
    to_node              TEXT,
    data_delta           JSONB,       -- data writes applied at this transition (null = no data change)
    triggered_by         UUID,        -- event_queue.event_id that triggered this entry
    recorded_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT pk_journey_log PRIMARY KEY (entry_id)
);
CREATE INDEX idx_journey_log_instance ON journey_log (instance_id, sequence);
CREATE INDEX idx_journey_log_token    ON journey_log (token_id, sequence);

-- Active token positions: one row per live token
CREATE TABLE active_token (
    token_id             UUID         NOT NULL DEFAULT gen_random_uuid(),
    instance_id          UUID         NOT NULL REFERENCES workflow_instance(instance_id),
    current_node         TEXT         NOT NULL,
    parent_fork          TEXT,        -- gateway id of the fork that emitted this token
    branch_lineage       TEXT[],      -- ordered path of fork node ids from root to this token
    write_log            JSONB        NOT NULL DEFAULT '[]',
                         -- array of {location, value, sequence} — writes since last fork
    expected_arrival_set TEXT[],      -- for ParallelJoin and InclusiveJoin tokens: remaining expected arrivals
    created_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT pk_active_token PRIMARY KEY (token_id)
);
CREATE INDEX idx_active_token_instance ON active_token (instance_id);
CREATE INDEX idx_active_token_node     ON active_token (current_node, instance_id);

-- Versioned instance data: application data the process operates on
CREATE TABLE instance_data (
    instance_id          UUID         NOT NULL REFERENCES workflow_instance(instance_id),
    key                  TEXT         NOT NULL,  -- data location name
    value                JSONB        NOT NULL,
    version              BIGINT       NOT NULL,  -- incremented on each write
    written_by_token     UUID,                   -- token that last wrote this location
    written_at_sequence  BIGINT,                 -- journey_log sequence at time of write
    CONSTRAINT pk_instance_data PRIMARY KEY (instance_id, key)
);

-- Pending waits: instances waiting for an external event
CREATE TABLE pending_wait (
    wait_id              UUID         NOT NULL DEFAULT gen_random_uuid(),
    instance_id          UUID         NOT NULL REFERENCES workflow_instance(instance_id),
    token_id             UUID         NOT NULL,
    current_node         TEXT         NOT NULL,
    wait_kind            TEXT         NOT NULL  -- timer|message|human-task|signal|external
                         CHECK (wait_kind IN ('timer','message','human-task','signal','external')),
    correlation_key      TEXT,        -- for message waits: the key to match incoming messages
    correlation_data     JSONB,       -- additional match criteria
    timeout_at           TIMESTAMPTZ, -- for timer waits
    created_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT pk_pending_wait PRIMARY KEY (wait_id)
);
CREATE INDEX idx_pending_wait_correlation ON pending_wait (correlation_key, wait_kind) WHERE correlation_key IS NOT NULL;
CREATE INDEX idx_pending_wait_timeout     ON pending_wait (timeout_at) WHERE wait_kind = 'timer';
CREATE INDEX idx_pending_wait_instance    ON pending_wait (instance_id);

-- Switch decision requests in flight: gateway waiting for adaptor reply
CREATE TABLE switch_decision_request (
    request_id           UUID         NOT NULL DEFAULT gen_random_uuid(),
    instance_id          UUID         NOT NULL REFERENCES workflow_instance(instance_id),
    token_id             UUID         NOT NULL,
    gateway_node         TEXT         NOT NULL,
    gateway_kind         TEXT         NOT NULL,
    data_context         JSONB        NOT NULL,   -- relevant data subset for decision
    legal_branches       TEXT[]       NOT NULL,   -- outgoing flow target node ids
    adaptor_binding      TEXT         NOT NULL,   -- which adaptor handles this request
    expected_reply_schema JSONB,
    timeout_at           TIMESTAMPTZ,
    requested_at         TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT pk_switch_decision_request PRIMARY KEY (request_id)
);

-- Human task records: created when a user-task verb fires
CREATE TABLE human_task (
    task_id              UUID         NOT NULL DEFAULT gen_random_uuid(),
    instance_id          UUID         NOT NULL REFERENCES workflow_instance(instance_id),
    token_id             UUID         NOT NULL,
    node_id              TEXT         NOT NULL,
    task_type            TEXT         NOT NULL,
    assignee             TEXT,
    candidate_groups     TEXT[],
    form_key             TEXT,
    data_context         JSONB,
    priority             INTEGER      NOT NULL DEFAULT 50,
    due_at               TIMESTAMPTZ,
    created_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    completed_at         TIMESTAMPTZ,
    completed_by         TEXT,
    completion_data      JSONB,
    status               TEXT         NOT NULL DEFAULT 'active'
                         CHECK (status IN ('active','completed','cancelled')),
    CONSTRAINT pk_human_task PRIMARY KEY (task_id)
);
CREATE INDEX idx_human_task_instance ON human_task (instance_id) WHERE status = 'active';
CREATE INDEX idx_human_task_assignee ON human_task (assignee) WHERE status = 'active';

-- Audit log: decision adaptor replies, verb invocations, errors (regulatory surface)
CREATE TABLE audit_log (
    audit_id             BIGSERIAL    NOT NULL,
    instance_id          UUID         NOT NULL,
    token_id             UUID,
    node_id              TEXT,
    audit_kind           TEXT         NOT NULL,  -- verb-invoked|adaptor-replied|error-raised|boundary-fired|compensation-invoked
    actor                TEXT,
    payload              JSONB,
    recorded_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT pk_audit_log PRIMARY KEY (audit_id)
);
CREATE INDEX idx_audit_log_instance ON audit_log (instance_id, recorded_at);

-- Event queue: inbound events awaiting processing
CREATE TABLE event_queue (
    event_id             UUID         NOT NULL DEFAULT gen_random_uuid(),
    instance_id          UUID,        -- null for broadcast events (signals)
    event_kind           TEXT         NOT NULL,
    payload              JSONB        NOT NULL,
    correlation_key      TEXT,
    status               TEXT         NOT NULL DEFAULT 'pending'
                         CHECK (status IN ('pending','processing','processed','failed')),
    enqueued_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    processed_at         TIMESTAMPTZ,
    attempts             INTEGER      NOT NULL DEFAULT 0,
    CONSTRAINT pk_event_queue PRIMARY KEY (event_id)
);
CREATE INDEX idx_event_queue_pending    ON event_queue (enqueued_at) WHERE status = 'pending';
CREATE INDEX idx_event_queue_instance   ON event_queue (instance_id, status);
CREATE INDEX idx_event_queue_correlation ON event_queue (correlation_key) WHERE correlation_key IS NOT NULL;

-- Timer service: pending timers with absolute fire times
CREATE TABLE pending_timer (
    timer_id             UUID         NOT NULL DEFAULT gen_random_uuid(),
    wait_id              UUID         NOT NULL REFERENCES pending_wait(wait_id) ON DELETE CASCADE,
    instance_id          UUID         NOT NULL,
    fire_at              TIMESTAMPTZ  NOT NULL,
    fired                BOOLEAN      NOT NULL DEFAULT FALSE,
    CONSTRAINT pk_pending_timer PRIMARY KEY (timer_id)
);
CREATE INDEX idx_pending_timer_fire_at ON pending_timer (fire_at) WHERE NOT fired;

-- Idempotency ledger: deduplicates verb side-effects across crash-restart
CREATE TABLE idempotency_ledger (
    idempotency_key      TEXT         NOT NULL,  -- (instance_id, event_id, verb_fqn)
    result_payload       JSONB,                  -- cached side-effect result
    created_at           TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CONSTRAINT pk_idempotency_ledger PRIMARY KEY (idempotency_key)
);

-- Compensation log: ordered log of completed nodes within a transaction-subprocess
-- Used to drive reverse-order compensation on transaction failure
CREATE TABLE compensation_log (
    log_id               BIGSERIAL    NOT NULL,
    instance_id          UUID         NOT NULL,
    transaction_node     TEXT         NOT NULL,  -- transaction-subprocess node id
    completed_node       TEXT         NOT NULL,  -- the node that completed
    completed_at_sequence BIGINT      NOT NULL,  -- journey_log sequence at completion
    CONSTRAINT pk_compensation_log PRIMARY KEY (log_id)
);
CREATE INDEX idx_compensation_log_transaction ON compensation_log (instance_id, transaction_node, completed_at_sequence);
```

**Access pattern justification:**

| Access pattern | Primary index |
|---|---|
| Event arrives for instance X → hydrate | `active_token(instance_id)` — all live token positions |
| Message arrives with correlation key K → find waiting instance | `pending_wait(correlation_key, wait_kind)` |
| Timer fires → find expired waits | `pending_timer(fire_at) WHERE NOT fired` |
| REPL queries instance state | `active_token(instance_id)` + `instance_data(instance_id)` |
| Audit query: history for instance X | `journey_log(instance_id, sequence)` — append-only; fast sequential scan |
| Operations query: all instances at node Y | `active_token(current_node, instance_id)` |
| Human task inbox for user A | `human_task(assignee) WHERE status = 'active'` |

**Partitioning**: `journey_log` and `audit_log` should be partitioned by `recorded_at` (monthly ranges) for retention management. All other tables remain unpartitioned at initial deployment; partition if instance volume exceeds 10M rows per table (expected at year 2+). `workflow_instance` may be partitioned by `tenant_id` for multi-tenant deployments.

**Retention policy**: `journey_log` is append-only and regulatory — minimum 7-year retention (EU MiFID II Article 16 obligation). `event_queue` can be truncated after `processed` entries age past 30 days. `audit_log` follows the same 7-year minimum as the journey log. `idempotency_ledger` entries can be purged after 7 days (side-effect deduplication window).

### 6.3 Event model

The runtime processes eight event kinds. Each event type has a defined schema.

---

**1. InstanceStart**

Emitted when a client calls `start_instance(workflow_id, input_data, options)`.

```json
{
  "event_kind": "instance_start",
  "instance_id": "<uuid>",
  "workflow_id": "<string>",
  "version_hash": "<hex-32>",
  "input_data": { "<key>": "<value>" },
  "initiated_by": "<actor>",
  "correlation_id": "<string|null>"
}
```

Processing: load JourneySpec for `workflow_id`/`version_hash`; create `workflow_instance` row; create initial token at the start-event node; enqueue `TokenAdvance` for the start-event node.

---

**2. TokenAdvance**

Internal event: advance a token from its current position to the next node(s).

```json
{
  "event_kind": "token_advance",
  "instance_id": "<uuid>",
  "token_id": "<uuid>",
  "from_node": "<string>",
  "event_type": "instance_start|verb_complete|adaptor_reply|message_arrived|timer_fired|human_task_complete|sub_process_complete|error_raised|cancellation"
}
```

Processing: determine next node(s) from JourneySpec edges; if next node is a gateway, emit `SwitchDecisionRequest`; if next node is a task, invoke verb; if next node is a wait, insert `pending_wait`; append `journey_log` entries.

---

**3. VerbCompletion**

Emitted when a verb invocation completes (always synchronous — the verb returns immediately).

```json
{
  "event_kind": "verb_completion",
  "instance_id": "<uuid>",
  "token_id": "<uuid>",
  "node_id": "<string>",
  "verb_fqn": "<string>",
  "outputs": { "<key>": "<value>" },
  "wait_initiated": { "wait_kind": "<string>", "correlation_key": "<string|null>", "timeout_at": "<ts|null>" } | null,
  "error_raised": { "code": "<string>", "message": "<string>" } | null
}
```

If `wait_initiated` is non-null, a `pending_wait` row is inserted and the token remains at the current node. If `error_raised` is non-null, the token transitions to error boundary processing. Otherwise, a `TokenAdvance` is enqueued for the outgoing flow.

---

**4. SwitchDecisionReply**

Reply from a switch adaptor after processing a gateway decision request.

```json
{
  "event_kind": "switch_decision_reply",
  "request_id": "<uuid>",
  "instance_id": "<uuid>",
  "token_id": "<uuid>",
  "gateway_node": "<string>",
  "chosen_branches": ["<target_node_id>"],  // 1 for exclusive, 1..N for inclusive, all for parallel
  "reasoning": "<string|null>",             // optional adaptor audit trail
  "error": { "code": "<string>" } | null
}
```

Processing: delete `switch_decision_request` row; enqueue `TokenAdvance` events for each chosen branch (with fork if N > 1).

---

**5. TimerFired**

Emitted by the timer service when a `pending_timer` entry's `fire_at` time has elapsed.

```json
{
  "event_kind": "timer_fired",
  "timer_id": "<uuid>",
  "wait_id": "<uuid>",
  "instance_id": "<uuid>",
  "token_id": "<uuid>",
  "node_id": "<string>"
}
```

Processing: mark `pending_timer.fired = true`; delete `pending_wait` row; enqueue `TokenAdvance` (timer fired, token can proceed).

---

**6. MessageArrival**

Emitted when an external message arrives at the runtime's message endpoint.

```json
{
  "event_kind": "message_arrived",
  "message_name": "<string>",
  "correlation_key": "<string>",
  "payload": { "<key>": "<value>" },
  "arrived_at": "<timestamp>"
}
```

Processing: look up `pending_wait` rows matching `correlation_key` and `wait_kind = 'message'`; for each match (typically 1), enqueue `TokenAdvance` (message correlation satisfied). If no match: enqueue in `event_queue` with `status = 'pending'` for retry (message may arrive before instance reaches wait state).

---

**7. HumanTaskCompletion**

Emitted when a user completes a human task via the task interface.

```json
{
  "event_kind": "human_task_complete",
  "task_id": "<uuid>",
  "instance_id": "<uuid>",
  "token_id": "<uuid>",
  "node_id": "<string>",
  "completed_by": "<actor>",
  "completion_data": { "<key>": "<value>" }
}
```

Processing: update `human_task.status = 'completed'`; delete `pending_wait`; enqueue `TokenAdvance`.

---

**8. SubProcessCompletion**

Emitted when a call-activity child instance completes.

```json
{
  "event_kind": "sub_process_complete",
  "parent_instance_id": "<uuid>",
  "parent_token_id": "<uuid>",
  "parent_node_id": "<string>",
  "child_instance_id": "<uuid>",
  "output_data": { "<key>": "<value>" }
}
```

Processing: apply output mapping (JourneySpec `output_mapping` for the call-activity node); enqueue `TokenAdvance` on the parent token.

### 6.4 Main event loop

Pseudocode for the runtime's core event processor:

```rust
async fn process_event(event: Event, ctx: &RuntimeContext) -> Result<()> {
    // 1. Identify target instance and node.
    let instance_id = event.instance_id()?;
    let journey = ctx.journey_store.load_current(instance_id).await?;

    // 2. Begin transaction (serialises events for this instance).
    let tx = ctx.db.begin().await?;
    let _lock = tx.advisory_lock(instance_id_to_lock_key(instance_id)).await?;

    // 3. Verify instance is in a state where the event is legal.
    let instance = load_instance(&tx, instance_id).await?;
    if !event.is_legal_for(&instance.status) {
        // E.g., event arrived for a completed instance.
        tx.rollback().await?;
        return Ok(()); // silently discard; event is stale
    }

    // 4. Hydrate relevant node(s).
    let tokens = load_active_tokens(&tx, instance_id).await?;
    let target_tokens = tokens.iter()
        .filter(|t| event.targets_token(t.token_id))
        .collect::<Vec<_>>();

    // 5. For each targeted token: determine action.
    let mut journal: Vec<JourneyEntry> = Vec::new();
    let mut follow_on_events: Vec<Event> = Vec::new();
    let mut data_writes: Vec<DataWrite> = Vec::new();
    let mut wait_ops: Vec<WaitOp> = Vec::new();

    for token in &target_tokens {
        let node = journey.nodes.get(&token.current_node)?;

        match &node.kind {
            JourneyNodeKind::ServiceTask | JourneyNodeKind::UserTask | ... => {
                // 5a. Invoke verb (synchronous).
                let verb_result = invoke_verb(node, token, &instance, &event, ctx).await?;

                // 5b. Handle verb outcome.
                match verb_result.outcome {
                    VerbOutcome::Completed { outputs } => {
                        // Write outputs to token write_log and instance_data.
                        data_writes.extend(outputs.into_writes(token.token_id));
                        // Enqueue token advance.
                        follow_on_events.push(Event::TokenAdvance {
                            instance_id, token_id: token.token_id,
                            from_node: token.current_node.clone(),
                            event_type: EventType::VerbComplete,
                        });
                    }
                    VerbOutcome::WaitInitiated { wait_spec } => {
                        wait_ops.push(WaitOp::Insert(wait_spec, token.token_id));
                        // Token stays at current node; no advance event.
                    }
                    VerbOutcome::ErrorRaised { code, message } => {
                        follow_on_events.push(Event::ErrorRaised {
                            instance_id, token_id: token.token_id,
                            node_id: token.current_node.clone(),
                            error: ErrorRaised { code, message },
                        });
                    }
                }
            }

            JourneyNodeKind::ExclusiveGateway | JourneyNodeKind::InclusiveGateway => {
                // 5c. Emit switch decision request.
                let request = build_switch_request(node, token, &instance);
                wait_ops.push(WaitOp::InsertSwitchRequest(request));
                // Token waits for SwitchDecisionReply; no advance yet.
            }

            JourneyNodeKind::ParallelGateway => {
                // 5d. Fork: emit N tokens.
                let outgoing = journey.edges_from(&token.current_node);
                for edge in &outgoing {
                    let new_token = new_forked_token(token, edge.target.clone());
                    follow_on_events.push(Event::TokenAdvance {
                        instance_id, token_id: new_token.token_id,
                        from_node: token.current_node.clone(),
                        event_type: EventType::ParallelFork,
                    });
                    journal.push(JourneyEntry::TokenForked { ... });
                }
                // Original token is consumed; new tokens carry branch_lineage.
            }

            JourneyNodeKind::ParallelJoin { expected_forks, merge_clauses } => {
                // 5e. Join: check if all expected tokens have arrived.
                let arrived_so_far = count_arrived_at_join(&tx, instance_id,
                                                            &token.current_node).await?;
                let expected_count = compute_expected_count(expected_forks, &instance,
                                                             &tx).await?;
                if arrived_so_far < expected_count {
                    // Not all tokens present; record arrival, wait.
                    journal.push(JourneyEntry::JoinTokenArrived { ... });
                } else {
                    // All tokens present; run merge protocol.
                    let merge_result = apply_merge_protocol(
                        instance_id, &token.current_node, merge_clauses, &tx).await?;
                    match merge_result {
                        MergeResult::Ok { resolved_data } => {
                            data_writes.extend(resolved_data);
                            follow_on_events.push(Event::TokenAdvance { ... });
                            journal.push(JourneyEntry::JoinFired { ... });
                        }
                        MergeResult::Conflict { location, branches, values } => {
                            follow_on_events.push(Event::ErrorRaised {
                                error: ErrorRaised {
                                    code: "merge-conflict",
                                    message: format!(
                                        "Undeclared data conflict at {}: branches {:?} wrote {:?}",
                                        location, branches, values
                                    ),
                                },
                                ..
                            });
                        }
                    }
                }
            }

            JourneyNodeKind::EndEvent(kind) => {
                // 5f. End event.
                match kind {
                    EndEventKind::None | EndEventKind::Message | ... => {
                        if token_is_last_live_token(tokens, token) {
                            // Instance complete.
                            follow_on_events.push(Event::InstanceComplete { instance_id });
                        } else {
                            // Other tokens still live; this branch ended.
                            journal.push(JourneyEntry::TokenEnded { ... });
                        }
                    }
                    EndEventKind::Terminate => {
                        // Cancel all other live tokens.
                        follow_on_events.push(Event::InstanceTerminate { instance_id });
                    }
                    EndEventKind::Error { code } => {
                        follow_on_events.push(Event::ErrorRaised {
                            error: ErrorRaised { code, message: String::new() }, ..
                        });
                    }
                }
            }

            // ... other node kinds handled similarly
        }
    }

    // 6. Persist new state (all within the open transaction).
    append_journey_log(&tx, instance_id, &journal).await?;
    apply_data_writes(&tx, instance_id, &data_writes).await?;
    apply_wait_ops(&tx, &wait_ops).await?;
    update_instance_status_if_needed(&tx, instance_id, &follow_on_events).await?;

    // 7. Commit.
    tx.commit().await?;

    // 8. After commit: enqueue follow-on events (after commit to avoid phantom processing).
    for event in follow_on_events {
        ctx.event_queue.enqueue(event).await?;
    }

    Ok(())
}
```

**Idempotency properties**: the event processor checks `idempotency_ledger` before executing any verb with external side effects. If an entry exists, the cached result is used. The ledger entry is written within the transaction that commits the verb result. If the transaction commits but the ledger insert fails (impossible in practice within the same transaction), the idempotency check on re-delivery returns the cached result.

**Crash-recovery properties**: if the runtime crashes after computing but before committing, the uncommitted transaction rolls back atomically. The event remains in `event_queue` with `status = 'pending'` and is re-delivered. The re-delivery is handled identically to the first delivery; the idempotency ledger prevents duplicate side effects.

**Event ordering guarantee**: within a single instance, events are serialised through the advisory lock. The advisory lock is held for the duration of the database transaction. Two events for the same instance cannot be processed concurrently.

### 6.5 Verb invocation interface

Verbs are synchronous in-process functions. The runtime calls them within the open database transaction.

**Rust function signature pattern:**

```rust
#[async_trait]
pub trait JourneyVerb: Send + Sync {
    fn fqn(&self) -> &str;
    async fn invoke(
        &self,
        ctx: &VerbContext<'_>,
        effects: &mut EffectEmitter,
    ) -> Result<VerbOutcome, VerbError>;
}
```

**`VerbContext`** carries the runtime-injected @-slot bindings and resolved input values:

```rust
pub struct VerbContext<'tx> {
    pub at_process:    ProcessRef,     // instance_id + tenant_id
    pub at_token:      TokenRef,       // token_id + current_node + write_log
    pub at_node:       NodeRef,        // node_id + node_kind (assembly-bound)
    pub at_subprocess: Option<SubprocessRef>,
    pub at_parent:     Option<ParentRef>,
    pub at_decision:   Option<DecisionRef>,
    pub inputs:        HashMap<String, ResolvedValue>,
    pub db:            &'tx mut PgConnection,  // within-transaction connection
    pub services:      &'tx dyn ServiceRegistry,
}
```

The `db` connection is the within-transaction connection passed down from the event processor. Verbs that write data use this connection, ensuring writes are part of the atomic event transaction.

**`EffectEmitter`** provides the API for verbs to declare runtime effects:

```rust
pub struct EffectEmitter {
    effects: Vec<Effect>,
}

impl EffectEmitter {
    pub fn write_data(&mut self, location: &str, value: ResolvedValue, token_id: Uuid) {
        self.effects.push(Effect::DataWrite { location: location.to_owned(), value, token_id });
    }

    pub fn schedule_timer(&mut self, spec: TimerSpec) {
        self.effects.push(Effect::ScheduleTimer(spec));
    }

    pub fn create_human_task(&mut self, spec: HumanTaskSpec) {
        self.effects.push(Effect::CreateHumanTask(spec));
    }

    pub fn send_message(&mut self, spec: MessageSpec) {
        // Message sending is wrapped in idempotency; the effect records the
        // idempotency key; actual send happens post-commit.
        self.effects.push(Effect::SendMessage(spec));
    }

    pub fn request_switch_decision(&mut self, spec: SwitchDecisionSpec) {
        self.effects.push(Effect::RequestSwitchDecision(spec));
    }

    pub fn raise_error(&mut self, code: &str, message: &str) {
        self.effects.push(Effect::RaiseError { code: code.to_owned(), message: message.to_owned() });
    }
}
```

**`VerbOutcome`:**

```rust
pub enum VerbOutcome {
    Completed {
        outputs: HashMap<String, ResolvedValue>,
    },
    WaitInitiated {
        wait_spec: WaitSpec,
    },
    ErrorRaised {
        code: String,
        message: String,
    },
}
```

**Verb error model:**

- **Recoverable errors** (`VerbOutcome::ErrorRaised`): raised as workflow errors. The runtime attempts to route to a matching boundary error event. If no matching boundary exists, the error propagates up subprocess boundaries. If it reaches the top level without a match, the instance fails.
- **Unrecoverable errors** (`Err(VerbError::Unrecoverable { ... })`): the verb returns a Rust `Err`. The instance is immediately marked `failed`; no boundary event processing. Used for programming errors, data corruption, infrastructure failures that indicate the process cannot continue regardless of boundary events.
- **Idempotency**: verbs that interact with external systems declare their idempotency key via `ctx.services.idempotency()`. If the same (instance_id, event_id, verb_fqn) has been seen before, the verb short-circuits and returns the cached outcome.

### 6.6 Switch adaptor protocol

Every gateway (exclusive, inclusive, parallel, event-based) emits a decision request to a pluggable switch adaptor. The runtime itself contains no decision logic.

**Decision request schema:**

```rust
pub struct SwitchDecisionRequest {
    pub request_id:        Uuid,
    pub instance_id:       Uuid,
    pub token_id:          Uuid,
    pub gateway_node:      String,
    pub gateway_kind:      GatewayKind,
    pub data_context:      HashMap<String, ResolvedValue>,  // relevant data for decision
    pub legal_branches:    Vec<BranchSpec>,
    pub adaptor_binding:   AdaptorBinding,
    pub timeout_at:        Option<DateTime<Utc>>,
}

pub struct BranchSpec {
    pub target_node:  String,
    pub condition:    Option<ResolvedExpr>,  // pre-compiled expression, null for default
    pub is_default:   bool,
}

pub enum AdaptorBinding {
    Inline,                          // evaluate conditions in process (expressions only)
    DmnLite { decision_id: String }, // delegate to dmn-lite via switch adaptor protocol
    External { endpoint: String },   // cross-process, async reply
}
```

**Decision reply schema:**

```rust
pub struct SwitchDecisionReply {
    pub request_id:      Uuid,
    pub chosen_branches: Vec<String>,  // target node ids
    // For exclusive: exactly 1.
    // For inclusive: 1..N.
    // For parallel: all legal_branches (adaptor may omit for parallel — runtime takes all).
    pub reasoning:       Option<String>,   // optional audit trail from adaptor
    pub error:           Option<SwitchError>,
}
```

**Adaptor registration — in-process (trait-based):**

```rust
pub trait SwitchAdaptor: Send + Sync {
    fn name(&self) -> &str;
    async fn decide(&self, request: &SwitchDecisionRequest) -> SwitchDecisionReply;
}
```

The runtime maintains a `SwitchAdaptorRegistry: HashMap<String, Arc<dyn SwitchAdaptor>>`. On gateway activation, the runtime dispatches to the adaptor named by `AdaptorBinding`.

**Inline adaptor**: evaluates the `(condition ...)` expression on each outgoing flow against the current `data_context`. Uses the unified boolean composition evaluator (§3.9). For exclusive gateways: returns the first branch whose condition evaluates true; if none, returns the default branch. For inclusive gateways: returns all branches whose condition evaluates true; if none, returns the default branch.

**DmnLite adaptor**: sends the request to the dmn-lite engine via the FFI mechanism (in-process). Encodes the request as a `FfiCall` input payload; receives the output as a `FfiResult`. Maps the output to `chosen_branches`.

**External adaptor (cross-process, asynchronous)**: sends the request over the message bus to a remote decision service. The gateway token enters `waiting` state. When the reply arrives as an external message, it is correlated to the `switch_decision_request` row by `request_id` and the `SwitchDecisionReply` event is enqueued.

**Adaptor timeout**: if the adaptor does not reply within `timeout_at`, a `SwitchDecisionTimeout` event is enqueued. The default behaviour is to fail the instance with a `switch-decision-timeout` error. The `(gateway ...)` atom may declare a `(timeout-handler ...)` slot naming an alternative path; if present, the timeout routes there instead.

**Test harness adaptor**: the test harness registers a `SwitchAdaptor` implementation that returns scripted replies in sequence. This is the mechanism for testing gateway logic without a live dmn-lite instance. See Session 3 §7 for the test harness design.

### 6.7 Multi-token semantics

**Token state:**

Each active token is represented by an `active_token` row (§6.2). Key fields:

- `token_id` — unique per token
- `current_node` — where this token currently is
- `parent_fork` — the gateway node that emitted this token (null for the initial token)
- `branch_lineage` — ordered array of fork gateway ids from process root to this token's current position. Used to match tokens to their expected joins.
- `write_log` — JSONB array of `{location, value, sequence}` entries. Each write made by verbs on this token's branch since the last fork is recorded here. Used by the merge protocol at joins.
- `expected_arrival_set` — for `ParallelJoin` and `InclusiveJoin` tokens: the set of sibling token ids (or branch ids) still expected. When all have arrived, the join fires.

---

**Parallel fork (AND):**

When a token arrives at a `(gateway N :kind parallel)`:

1. Load all outgoing flows from the JourneySpec.
2. Create one new token per outgoing flow. Each new token:
   - Gets a new `token_id`.
   - Sets `parent_fork = N` (the parallel gateway's node id).
   - Sets `branch_lineage = parent_token.branch_lineage ++ [N]`.
   - Sets `write_log = []` (clean log; each branch accumulates its own writes).
3. Delete the original token (consumed by the fork).
4. For each new token, the corresponding `expected_arrival_set` on the matching `(parallel-join ...)` is updated to include this token's id.
5. Enqueue `TokenAdvance` for each new token at its assigned outgoing flow target.

Journey log entries: `TokenForked { gateway: N, child_tokens: [t1, t2, t3] }`.

---

**Parallel join (AND):**

When a token arrives at a `(parallel-join N :expects [fork-gateway-ids])`:

1. Record the arrival: append `JourneyEntry::JoinTokenArrived { token_id, join_node: N }` and remove `token_id` from `expected_arrival_set` for this join.
2. Check: is `expected_arrival_set` now empty?
   - No: token is dehydrated (its `active_token` row is updated to `current_node = N`). Wait for remaining tokens.
   - Yes: all expected tokens have arrived. Run merge protocol (§6.8). If merge succeeds, create a new unified token at the join's outgoing node. If merge fails (conflict), raise error.
3. The unified token's `write_log` is the merged write log produced by the merge protocol.
4. Token `branch_lineage` reverts to the lineage prior to the fork (stripping the fork gateway from the end).

---

**Inclusive fork/join (OR):**

Inclusive gateway fan-out: the switch adaptor returns a subset of branches (1..N). The runtime emits tokens only on the selected branches.

**Critical: expected-set tracking for inclusive join.** At fork time, the runtime records in each newly created token's `expected_arrival_set` only the tokens that were actually emitted. Example: if the inclusive gateway has 3 outgoing flows and the switch adaptor selects branches 1 and 3, only 2 tokens are created. The `(parallel-join ...)` for this gateway expects exactly these 2 tokens — not all 3 possible branches.

Implementation: when the runtime creates N tokens for an inclusive fork, it simultaneously updates the `expected_arrival_set` field on all `active_token` rows for this join node. The set is the list of the N newly created token ids.

**Token death short-circuits the join (Commitment 12)**: if a branch token terminates before reaching the join (e.g., via an error boundary that routes elsewhere), the runtime removes that token id from `expected_arrival_set` of the join. The join then fires when all remaining expected tokens arrive. The journey log records the token death and the short-circuit.

**Token excess**: if more tokens arrive at a join than the current `expected_arrival_set` (which should not happen in a correct process), this is a runtime error: `JoinTokenExcess`. The instance fails with a diagnostic naming the join node and the unexpected token.

---

**Event-based gateway (race):**

When a token arrives at a `(gateway N :kind event-based)`:

1. Insert N `pending_wait` rows — one per outgoing catching event node.
2. The token remains at the gateway node (`active_token.current_node = N`).
3. When the first event fires (message, timer, or signal matching one of the catching event nodes):
   a. Delete all N `pending_wait` rows for this gateway.
   b. Advance the token to the winning catching event node.
   c. Cancel any in-flight waits for the other arms (e.g., if an intermediate-catch-timer had created a `pending_timer`, mark it as cancelled).

**Non-interrupting boundary events**: when a non-interrupting boundary event fires on a host node:
1. The host node's token continues at the host node (unchanged `active_token` row).
2. A *new* token is spawned at the boundary event's outgoing flow.
3. The new token has `parent_fork = <boundary-attachment-id>` and a clean write_log.
4. The spawned token and the host token are independent; they do not share a join. The spawned path typically terminates at a non-interrupting end (e.g., an escalation notification path that ends without affecting the main flow).

### 6.8 Parallel-join data merge protocol

**Write-log model:**

Each `active_token` row carries a `write_log`: a JSON array of `{location, value, sequence}` entries. Every `effects.write_data(location, value, token_id)` call from a verb appends an entry to the calling token's write log. The `sequence` is the journey log sequence number at the time of the write.

At join time, the runtime has the write logs of all converging tokens.

**Merge resolution algorithm:**

```
For each data location L that appears in any converging token's write_log:
    let writers = { token_id: final_value }  -- one entry per token that wrote L
                                               -- "final value" = last write to L per token

    if |writers| == 0:
        skip  // L was not written by any branch; no action needed

    if |writers| == 1:
        apply writers.values()[0] to instance_data[L]
        // Only one branch wrote L; no conflict

    if |writers| > 1:
        let distinct_values = writers.values().collect::<HashSet<_>>()

        if |distinct_values| == 1:
            apply distinct_values.iter().next() to instance_data[L]
            // Multiple branches wrote L but all wrote the same value; idempotent

        if |distinct_values| > 1:
            let merge_clause = join_atom.merge_clauses.find(|c| c.location == L)

            if merge_clause is None:
                // No declared merge; detect-and-fail (Commitment 11)
                raise MergeConflict {
                    location: L,
                    branches: writers.keys(),
                    values: distinct_values,
                }

            match merge_clause.operator:
                Max     => apply max(distinct_values) to instance_data[L]
                Min     => apply min(distinct_values) to instance_data[L]
                Sum     => apply sum(distinct_values) to instance_data[L]
                Union   => apply union(distinct_values) to instance_data[L]  // set union
                Concat  => apply concat(distinct_values, order_by: sequence) to instance_data[L]
                Latest  => apply value_with_max_sequence(writers) to instance_data[L]
                Earliest => apply value_with_min_sequence(writers) to instance_data[L]
                Custom { verb_fqn } =>
                    invoke verb_fqn with inputs = { "values": distinct_values, "branches": writers }
                    apply result["merged_value"] to instance_data[L]
```

**Worked example:**

A parallel-fork → three branches (KYC, deal, IM) → parallel-join with merge clauses:

```lisp
(parallel-join onboarding-join
  :expects [onboarding-fork]
  :merge [
    {:location kyc-outcome    :operator latest}
    {:location deal-id        :operator latest}
    {:location im-active      :operator latest}
  ])
```

KYC branch writes: `{kyc-outcome: "approved", sequence: 14}`, `{kyc-risk-score: 35, sequence: 16}`.
Deal branch writes: `{deal-id: "deal-uuid-xxx", sequence: 18}`.
IM branch writes: `{im-active: true, sequence: 20}`.

All three tokens arrive at `onboarding-join`.

Resolution:
- `kyc-outcome`: written by KYC branch only → apply "approved".
- `kyc-risk-score`: written by KYC branch only → apply 35.
- `deal-id`: written by deal branch only → apply "deal-uuid-xxx".
- `im-active`: written by IM branch only → apply true.
- No multi-branch conflicts → no merge operator invoked.

Journey log entries produced:
```json
{ "entry_kind": "join_fired",    "join": "onboarding-join", "tokens": [t_kyc, t_deal, t_im] }
{ "entry_kind": "data_write",    "location": "kyc-outcome",   "value": "approved",        "from_token": t_kyc  }
{ "entry_kind": "data_write",    "location": "kyc-risk-score","value": 35,                 "from_token": t_kyc  }
{ "entry_kind": "data_write",    "location": "deal-id",        "value": "deal-uuid-xxx",   "from_token": t_deal }
{ "entry_kind": "data_write",    "location": "im-active",      "value": true,               "from_token": t_im   }
{ "entry_kind": "token_created", "token": t_merged, "at_node": "complete-task" }
```

**Conflict example** (demonstrating detect-and-fail):

Suppose both KYC and IM branches write to `review-status`:
- KYC: `{review-status: "approved", sequence: 14}`
- IM: `{review-status: "pending-doc", sequence: 20}`

No merge clause covers `review-status`.

Result: `MergeConflict { location: "review-status", branches: [t_kyc, t_im], values: ["approved", "pending-doc"] }`.

The instance fails. Audit log records: the conflict, the two values, the two branches, the join node. A human reviewer must diagnose whether the process definition is incorrect (should have a merge clause) or whether a data modelling error exists.

### 6.9 Error boundaries and compensation

**Error propagation:**

When a verb raises an error (either `VerbOutcome::ErrorRaised` or by throwing an `end-event-error`):

1. The runtime searches for a matching boundary error event on the current node (`active_token.current_node`). Match = the boundary's `error_definition.code` matches the raised error code (or the boundary has no code filter, matching any error).
2. If found (interrupting): the host node's token is cancelled; the boundary event fires, spawning a token at the boundary's outgoing flow.
3. If found (non-interrupting): the host node continues; the boundary spawns a parallel token.
4. If not found on the current node: the error propagates up to the enclosing subprocess (if any). The subprocess's `active_token` is checked for a matching boundary. Propagation continues up nested subprocess boundaries.
5. If the error reaches the top-level process with no matching boundary: the instance is marked `failed`. The error code and message are recorded in `audit_log` and the instance's final journey log entry.

---

**Compensation (simplified scope for v0.1):**

Compensation in bpmn-lite v0.1 is bounded to the enclosing `transaction-subprocess` scope. Full out-of-scope compensation (BPMN §10.4.6, footnote 6) is deferred to v0.2.

When a `transaction-subprocess` is active:

1. Every task that completes within the scope is recorded in `compensation_log` with `completed_at_sequence` (see schema §6.2).
2. If the transaction-subprocess fails (triggered by `end-event-cancel`, `end-event-error`, or a cancel boundary event on the subprocess):
   a. Load the compensation_log for this transaction scope, ordered by `completed_at_sequence DESC` (reverse order).
   b. For each entry, if the completed node has a `(boundary-attachment :event-kind compensation :compensation-handler H)`, invoke the compensation handler H (synchronous verb invocation, same event processing model).
   c. Continue until all compensation handlers have been invoked.
3. After compensation, the transaction-subprocess token terminates with an error; error propagation continues to the parent scope.

**Compensation ordering** (BPMN 2.0 §10.4.6): handlers are invoked in reverse completion order. This is enforced by the `completed_at_sequence DESC` ordering of the compensation log.

[GAP: Compensation scope across multiple transaction-subprocesses within the same parent; compensation via intermediate-throw-compensation event (rather than transaction failure); deferred to v0.2.]

### 6.10 Long-lived waits

**Timer events:**

*Timer service design*: a dedicated worker process polls the `pending_timer` table for entries where `fire_at <= now() AND NOT fired`. The polling interval is configurable (default 5 seconds). When a timer entry is found, the worker:

1. Marks `pending_timer.fired = true` within a transaction (using `FOR UPDATE SKIP LOCKED` for concurrency safety).
2. Inserts a `TimerFired` event into `event_queue`.
3. Commits the transaction.

Rationale for poll-based timer service over scheduled-execution: Postgres `FOR UPDATE SKIP LOCKED` provides safe concurrent polling without external coordination. The timer accuracy (±polling interval) is acceptable for custody banking workflows — timer precision to the minute is standard (monthly KYC refresh, 5-day notification windows, etc.). Fine-grained timer accuracy (sub-second) is not a requirement for the identified use cases.

**Crash recovery for timers**: if the timer worker crashes between marking `fired = true` and inserting the `TimerFired` event into `event_queue`, the timer is marked fired but no event was enqueued. A recovery scan (run at timer worker startup) checks for `pending_timer.fired = true` entries that have no corresponding `event_queue` entry with `event_kind = 'timer_fired'` and the matching `wait_id`. Any such orphans are re-enqueued.

[GAP: timer cycle support (recurring timers); deferred to v0.2. Cycle expression parsing is defined in the JourneySpec but cycle execution requires the timer worker to re-insert `pending_timer` entries after each firing.]

---

**Message correlation:**

Messages arrive at the runtime via the `deliver_message(message_name, payload, headers)` API. The correlation key is extracted from the payload (using the expression declared in the message-definition's `:correlation-key` slot) and used to look up matching `pending_wait` rows.

The lookup query:
```sql
SELECT pw.*, at.instance_id
FROM pending_wait pw
JOIN active_token at ON at.token_id = pw.token_id
WHERE pw.wait_kind = 'message'
  AND pw.correlation_key = $1
  AND pw.current_node IN (
    SELECT id FROM journey_nodes WHERE message_def = $2  -- message name filter
  )
LIMIT 1;
```

If no match: the message is stored in `event_queue` with `status = 'pending'` for up to 7 days (configurable). If a matching `pending_wait` is inserted within that window, the pending message is consumed. This handles the common race condition where a message arrives slightly before the instance reaches its wait state.

If multiple instances match (fan-out correlation): all matching instances receive the message. This is the expected behaviour for signals and broadcasts; for point-to-point messages, the process design must ensure correlation keys are unique per target instance.

**Idempotency**: message delivery uses the message's `message_id` (from headers, if present) as an idempotency key. Re-delivering the same message_id is a no-op if the prior delivery committed successfully.

---

**Human task completion:**

A human task is created when a `user-task` node's verb (invoking `VerbOutcome::WaitInitiated { wait_spec: WaitSpec::HumanTask { ... } }`) fires. The runtime:

1. Inserts a `human_task` row.
2. Inserts a `pending_wait` row with `wait_kind = 'human-task'` and `correlation_key = task_id.to_string()`.
3. The token remains at the user-task node.

When a user completes the task via the task API:

1. The API calls `complete_human_task(task_id, completed_by, completion_data)`.
2. The runtime updates `human_task.status = 'completed'`.
3. Inserts a `HumanTaskCompletion` event into `event_queue`.
4. The event loop picks up the event, resolves the correlation to the waiting token, and advances it.

The task API is deliberately outside the core runtime specification — it's a product-level API surface that varies by deployment. The core runtime's contract with human tasks is the `HumanTaskCompletion` event schema.

### 6.11 Concurrency and scaling

**Per-instance serialisation:**

Events for the same instance are serialised using a PostgreSQL advisory lock keyed by instance_id. The lock is acquired at the start of `process_event` and released on transaction commit. This ensures:
- No two events for the same instance are processed concurrently.
- Events for different instances are fully independent.

The advisory lock key is derived as: `hashtext(instance_id::text) % 2^31` (fitting into PostgreSQL's `bigint` advisory lock key space while maintaining per-instance identity).

**Worker pool:**

The event loop runs as a pool of N workers (N = CPU count × configurable multiplier, default 4). Each worker:

1. `SELECT event_id, instance_id, event_kind, payload FROM event_queue WHERE status = 'pending' ORDER BY enqueued_at LIMIT 1 FOR UPDATE SKIP LOCKED`
2. Mark `status = 'processing'`.
3. Call `process_event(event)`.
4. Mark `status = 'processed'`.

`FOR UPDATE SKIP LOCKED` ensures N workers never attempt the same event concurrently. Events are consumed in FIFO order per instance (due to the instance-level advisory lock — if two events for instance A are dequeued concurrently by two workers, the second worker will block on the advisory lock until the first completes; effectively processing them in order).

**Throughput estimate**: a single Postgres server can support ~500–1,000 event commits per second at low latency (< 50ms per event including advisory lock acquisition, journey log append, and data write). For the custody banking onboarding use case (hundreds to low thousands of concurrent instances, not millions), this is more than sufficient.

**Horizontal scaling**: additional worker pool instances can be deployed. `FOR UPDATE SKIP LOCKED` in the event queue ensures no duplicate processing without a separate coordination service. The per-instance advisory lock ensures correctness regardless of how many worker processes exist.

[GAP: backpressure and per-instance queue ordering under high-concurrency conditions; deferred to v0.2 operational guide.]

### 6.12 Observability

**Journey log as primary audit surface:**

The `journey_log` table is the regulatory audit surface. Every state transition is append-only. Regulators and compliance teams can reconstruct the full history of any instance by replaying the log. The log entries are human-readable (node ids are stable string identifiers from the JourneySpec, not opaque integers).

For regulatory contexts (MiFID II, COBS, MAR), the audit_log table provides the additional detail required: which actor completed a human task, which adaptor replied to a gateway, what the adaptor's reasoning string was (if provided), what error messages were raised.

**Structured operational events:**

For monitoring tooling (Prometheus, Grafana, PagerDuty), the runtime emits structured events at the following points:

| Event | Metric | Labels |
|---|---|---|
| Instance started | `workflow_instance_started_total` | `workflow_id`, `tenant_id` |
| Instance completed | `workflow_instance_completed_total` | `workflow_id`, `tenant_id`, `duration_ms_bucket` |
| Instance failed | `workflow_instance_failed_total` | `workflow_id`, `tenant_id`, `error_code` |
| Verb invoked | `workflow_verb_invoked_total` | `verb_fqn`, `workflow_id` |
| Verb duration | `workflow_verb_duration_ms` | `verb_fqn` — histogram |
| Switch decision requested | `workflow_gateway_decision_requested_total` | `gateway_kind`, `adaptor` |
| Timer fired | `workflow_timer_fired_total` | — |
| Message delivered | `workflow_message_delivered_total` | `message_name` |
| Merge conflict | `workflow_merge_conflict_total` | `workflow_id`, `join_node` — alert on this |
| Event queue depth | `workflow_event_queue_depth` | `status` — gauge |
| Human task age | `workflow_human_task_age_hours` | `task_type` — histogram |

Operational alerts:
- `workflow_instance_failed_total` rate > threshold → SLA alert.
- `workflow_merge_conflict_total` any nonzero → process definition defect alert.
- `workflow_event_queue_depth{status="pending"}` > 1,000 → worker backlog alert.
- `workflow_human_task_age_hours` > SLA threshold → task overdue alert.

**Distributed tracing**: each event processing transaction emits an OpenTelemetry span with `trace_id = instance_id`, `span_name = node_id`, `attributes = { verb_fqn, token_id, event_kind }`. Span duration covers verb invocation time. This enables tracing the critical path through long-running processes.

---

*Session 2 ends here. Session 3 covers:*
- *§7 Regression and validation strategy (SemOS regression, bpmn-lite test corpus, multi-token validation, pack-authored validation)*
- *§8 Decision pack catalogue (pack atom model, expansion semantics, 12 seed packs, Sage interaction model, governance lifecycle)*
- *§9 Worked examples (12 Camunda 8 → bpmn-lite translations + pack-authored example)*
- *§10 Risk register*
- *§11 Phase execution plan*
- *§12 Out-of-scope statement*
- *Appendices A–E*
