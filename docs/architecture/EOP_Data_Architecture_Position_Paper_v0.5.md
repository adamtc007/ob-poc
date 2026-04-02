# Enterprise Onboarding Platform (EOP)

## Data Approach, Model, and Implementation Strategy

**Solution Architecture Position Paper — v0.5**

Document Type: Solution Architecture Paper
Status: Formal position paper
Date: 31 March 2026

---

## 1. Purpose

This paper responds to the architecture brief:

> *What is the Enterprise Onboarding Platform (EOP) data approach, model, and implementation strategy?*

It sets out the intended data strategy, the reasoning behind the model, the capabilities it must support, and the implementation consequences that follow. It also identifies the known risks, operational costs, and governance obligations of the proposed approach, so that the architectural position can be evaluated on its merits rather than on omission of inconvenient tradeoffs.

Its focus is the business shape of the problem and the platform model required to address it, independent of any specific framework or technology selection.

---

## 2. Executive Summary

EOP spans two materially interdependent domains — onboarding and KYC — that share underlying business facts: entity structures, ownership and control relationships, documents, attestations, evidence, policy obligations, and downstream service activation context.

The architectural question is not how to persist isolated service-local object graphs. It is how to represent and govern one coherent business state model that supports multiple valid operational and compliance views without fragmenting truth.

The proposed position is that EOP should be based on:

- a canonical shared relational state model
- explicit domain windows over that model, each with defined scope, interpretation, consistency guarantees, and mutation rights
- explicit, auditable business mutations (verb-first writes)
- first-class policy, evidence, provenance, and temporal semantics
- data governance as a structural part of the platform, not a peripheral compliance layer
- deterministic execution paths for operational change
- explicit invalidation and coordination rules where shared truth changes affect multiple windows

This paper also examines why ORM-first architecture optimises for a different set of properties than EOP requires and, equally, identifies the costs and risks inherent in the model-led approach proposed here, including cross-window coordination, broader testing obligations, governance overhead, operational observability, schema evolution discipline, and higher architectural demand on model stewardship.

The conclusion is that EOP is best treated as a **shared business state platform**. Its data model is the architecture. Implementation exists to realise it faithfully.

---

## 3. The Problem Shape

EOP is not a narrow transaction processing application. It is a platform concerned with the governed representation and progression of client, entity, document, policy, evidence, and service activation state across related business processes.

The distinguishing characteristics of this problem are:

**Shared underlying truth.** The same legal entity, the same UBO relationship, the same evidence document may be relevant to both an onboarding journey and a KYC periodic review. These are not copies — they are the same business fact viewed through different operational lenses.

**Incomplete and evolving information.** At any point in an onboarding or KYC lifecycle, the platform holds a mixture of allegations, partially verified facts, pending evidence, and confirmed state. The model must represent this gradient, not just the endpoints.

**Policy-relevant interpretation.** The same underlying ownership structure may satisfy one jurisdictional threshold and fail another. The platform must bind policy meaning to state, not leave it to consuming code.

**Provenance as a load-bearing concern.** In custody banking, the question is not only "what is the current state" but "what was the state at decision time T, and what evidence supported it." This is not metadata — it is structural.

**Temporal truth as a first-class requirement.** The platform must support current truth, decision-time truth, and the ability to reconstruct the state known at a prior point in time. This is essential for reviewability, certification, challenge handling, and audit.

**Cross-domain reuse.** A KYC certification produces evidence and entity state that onboarding consumes. An onboarding journey captures entity and relationship data that KYC must later review. These are not integration problems between separate systems — they are natural consequences of operating over shared truth.

**Regulatory governance as an operational reality.** In a regulated domain, data governance is not a documentation exercise performed after the architecture is complete. It is a set of binding constraints — on data quality, lineage, classification, retention, access, and erasure — that shape what the architecture must do. For KYC in particular, governance obligations are functional requirements, not non-functional preferences.

