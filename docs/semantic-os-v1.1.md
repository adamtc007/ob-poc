# ob-poc Semantic Operating System

**Registry · Context · Control Plane · Security · Governance · Derived Semantics for an Agentic Manager**

Version: 1.1 — February 2026
Author: Adam / Lead Solution Architect, BNY Mellon Enterprise Onboarding Platform
Status: Draft — for peer review
Lineage: v0.1 Attribute Dictionary → v0.3 Semantic Registry → v0.6 governance elevation → v0.8 governed/operational + security + derivations → v0.9 consolidation → v1.0 immutable snapshot architecture → v1.1 enforcement precision and implementation readiness

---

## 1. Vision

ob-poc requires a **Semantic Operating System** (Semantic OS): a single, queryable, versioned semantic substrate that enables an agentic manager to:

- **Discover** what exists — data, entities, documents, evidence, state
- **Understand** what it means — types, constraints, policy semantics, classifications
- **Handle securely** who may see, do, export, and derive what — confidentiality, residency, jurisdiction, purpose limitation, masking, export controls
- **Navigate** how things relate — ownership chains, derivation paths, evidence linkages, jurisdictional scope
- **Decide** what can be done — verbs, preconditions, postconditions, durable continuations
- **Act** safely and deterministically — publish-time gates, runtime guards, policy enforcement
- **Explain and prove** why — evidence, negative evidence, audit trail, decision records
- **Prove retrospectively** what was true at any prior point in time — immutable snapshots, pinned references, deterministic replay
- **Degrade gracefully** when uncertain — disambiguation, escalation, partial plans with explicit gaps
- **Govern continuously** — stewardship, change control, classification coverage, regulatory traceability, data quality, retention, promotion of maturing semantics

This is a runtime-grade control plane for an agent that manages institutional onboarding and KYC end-to-end. Every capability described in this document exists because the agent, a deterministic gate, a security decision, or a governance function depends on it.

### 1.1 Two audiences, one architecture

This document addresses two communities whose concerns are complementary, not competing:

**Data governance** needs assurance that the Semantic OS embeds their discipline as a structural property of the platform — not a bolt-on compliance layer, not a dashboard alongside the system, but woven into the definitions, gates, lineage, security labels, and audit mechanisms that the platform enforces automatically. Governance professionals should be able to trace any data point from its definition through its classification, policy attachment, security label, lineage, evidence chain, and retention rule — and should see that this traceability is not optional but enforced. Critically, governance should see that the architecture distinguishes governed (audit-grade) from operational (convenience) semantics, that every definition is an immutable snapshot with full version history, and that point-in-time reconstruction is a direct lookup — not a best-effort replay.

**Application development and engineering** needs assurance that the Semantic OS provides a reliable, queryable substrate for building platform surfaces — form generation driven by attribute definitions and constraints, data-driven UI layout via view definitions and edge classifications, graph visualisation via taxonomy grouping and layout hints, workflow orchestration (BPMN/DMN) via verb contracts and precondition/postcondition chains, decision automation via policy predicates, and derived/composite attributes via explicit derivation specs. Engineers should see that registry objects carry sufficient metadata to drive rendering, validation, orchestration, and derivation without encoding domain knowledge in application code — and that operational-tier semantics can iterate at development speed without triggering governed-tier review cycles. The immutable snapshot model means engineers never face "what version was active when that bug occurred?" — every execution pins the snapshot versions it operated against.

---

## 2. What This Is — and How It Differs

### 2.1 What this IS

**An executable semantic substrate for agentic control, security enforcement, and continuous governance.** Every object exists to answer a question that the agent, a gate, a security decision, or a governance function must answer:

- "What attributes does this verb read and write?" → Verb Dictionary
- "Is this entity's UBO chain complete for this jurisdiction and risk tier?" → Entity Model + Policy Registry + Evidence Registry
- "What must be masked for this actor and purpose?" → Security Framework + ABAC
- "What should the analyst see right now?" → View Definitions + Taxonomy + Context Resolution
- "Why did the agent choose this action over that one?" → Decision Records + Evidence + Negative Evidence
- "Can this workflow step fire?" → Preconditions + Policy Gates + State Snapshot
- "What did the platform believe about this entity six months ago, and what definitions and policies were in force?" → Immutable Snapshots + Pinned References
- "Is this attribute safe to use as regulatory proof?" → Governance Tier + Trust Class
- "If this source is invalidated, what downstream assertions and derivations are affected?" → Lineage + Impact Analysis

If a registry object cannot be traced to a runtime question, a security decision, or a governance question, it does not belong here.

**An immutable, snapshot-based semantic store.** Registry objects are never updated in place. Every change produces a new immutable snapshot. "Current" is a pointer to the latest active snapshot, not a mutable record. This is not a versioning strategy — it is the fundamental data architecture that makes audit, compliance, and deterministic replay possible.

**A publish-time enforcement surface.** At `ob publish`, contracts reconcile, types check, security labels validate, policies attach, continuation paths verify, and classification coverage enforces. Violations reject the publish.

**A shared context model for multiple consumers.** Agent, UI, CLI, workflow engine, and governance surfaces all consume the same Context Resolution contract.

### 2.2 What this is NOT delivered as (but subsumes)

The Semantic OS includes capabilities commonly associated with 2024 tooling, but does not deliver them as disconnected products alongside the system:

- **Not delivered as a standalone data catalog** (Collibra/Erwin/Alation). The Semantic OS subsumes definitions, classifications, lineage, and stewardship as runtime self-knowledge enforced by gates. Any governance team can query it — and the answers are guaranteed to reflect the actual running system.
- **Not delivered as a standalone MDM platform.** It governs and constrains operational entity data creation and transformation through the meta-model. It does not attempt enterprise golden-record reconciliation.
- **Not delivered as an instance knowledge graph.** It is primarily a schema-level semantic graph. Instance-level entity graphs are downstream consumers of the meta-model.
- **Not delivered as "just RAG" or a prompt library.** Embeddings are derived projections. Deterministic semantics are primary; semantic search is secondary and explainable.
- **Not delivered as a standalone governance or security tool.** Governance and security are embedded as enforcement: gates, audit, evidence-grade semantics, ABAC, and tiered governance rigour.
- **Not an implementation plan.** This document defines capabilities and outline object shapes, not technology choices or delivery phases.

### 2.3 The 2024 → 2026 pivot

| Concern | 2024 Pattern | 2026 Semantic OS Pattern |
|---------|-------------|------------------------|
| Primary consumer | Human browser / governance analyst | Agent + deterministic gates + governance (automated) |
| Coupling to runtime | Weak (descriptive) | Strong (prescriptive, enforced) |
| Data architecture | Mutable records + audit log | Immutable snapshots; point-in-time by direct lookup |
| Security model | App-layer RBAC + policy documents | Semantic ABAC: labels + purpose + residency + handling |
| Governance model | Workflow outside the system | Gates and proofs inside the platform; tiered rigour |
| Lineage | Static diagrams, manually updated | Derived from verb execution; snapshot-pinned; queryable |
| Derived fields | Ad-hoc SQL views, undocumented | First-class DerivationSpec with security inheritance and evidence-grade rules |
| "Catalog" | Separate product | Subsumed function; always in sync with runtime |

