# OB-POC Solution Architecture Brief

## The Problem Space

### Why Onboarding needs a new approach

Financial services client onboarding—particularly for institutional clients requiring custody, prime brokerage, and alternatives services—suffers from systemic architectural failures:

**1. Complexity Scattered Across Silos**

Business rules live in screens, workflows, stored procedures, and tribal knowledge. A single KYC policy change requires coordinated updates across 15 systems. Nobody can answer "what are the rules?" without archaeology.

**2. Code-Heavy Change Management**

Every new document type, jurisdiction, or product requires developer sprints. Configuration is an afterthought. The backlog of "simple" regulatory changes stretches to years.

**3. AI as a Toy, Not a Tool**

AI demos extract data from documents. Then what? The extracted data enters the same workflow, requiring the same manual review. AI creates more work, not less.

**4. Fragmented Data Models**

Each domain (KYC, custody, investor services) has its own schema, its own entity model, its own "customer." Reconciliation is a full-time job. Golden source is a myth.

**5. Human-in-the-Loop by Default**

Every process assumes manual review. STP is the exception, not the rule. Operations scales linearly with volume.

**6. Audit as Archaeology**

"Why was this client approved?" requires forensic reconstruction across email threads, workflow comments, and analyst memories. Re-running a process means starting from scratch.

**7. Static Runbooks, Dynamic Reality**

Procedures live in Word documents. Execution lives in human heads. The gap between "what we should do" and "what we did" is unknowable.

**8. The Product-Engineering Translation Gap**

Product defines requirements in documents. Engineering translates to code. Translation is lossy, slow, and expensive. By the time it's built, the requirement has changed. Product can't read the code; engineering can't validate against intent.

---

## Design Principles

The following principles guided the OB-POC architecture:

### 1. Configuration, Not Code

> *Functional and data additions should require configuration changes, not code deployments.*

Adding a new verb, document type, entity type, or validation rule should be a YAML edit—not a pull request requiring code review, testing, and release management.

### 2. Centralised Complexity

> *Handle complexity in a concentrated, centralised DSL—not scattered across screens and workflows.*

The DSL is the single source of truth for what the system can do and what it has done. UI is a projection. Workflow is an execution trace. The DSL program is the canonical representation of client state.

### 3. Deterministic AI Integration

> *Integrate AI everywhere, but ensure deterministic, auditable outcomes.*

AI extracts, suggests, and generates. But execution is deterministic: the same DSL program produces the same state transitions. AI proposes; the DSL disposes.

### 4. Platform-Level Data Model

> *A shared data model and entity strategy across all sub-domains.*

One entity model. One CBU model. One document catalog. KYC, custody, investor services, and compliance all operate on the same graph. No reconciliation because there's nothing to reconcile.

### 5. STP by Default

> *Straight-through processing as the norm; human-in-the-loop as the exception.*

Design for automation first. Humans intervene on exceptions (red flags, hits, discrepancies)—not on happy paths. Scale operations sub-linearly with volume.

### 6. Full Auditability and Re-Runnability

> *Every process must be fully auditable and re-executable from its declarative definition.*

The DSL program that onboarded a client can be inspected, diffed, and re-executed. "Why was this approved?" has a single, complete answer: the DSL execution trace.

### 7. Adaptive Runbooks

> *Define executable processes declaratively, then run them adaptively.*

Runbooks are not documents—they are DSL programs. Process definitions are version-controlled, validated, and executed. The gap between policy and practice closes to zero.

### 8. Shared Language Between Product and Engineering

> *The DSL should be readable by product and executable by engineering—the same artifact serves both.*

Product owners should be able to read a DSL program and understand what it does. Engineers should be able to execute it and know it matches intent. The DSL is the contract, not a translation of a contract.

---

## Solution Architecture

### Because... Therefore...

