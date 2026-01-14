# OB-POC: Deterministic Agentic Onboarding at BNY Scale  
## A platform strategy: Domian Specific Language (DSL)-runbooks + enterprise workflow governance + interactive “cockpit” UX

> **Core message:**  
> It’s a platform vision decision: **move onboarding/KYC/resource activation to a deterministic, auditable runbook model** that enables **AI everywhere** safely, with a **shared enterprise data model** and a **minimal-forms, high-signal UI**.

### client outcomes
Make onboarding a **predictable, transparent, and scalable client experience**:  
- reduce “back-and-forth” data requests (ask once),  
- shorten time-to-**good-to-transact**,  
- and give clients clear visibility into progress, requirements, and blockers.

This is a competitiveness objective as much as an internal efficiency objective.


---

## 1) The problem BNY must solve (what traditional onboarding misses)

Institutional onboarding is not “a form.” It is a **regulated, multi-system state transition** problem:

- KYC/UBO is a living graph (entities, ownership/control, directors, evidence, periodic reviews).
- Products (Custody / TA / Fund Accounting / Collateral) require **service option clarification** (markets, SSI modes, channels).
- Delivering services requires **resource activation** across many BNY systems and apps (accounts, entitlements, connectivity, instruction sets).
- Operational risk is dominated by:
  - manual handoffs and ticket-driven provisioning,
  - inconsistent data definitions across product teams,
  - non-reproducible outcomes (“why did we do that?”, “who approved?”, “what evidence?”),
  - and the lack of a deterministic “plan/apply” discipline.

BNY needs a platform that turns onboarding into a **repeatable, provable execution process**—not an ad hoc sequence of emails, forms, and tribal knowledge.

---

## 2) The strategic answer: a deterministic runbook ledger

### 2.1 DSL-as-State (“runbooks are the ledger”)
OB-POC introduces a discipline: every onboarding action is represented as a **versioned runbook** (DSL) that is:

- **Validated** (schema + grammar)
- **Deterministically ordered** (topological sorting / dependency resolution)
- **Idempotent** (safe re-run; no duplicate side effects)
- **Replayable** (rebuild state from the runbook + evidence)
- **Auditable** (the runbook is the source of truth; execution emits explain trails)

This is the architectural foundation that makes large-scale onboarding governable.

### 2.2 AI visibility without losing control: “AI proposes; the runbook proves”
BNY wants “AI everywhere,” but regulated operations demand:

- deterministic outputs
- explicit approvals
- evidence provenance
- full audit trails

OB-POC makes AI safe and useful by placing the agent at the **planning/authoring layer**:

- The agent produces **structured DSL deltas**, not free-text “opinions.”
- The platform produces a **Plan** artifact (“what will change, what’s missing, what will be created”).
- Humans approve (four-eyes) and workflow triggers **Apply**.
- Execution produces:
  - SRIDs (resource instance identifiers)
  - deep links/URLs/handles into downstream systems
  - explain references (“why this happened”)

**Result:** AI is embedded into every step without sacrificing governance.

#### Agentic AI as executable intent (not just document extraction)
In OB-POC, the agent’s primary job is **operational orchestration**: translating user intent into a concrete, executable artifact.

- The user expresses intent (e.g., “Enable Custody + TA for this CBU in these markets; compute control anchor; open required accounts”).
- The agent produces an **editable runbook delta** (DSL), aligned to verb schemas and the domain data model.
- The platform compiles a **Plan**: required data/evidence, derived dependencies, and exactly what will be executed.
- Only approved steps are applied; execution is deterministic and idempotent.

This is qualitatively different from “AI as document summariser.” The output is a governed operational change set.

#### Guardrails (how BNY controls agentic automation)
- **Constrained language:** agents can only emit approved verbs and schemas (“config, not free-form code”).
- **Plan/Apply discipline:** no side effects without an explicit plan and approval gate.
- **Deterministic execution:** the engine (not the model) performs ordering, validation, and idempotent apply.
- **Evidence and explainability:** every derived result and every action links back to evidence and a machine-readable explain trail.
- **Policy controls:** thresholds, required approvals, and permitted actions are centrally configured and auditable.

### 2.3 Configuration-led extensibility (measured “config over code”)
OB-POC is designed so onboarding capability expands using **repeatable patterns** that are friendly to automated testing:

- **Verb schemas and dictionaries** define what can be expressed and validated (approved verbs, argument types, search profiles, and required attributes).
- **Resource and service definitions (SRDEF profiles)** drive downstream activation requirements and dependency ordering.
- **Regression fixtures** are authored as DSL runbooks: new capabilities come with executable examples that become automated tests.

This is not “no-code”. New downstream integrations and provisioning adapters still require engineering. The benefit is that a large portion of change is expressed as **versioned configuration + runbook fixtures** rather than bespoke, domain-by-domain application logic—reducing drift and lowering the risk/cost of adding new products and onboarding behaviours.