---

## 3. Foundational Model: Immutable Snapshot Architecture

Before describing individual components, we define the data architecture that underpins the entire Semantic OS. This is the most fundamental architectural decision in the system — the governed/operational boundary (section 4), the security framework (section 5), the gate model (section 12), and the audit capability all depend on it.

### 3.1 The principle

**Registry objects are never updated in place. Every change produces a new immutable snapshot.**

"Current" is a pointer to the latest active snapshot for a given object identity — not a mutable record. Previous snapshots are retained, queryable, and referenceable. The Semantic OS does not have an "update" operation; it has "publish new snapshot" and "deprecate/retire previous snapshot."

This is not a versioning convenience. It is the architectural property that makes the following requirements achievable:

- **Compliance-grade point-in-time reconstruction**: "What did attribute X's definition look like when we onboarded Client Y in Q3 2025?" is answered by a direct lookup against the snapshot that was active at that date — not by replaying an audit log.
- **Decision provenance**: every decision record, every verb execution, every derivation evaluation pins the exact snapshot versions of the definitions, contracts, and policies that were in force. The proof chain is immutable and complete.
- **Deterministic replay**: given the same input data and the same snapshot versions, the same derivation produces the same output. Always. This is essential for audit, debugging, and regulatory review.
- **Safe promotion**: operational → governed promotion produces a new snapshot. The previous operational snapshot remains as historical fact. You can always answer "when was this promoted, what did it look like before, and what changed?"
- **Non-destructive deprecation**: deprecated objects are not deleted or modified. They are superseded by new snapshots with `status = Deprecated`. Queries against historical dates still find the original definitions.

### 3.2 Snapshot identity model

Every registry object has two identifiers:

- **Object identity** (`attribute_id`, `verb_id`, `policy_id`, etc.): stable across all snapshots. This is "the concept" — e.g., the attribute `beneficial_owner_percentage` as a persistent identity.
- **Snapshot identity** (`snapshot_id`): unique to each immutable version. This is "the concept as defined at this point in time."

"Current" resolution: given an object identity and a point in time (defaulting to now), return the snapshot that was active at that time. This is the fundamental query primitive of the Semantic OS.

### 3.3 What a snapshot contains

Every snapshot carries:

- The full object definition (not a diff from the previous version)
- `snapshot_id`: unique, immutable
- `object_id`: stable identity
- `version`: semantic version (major.minor for breaking/non-breaking distinction)
- `status`: Draft | Active | Deprecated | Retired
- `effective_from`: when this snapshot became the active definition
- `effective_until`: when this snapshot was superseded (null if current)
- `predecessor_snapshot_id`: the snapshot this one supersedes (null if first)
- `change_type`: Created | NonBreaking | Breaking | Promotion | Deprecation | Retirement
- `change_rationale`: human-readable reason for the change
- `changed_by`: actor identity
- `approved_by`: approval authority (required for governed-tier snapshots; operational-tier snapshots use auto-approve semantics — still recorded for traceability, but no human approval gate required by default)

Full definitions (not diffs) ensure that any snapshot is self-contained and interpretable without reconstructing a chain. This is a deliberate trade-off: modest storage cost in exchange for O(1) point-in-time queries and zero reconstruction risk.

### 3.4 How snapshots participate in execution

**Verb execution** pins snapshot versions. When a verb fires, the execution record captures the snapshot IDs of:

- The verb contract that was invoked
- The attribute definitions that were read and written
- The policy rules that were evaluated
- The preconditions that were checked

This means that months or years later, you can reconstruct exactly what the system understood at the moment of execution — not what it understands now.

**Derivation evaluation** pins snapshot versions. When a derived attribute is computed, the derivation record captures the DerivationSpec snapshot and the input attribute definition snapshots. Given the same inputs and the same snapshots, the same output is produced. This is deterministic replay.

**Context Resolution** operates against current active snapshots by default. But governance and audit modes can supply a `point_in_time` parameter, resolving against the snapshots that were active at that date. This enables: "given the registry state on March 15th, what would Context Resolution have returned for this case?"

**Decision records** pin everything. An agent decision record references the specific snapshots of every definition, contract, policy, and evidence item that contributed to the decision. The decision is reproducible and auditable with zero ambiguity about "which version of the rules applied."

### 3.5 Lifecycle transitions as snapshots

Lifecycle changes are not field updates — they are new snapshots:

| Transition | What happens |
|---|---|
| Draft → Active | New snapshot with `status = Active`, `effective_from = now`. Publish-time gates evaluate this snapshot. |
| Active → Active (non-breaking change) | New snapshot with incremented minor version, `predecessor_snapshot_id` pointing to previous. Previous snapshot gets `effective_until = now`. |
| Active → Active (breaking change) | New snapshot with incremented major version. Publish-time gates check for downstream compatibility. |
| Active → Deprecated | New snapshot with `status = Deprecated`, `effective_until = now`. Object excluded from Context Resolution but remains queryable for historical purposes. |
| Deprecated → Retired | New snapshot with `status = Retired`. Object excluded from all active queries but retained in archive. |
| Operational → Governed (promotion) | New snapshot with `governance_tier = Governed`, `change_type = Promotion`. All governed-tier gates apply to this new snapshot. Previous operational snapshot retained as historical fact. |

### 3.6 Implications for storage and query

Full-definition snapshots are larger than diffs, but the trade-off is justified:

- **Query simplicity**: "show me the definition at time T" is a single indexed lookup, not a fold over a change log.
- **Audit integrity**: each snapshot is independently verifiable. No chain of diffs to reconstruct and validate.
- **Replication safety**: snapshots can be replicated, archived, and exported without dependency on a complete ordered history.
- **Compaction**: old snapshots of Retired objects can be moved to cold storage after regulatory retention periods elapse. The retention policy framework (section 9.7) governs this.

Implementation may choose to compress or deduplicate storage, but the logical model is always full snapshots. Compression is an optimisation; immutability is the invariant.

---

## 4. Foundational Model: Governed vs. Operational Semantics

### 4.1 The problem this solves

Not all semantics carry equal regulatory weight. An attribute used in a Suspicious Activity Report is fundamentally different from an attribute used to route work to the right operations team. Treating them with identical governance rigour produces two failure modes:

- **Under-governance**: operational fields escape scrutiny and silently become evidence or decision inputs, creating unauditable assertions.
- **Over-governance**: every field change triggers full steward review, policy attachment, and classification cycles — producing governance drag that throttles legitimate iteration.