| Because... | Therefore... |
|------------|--------------|
| Business rules scattered across systems cause change friction and inconsistency | **All verbs, validations, and domain rules are defined in YAML configuration** (`config/verbs.yaml`, `config/csg_rules.yaml`). Adding a new document type or entity attribute requires no code. |
| UI-embedded logic creates maintenance nightmares and testing gaps | **The DSL is the single source of truth**. UI renders state; it doesn't compute it. Every screen is a projection of DSL execution results. |
| AI extraction without deterministic integration creates more manual work | **AI generates DSL programs, not data**. Claude extracts intent → deterministic Rust planner → DSL generation → validated execution. The AI proposes; the parser/linter/executor ensure correctness. |
| Fragmented entity models require constant reconciliation | **One entity graph serves all domains**. `entities` + `entity_types` + Class Table Inheritance gives type-specific attributes without model fragmentation. KYC, custody, and investor services share the same `@entity-id`. |
| Human review on every transaction doesn't scale | **Threshold-driven automation**: risk bands determine document requirements. Clean screenings auto-clear. Red flags route to humans. STP is the default path. |
| Audit reconstruction is forensic guesswork | **DSL programs are the audit trail**. Every client has a complete, executable history. `case-event.log` captures every state transition. Re-run the DSL to reproduce the outcome. |
| Paper runbooks diverge from actual practice | **Runbooks are DSL templates**. Onboarding a hedge fund executes `hedge_fund_onboarding.dsl`. Policy changes update the template. Execution is the documentation. |
| Product requirements lose fidelity in translation to code | **The DSL is the requirement and the implementation**. Product reads the same artifact engineering executes. Validation happens at the language level, not in code review. |

---

## The DSL as Shared Language

### Closing the Product-Engineering Gap

In conventional development, the path from business requirement to running code is:

```
Product Requirement → User Stories → Technical Design → Code → Tests → Deployment
        ↓                   ↓               ↓            ↓
    (ambiguity)        (interpretation)  (translation) (divergence)
```

At each stage, fidelity is lost. Product can't validate the code matches intent. Engineering can't be certain the requirement was understood. The feedback loop is weeks or months.

The DSL collapses this:

```
Business Intent → DSL Program → Execution
        ↓               ↓
    (same artifact)  (validated, deterministic)
```

**The DSL is simultaneously**:
- A specification that product can read and validate
- An executable program that engineering can run
- An audit trail that compliance can inspect
- A test case that QA can verify

### Why This Matters

Consider expressing "onboard a Cayman hedge fund with two UBOs":

**Traditional approach** (user story → code):
```
Story: As an onboarding analyst, I want to create a fund client...
→ 2 weeks later: 500 lines of Java service code
→ Product review: "That's not quite what I meant..."
→ Rework cycle begins
```

**DSL approach** (intent → validated program):
```clojure
(cbu.ensure :name "Atlas Fund" :jurisdiction "KY" :client-type "FUND" :as @fund)
(entity.create-proper-person :first-name "John" :last-name "Chen" :as @ubo1)
(cbu.assign-role :cbu-id @fund :entity-id @ubo1 :role "BENEFICIAL_OWNER" :ownership-percentage 60)
```

Product can read this. It's obvious what it does. If it's wrong, the correction is immediate—edit the DSL, re-run. No translation layer. No interpretation gap.

### Readable Verbs as Business Operations

The DSL verb vocabulary is designed to mirror business language:

| Business Operation | DSL Verb | What It Does |
|-------------------|----------|--------------|
| "Create the client" | `cbu.ensure` | Insert/update CBU record |
| "Add a beneficial owner" | `cbu.assign-role` | Link entity to CBU with role |
| "Request their passport" | `doc-request.create` | Create document request |
| "Run sanctions screening" | `case-screening.run` | Initiate screening |
| "Approve the case" | `kyc-case.close :status "APPROVED"` | Close case with approval |

Product owners don't need to understand Rust, PostgreSQL, or microservices. They need to understand the vocabulary of their domain—and the DSL speaks that vocabulary.

---

## The DSL Layer: Beyond ORM

### The Database-Business Logic Gap

Every application faces the same architectural question: how do you bridge the gap between your relational database (tables, rows, SQL) and your business operations (create client, approve case, provision account)?

The Java/ORM approach and the DSL approach represent fundamentally different answers.

### The ORM Approach (Hibernate, JPA, etc.)

```
┌─────────────────────────────────────────────────────────────────┐
│                    Business Logic (Java)                         │
│   ClientService.createClient(dto)                               │
│   - Validation logic in Java code                               │
│   - Business rules in Java code                                 │
│   - Transaction boundaries in Java code                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    ORM Layer (Hibernate)                         │
│   @Entity Client { ... }                                        │
│   - Object-relational mapping                                   │
│   - Lazy loading, caching, session management                   │
│   - Query abstraction (HQL, Criteria API)                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Database (PostgreSQL)                         │
│   Tables, indexes, constraints                                  │
└─────────────────────────────────────────────────────────────────┘
```