### 2.4 Product alignment: intent-based change that fits an agile operating model
A practical advantage of an intent-based DSL is that it aligns naturally with an agile product model:

- **Business intent becomes the unit of change.** New capability is expressed as a verb (or a refinement to verb schemas, dictionaries, and rules) rather than a new screen or a bespoke workflow branch.
- **Shared language across teams.** Product, operations, and engineering can converge on the same artifacts: “what intent is supported?”, “what data is required?”, “what evidence/gates apply?”, “what resources will be activated?”.
- **Small, testable increments.** Each increment can ship with runbook fixtures that encode expected behaviour (plan/apply outcomes, readiness rules), supporting safe iteration and clear acceptance criteria.

This does not remove the need for engineering work. It makes the work more legible and product-led by tying delivery directly to expressed intent, measurable readiness outcomes, and regression-tested runbooks.


---

## 3) DSL supports enterprise workflow; it does not replace it

### Enterprise workflow remains the system of engagement
Workflow owns:
- human task routing (ops/compliance/tax/legal)
- approvals and four-eyes controls
- SLAs, escalations, reminders
- governance reporting and dashboards

### DSL is the deterministic execution substrate
DSL owns:
- canonical domain state (entities/roles/CBU/service intent/resources)
- deterministic machine actions (runbooks, idempotency, replay)
- evidence binding (documents, snapshots, provenance)
- derived outcomes (UBO/control anchors, readiness)
- machine-level audit (“what happened and why” per action)

### Stable integration contract (how workflow calls OB-POC)
Workflow interacts via a small surface area:

- **PLAN**: What will happen? What data/evidence is missing? What resources will be required?
- **APPLY**: Approved—execute the next deterministic step(s).
- **AWAIT / SIGNAL**: OB-POC pauses at workflow gates; workflow resumes with approved/rejected.
- **STATUS / READINESS**: Is the service “good-to-transact”? What is blocking?

This makes OB-POC an enabler for workflow—not a competitor.

---

## 4) The STP onboarding pipeline (the closed-loop model)

OB-POC standardizes onboarding as a closed loop:

### 4.1 “Ask once” onboarding: data-driven resource activation with a unified CBU requirement dictionary
A major source of delay in institutional onboarding is **repeated outreach**: each product team and each downstream system asks for overlapping data in different formats.

OB-POC removes this by treating onboarding as **data-driven resource activation**:

- A CBU selects products and service options (markets, SSI mode, channels).
- Rules discover the downstream **ServiceResourceDefinitions (SRDEFs)** required to deliver those services (accounts, entitlements, connectivity, instruction sets).
- **Each SRDEF has an attribute profile** (a subset of the global Attribute Dictionary) describing exactly what data is required to open/activate that resource.
- OB-POC rolls up all SRDEF profiles into a **single, de‑duplicated CBU requirement dictionary**.

**Result:** the platform can produce one consolidated checklist of required onboarding data.  
Where client input is needed, the client is asked **once**—not separately by each product/system.

This directly enables **straight‑through processing (STP)** for account opening and activation:
- when the unified dictionary is complete and validated, OB-POC can drive provisioning deterministically,
- and downstream resource owners return the final SRID + resource URL/handle to close the loop.

1) **Intent capture**  
   CBU subscribes to products and confirms service options (markets, SSI, channels, instrument scope).

2) **Resource discovery**  
   Rules derive which downstream system resources must exist to deliver the services.

3) **Unified data requirements (de-duped)**  
   Each ServiceResourceDefinition (SRDEF) has its own attribute profile (subset of the global dictionary).  
   OB-POC rolls these up to a **single CBU-level unified requirement dictionary**, removing duplication across products.

4) **Populate & validate**  
   Values are sourced from:
   - existing CBU/entity data
   - document extraction (evidence-driven)
   - derived computations (e.g., board controller)
   - manual entry as last resort  
   All validations and required gates are explicit.

5) **Provisioning**  
   The platform issues provisioning requests to resource owners/systems.

6) **Owner response closes the loop**  
   The resource owner/system returns:
   - **SRID** = app mnemonic + native key (account/object id)
   - **resource URL/handle** (deep link into the platform)  
   This becomes the “last-mile completion” artifact.

7) **Good-to-transact readiness**  
   OB-POC computes readiness per service intent:
   - READY / BLOCKED with concrete reasons (missing attributes, pending provisioning, failed setup).

This replaces “progress by email” with “progress by deterministic state.”

---

## 5) Shared platform data model: the real enterprise unlock

The strategic advantage is not a feature—it’s a **shared model** that spans:

- KYC cases, evidence packs, periodic reviews
- UBO/control graphs (board controller / GP rights)
- product/service intent and configuration
- discovered resources and provisioning lifecycles
- SRIDs and instance URLs returned from resource owners

This creates an enterprise-wide onboarding “spine” that product teams can plug into rather than reinvent.

### Control vs Economic: avoid false UBOs and avoid edge explosions
For complex fund structures (FoF → master pool → holdcos → SPVs), OB-POC separates:

- **Control (UBO):** who controls decisions/boards/GP rights (control edges + evidence)
- **Economic exposure:** who has NAV exposure

Crucially: economic look-through is computed **on demand** into bounded “exposure slices”—it is not materialized as implied edges (which would explode combinatorially).

---

## 6) Minimal-forms UX: agent chat + interactive structural inspectors (egui)

### The UI principle: forms are the wrong tool for graph/matrix onboarding
Traditional onboarding pushes complexity into hundreds of forms and repeated questions.  
OB-POC flips the model:

The interaction pattern is intentional:
- **Agent chat** captures intent and proposes runbook changes (reviewable diffs).
- **Inspectors** provide immediate visual validation of complex structures (graphs/matrices/dependencies).
- **Approvals** remain explicit (four-eyes and workflow gates) before provisioning or activation.

- **Agent sessions** capture intent and missing items.
- The UI becomes a **high-signal cockpit** for inspecting complex structures.

### Egui “cockpit” benefits
Egui is used to render the hard parts visually:

- Investor register and ownership/control structures
- Instrument matrix and market eligibility
- Product/service taxonomy and discovered resource plan
- Resource instances (SRIDs) with live status and deep links (URLs)

Instead of a form maze, operators get:
- instant structural feedback,
- drill-down inspectors,
- and fewer places to make mistakes.

This is a step-change in usability for complex institutional servicing.

---

## 7) What success looks like (outcomes senior management cares about)

### Faster time-to-revenue
- unified de-duped data requirements reduce repeated outreach
- auto-discovery prevents missed setup steps
- deterministic execution reduces rework and “reset to start”

### Better experience for BNY clients
- **Ask-once onboarding:** clients provide required data once; the platform reuses it across products and downstream systems.
- **Predictable time-to-value:** readiness is computed (READY/BLOCKED with reasons), reducing uncertainty and chase-ups.
- **Fewer surprises:** discovered resource requirements and missing evidence are visible early (plan output), before execution.
- **Clear accountability:** approvals and evidence provenance are explicit, so clients see what is pending and why.

### Lower operational risk
- auditable, replayable runbooks
- explicit approvals and evidence provenance
- fewer manual tickets and human transcription errors

### Better governance
- one version of “the truth” for what’s required, what’s done, and why
- readiness is computed, not guessed

### Scalable change delivery
- New onboarding functionality is introduced through **standard patterns**: verb schema updates, dictionary/resource profile additions, and deterministic runbook fixtures.
- Changes are naturally **regression-testable** because expected behaviour is encoded as executable runbooks (plan/apply outcomes, readiness checks, provisioning responses).
- Where code changes are required (e.g., new system adapters), the platform still benefits from a stable execution substrate and shared data model—reducing the amount of bespoke workflow logic each domain must own.



### Where OB-POC helps operations (mechanisms, not promises)
- **Single “source of onboarding truth”:** a runbook + derived readiness replaces scattered emails and ad hoc status tracking.
- **Fewer handoffs and less re-keying:** the unified CBU requirement dictionary reduces duplicate data collection and downstream re-entry.
- **Clear exception handling:** when something blocks (missing evidence, pending approval, provisioning failure), the platform can surface concrete blockers with drill-down.
- **Repeatability for recurring events:** periodic reviews, account changes, or service amendments can be executed as new runbook deltas with the same audit posture.
- **Operational visibility:** provisioning requests and responses (SRID + resource URL/handle) create a consistent “closure artifact” operations can rely on.

### Better operational integrations (complements existing tools)
OB-POC can integrate with existing operational tooling without replacing it:
- **Enterprise workflow** remains the owner of queues, assignments, SLAs, and escalations; OB-POC supplies deterministic Plan/Apply execution and status/readiness.
- **Provisioning/ticketing processes** can be supported via an append-only provisioning request/event ledger (request sent → ack → result), enabling clear ownership and traceability.
- **Downstream system deep links** (resource URLs/handles) reduce time spent hunting for accounts/records across platforms.

**Positioning:** OB-POC supports operational standardisation and transparency by turning onboarding into a governed, inspectable execution process—while allowing BNY to retain existing workflow and operational controls.

---

## 8) Recommended adoption path (BNY-friendly)
1) **Integrate with enterprise workflow** (Plan/Apply/Status) and keep workflow ownership intact.
2) Start with one “high pain” slice:
   - KYC evidence ledger + control anchor computation, or
   - Custody settlement intent → resource discovery → provisioning lifecycle.
3) Expand SRDEF inventory across products (TA, Fund Accounting, Collateral) using the same shared model.
4) Add provisioning adapters to downstream systems incrementally.
5) Scale agent session templates + guardrails as usage grows.

---

## Closing line
**OB-POC is the platform strategy that enables AI everywhere safely:**
agent-driven intent capture and orchestration, deterministic runbook execution, enterprise workflow governance, and a shared onboarding/KYC/resource activation spine that makes BNY faster, safer, and more scalable.