### 4.2 Above the Line — Governed semantics

Governed semantics are regulatory-critical. They support KYC/AML assertions, satisfy evidence requirements, feed policy predicates, and produce audit-grade outcomes. Full governance rigour applies:

- Stewardship assignment required
- Taxonomy classification required
- Policy attachment required in regulated contexts
- Regulatory traceability required where applicable
- Review-cycle compliance enforced
- Evidence-grade constraints on derived attributes enforced

### 4.3 Below the Line — Operational semantics

Operational semantics are convenience and servicing fields. They drive workflow automation, operational setup, case-handling acceleration, and planning hints. Lighter governance applies:

- Stewardship encouraged but not gate-enforced at publish time
- Taxonomy classification encouraged but not gate-enforced
- May iterate at development speed without governed-tier review cycles

### 4.4 The Proof Rule

**The single most important governance invariant.**

Only governed semantics may satisfy:

- Policy evidence requirements
- Audit-grade assertions ("we know X because of evidence Y")
- Regulated decision predicates

Operational semantics may drive workflow, accelerate handling, and assist planning — but must never silently become proof or enter regulated predicates without explicit promotion.

This is enforced structurally:

- **Publish time**: governed policy predicates cannot reference operational-tier attributes
- **Runtime**: Context Resolution flags operational candidates as non-proof and excludes them from strict/normal evidence modes
- **Evidence requirements**: `governance_tier_minimum = Governed` on evidence requirement objects ensures only governed sources satisfy policy

### 4.5 Governance Tier and Trust Class

Every key registry object declares:

- **governance_tier**: Governed | Operational
- **trust_class**: Proof | DecisionSupport | Convenience

| trust_class | May satisfy evidence requirements | May feed policy predicates | May drive workflow/UI | May assist agent planning |
|---|---|---|---|---|
| Proof | Yes | Yes | Yes | Yes |
| DecisionSupport | No | Yes (only where PolicyRule.enforcement = Soft) | Yes | Yes |
| Convenience | No | No | Yes | Yes (flagged as non-proof) |

**Invariant**: Operational tier objects cannot have trust_class = Proof.

### 4.6 Both tiers are secured

Security labels apply to both tiers. An operational PII field is still masked, still residency-constrained, still export-controlled. What differs is governance workflow rigour and allowable usage in regulated contexts — not security posture.

### 4.7 Promotion path

Operational → Governed promotion produces a new snapshot (per section 3.5) with:

- Steward approval
- Policy and evidence linkage checks
- Classification into at least one taxonomy
- Impact analysis
- Audit promotion record capturing rationale, approver, and effective date
- Updated review-cycle assignment

Promotion is a one-way ratchet: governed semantics do not demote to operational. If a governed field is no longer needed, it is deprecated and retired through the standard lifecycle.

---

## 5. Cross-cutting: Data Classification & Security Framework

In a gSIFI KYC environment, meaning and permission are inseparable. The agent can only operate safely if every semantic object and every access path is evaluated against security semantics.

### 5.1 Security dimensions

- **Confidentiality & sensitivity** — PII, restricted data, credentials
- **Jurisdiction and regional handling** — e.g., Saudi elevated handling requirements
- **Purpose limitation** — CDD vs. EDD vs. sanctions screening vs. tax vs. audit
- **Residency constraints** — where data may be stored and processed
- **Handling controls** — masking, secure-only surfaces, dual-control, export blocks, LLM exclusion
- **Audit strength** — standard, high, or forensic logging

### 5.2 SecurityLabel

```
SecurityLabel {
    confidentiality:        Public | Internal | Confidential | Restricted
    data_category:          None | PII | SPI | Financial | Sanctions |
                            Credentials | Other
    jurisdiction_tags[]:    [UK, EU, SA, …]
    regional_handling[]:    [SAUDI_ELEVATED, …]
    purpose_tags[]:         [KYC_CDD, KYC_EDD, SANCTIONS, TAX, AUDIT, …]
                            // Allowed/intended purposes, evaluated via
                            // ABAC intersection with ActorContext. NOT a
                            // hard single-purpose lock unless a specific
                            // PolicyRule demands it. An empty list means
                            // "no purpose restriction" (general use).
    residency_class:        Global | EEA | UK | SA | ClientRegionBound
    handling_requirements[]: [MaskByDefault, NoExport, DualControl,
                             SecureViewerOnly, NoLLMExternal, …]
    audit_level:            Standard | High | Forensic
}
```

SecurityLabels are carried on every registry object. They are part of the snapshot — when a security label changes, a new snapshot is produced.

### 5.3 ABAC decisions

```
ActorContext {
    actor_type:             Agent | Analyst | System | GovernanceReviewer
    roles[]
    clearance_level
    jurisdiction_scope[]
    allowed_purposes[]
    client_assignment_scope?
}

AccessDecision {
    verdict:                Permit | Deny | Escalate | PermitWithMasking
    reason_codes[]
    applied_rules[]:        PolicyRule snapshot references
    masking_plan?:          { attribute_id → masking_strategy }
    export_controls?:       { channel → allowed | denied }
    residency_constraints?: { region → allowed | prohibited }
    required_controls?:     [DualControl, Approval, SecureViewerOnly, …]
    audit_level
}
```

Security decisions are usable as agent planning inputs, not only as post-hoc denials. The agent can ask "what can I see and do in this context?" before attempting an action. AccessDecision references specific PolicyRule snapshots, making the security verdict reproducible.

### 5.4 Security inheritance on derivations

Derived attributes inherit security from their inputs:

- **Confidentiality**: most restrictive of all inputs
- **Residency**: most restrictive of all inputs (always strict, no override)
- **Handling requirements**: union of all inputs
- **Purpose tags**: intersection of inputs

Overrides that make the derived output *less restrictive* than the inherited default are exceptional. They require steward approval, explicit written rationale, and produce a new DerivationSpec snapshot with the override audit-logged. A less-restrictive override must never be the default path — it exists for cases where the derivation genuinely reduces sensitivity (e.g., an aggregation that anonymises individual PII into a population statistic) and the rationale is defensible under regulatory scrutiny. Overrides that *tighten* security beyond the inherited default are simply new declarations and do not require special approval.

### 5.5 Security inheritance through verb side-effects

Verb contracts that write attributes must declare security implications of those writes. A verb running under purpose KYC_CDD cannot produce output labelled SANCTIONS-only without explicit policy authorisation. This prevents security label laundering — data entering under one classification, passing through a verb, and emerging with a less restrictive classification.

---

## 6. Cross-cutting: Governance Framework

Every registry object carries structural governance properties. These are not a separate registry — they are fields on every snapshot, enforced by publish-time gates.

### 6.1 Governance properties