**What ORM gives you**: Object-oriented access to relational data. You work with `Client` objects, not `INSERT` statements. The mapping is handled.

**What ORM doesn't give you**: Business operations are still code. `createClient()` is a Java method. Adding a new operation means writing a new service class, with validation, transaction handling, and error management—all in code.

**The problems**:
- Business rules are scattered across service classes
- Adding a new operation requires Java development
- The "what" (create a client) is buried in the "how" (50 lines of service code)
- Non-developers can't read or validate the logic

### The DSL Approach

```
┌─────────────────────────────────────────────────────────────────┐
│                    DSL Program                                   │
│   (cbu.ensure :name "Fund" :jurisdiction "KY" :as @fund)        │
│   - Business operation expressed declaratively                  │
│   - Validated at parse time                                     │
│   - Human-readable, diffable, auditable                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              YAML-Driven Verb Registry                           │
│   verbs.yaml defines:                                           │
│   - Verb → table mapping                                        │
│   - Argument → column mapping                                   │
│   - Validation rules                                            │
│   - CRUD operation type                                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              GenericCrudExecutor (Rust)                          │
│   - Reads verb config from YAML                                 │
│   - Generates SQL dynamically                                   │
│   - Handles all 13 CRUD operation types                         │
│   - NO business-specific code                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Database (PostgreSQL)                         │
│   Tables, indexes, constraints                                  │
└─────────────────────────────────────────────────────────────────┘
```

**What the DSL gives you**: Business operations as first-class, declarative constructs. The verb IS the operation. The configuration IS the mapping. The program IS the audit trail.

**The key difference**: In the ORM world, you write code to implement operations. In the DSL world, you configure operations—the executor is generic.

### Side-by-Side Comparison

| Aspect | ORM (Java/Hibernate) | DSL (YAML-Driven) |
|--------|---------------------|-------------------|
| Adding a new operation | Write Java service class | Add YAML verb definition |
| Validation | Code in service layer | CSG rules in YAML |
| Business rules | Scattered in services | Concentrated in config |
| Readability by product | None (it's Java code) | High (it's domain vocabulary) |
| Audit trail | Log statements, if any | The DSL program itself |
| Testing | Unit tests for each service | Validate DSL programs |
| Change velocity | Sprint-based development | Configuration deployment |

### Complex CRUD: Beyond Simple Insert/Update

Real business operations aren't simple CRUD. "Assign a role to an entity" involves:

1. Validate the role exists
2. Validate the entity exists
3. Check for duplicate assignments
4. Insert the junction record
5. Optionally set ownership percentage
6. Return the created ID for chaining

In Java/ORM, this is a service method with 30+ lines of code.

In the DSL, this is a verb definition in YAML:

```yaml
cbu:
  verbs:
    assign-role:
      description: "Assign a role to an entity within a CBU"
      behavior: crud
      crud:
        operation: insert
        table: cbu_entity_roles
        schema: ob-poc
        returning: cbu_entity_role_id
      args:
        - name: cbu-id
          type: uuid
          required: true
          maps_to: cbu_id
        - name: entity-id
          type: uuid
          required: true
          maps_to: entity_id
        - name: role
          type: string
          required: true
          maps_to: role_name
          validation:
            lookup_table: roles
            lookup_column: name
        - name: ownership-percentage
          type: number
          required: false
          maps_to: ownership_percentage
```

The executor handles the mechanics. The configuration expresses the intent.

### Why This Complexity Simplifies

Building a YAML-driven DSL executor is more complex than writing a single service class. But:

| Upfront Complexity | Downstream Simplification |
|--------------------|--------------------------|
| Parser implementation | Every new verb is configuration |
| Generic executor | No service class proliferation |
| YAML schema design | Product can read operations |
| CSG linter | Validation is declarative |
| Execution plan compiler | Audit trail is automatic |

**The investment is O(1); the payoff is O(n)** where n is the number of operations. The 50th verb costs the same as the 5th: a YAML definition.

---

## Agentic AI Integration: Constrained Creativity

### The Problem with Unconstrained AI

AI language models are powerful but probabilistic. Ask Claude to "help onboard a hedge fund" and you'll get:
- Natural language explanation
- Suggestions and recommendations
- Perhaps some JSON or pseudo-code
- No guarantee of correctness or executability

This is useful for conversation but useless for automation. The output isn't actionable without human interpretation and translation.

### RAG Is Not Enough

