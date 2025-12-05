# OB-POC: A DSL-Driven Onboarding & KYC Platform  
**Technical Sell Sheet for Institutional & Custody Businesses**

---

## 1. One-Minute Overview

OB-POC is a **developer-grade onboarding and KYC platform** built around three simple but radical ideas:

1. **Onboarding as Code, Not Workflows**  
   Every onboarding / KYC case is an **executable DSL runbook** – not a tangle of screens, rules, and hand-offs.

2. **AI at the Edge, Deterministic Core**  
   AI helps draft DSL programs, read documents and suggest next steps – but **only validated, typed programs** can change state.

3. **A Unified Entity Graph, Not 5 “Client Models”**  
   KYC, custody, investor services, tax, screening: all share a **single entity & CBU graph**, so onboarding logic lives in one place.

The result: **faster onboarding, lower operational drag, and clean auditability**, without forcing a risky “rip and replace” of your existing stack.

---

## 2. The Problem We’re Actually Solving

In most large institutions, onboarding and KYC suffer from the same structural issues:

- **Rules live everywhere**:  
  - UI workflows  
  - Stored procedures and microservices  
  - Word/PDF runbooks & tribal knowledge  
- **Every policy tweak is a dev project**  
  New product? New country? New UBO rule? → Ticket, backlog, release train.
- **AI that generates *more* work**  
  Document extraction that still needs manual triage and keying into legacy flows.
- **Multiple overlapping “client models”**  
  KYC ≠ Custody ≠ Tax ≠ Screening – each has its own data model and rule set.
- **Audit = forensic reconstruction**  
  “What did we do, why, and based on which evidence?” takes days, not minutes.
- **Enterprise inertia**  
  What you have is “good enough” and deeply embedded – so anything new must **coexist** rather than demand a full rewrite.

OB-POC is designed specifically **for this reality**: it adds a **precise, executable layer of intent and evidence** *on top of* what you already run.

---

## 3. Core Concept: Onboarding as Executable DSL

Instead of hard-coding onboarding behaviour into workflows and services, OB-POC represents each case as a **small program** written in a constrained S-expression DSL.

Example (simplified):

```clojure
(cbu.ensure :name "Atlas Fund" :jurisdiction "KY" :client-type "FUND" :as @fund)
(entity.create-person :first-name "John" :last-name "Chen" :as @ubo1)
(cbu.assign-role :cbu-id @fund :entity-id @ubo1 :role "BENEFICIAL_OWNER" :ownership 60)
(doc.request :for @ubo1 :type "PASSPORT")
(screening.run :cbu-id @fund)
(risk.evaluate :cbu-id @fund)
```

This one artefact is:

- **Readable** by product / KYC SMEs.
- **Executable** by the engine.
- **Testable** as a unit.
- **Auditable** as the exact trace of what happened.

> **The DSL is the state** – the database is a projection of DSL execution, not an independent, divergent source of truth.

---

## 4. Architecture at a Glance

### 4.1 High-Level Structure

- **DSL Core (Rust)**  
  - Parser & AST  
  - Semantic validation (CSG rules)  
  - Execution planner  
  - Deterministic executor (SQL / side-effects)  
  - Exposed as a service and compiled to WASM for UI reuse

- **Platform & Services (Go)**  
  - API gateways (REST/gRPC)  
  - Orchestration / routing  
  - Integration with existing KYC / screening / custody stacks  
  - CLIs for migrations, linting, batch runs

- **Data Layer (Postgres)**  
  - Shared entity & CBU graph  
  - Attribute dictionary & document library  
  - DSL program store & execution logs  
  - Risk bands, document matrices, policy metadata

- **AI Edge (Model-agnostic)**  
  - “Intent → DSL” drafting  
  - Document understanding → structured attributes & observations  
  - Auto-remediation of DSL validation errors

Everything that *changes state* flows through the **same deterministic Rust pipeline**, regardless of whether it was triggered by a human, AI, or an external system.

---