- **Stewardship**: named owner responsible for accuracy, currency, and appropriateness
- **Lifecycle**: Draft → Active → Deprecated → Retired (each transition is a new snapshot)
- **Change control**: every change produces a new snapshot with actor, timestamp, rationale, and approval reference
- **Classification obligation**: governed objects require taxonomy membership before publish
- **Regulatory traceability**: governed PolicyRules carry explicit regulatory references
- **Retention and disposal**: evidence objects carry retention policies
- **Data quality semantics**: attribute constraints enforced at write time

### 6.2 Tier-aware governance posture

| Governance concern | Governed tier | Operational tier |
|---|---|---|
| Stewardship | Required (publish-time gate) | Encouraged (flagged, not gated) |
| Taxonomy classification | Required (publish-time gate) | Encouraged |
| Policy attachment | Required in regulated contexts | Not required |
| Regulatory traceability | Required where applicable | Not applicable |
| Review-cycle compliance | Enforced | Not enforced |
| Security labels | Required (both tiers) | Required (both tiers) |
| Evidence-grade constraints | Enforced on derived attributes | evidence_grade = Prohibited |
| Change velocity | Controlled (snapshots with steward review) | Development speed (snapshots with lighter review) |

---

## 7. The Semantic OS Components

### 7.1 Component Map

```
┌──────────────────────────────────────────────────────────────────────┐
│                      SEMANTIC OPERATING SYSTEM                       │
│                                                                      │
│  ╔════════════════════════════════════════════════════════════════╗   │
│  ║  FOUNDATIONAL ARCHITECTURE                                   ║   │
│  ║  Immutable snapshots · Governed/Operational tiers             ║   │
│  ║  Security (ABAC, labels, residency, purpose, handling)       ║   │
│  ║  Governance (stewardship, lifecycle, change control, tiers)  ║   │
│  ╚════════════════════════════════════════════════════════════════╝   │
│                                                                      │
│  ┌─────────────────┐  ┌──────────────────┐  ┌───────────────────┐   │
│  │ 1. Attribute     │  │ 2. Entity &      │  │ 3. Verb           │   │
│  │    Dictionary    │──│    Relationship   │──│    Dictionary     │   │
│  │  (incl. derived  │  │    Model          │  │    (Contracts)    │   │
│  │   & composite)   │  │                   │  │                   │   │
│  └────────┬─────┬──┘  └────────┬─────────┘  └───┬──────────┬────┘   │
│           │     │              │                 │          │        │
│  ┌────────▼─────▼──────────────▼─────────────────▼──┐       │        │
│  │ 4. Taxonomy Registry                             │       │        │
│  │    (classification + clustering + layout roles)   │       │        │
│  └────────┬─────────────────────────────────────────┘       │        │
│           │                                                 │        │
│  ┌────────▼─────────────────┐  ┌────────────────────────────▼───┐   │
│  │ 5. View Definitions      │  │ 6. Policy & Controls Registry  │   │
│  │    (context projections) │  │    (enforcement, not docs)      │   │
│  └────────┬─────────────────┘  └────────────────┬───────────────┘   │
│           │                                     │                   │
│  ┌────────▼─────────────────────────────────────▼───────────────┐   │
│  │ 7. Source & Evidence Registry                                │   │
│  │    (provenance, documents, observations, proofs)             │   │
│  └──────────────────────────┬───────────────────────────────────┘   │
│                             │                                       │
│  ══════════════════════════════════════════════════════════════     │
│  DERIVED PROJECTIONS  (never sources of truth; rebuildable)         │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ • Lineage & derivation graph (snapshot-pinned)               │   │
│  │ • Embeddings / vector indices (staleness-tracked, policy-    │   │
│  │   bound, NoLLMExternal enforced)                             │   │
│  │ • Read models for UI (CQRS flattened projections)            │   │
│  │ • Governance + security coverage metrics                     │   │
│  └──────────────────────────┬───────────────────────────────────┘   │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐   │
│  │              CONTEXT RESOLUTION API                          │   │
│  │  (single contract: agent, UI, CLI, workflow, governance)     │   │
│  │  (default: current active snapshots; audit: point-in-time)   │   │
│  └──────────────────────────┬───────────────────────────────────┘   │
│                             │                                       │
│  ┌──────────────────────────▼───────────────────────────────────┐   │
│  │              AGENT MANAGER CONTROL PLANE                     │   │
│  │  (plans, decisions, escalations — all snapshot-pinned)       │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
```

**Structural boundary**: registries contain authoritative immutable snapshots. Derived projections are rebuildable and disposable. If a projection diverges from registry state, the projection is wrong.

---

## 8. Universal Contract: Context Resolution

Every consumer interacts with the Semantic OS through one contract.

### 8.1 ContextResolutionRequest

```
ContextResolutionRequest {
    subject:                case_id | entity_id | document_id |
                            task_id | view_id
    intent?:                natural language (optional)
    current_state_snapshot:  references to relevant state nodes
    actor:                  ActorContext
    goals?:                 [resolve_ubo, collect_proof, …]
    constraints:            { jurisdiction, thresholds, risk_posture }
    evidence_mode:          Strict | Normal | Exploratory | Governance
    point_in_time?:         timestamp (default: now)
}
```

The `point_in_time` parameter enables historical resolution: "given the registry state at this date, what would the response be?" When omitted, resolution operates against current active snapshots.

### 8.2 ContextResolutionResponse

```
ContextResolutionResponse {
    as_of_time:                   timestamp  // the point-in-time that was
                                             // resolved (from request, or now)
    resolved_at:                  timestamp  // when resolution was computed
    applicable_view_definitions:  ViewDef[] (ranked, snapshot-pinned)
    candidate_verbs:              VerbCandidate[] (ranked)
        // each: parameter hints, precondition status,
        // governance_tier, trust_class, usable_for_proof,
        // verb_contract_snapshot_id
    candidate_attributes:         AttributeCandidate[] (ranked)
        // each: missing/required flags, governance_tier,
        // trust_class, attribute_snapshot_id
    required_preconditions:       Precondition[] with remediation hints
    disambiguation_questions:     DisambiguationPrompt[]
    evidence:                     { positive[], negative[] }
    policy_verdicts:              PolicyVerdict[]
        // each: snapshot_id of policy rule applied,
        // regulatory reference, blocked/required/permitted
    security_handling:            AccessDecision
    governance_signals:           GovernanceSignal[]
}
```

Every reference in the response is snapshot-pinned. The consumer (agent, UI, governance reviewer) can reconstruct exactly which versions of which definitions drove the response.

### 8.3 Trust-aware behaviour

- **Strict and Normal modes** prioritise Governed + Proof/DecisionSupport. Operational + Convenience appear only when the view explicitly includes operational aids, and are always flagged as non-proof.
- **Exploratory mode** includes all candidates across both tiers with annotations.
- **Governance mode** focuses on coverage, stewardship, classification, and policy-attachment assessment.

### 8.4 Failure and degradation