Retrieval-Augmented Generation (RAG) improves relevance by providing context. Give Claude examples of DSL programs, and it will generate something that looks like DSL. But:

- RAG provides context, not constraints
- The model can still hallucinate verbs that don't exist
- Argument names and types can be wrong
- The output might parse but fail validation

RAG makes AI outputs more likely to be correct. It doesn't guarantee correctness.

### The DSL as Semantic Constraint

The OB-POC architecture uses the DSL validation pipeline as an active constraint on AI output:

```
┌─────────────────────────────────────────────────────────────────┐
│                 AI Generation (Unconstrained)                    │
│   Claude generates DSL based on intent + examples               │
│   Output: candidate DSL program                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Parser (Syntactic Gate)                       │
│   ✗ Malformed S-expressions rejected                            │
│   ✗ Invalid tokens rejected                                     │
│   ✓ Syntactically valid AST produced                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 CSG Linter (Semantic Gate)                       │
│   ✗ Unknown verbs rejected                                      │
│   ✗ Missing required arguments rejected                         │
│   ✗ Type mismatches rejected                                    │
│   ✗ Invalid entity-document combinations rejected               │
│   ✗ Undefined symbol references rejected                        │
│   ✓ Semantically valid program produced                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Execution Plan Compiler (Logical Gate)              │
│   ✗ Circular dependencies rejected                              │
│   ✗ Unreachable statements rejected                             │
│   ✓ Executable plan produced                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 Validated, Executable DSL                        │
│   Guaranteed to: parse, validate, compile, execute              │
│   AI creativity constrained to valid action space               │
└─────────────────────────────────────────────────────────────────┘
```

### The Feedback Loop

When validation fails, structured errors feed back to the AI:

```
Error: Unknown verb 'client.create'. Did you mean 'cbu.create' or 'cbu.ensure'?
Error: Missing required argument 'jurisdiction' for verb 'cbu.ensure'
Error: Entity type 'passport' cannot have document type 'CERTIFICATE_OF_INCORPORATION'
```

The AI retries with corrections. This loop (up to 3 attempts) means:

- AI proposes freely (creativity preserved)
- Invalid proposals are rejected with specific feedback
- The AI learns from errors within the conversation
- Only valid programs reach execution

### Determinism Through Validation

This architecture achieves something subtle but powerful: **non-deterministic generation with deterministic outcomes**.

| Stage | Deterministic? | Guarantee |
|-------|---------------|-----------|
| Intent extraction | No | Best-effort understanding |
| DSL generation | No | Probabilistic, may vary |
| Parser | Yes | Same input → same AST |
| CSG Linter | Yes | Same AST → same validation |
| Executor | Yes | Same program → same state |

The AI boundary is explicitly non-deterministic. But because the validation pipeline is deterministic and total (every invalid program is rejected), the output that reaches execution is guaranteed correct.

**This is the key insight**: You don't need deterministic AI. You need deterministic validation of AI output.

### Contrast with Traditional AI Integration

| Approach | Mechanism | Guarantee |
|----------|-----------|-----------|
| Chat-based AI | Natural language response | None—human must interpret |
| RAG-enhanced AI | Context-improved response | Probabilistically better |
| Tool-use AI | Structured function calls | Schema-validated, but limited |
| **DSL-constrained AI** | Validated executable program | Syntactically, semantically, logically correct |

The DSL approach is more complex to build but provides the strongest guarantee: if the AI output passes validation, it will execute correctly. This is what enables genuine automation rather than AI-assisted manual work.

### Focusing the Action Space

The DSL vocabulary defines a bounded action space. Claude can only generate programs using verbs that exist in `verbs.yaml`. This isn't a limitation—it's a feature:

- **Finite vocabulary**: ~80 verbs across 20 domains
- **Typed arguments**: Each verb has a defined schema
- **Constrained composition**: CSG rules limit how verbs combine
- **Executable semantics**: Every valid program has defined behavior

This bounded space means:
- AI can't hallucinate operations that don't exist
- AI can't produce programs that "look right but fail"
- Every generated program maps to concrete database operations
- The gap between intent and execution is closed

---

## Architecture Components