## 5. Why a DSL? Why Now?

### 5.1 From Runbooks in Word to Runbooks as Code

Today’s reality:

- Runbooks describe onboarding & KYC policies in prose.
- Analysts and operations **interpret** those runbooks manually.
- IT re-implements an approximation in code and workflows.

With OB-POC:

- Runbooks *are* **versioned DSL templates** (e.g. `hedge-fund-onboarding.dsl`).
- A new policy or product often means:
  - Updating a config file (risk matrix, document requirements)
  - Updating a small DSL template – no major code change.

### 5.2 From “AI as an Assistant” to “AI as a Program Author”

A generic large language model is strong at:

- Explaining rules in natural language.
- Drafting candidate sequences of steps.

But those drafts are typically **not executable**.

OB-POC gives AI a **real target language**:

- The model proposes a candidate DSL program.
- The Rust core parses and validates it:
  - Syntax, verb existence, argument types.
  - Cross-verb constraints and policy rules.
- If invalid, it returns structured errors for the AI to fix.

> AI becomes a **first-pass programmer**, but **only valid DSL** can ever touch your data.

---

## 6. Technology Choices: Rust + Go (and Why Not Java/Spring in the Core)

### 6.1 Operation-Centric vs Class-Centric

Our domain is **verb-heavy**:

- `create-case`, `assign-role`, `link-ubo`, `request-document`, `run-screening`…

These are **operations** over a relatively simple data graph.

- Rust and Go treat data as structs and behaviour as **composable traits/interfaces**:
  - A natural fit for “lots of small operations over a shared graph”.
- Java/Spring is optimised for a **class-centric**, OO world:
  - Deep inheritance, heavy use of ORMs and metadata annotations.
  - Easy to end up with large service & entity hierarchies just to express simple operations.

We’re not anti-Java; we’re aligning language choice with problem shape.

### 6.2 Agentic Development: Compiler as Co-Pilot

OB-POC is designed to be **built and evolved with AI agents** running in your IDE and CI:

- **Rust**
  - Strong static guarantees; Clippy & the compiler produce excellent, precise error messages.
  - When a module compiles and passes linting, a large class of bugs is already ruled out.
- **Go**
  - Small, orthogonal language; one obvious way to do things.
  - Fast build & test cycles; `gofmt` unifies style.

For AI-assisted workflows:

- Rust provides **high-quality feedback** → good for **core correctness** (DSL engine).
- Go provides **fast, predictable iteration** → good for **APIs & orchestration**.

Java/Spring, while mature, tends to be:

- Slower to build/test for small, iterative changes.
- More dependent on runtime reflection and annotations, which are less visible to static tooling and AI.

### 6.3 Operational Footprint

- **Rust/Go**: single static binaries, fast start-up, small container images, low memory.
- **Java/Spring**: JVM + Spring stack; powerful but heavier for small, focused services and CLIs.

In a microservice + CLI world, especially under Kubernetes, the Rust/Go profile is more aligned with how we expect to deploy and scale OB-POC.

---

## 7. Enterprise Reality: “Good Enough” and Inertia Are Features, Not Bugs

We assume you **already** have:

- One or more KYC systems.
- Workflow engines and case managers.
- Screening providers.
- Established operational runbooks and control frameworks.

OB-POC is explicitly **not**:

- A demand to rip all that out.
- A theoretical “greenfield only” architecture.

Instead, OB-POC is:

- A **sidecar platform**:
  - Start by expressing *one* onboarding flow as DSL.
  - Integrate with existing KYC / screening / document stores via APIs.
- A **safety net**:
  - The same DSL runbook can be replayed under new rules:
    - “What if our risk policy had changed six months ago?”
- An **alignment tool**:
  - Product, compliance, operations, and technology can finally *read the same artefact*.

> We accept that today’s stack is “good enough”. OB-POC’s value is making tomorrow’s changes **cheaper, safer and faster**.

---

## 8. Evidence-Based KYC: Allegations vs Observations