Normal operation, not exceptional error:

- **Confidence levels** on all ranked candidates
- **Gap identification**: "to proceed with verb X, you need attribute Y"
- **Partial plans**: "steps 1–3 executable now; step 4 requires human input"
- **Escalation signals**: "I need a human decision on X — here is context"
- **Governance alerts**: unowned definitions, unclassified attributes, expired evidence
- **Security constraints as planning inputs**: "you cannot perform verb V due to residency constraint R — here are alternatives"

---

## 9. Capability Catalogue

Each capability is described as a function the agent, a gate, a security decision, or a governance function depends on. All outline data objects are snapshottable — every object listed below participates in the immutable snapshot model (section 3).

### 9.1 Attribute Dictionary

**Function**: canonical definitions of every data point used anywhere in the platform.

```
AttributeDef {
    attribute_id                    // stable identity
    snapshot_id                     // unique to this version
    name, display_name, description
    kind:           Primitive | Captured | ExternalSourced |
                    Derived | Composite
    type_spec
    constraints[]:  AttributeConstraint[]
    governance_tier, trust_class
    security_label: SecurityLabel
    jurisdiction_tags[]
    aliases[]:      AttributeAlias[]
    examples[]
    steward, last_reviewed?
    status, version
    effective_from, effective_until?
    predecessor_snapshot_id?
    change_type, change_rationale, changed_by, approved_by?
}
```

**What the agent needs**: type, constraints, sensitivity, jurisdictional restrictions, evidence eligibility, derivation inputs, common analyst names.

**What governance needs**: classification status, steward identity, last review date, data quality rules, sensitivity and security classification, GDPR applicability, tier assignment, version history.

**What engineering needs**: `type_spec`, `constraints[]`, `kind`, `aliases[]` — sufficient metadata for form generation, validation rules, display labels, and conditional visibility without hardcoded domain knowledge.

### 9.2 Entity & Relationship Model

**Function**: the agent's world model — what entity types exist and how they connect.

```
EntityTypeDef {
    entity_type_id, snapshot_id
    name, description
    attributes[]:       { attribute_id, role, required }
    identity_keys[], lifecycle_states[]
    governance_tier, security_label
    steward, status, version
    // snapshot metadata fields
}

RelationshipTypeDef {
    rel_type_id, snapshot_id
    name, description
    source_entity_type, target_entity_type
    edge_class:         Structural | Derivation | Reference |
                        Association | Temporal
    directionality, cardinality
    constraints[], semantics
    governance_tier, security_label?
    steward, version
    // snapshot metadata fields
}
```

**Edge classes** drive layout semantics:

| Edge class | Semantic meaning | Layout implication |
|---|---|---|
| Structural | Ownership, corporate hierarchy | Hierarchical top-down |
| Derivation | Computed-from relationships | Data-flow directed |
| Reference | Document linkages, evidence | Secondary, no flow |
| Association | Related entities, cross-references | Proximity, no direction |
| Temporal | Sequence (request → response → validation) | Timeline / swimlane |

**Clarification**: this is a meta-model, not an entity store. It defines what is valid. Instance data lives in the operational tables.

### 9.3 Verb Dictionary (Executable Contracts)

**Function**: first-class contracts for every platform action. Enables safe planning, static analysis, explainable execution, and workflow composition.

```
VerbContract {
    verb_id, snapshot_id
    canonical_name, display_name, description
    exec_mode:          Sync | Research | DurableStart | DurableResume
    inputs[]:           VerbIO[]
    outputs[]:          VerbIO[]
    side_effects[]:     SideEffect[]
    preconditions[]:    Precondition[]
    postconditions[]:   Postcondition[]
    continuation?:      ContinuationContract
    expansion?:         ExpansionContract
    governance_tier, trust_class
    security_label
    steward, status, version
    // snapshot metadata fields
}

Precondition {
    predicate_expression, description
    enforcement:        Hard | Soft
    remediation_hint
}

Postcondition {
    invariant_expression, description
    compliance_tag
}

ContinuationContract {
    correlation_key_spec, resume_signal
    timeout_policy, resume_verb_id
}

ExpansionContract {
    macro_verb_id, expands_to_verb_ids[]
    mapping_notes
}
```

**Preconditions vs. policies**: a precondition is intrinsic to the verb ("entity must have at least one registered address"). A policy rule is extrinsic ("PEP screening must complete before approval in EMEA"). Preconditions travel with the verb across all contexts; policies are context-dependent. Litmus test: "does this constraint apply regardless of jurisdiction?" If yes, precondition. If it depends on context, policy.

### 9.4 Taxonomy Registry

**Function**: overlapping DAG taxonomies for classification, navigation, candidate narrowing, layout grouping, and governance coverage measurement.

```
Taxonomy {
    taxonomy_id, snapshot_id
    name, purpose, description, node_scheme
    governance_owner, review_cycle
    status, version
    // snapshot metadata fields
}

TaxonomyNode {
    node_id, taxonomy_id, snapshot_id
    path, label, description
    parents[], metadata
}

Membership {
    subject_type, subject_id
    taxonomy_id, node_id
    weight
    layout_role?:   Group | Container | Swimlane | Hidden | null
    notes
}
```

**Canonical taxonomies** (initial set — minimal, expanded under governance):

- **KYC Review Navigation** — Entity Identity, Ownership & Control, Evidence & Proofs, Sanctions / Adverse Media, Share-class / Voting Rights, Exceptions / Escalations (DAG — nodes participate in multiple branches)
- **Regulatory Domain** — AML, CDD, EDD, sanctions, tax, FATCA/CRS
- **Data Sensitivity** — PII, confidential, restricted, internal, public
- **Execution Semantics** — verbs classified by execution pattern and side-effect profile

### 9.5 View Definitions

**Function**: context projections defining what matters for a given consumer. Unifies agent reasoning, rendering, workflow step availability, and governance review scoping.

```
ViewDef {
    view_id, snapshot_id
    name, description
    taxonomy_slices[]
    primary_edge_class
    layout_strategy_hint:   Hierarchical | ForceDirected |
                            Swimlane | Tabular | Timeline
    verb_surface[]:         VerbSurfaceEntry[]
    attribute_prominence[]: { attribute_id, weight }
    filters[]:              ViewFilter[]
    includes_operational?:  boolean
                            // Controls whether operational-tier objects
                            // appear in this view's Context Resolution
                            // responses under Strict/Normal modes.
                            // evidence_mode alone does not control this;
                            // the view must explicitly opt in.
    view_security_profile?
    governance_tier
    steward, status, version
    // snapshot metadata fields
}
```