### The DSL Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│                    Natural Language / UI                         │
│            "Onboard Meridian Fund for custody"                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 AI Layer (Non-Deterministic)                     │
│   Intent Extraction → Pattern Classification → DSL Generation   │
│   Claude API with retry loop on validation failure              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                DSL Source (Deterministic Boundary)               │
│   S-expression syntax, symbol bindings, typed arguments         │
│   Human-readable, version-controllable, diffable                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Validation Pipeline                           │
│   Parser (Nom) → CSG Linter → Execution Plan Compiler           │
│   Errors are structured, actionable, fed back to AI             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Executor (YAML-Driven, Deterministic)               │
│   GenericCrudExecutor reads verb config from YAML               │
│   Same program → Same state transitions → Same outcome          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 Unified Data Platform                            │
│   PostgreSQL: ob-poc (55 tables) + custody (17) + kyc (11)      │
│   Single entity graph, shared across all domains                │
└─────────────────────────────────────────────────────────────────┘
```

### Configuration-Driven Extensibility

```yaml
# config/verbs.yaml - Adding a new verb requires NO code changes
domains:
  new-domain:
    verbs:
      new-verb:
        description: "What this verb does"
        behavior: crud
        crud:
          operation: insert
          table: new_table
          schema: ob-poc
          returning: new_id
        args:
          - name: my-arg
            type: string
            required: true
            maps_to: db_column