Taken together, these characteristics define a platform problem, not an application-local persistence problem.

---

## 4. Architectural Position

### 4.1 Canonical Shared State

At the centre of EOP is a canonical representation of durable business truth. This is the platform's shared state substrate — the place where entity identity, legal structures, roles, relationships, ownership assertions, evidence linkages, product activation context, and obligation state are maintained.

The purpose is not centralisation for its own sake. It is to ensure that business truth is represented once, governed consistently, and reused safely across multiple platform capabilities.

### 4.2 Domain Windows Over Canonical State

The platform must not expose canonical state as one monolithic object model. Different consumers require different interpretations and slices.

A **domain window** is a governed operational contract over canonical state. It is not merely a read projection, DTO, or reporting view. A useful conceptual model is that of a sliding window over a larger standard business state machine — the underlying state machine represents platform truth, and different windows expose the subset, interpretation, or operational shape required for a given domain or workflow stage. But the metaphor must be used precisely: a window implies contract, interpretation, consistency semantics, and permitted business operations over shared state, not simply a filtered query.

Each window defines:

- **what** subset of canonical state it exposes
- **how** it interprets that state (for example policy thresholds, completeness rules, or sufficiency criteria)
- **what consistency guarantee** it provides (transactionally consistent read, or eventually consistent materialised view)
- **what mutations** it can initiate against canonical state
- **what invalidation rules** apply when canonical truth changes beneath that window

The principal windows for EOP are:

| Window | Canonical state consumed | Interpretation |
|--------|--------------------------|----------------|
| Onboarding | Entity, relationship, document, service activation | Completeness toward operational go-live |
| KYC | Entity, UBO, ownership/control, evidence, certification | Regulatory sufficiency and risk classification |
| Document & Evidence | Document, attestation, verification status | Evidentiary completeness and provenance |
| Policy & Obligation | All policy-relevant state | Jurisdictional threshold evaluation |
| Service Activation | Entity, product, resource, operational readiness | Downstream system enablement |

These are not alternate truths. They are governed lenses over the same substrate.

### 4.3 Worked Example: UBO Ownership Chain

To make this concrete, consider a single UBO ownership chain during LU UCITS SICAV onboarding.

**Canonical state** holds: the legal entity hierarchy (fund → ManCo → holding company → natural person), the ownership percentages at each level, the evidence documents submitted, and the verification status of each link.

**The onboarding window** sees this chain as: "UBO identification is complete / incomplete for onboarding purposes. Required evidence has been received for 3 of 4 entities. Blocking dependency for service activation."

**The KYC window** sees the same chain as: "Beneficial owner X holds 28% indirect control, exceeding the 25% LU threshold. Screening status: pending. CDD tier: enhanced. Certification: not yet issued."

Architecturally, both windows project from the same canonical ownership and evidence structures, but through different interpretive contracts: the onboarding projection evaluates against a completeness rule set (which evidence items are required per entity type), while the KYC projection evaluates against jurisdictional threshold policy (which ownership percentage triggers enhanced due diligence under the applicable AML directive).

Same underlying facts. Different operational interpretations. Neither window owns the data — both read from and write to canonical state through their respective governed contracts.

**Conflict scenario:** A KYC periodic review downgrades UBO verification status from "confirmed" to "requires refresh" while onboarding is mid-flight relying on that confirmation. The canonical model records the status change with provenance: who changed it, when, why, and under what authority. The onboarding window's completeness projection is invalidated, and its next read reflects the downgrade. The onboarding orchestration must handle this as a blocking dependency reappearing, not as a surprise.

This is an explicit and visible consequence of shared truth. The alternative — siloed copies that drift — is worse, but the coordination cost is real and must be designed for.

### 4.4 Authority and Conflict Arbitration

Where windows diverge in interpretation or where a mutation initiated through one window affects assumptions held by another, **canonical state remains authoritative**.