| View | Taxonomy slice | Primary edge | Layout | Verb surface |
|------|---------------|-------------|--------|-------------|
| UBO Discovery | Ownership & Control + Entity Identity | Structural | Hierarchical | ubo.resolve, ubo.confirm, evidence.request |
| Sanctions Screening | Sanctions / Adverse Media + Entity Identity | Association | Force-directed | screening.check, screening.escalate, screening.clear |
| Proof Collection | Evidence & Proofs | Temporal | Swimlane | proof.request, proof.receive, proof.validate |
| Case Overview | All | Structural | Hierarchical | Filtered by case state and role |
| Governance Review | All | Classification | Tabular | review, approve, flag, reassign |
| Operational Setup | Servicing slices | Structural | Tabular | servicing.assign, servicing.configure |

### 9.6 Policy & Controls Registry

**Function**: machine-checkable constraints producing explicit blocked/required/permitted outcomes. Enforcement, not documentation.

```
PolicyRule {
    policy_id, snapshot_id
    name, description
    scope:              Verb | Attribute | EntityType | View |
                        DocumentType | Jurisdiction
    predicate
    predicate_trust_minimum: DecisionSupport | Proof
                        // Proof for hard regulated predicates;
                        // DecisionSupport for soft advisory predicates.
                        // Mechanically enforces which trust_class
                        // of attributes may participate in this
                        // rule's predicate evaluation.
    enforcement:        Hard | Soft
    remediation_hint
    evidence_requirements[]: EvidenceRequirement[]
    jurisdiction_tags[]
    security_implications?
    regulatory_reference: RegulatoryReference
    effective_date, review_date
    steward, version
    // snapshot metadata fields
}

EvidenceRequirement {
    required_doc_type?, required_attribute?
    acceptable_sources[]
    freshness_window, confidence_threshold?
    governance_tier_minimum: Governed
    trust_class_minimum:    Proof      // default; mechanically enforces
                                        // the Proof Rule — no interpretation
}

RegulatoryReference {
    regulation_id, regulation_name, section
    jurisdiction, effective_date, sunset_date?
    uri?
}
```

**Regulatory traceability** enables forward trace ("regulation changed — what is affected?") and reverse trace ("this verb was blocked — which regulation?"). Both traces operate against snapshot-pinned policy rules, so you can answer "which version of the policy was in force when this decision was made?"

### 9.7 Source & Evidence Registry

**Function**: provenance as first-class. What data came from where, with what confidence, at what time, under what retention obligations.

```
DocumentTypeDef {
    doc_type_id, snapshot_id
    name, description
    required_fields[], acceptable_formats[]
    retention_policy:   RetentionPolicy
    policies[]
    steward
    // snapshot metadata fields
}

DocumentInstance {
    doc_id, doc_type_id, storage_ref
    extracted_fields[], source_actor
    received_at, validated_at, expiry
    retention_until
    security_label, status
}

Observation {
    obs_id
    subject_ref, attribute_id, value_ref
    source, confidence, timestamp
    supporting_doc_ids[]
    governance_tier, security_label
    supersedes?:        obs_id
}

RetentionPolicy {
    policy_id, doc_type_id
    retention_window, disposal_method
    regulatory_basis, jurisdiction
}
```

**Observation supersession**: new observations supersede, never overwrite. The chain of superseded observations is the attribute-level audit trail. Combined with the snapshot model, this means: "what did we believe about Entity X's ownership at time T?" is answered by finding the active observation at T, then looking up the attribute definition snapshot that was active at T to understand what the field meant and what constraints applied.

**Retention and snapshot compaction**: retention policies govern how long evidence and document instances must be retained. They also govern when Retired registry snapshots can be moved to cold storage. Snapshots referenced by active decision records or open cases are retained regardless of the object's retirement status — you cannot archive a snapshot that is part of an active proof chain.

### 9.8 Derived & Composite Attribute System

**Function**: explicit derivation recipes with security inheritance, evidence-grade constraints, and promotion paths. Prevents the common pattern where derived fields are created ad-hoc and silently become regulatory inputs.

```
DerivationSpec {
    derivation_id, snapshot_id
    output_attribute_id
    inputs[]:               { attribute_id, role, required }
    expression_ast | query_plan | function_ref
    null_semantics:         NullOnAnyNull | NullOnAllNull |
                            DefaultValue | Error
    freshness_rule?:        { max_staleness, refresh_trigger }
    security_inheritance:   Strict | DeclaredOverride
    residency_inheritance:  Strict  // always strict
    evidence_grade:         Prohibited | AllowedWithConstraints
    tests[]:                { input_fixture, expected_output }
    steward, status, version
    // snapshot metadata fields
}
```

**Evidence-grade rule**: operational derived/composites are `evidence_grade = Prohibited`. Governed derived attributes are `AllowedWithConstraints` only when policy-linked, constraints satisfied, and tests pass.

**Designer capability** (requirement, not implementation prescription): the Semantic OS must support authoring DerivationSpec objects with type checking, cycle detection, impact analysis, test attachment, promotion workflow, deterministic evaluation, and security label preview. Because DerivationSpecs are snapshottable, every version of a derivation recipe is retained — you can always answer "what formula produced this value at time T?"

---

## 10. Agent Manager Control Plane

The registries define the world. The control plane defines how the agent operates — planning, decision-making, escalation, and accountability. All control plane records are immutable and snapshot-pinned.

### 10.1 Planning

```
AgentPlan {
    plan_id, case_id, goal
    context_resolution_ref:  snapshot-pinned response
    steps[]:                PlanStep[]
    assumptions[]
    risk_flags[]
    security_clearance:     AccessDecision
    created_at, approved_by?, status
}

PlanStep {
    step_id
    verb_id, verb_snapshot_id       // pinned to exact contract version
    params
    expected_postconditions[]
    fallback_steps[]
    depends_on_steps[]
    governance_tier_of_outputs
}
```

### 10.2 Decision records

```
DecisionRecord {
    decision_id, plan_id?, context_ref
    chosen_action
    alternatives_considered[]
    evidence_for[], evidence_against[]
    negative_evidence[]
    policy_verdicts[]:      // each with policy_snapshot_id
    security_handling:      AccessDecision
    tier_trust_annotations
    snapshot_manifest:      { object_id → snapshot_id }[]
        // complete record of every definition version
        // that contributed to this decision
    confidence, escalation_flag
    timestamps
}
```

The `snapshot_manifest` is the decision's proof chain. Given the manifest, the decision can be reconstructed, audited, or replayed against the exact definitions that were in force. No ambiguity about "which version of the rules applied."

### 10.3 Disambiguation and escalation

```
DisambiguationPrompt {
    prompt_id, decision_id
    question, options[]
    evidence_per_option[]
    required_to_proceed, rationale
}

EscalationRecord {
    escalation_id, decision_id
    reason, context_snapshot
    required_human_action
    assigned_to?, resolved_at?, resolution
}
```

### 10.4 What the agent manager is NOT

Not a general-purpose AI orchestration framework. It does not manage model selection, prompt engineering, or inference infrastructure. It is a domain-specific control structure for onboarding cases within the Semantic OS. The Agent Manager is subject to the same governance and security frameworks as every other component.