```

### The Observation Model (Evidence-Based KYC)

```
┌─────────────────────────────────────────────────────────────────┐
│                    CLIENT ALLEGATIONS                            │
│   "The client claims..."                                         │
│   Unverified starting point from onboarding forms               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ verification
┌─────────────────────────────────────────────────────────────────┐
│                 ATTRIBUTE OBSERVATIONS                           │
│   Multiple observations per attribute from different sources    │
│   Each with: source_type, document_id, confidence, authority    │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│    EXACT_MATCH          │     │  DISCREPANCY            │
│    ACCEPTABLE_VARIATION │     │  → Escalate / Resolve   │
│    → Auto-verify        │     │  → Human review         │
└─────────────────────────┘     └─────────────────────────┘
```

This model enables:
- **STP for clean cases**: Allegation matches observation → auto-verified
- **Human escalation for exceptions**: Discrepancies route to analysts
- **Full evidence chain**: Every verified fact traces to a document

### Threshold-Driven Automation

```
┌─────────────────────────────────────────────────────────────────┐
│                      Risk Factors                                │
│   Jurisdiction + Entity Type + Role + AUM + PEP Status          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Risk Band Derivation                          │
│   LOW → MEDIUM → HIGH → VERY_HIGH                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│               Document Requirements Matrix                       │
│   Risk Band × Entity Role → Required Documents + Screenings     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Automated Evaluation                            │
│   All requirements met? → STP approval                          │
│   Missing documents? → Generate RFI                             │
│   Screening hits? → Route to human                              │
└─────────────────────────────────────────────────────────────────┘
```

---

## Key Differentiators

### 1. DSL-as-State

Traditional systems store state in database rows and reconstruct meaning through queries. OB-POC stores meaning in DSL programs and projects state to the database.

**Implication**: The DSL program that onboarded a client is a complete, executable specification. Re-run it to reproduce the state. Diff two programs to see what changed.

### 2. AI at the Boundary, Determinism at the Core

AI operates at the boundary (intent extraction, DSL generation) but never in the execution path. The boundary is non-deterministic; the core is fully deterministic.

**Implication**: AI can be improved, swapped, or removed without affecting execution semantics. Audit trails remain valid regardless of AI evolution.

### 3. Evidence-Based Verification

The observation model separates claims from evidence. Clients allege; documents prove. Verification is the explicit act of matching allegations to observations.

**Implication**: "Why do we believe X about this client?" has a precise answer: allegation A was verified by observation O extracted from document D with confidence C.

### 4. Configuration as the API

Verbs, validations, entity types, document types, and risk rules are all configuration. The Rust codebase provides execution semantics; configuration provides domain semantics.

**Implication**: Domain experts can extend the system without developers. Regulatory changes deploy as configuration updates.

### 5. Shared Language Across Roles

The DSL is readable by product, executable by engineering, auditable by compliance, and testable by QA. The same artifact serves all stakeholders.

**Implication**: No translation gaps. No "that's not what I meant." The specification is the implementation.

---

## Why This Complexity Simplifies Everything

Building a DSL-first architecture requires significant upfront investment:

- Custom parser (Nom-based, ~2,000 lines of Rust)
- CSG linter with YAML-driven rules (~1,500 lines)
- Generic executor with 13 CRUD operation types (~3,000 lines)
- YAML schema for verb definitions (~1,500 lines of config)
- AI integration with validation feedback loop (~2,000 lines)

This is genuinely complex. But the complexity is **concentrated and paid once**. In return:

| Complexity Invested | Simplification Gained |
|--------------------|----------------------|
| Parser implementation | Every DSL program parses identically forever |
| YAML-driven verbs | New operations require no code |
| Generic executor | No service class proliferation |
| Validation pipeline | AI outputs are guaranteed correct |
| Single entity model | No cross-domain reconciliation |
| Evidence-based KYC | Audit is inspection, not reconstruction |

**The architecture front-loads complexity to eliminate it from the ongoing operation.**

Traditional systems spread complexity everywhere:
- Every new screen: validation logic
- Every new service: transaction handling
- Every new integration: data mapping
- Every new report: query archaeology
- Every new AI feature: manual review workflow

The DSL architecture says: invest once in the execution engine, and every extension is configuration.

---

## Operational Model

### STP Metrics

| Metric | Target | Mechanism |
|--------|--------|-----------|
| Clean case STP rate | >80% | Threshold automation + observation matching |
| Time to first human touch | 0 for clean cases | Red flag routing only |
| Re-work rate | <5% | Validation before execution |
| Audit preparation time | Minutes | DSL execution trace |

### Human Intervention Points

Humans engage only at defined exception points:

1. **Screening hits** requiring disposition (true match vs. false positive)
2. **Discrepancies** between allegations and observations
3. **Red flags** raised by rules engine (severity: ESCALATE or HARD_STOP)
4. **Threshold waivers** requiring senior approval
5. **Final sign-off** for high-risk cases (VERY_HIGH risk band)

### Continuous Compliance

The DSL-as-state model enables:

- **Policy replay**: Apply new rules to historical cases to identify gaps
- **Periodic review automation**: Re-run threshold evaluation on existing clients
- **Regulatory reporting**: Query DSL execution history for compliance evidence
- **What-if analysis**: Test policy changes against production data

---

## Technology Choices

| Component | Choice | Rationale |
|-----------|--------|-----------|
| DSL Parser | Nom (Rust) | Zero-copy parsing, excellent error messages, composable |
| Execution Engine | Rust | Type safety, performance, WASM compilation for UI |
| Configuration | YAML | Human-readable, diffable, widely tooled |
| Database | PostgreSQL 17 | pgvector for embeddings, JSONB for flexibility |
| AI Integration | Claude API | Structured output, tool use, large context |
| UI | egui (WASM) | Single codebase for web + native, Rust integration |

---

## Summary

OB-POC exists because the current state of financial services onboarding is characterised by:

- **Scattered complexity** that resists change
- **Code-heavy extension** that bottlenecks delivery
- **AI as demo-ware** that doesn't integrate
- **Fragmented data** that requires reconciliation
- **Human-default processing** that doesn't scale
- **Forensic audit** that can't answer simple questions
- **Paper runbooks** that diverge from practice
- **Product-engineering translation loss** that delays everything

The solution is an architecture where:

- **Configuration defines capability** (YAML-driven verbs)
- **DSL centralises complexity** (single source of truth)
- **AI proposes, DSL validates** (deterministic core)
- **One entity graph** serves all domains (no reconciliation)
- **STP is default** (humans handle exceptions)
- **DSL is the audit trail** (re-runnable, diffable)
- **Runbooks are executable** (policy = practice)
- **Product reads what engineering runs** (shared language)

This is not a better workflow engine. This is a different paradigm: **declarative, evidence-based, AI-assisted but deterministic client lifecycle management**.

The approach is complex to build. It is simple to operate, extend, audit, and evolve.

---

## Appendix: Design Principles as Test Cases

Each principle implies testable properties:

| Principle | Test |
|-----------|------|
| Configuration not code | Add a new verb type via YAML only; execute successfully |
| Centralised DSL | All UI state derivable from DSL execution; no hidden state |
| Deterministic AI | Same valid DSL program → identical execution outcome |
| Platform data model | Query any entity from any domain context without joins to external systems |
| STP by default | Clean synthetic case completes with zero human interaction |
| Full auditability | Re-execute historical DSL; produce identical state |
| Adaptive runbooks | Template change reflects in next execution; no manual sync |
| Shared language | Product owner can read DSL program and identify errors |
| AI constraint | Invalid DSL from AI rejected; retry produces valid alternative |