Windows do not carry independent truth claims. They carry operational interpretations over shared truth. A conflict is therefore not resolved by allowing each window to preserve its prior local assumption. It is resolved by:

1. recording the canonical fact change with provenance and temporal trace
2. invalidating or recomputing dependent window projections
3. requiring orchestration or downstream workflow to respond according to that window's contract
4. using explicit concurrency guards, reservations, or workflow compensation where the business process requires temporary coordination

This principle matters because it prevents the platform drifting toward hidden, window-local truth ownership.

---

## 5. Data Governance

### 5.1 Governance as Architecture, Not Afterthought

In a regulated domain — principally but not exclusively KYC — data governance is not a compliance layer bolted onto a completed system. It is a set of binding constraints that shape what the architecture must do, how data flows through the platform, and what the platform must be able to demonstrate at any point in time.

For EOP, data governance is best understood as partially functional: some governance obligations directly determine system behaviour (a data quality rule that blocks a certification, a retention policy that prevents deletion, an access classification that restricts a window's projection). Others are operational (lineage reporting, stewardship accountability, audit response). Both categories must be addressed structurally, not assumed to be someone else's problem.

The canonical shared state model makes governance harder in some respects (broader impact surface, shared stewardship) and easier in others (single source of truth for lineage, no reconciliation across duplicate stores, consistent provenance). This section identifies what the architecture must provide.

### 5.2 Data Quality as an Executable Concern

In a KYC context, data quality is not an aspiration — it is a regulatory obligation. The platform must be able to assert, at any point, that the data underpinning a certification or risk decision meets defined quality standards.

An important distinction applies here. **Structural data quality** — completeness, referential validity, allowed value domains — establishes that data is well-formed and internally consistent. **Regulatory sufficiency** — whether the data, taken together with its evidence and provenance, is adequate to support a KYC decision — is a higher bar. Data can be structurally valid but insufficient for certification. The platform must support both levels: structural quality as a baseline enforced at the canonical layer, and regulatory sufficiency as a policy-bound assessment within the KYC window.

This means data quality rules must be:

- **defined against canonical state**, not against window-local copies or downstream extracts
- **executable as part of the verb path**, so that a business operation can be blocked, flagged, or conditioned on quality status
- **versioned**, because the quality rules applicable at certification time may differ from those applicable today
- **auditable**, so that it is possible to demonstrate what quality checks were applied, when, and with what result

Data quality is therefore not a batch process that runs overnight and produces a report. It is part of the operational semantics of the platform.

### 5.3 Data Lineage and Provenance

The platform must be able to answer lineage questions at the individual fact level:

- where did this fact originate (manual entry, document extraction, screening provider, corporate registry, upstream system)
- what transformations or derivations have been applied
- what evidence supports the current value
- what prior values existed and why they changed

This is not optional for KYC. Regulatory examination can and does require the institution to demonstrate the provenance chain for specific data points used in customer risk assessment and beneficial ownership determination.

The canonical model's verb-first write path and temporal semantics provide the structural foundation for lineage. Every mutation is a named operation with provenance metadata. The architecture must ensure this chain is unbroken and queryable, not merely logged.

### 5.4 Data Classification and Access Governance

Not all canonical state carries the same sensitivity. PII, beneficial ownership information, screening results, and adverse media findings have different classification levels and different access requirements — regulatory, contractual, and organisational.

The architecture must support:

- **classification at the attribute level**, not only at the entity or table level
- **window-scoped access enforcement**, so that a window's projection can be constrained by the caller's access rights and the data's classification
- **purpose limitation**, so that data collected for KYC purposes is not silently repurposed for unrelated operational use without explicit governance approval

In a shared canonical model, this is more important than in a siloed architecture, because the canonical substrate contains data from multiple regulatory and business contexts in one place. The access governance model must ensure that shared storage does not become shared access.

### 5.5 Retention, Archival, and Right to Erasure

Custody banking and KYC operate under retention obligations that frequently conflict with erasure rights. AML directives typically require retention of KYC records for five to ten years post-relationship. GDPR and equivalent regimes grant data subjects rights that must be reconciled against those obligations.

The architecture must support:

- **retention policies bound to canonical state**, with explicit rules per data classification and regulatory context
- **legal hold capability**, so that data subject to regulatory inquiry or litigation is protected from routine archival or deletion
- **erasure with audit trace**, so that where erasure is permitted and executed, the platform records that erasure occurred, when, and under what authority — without retaining the erased data itself
- **archival that preserves temporal reconstructability**, so that archived state can still support point-in-time queries for regulatory reporting

These are not edge cases. They are operational realities for a custody banking onboarding and KYC platform.

### 5.6 Model Stewardship

A shared canonical model requires active stewardship. Without disciplined ownership of naming, semantics, lifecycle rules, and cross-window impact analysis, the canonical model degenerates into a platform-scale junk drawer.

The architecture must include:

- **stewardship accountability** — clear ownership of canonical model semantics, not delegated to whichever team last touched a table
- **named decision rights** — governance requires explicit authority over canonical model semantic changes, data quality policy, access classification, retention policy, and window contract changes that alter governance posture; these decision rights must be assigned, not assumed
- **review discipline for shared semantic changes** — changes to canonical state structure or meaning must be assessed for cross-window impact before implementation
- **explicit criteria for canonical inclusion** — not every data point belongs in the canonical substrate; the architecture must define what qualifies for canonical status versus what remains local to a window

Governance overhead is a real cost of this approach. But it is also a signal of architectural seriousness. Shared truth without stewardship does not remain shared truth for long.

### 5.7 Policy-Rule Governance

The architecture leans heavily on policy-bound interpretation: jurisdictional thresholds, completeness rules, obligation conditions, and sufficiency criteria that determine how windows interpret canonical state. These policy rules are themselves governed artefacts.

This means policy rules must be:

- **versioned**, so that the rule set applicable at any historical decision point can be reconstructed
- **traceable to decision-time**, so that a certification or risk assessment can demonstrate which rules were applied and what their parameters were
- **subject to change governance**, so that a threshold change (for example, a jurisdiction lowering its UBO control threshold from 25% to 10%) follows a governed approval path with impact analysis across affected windows
- **independently auditable**, so that the rule itself, not only its output, can be examined

This connects governance directly back to the domain-window concept: a window's interpretation of canonical state is only as trustworthy as the policy rules that govern it. Ungoverned policy rules produce ungoverned interpretation, regardless of how well the underlying data is managed.

### 5.8 Governance and the Canonical Model: The Structural Advantage

It is worth stating plainly that a canonical shared state model, despite its governance costs, is structurally better positioned for regulatory data governance than a fragmented one.

In a siloed architecture, governance must be applied to each silo independently, lineage must be reconstructed across integration boundaries, quality rules must be duplicated and kept consistent, and retention policies must be coordinated across stores that may not share identifiers. Every governance obligation becomes a reconciliation problem.

In a canonical model, governance is applied once to one substrate. Lineage is inherent in the mutation history. Quality rules execute against a single truth. Retention policies bind to canonical state directly. The governance surface is larger but unified, which is fundamentally more tractable than a governance surface that is smaller per silo but multiplied across many.

This does not make governance easy. It makes it possible to do correctly.

---

## 6. Temporal Semantics

Temporal semantics are not a storage convenience. They are part of the business architecture.

In custody banking and KYC operations, the platform must be able to answer not only:

- what is true now

but also:

- what was believed to be true when a certification was granted
- what evidence existed at that time
- what policy interpretation applied at that time
- what mutation sequence produced the later change in state

The canonical model must therefore support:

- **current truth** — the latest known state of each fact
- **decision-time truth** — the state as known when a business decision was taken
- **mutation history** — the auditable chain of changes over time

This may be implemented through event sourcing, bitemporal tables, append-only fact history, or snapshot-based approaches. The architectural requirement is the capability, not a single mandatory mechanism.

Temporal support should therefore be regarded as part of the EOP model itself, not merely as an implementation concern.

---

## 7. Implementation Consequences

### 7.1 Write Model: Verb-First

All material state changes are performed through explicit business operations — named, validated, policy-aware, transactionally controlled, and auditable.

Each verb carries the operation name, the actor, the target entities, the policy context under which it executes, and the prior state it expects. The platform records what changed, why, under which operation, and what evidence or authority supported the change.

This avoids the architectural weakness of allowing business transitions to occur as side effects of persistence session behaviour such as dirty tracking, implicit flush, or cascade.

### 7.2 Read Model: Projection-First

Read access is shaped explicitly according to consumer need rather than navigated through a generic entity graph. Each domain window defines its own read projection — the subset and interpretation of canonical state it requires.

**Consistency model.** Not all windows require the same consistency guarantee. The onboarding window checking completeness status may tolerate a read replica with sub-second lag. The KYC window issuing a certification against current evidence state requires transactional consistency. The architecture must make this explicit per window rather than applying a single consistency model everywhere.

### 7.3 Invalidation and Dependency Contracts

Because truth is shared, mutations in canonical state may invalidate assumptions in one or more windows.

The architecture must therefore define:

- which facts are load-bearing for which windows
- what events or dependency markers signal invalidation
- whether the affected window recomputes synchronously or asynchronously
- what orchestration obligations arise when a previously satisfied dependency becomes unsatisfied

This is not an implementation detail to be deferred. It is part of the operating model for shared truth.

### 7.4 Administrative Corrections versus Governed Business Verbs

A verb-first write model must not collapse all changes into one governance path. A platform of this kind requires a distinction between:

- **governed business verbs**, which perform business-significant transitions subject to policy, validation, and full operational semantics
- **administrative corrections**, which may update or repair data under tighter operational control but without pretending to be business events of the same class

Without this distinction, the platform either becomes operationally cumbersome or allows ungoverned change to leak into business-significant state transitions.

---

## 8. ORM-First Architecture: A Different Optimisation

This section is not an argument that ORM is bad technology. It is an observation that ORM-first architecture optimises for a different set of properties than EOP requires.

### 8.1 What ORM-First Optimises For

ORM frameworks are designed to reduce friction between application object models and relational storage. They optimise for:

- rapid development of application-local CRUD operations
- object graph navigation as the dominant read pattern
- convention-driven persistence with minimal boilerplate
- bounded aggregate ownership with clear repository patterns

These are legitimate engineering goals. For applications where the domain model is owned by a single service, where reads follow entity graph traversal, and where persistence convenience directly translates to productivity, ORM-first architecture is well-suited.

### 8.2 Where the Misalignment Occurs

EOP's problem shape does not match those assumptions. Specifically:

**Shared cross-domain state versus local aggregate ownership.** ORM's natural unit is the application-owned aggregate. EOP's canonical state is shared across onboarding and KYC. Forcing shared platform truth into single-owner aggregates either fragments truth or creates artificial ownership boundaries.

**Explicit business transitions versus implicit dirty tracking.** ORM's change detection and flush cycle is designed for persistence convenience — it tracks what changed in the object graph and synchronises to storage. EOP requires that every business-significant mutation is an explicit, named, auditable operation. These are different things.

**Projection-first reads versus entity graph navigation.** ORM encourages loading entities and navigating relationships. EOP's domain windows require shaped projections that may span, filter, or reinterpret the canonical model in ways that do not map naturally to entity traversal.

**Policy-bound interpretation versus flat entity state.** ORM entities represent current field values. EOP must represent facts alongside provenance, verification status, policy relevance, and temporal validity. Bolting these concerns onto entity fields produces a model that is neither clean ORM nor clean platform semantics.

### 8.3 False Ownership Boundaries

A particularly important risk is that ORM-first architecture pushes teams toward **false ownership boundaries**.

Every aggregate wants a home. Every entity graph wants an owning service. In a platform like EOP, that pressure leads to organisationally convenient but architecturally false claims such as:

- onboarding owns entity completeness
- KYC owns UBO truth
- documents own evidentiary state
- activation owns readiness

In reality, these concerns overlap on shared facts. If the model is forced into service-local ownership patterns, the result is not cleaner architecture. It is duplicated truth, integration drag, and eventual reconciliation overhead disguised as bounded context discipline.

### 8.4 What Tends to Happen

When ORM-first architecture is applied to a problem of this shape, a predictable pattern emerges: the initial implementation is fast and familiar, but the model progressively accumulates exceptions — DTOs that reshape what entities cannot, service-layer logic that compensates for persistence-driven structure, projection queries that bypass the ORM entirely, and audit or provenance added after the fact. The architecture becomes the sum of its exceptions rather than the expression of its design.

This is not a failure of ORM. It is a misapplication — the same way a B-tree index is not wrong, just wrong for a full-text search problem.

---

## 9. Known Risks and Costs of the Proposed Approach

The model-led architecture proposed here has real costs. Presenting it without acknowledging them would make this paper advocacy rather than architecture.

### 9.1 Cross-Window Coordination Complexity

Shared canonical state means that a mutation initiated through one window can invalidate assumptions held by another. The UBO downgrade scenario in §4.3 is one example. Others include document expiry affecting both KYC certification validity and onboarding completeness, or entity structure changes during an in-flight periodic review.

**Mitigation:** The architecture must define explicit invalidation contracts — when a canonical fact changes, which windows are notified, and what obligations that creates. This requires disciplined dependency tracking and event propagation.

### 9.2 Cognitive Load on Developers

A shared canonical model spanning onboarding and KYC is larger and more interconnected than a service-local entity model. Developers writing a mutation verb must understand not only their immediate domain but the downstream consequences across window boundaries.

**Mitigation:** Domain windows exist partly to contain this. A developer working within the KYC window operates through a bounded contract rather than through the full canonical schema. But this containment is only as good as the window contracts. Poorly defined windows leak complexity.

### 9.3 Testing Complexity

Changes to canonical state can ripple across windows. Testing a KYC verb may require validating that the onboarding window's projections remain correct. This produces a broader test surface than isolated service testing.

**Mitigation:** Window contracts should be independently testable. Given a canonical state fixture, each window's projection can be verified in isolation. Integration testing then validates cross-window consistency and invalidation behaviour.

### 9.4 Operational Cost of Explicit Verbs

A verb-first write model requires that every business operation be explicitly defined, validated, and instrumented. There is no generic "just update the field" path. For simple state corrections or data fixes, this can feel heavyweight.

**Mitigation:** The architecture should distinguish between governed business verbs and administrative corrections, each with an appropriate control model (§7.4).

### 9.5 Schema Evolution Discipline

A canonical shared model is harder to evolve than a service-local one. A schema change affects every window that projects over the changed state. Migration coordination across windows is non-trivial.

**Mitigation:** Window contracts must insulate consumers from schema internals. A well-designed projection contract can absorb underlying schema changes without breaking consumers, but this requires genuine abstraction rather than thin wrappers over table structure.

### 9.6 Operational Observability

A shared canonical model with multiple windows and invalidation contracts is harder to debug in production than a service-local model. When an onboarding journey stalls, the root cause may be a KYC-initiated downgrade several dependency hops away. The dependency chain is architecturally correct but operationally opaque unless the platform invests in cross-window traceability.

**Mitigation:** The platform's verb audit trail, provenance metadata, and mutation history provide the raw material for observability. But raw material is not observability. The architecture must include explicit investment in cross-window dependency visualisation, causal tracing from symptom to originating mutation, and operational dashboards that surface invalidation cascades. This tooling does not build itself and must be planned as part of the platform, not discovered as a gap after the first production incident.

### 9.7 Governance Overhead

A shared canonical model requires active stewardship — naming discipline, semantic review, cross-window impact analysis, and explicit criteria for what enters canonical state versus what remains window-local. This is a standing operational cost, not a one-time setup task.

**Mitigation:** §5.6 addresses stewardship directly. The cost is real but bounded. The alternative — governance applied independently to multiple siloed stores with reconciliation across integration boundaries — is higher in aggregate, but the cost is distributed and therefore less visible. Visible cost is not the same as higher cost.

---

## 10. Architectural Principles

These govern the EOP data architecture:

1. **The shared data model is a strategic platform asset.** It is not a byproduct of application code and must not be reduced to framework conventions.

2. **Canonical state before local convenience.** Durable business truth is defined independently of how individual consumers prefer to access it.

3. **Domain windows are governed operational contracts, not duplicate truths.** Each window defines its scope, interpretation, consistency guarantee, mutation rights, and invalidation rules.

4. **Canonical state is authoritative.** Windows interpret shared truth; they do not create competing versions of it.

5. **Writes are explicit business operations.** Business-significant change occurs through named, governed, auditable verbs.

6. **Evidence, provenance, and temporal validity are first-class.** The source, status, and time-bound meaning of facts are architecturally visible.

7. **Data governance is structural.** Quality, lineage, classification, retention, access, and policy-rule governance are part of the platform architecture, not a compliance layer applied after the fact.

8. **Projection-first reads.** Consumers receive the shape they need through defined contracts, not through framework-shaped entity navigation.

9. **Tooling follows architecture.** Technology choice is subordinate to architectural fidelity.

---

## 11. Decision Position

The architecture choice is between two approaches that optimise for different things:

| | Model-led platform architecture | ORM-first application architecture |
|---|---|---|
| **Optimises for** | Cross-domain truth preservation, governed views, explicit control, auditability, structural governance | Development velocity, convention-driven persistence, local service simplicity |
| **Natural fit** | Shared state platforms, multi-domain compliance, policy-bound operations, regulated domains | Single-service applications, bounded aggregates, CRUD-dominant workloads |
| **Primary cost** | Coordination complexity, governance overhead, cognitive load, observability investment, schema evolution discipline | Progressive architectural distortion when applied to cross-domain shared state |
| **Governance posture** | Governance applied once to one canonical substrate; lineage inherent in mutation history | Governance applied per silo; lineage reconstructed across integration boundaries |
| **Risk profile** | Higher upfront investment, lower long-term structural debt | Lower upfront investment, higher long-term structural debt |

For EOP — a cross-cutting platform where onboarding and KYC operate over shared entity, evidence, policy, and operational state in a regulated custody banking context — the model-led approach is the appropriate architectural position.

This is not a rejection of ORM as technology. ORM may be used tactically within specific implementation layers. The position is that ORM's assumptions must not be the architectural centre of gravity for this platform.

---

## 12. Position Statement

EOP is a shared business state platform. Its data architecture is defined by canonical truth, governed domain windows with explicit contracts, verb-driven mutation, authoritative canonical state, first-class provenance and temporal semantics, and structural data governance.

The proposed approach carries real costs — cross-window coordination, broader test surfaces, cognitive load, operational observability, governance overhead, and schema evolution discipline — and these must be actively managed. The costs are known, accepted, and manageable.

The costs of not adopting this approach — semantic drift, duplicate truth, reconciliation-driven operations, false ownership boundaries, governance applied independently to multiple silos rather than once to one substrate, and progressive structural debt — are higher.

The model-led approach concentrates governance effort in one place rather than distributing weaker governance across many. That is the appropriate trade for a regulated platform.

The EOP data model is the architecture. The implementation exists to realise it faithfully.