---

## 11. Derived Projections

Computed from registries, never sources of truth, rebuildable, staleness-tracked. If a projection diverges from registry state, the projection is wrong.

### 11.1 Lineage & Derivation Graph

Snapshot-pinned. Every derivation edge records which definition snapshots were active when the derivation executed. Supports impact analysis: "if this source is invalidated, what downstream assertions are affected?" and temporal queries: "show me the derivation chain as it existed at time T."

### 11.2 Embeddings / Vector Projection

Policy-bound and staleness-tracked. `NoLLMExternal` enforced on the embedding pipeline. Embeddings for restricted attributes use internal models only. The embedding layer is one input to Context Resolution — not the whole of it.

### 11.3 Governance & Security Coverage Metrics

Computed from registry state on demand:

- Classification coverage (governed objects with taxonomy membership)
- Stewardship coverage (governed objects with active stewards)
- Policy attachment (governed verbs/entities with applicable policy rules)
- Evidence freshness (active governed observations within freshness windows)
- Review currency (governed objects reviewed within cycle)
- Retention compliance (evidence within/approaching/exceeding retention windows)
- Regulatory coverage (applicable regulations with implementing policy rules)
- Security label completeness (all objects, both tiers)
- Proof Rule compliance (zero operational attributes in governed evidence requirements)
- Tier distribution (governed vs. operational across registries)
- Snapshot volume (active, deprecated, retired, archived — for capacity planning)

### 11.4 Read Models

CQRS flattened projections for specific UI surfaces. Disposable and rebuildable from registry + operational state.

---

## 12. Determinism Gates

### 12.1 Publish-time gates (`ob publish`) — hard failures

**Always enforced (both tiers)**:

- Type correctness; dependency correctness (no cycles)
- Security label presence
- Residency declarations where applicable
- Verb read/write surface disclosure (ABAC must be evaluable)
- Orphan detection: governed attributes referenced by zero verbs fail publish; operational attributes referenced by zero verbs produce a warning (legitimate for UI-only, reporting, or export fields)
- Macro expansion integrity
- Continuation completeness (DurableStart ↔ DurableResume)
- Snapshot integrity (new snapshot correctly references predecessor)

**Additional for Governed tier**:

- Taxonomy membership required
- Stewardship required
- Policy attachment in regulated contexts
- Regulatory linkage on policy rules
- Review-cycle compliance
- Evidence-grade constraints on derived attributes
- Version consistency (breaking changes require major version bump)

**Operational tier posture**: iterates with lighter review, but cannot bypass security, cannot be proof, cannot enter governed predicates.

### 12.2 Runtime gates — verdicts

- Precondition satisfaction
- Policy enforcement (with regulatory reference, snapshot-pinned)
- Evidence freshness
- Actor permissions (ABAC)
- State guards (entity lifecycle)
- Security handling (Permit / Deny / Escalate / PermitWithMasking)
- Purpose limitation and residency/export controls
- Proof Rule enforcement (operational attributes cannot satisfy evidence requirements)

### 12.3 Gate failures as agent inputs

Structured remediation descriptors:

- "Verb `ubo.confirm` blocked: precondition `entity.has_verified_address` is false. Remediation: execute `address.verify` for entity E-1234."
- "Policy `EMEA-CDD-007` (snapshot s-4821) requires certified UBO declaration. No qualifying document. Freshness: 180 days. Ref: EU 4AMLD Art. 30(1)."
- "Attribute `derived_risk_score` is Operational/Convenience. Cannot satisfy evidence requirement on `RISK-002`. Remediation: promote to Governed or use `assessed_risk_level`."
- "Publish rejected: snapshot for `beneficial_owner_percentage` has no taxonomy membership. Required for Governed tier."

---

## 13. Relationship to Existing ob-poc Schema

The Semantic OS is a meta-layer that describes, governs, secures, and enables reasoning about the existing 92+ table operational schema.

- **Operational tables** store instance data.
- **Semantic OS registries** store immutable snapshots of definitions, contracts, policies, classifications, security labels, and governance metadata.

The relationship is a type system to runtime values. The Semantic OS defines the types; the operational tables hold the values. `ob publish` is the type-checker. Immutable snapshots mean the type system itself has a complete, queryable history.

---

## 14. Acceptance Criteria

### Agent capability

- Context Resolution returns tier-aware, security-aware, snapshot-pinned candidates
- Plans use registry-sourced contracts, preconditions, policies, and security clearances
- Decisions are auditable with snapshot manifests — every contributing definition version recorded
- Operational candidates never silently treated as proof

### Governance capability

- Every governed object has steward, taxonomy membership, and lifecycle state
- Point-in-time reconstruction by direct snapshot lookup (not log replay)
- Regulatory traceability queryable (forward and reverse, snapshot-pinned)
- Promotion from operational to governed is explicit, gated, auditable, and produces a new snapshot
- Coverage metrics computable from registry state

### Security enforcement

- Security labels complete for all published objects (both tiers)
- ABAC decisions snapshot-pinned (reproducible against the policy versions that applied)
- Masking, residency, export controls enforced across both tiers
- NoLLMExternal enforced on projection paths
- Security inheritance on derivations automatic and auditable

### Snapshot architecture

- No registry object is ever modified in place; every change produces a new snapshot
- Point-in-time queries resolve to the correct snapshot in O(1)
- Decision records carry complete snapshot manifests
- Derivation evaluation is deterministically replayable given pinned snapshots
- Retired snapshots are archivable but not deletable while referenced by active proof chains

---

## 15. Key Risks

**Scope perception**: comprehensive by design. This is a vision document, not a build plan.

**Meta-model drift**: publish-time gates are the mitigation. Must be implemented early and made non-bypassable.

**Over-governance**: the tiered model (section 4) is the primary mitigation. The boundary must be actively defended.

**Under-governance by misclassification**: regulated fields classified as operational to avoid overhead. Mitigations: publish-time cross-checks, coverage metrics, periodic steward review.

**Snapshot volume**: full-definition snapshots accumulate. Mitigations: compaction of retired snapshots after retention periods; cold storage for archived snapshots; volume monitoring in coverage metrics. The logical model is always full snapshots; storage optimisation is an implementation concern.

**Snapshot reference integrity**: decision records, derivation records, and execution records all pin snapshot IDs. If snapshot storage is partitioned or archived, these references must remain resolvable. Mitigation: snapshot references are never invalidated; archive operations move data but preserve addressability.

**Security label sprawl**: mitigate with label templates for common patterns rather than hand-crafting each label.

**Derivation complexity**: cycle detection at publish time; impact analysis tooling; freshness enforcement.

**View Definition proliferation**: start with canonical views; expand deliberately.

**Layout hint over-specification**: semantic hints, not rendering instructions.