OB-POC cleanly separates two concepts:

- **Allegations**  
  What the client claims (forms, declarations, self-reported data).
- **Observations**  
  What the system and analysts observe (documents, screenings, external data).

Each observation carries:

- Source & authority (registry, passport, utility bill, etc.).
- Link to a document or data feed.
- Extraction method (AI vs manual).
- Confidence and timestamp.
- Analyst / agent responsible (where applicable).

The DSL then encodes **how you combine and reconcile** allegations and observations:

- Where they align → automated verification, potential STP.
- Where they diverge → escalations, waivers, senior approvals.

This structure gives you a crisp answer to:

> “Why did we approve this client, on what basis, and who signed off?”

---

## 9. What You Get Out of the Box

### 9.1 For Architecture & Engineering

- A **clear, bounded DSL** for onboarding and KYC runbooks.
- A **Rust DSL engine** with:
  - Parser & AST
  - Semantic validation rules
  - Execution planning & DB integration (Postgres as a starting point)
- A **Go-based service layer**:
  - REST/gRPC endpoints over the DSL engine
  - Integration points for existing systems
- A **shared data model**:
  - Entity & CBU graph
  - Attribute dictionary
  - Document library & evidence model

### 9.2 For Product, KYC & Operations

- DSL **templates** for common onboarding scenarios:
  - Corporate, fund, intermediary, managed account, etc.
- **Config-driven risk & document matrices**:
  - Adjust requirements without code changes.
- **STP pathways**:
  - Define when low-risk cases can auto-approve.
- **Explicit escalation logic**:
  - When and how analysts, approvers and committees get involved.

### 9.3 For Compliance & Audit

- **Immutable DSL runbooks per case**:
  - Every step is recorded as a program, not just log entries.
- **Replay under new policy**:
  - “If we apply today’s policy to last year’s onboarding, what changes?”
- **Traceability**:
  - Clear mapping from:
    - Policy → DSL template → DSL instance → execution logs → outcome.

---

## 10. Typical Adoption Path

1. **Discovery & Mapping (4–6 weeks)**  
   - Identify 1–2 onboarding flows (e.g. a specific fund type, a key jurisdiction).
   - Map existing runbooks and systems to candidate DSL verbs.

2. **Pilot Build (8–12 weeks)**  
   - Implement the DSL engine integration for those flows.  
   - Integrate with existing KYC / screening.  
   - Run in parallel (“shadow mode”) against live or replayed cases.

3. **Go-Live in a Controlled Segment**  
   - Route a controlled subset of new cases through OB-POC.  
   - Measure cycle times, manual touch points, error rates.

4. **Scale by Configuration, Not by Re-Platforming**  
   - Add new verbs, templates and risk/doc configs for additional flows.  
   - Gradually expand coverage, backed by measurable wins.

---

## 11. Why OB-POC for Your Organisation

If you are:

- **Tired of “just another workflow”** that doesn’t fundamentally change cost or risk.
- **Curious about harnessing AI safely** for onboarding and KYC, but uncomfortable with black-box decisioning.
- **Keen to move from documents & slides** to **executable artefacts** your teams can actually run and test.

…then OB-POC offers a **concrete, technical path**:

- **Forward-looking**: built for AI-assisted development and AI-assisted operations.
- **Pragmatic**: designed to sit **alongside** your existing estate, not replace it overnight.
- **Deterministic**: every decision traceable to a program, a policy, and evidence.

---

## 12. Next Steps

- **Architecture Deep Dive (2–3 hours)**  
  Map OB-POC’s DSL and entity graph to your current onboarding and KYC landscape.
- **Pilot Definition Workshop (1 day)**  
  Jointly select the first onboarding flow and define success metrics.
- **Proof-of-Concept Build**  
  Co-develop a pilot that lives in your environment, with your data and policies.

> OB-POC is not a slide-deck concept. It’s an opinionated, working pattern for how onboarding and KYC should look in a world where AI, DSLs and deterministic execution actually work together.