**Policy vs. precondition confusion**: maintain the intrinsic/extrinsic boundary.

---

## 16. Summary

The Semantic OS is the runtime self-knowledge, security enforcement, and governance plane of ob-poc. It is built on four structural properties:

1. **Immutable snapshots**: registry objects are never updated — every change produces a new snapshot. Point-in-time reconstruction is a direct lookup. Decision provenance is complete. Deterministic replay is guaranteed.
2. **Tiered governance**: the governed/operational boundary concentrates compliance rigour where it matters and preserves development velocity where it is safe, with the Proof Rule as the invariant that prevents silent erosion.
3. **Unified context resolution**: one contract serves every consumer with tier-aware, security-aware, snapshot-pinned, evidence-backed responses.
4. **Structural security**: ABAC, residency, purpose limitation, handling controls, and security inheritance are properties of every registry object, evaluated at every access path, inherited through every derivation.

It subsumes the functions of 2024 catalog, MDM, knowledge graph, and RAG approaches but delivers them as embedded, enforced, immutable semantics for an agentic manager.

It is not a catalog, not a data store, not a RAG pipeline, not a standalone governance or security tool, and not an implementation plan. It is the structured knowledge that makes agentic onboarding possible, safe, provable, and governed — with a complete, immutable history of what the platform believed and why, at every point in its existence.

---

## Appendix A: Version History

| Version | Date | Key Changes |
|---------|------|-------------|
| v0.1 | Jan 2026 | Attribute Dictionary — Architecture Vision & Scope |
| v0.2 | Feb 2026 | Scope expansion to Semantic Registry |
| v0.3 | Feb 2026 | View Definitions, layout semantics, Context Resolution API |
| v0.4 | Feb 2026 | Semantic Operating System reframing |
| v0.5 | Feb 2026 | Vision clarification; failure modes; meta-model relationship |
| v0.6 | Feb 2026 | Governance elevated to first-class cross-cutting framework |
| v0.8 | Feb 2026 | Governed/operational boundary; security ABAC; derived attributes |
| v0.9 | Feb 2026 | Consolidation; security inheritance; Proof Rule as structural invariant |
| v1.0 | Feb 2026 | Immutable snapshot architecture as foundational model; snapshot-pinned execution, derivation, and decisions; point-in-time Context Resolution; retention-aware compaction; snapshot manifest on decision records |
| v1.1 | Feb 2026 | Enforcement precision: trust_class_minimum on evidence requirements and policy predicates (mechanical Proof Rule); snapshot_epoch → as_of_time/resolved_at; auto-approve semantics for operational snapshots; purpose_tags framed as allowed-purposes not hard locks; less-restrictive security override hardened as exceptional; orphan detection tier-aware (fail governed, warn operational); includes_operational semantics clarified |

## Appendix B: Governance Quick Reference

| Governance Concern | Where Addressed | Mechanism |
|---|---|---|
| Stewardship | §6, all objects | `steward` field; publish-time gate for governed tier |
| Governed/operational boundary | §4 | `governance_tier` + `trust_class`; Proof Rule at publish and runtime |
| Promotion | §4.7, §3.5 | New snapshot with tier change; steward approval, classification, impact analysis |
| Classification coverage | §6, §12.1 | Publish-time gate (governed); coverage metrics §11.3 |
| Data quality | §9.1 | Constraints enforced at write time; data_quality_dimension |
| Regulatory traceability | §9.6 | PolicyRule → RegulatoryReference; forward/reverse trace; snapshot-pinned |
| Retention / disposal | §9.7, §3.6 | RetentionPolicy; freshness/expiry tracking; snapshot compaction rules |
| Change control | §3, §6 | Every change = new immutable snapshot; no updates; full history |
| Point-in-time reconstruction | §3 | Direct snapshot lookup; O(1); no log replay |
| Impact analysis | §9.6, §11.1 | Lineage + policy linkage + derivation chains; all snapshot-pinned |
| Audit trail | §3.3, §10.2 | Immutable snapshots; decision records with snapshot manifests |
| Coverage metrics | §11.3 | Computable from registry: classification %, stewardship %, policy %, freshness %, security %, Proof Rule |
| Agent accountability | §10.2 | Decision records with evidence, negative evidence, snapshot manifest |
| Security | §5 | SecurityLabel on every object; ABAC; inheritance; purpose limitation; residency |
| Proof Rule | §4.4, §9.6, §12 | Structural invariant: operational ≠ proof; enforced at publish, runtime, evidence requirements |

## Appendix C: Security Quick Reference

| Security Concern | Where Addressed | Mechanism |
|---|---|---|
| Confidentiality | §5.2 | SecurityLabel.confidentiality on every snapshot |
| PII / sensitive data | §5.2 | data_category; MaskByDefault handling |
| Jurisdictional handling | §5.2 | jurisdiction_tags[], regional_handling[] |
| Purpose limitation | §5.2, §5.3 | purpose_tags[]; ActorContext.allowed_purposes[]; intersection check |
| Residency | §5.2, §5.4 | residency_class; strict inheritance on derivations |
| Export controls | §5.2, §5.3 | NoExport handling; AccessDecision.export_controls |
| Masking | §5.3 | AccessDecision.masking_plan |
| LLM exclusion | §5.2, §11.2 | NoLLMExternal; enforced on embedding pipeline |
| Security inheritance (derivations) | §5.4 | Most-restrictive-of-inputs; override requires steward approval |
| Security inheritance (verb side-effects) | §5.5 | Declared implications; label laundering prevention |
| Access decisions | §5.3 | ABAC: ActorContext × SecurityLabel → AccessDecision; snapshot-pinned |
| Audit strength | §5.2 | audit_level (Standard / High / Forensic) |

## Appendix D: Snapshot Architecture Quick Reference

| Concern | Mechanism | Section |
|---|---|---|
| No updates, only new snapshots | Foundational invariant | §3.1 |
| Object identity vs. snapshot identity | Stable object_id + unique snapshot_id | §3.2 |
| Full definitions, not diffs | Each snapshot is self-contained | §3.3 |
| Point-in-time query | Lookup by object_id + timestamp → active snapshot | §3.2 |
| Execution pinning | Verb execution records capture snapshot IDs of all participating definitions | §3.4 |
| Derivation pinning | Derivation records capture DerivationSpec snapshot + input snapshots | §3.4 |
| Decision pinning | DecisionRecord carries snapshot_manifest | §10.2 |
| Context Resolution history | point_in_time parameter on ContextResolutionRequest | §8.1 |
| Lifecycle transitions | Each transition (activate, deprecate, promote, retire) = new snapshot | §3.5 |
| Compaction / archival | Retired snapshots archivable after retention; never while referenced by active proof chains | §3.6, §9.7 |
| Storage trade-off | Full snapshots cost more storage; gain O(1) queries and zero reconstruction risk | §3.6 |
