# The Stewardship Agent

## Semantic OS Registry Authoring, Attribute Governance, Research-Assisted Change Control, Execution Constraint Surface, Visual Control Loop, and the Stewardship Workbench

**Version:** 1.0.1 — February 2026  
**Author:** Adam / Lead Solution Architect  
**Status:** Implementation-ready conceptual specification  
**Constraint:** No Rust implementation detail; no kernel DB schema deltas (stewardship-layer tables are in scope)  
**Platform:** ob-poc Semantic OS  

---

## Revision History

| Version | Date | Summary |
|---|---|---|
| 0.1 | 2026-02-18 | Initial vision: two-agent model, capabilities, data model, DSL surface, use cases, open questions |
| 0.5 | 2026-02-18 | Workbench as operating model, guardrails (P8), worked journeys (FATCA + Promotion + Deprecation), Basis/Research Pack concept |
| 0.6 | 2026-02-18 | Standalone consolidation. Basis promoted to first-class entity. Guardrail taxonomy specified. Tool surface expanded. Conflict model defined. Template model defined. Observation continuity on promotion addressed. Open questions Q1–Q6 resolved. |
| 0.7 | 2026-02-19 | Integrates Stewardship Workbench as Semantic OS operating console; adds schema-driven lifecycle verbs, DSL↔SemReg binding model, and publish-event integration; retains all unchanged content. |
| 0.8 | 2026-02-19 | Semantic OS as Execution Constraint Surface. Resolution Contract (Action Surface, Verb Compilation Spec, Plan Validation). Embedding and Resolution Projection model. Intent Resolution Readiness as stewardship coverage check. GetEditSchema structural model. Link-graph consistency tiers. Verb composition model (Q10). Extended VerbContract fields for intent pipeline consumption. Separation of concerns: Semantic OS vs Intent Pipeline vs DSL Runtime. |
| 0.81 | 2026-02-19 | Visualisation + immediate feedback: Intent→Show→Refine (Show Loop). Workbench viewports. ShowPacket render contract and delta updates. Draft overlay execution preview. |
| 0.82 | 2026-02-19 | Show Loop hardened: ShowPacket emission contract (agent/tool/UI-initiated), SuggestedAction model, ViewportStatus lifecycle. Draft Overlay threaded through Resolution Contract (OverlayMode on Action Surface + Verb Compilation Spec). ActionSurfaceDelta for preview diffs. Action Surface explicitly built on implemented context resolution pipeline. Viewport data shapes specified per kind. ShowPacket manifest added to audit chain. Journey E (Show Loop walkthrough). Changeset↔snapshot_set bridge to implemented model. Tool surface classified (Query/Stewardship/Resolution/Visualisation). Principle numbering fixed (P12/P13 for Show Loop principles). |
| 0.83 | 2026-02-19 | Draft storage alignment: Drafts stored as `sem_reg.snapshots` with `status = Draft` inside snapshot_sets; stewardship-layer tables hold only governance wrapper (intent, basis, review, guardrails). Transport model: WorkbenchPacket as peer to DecisionPacket on shared streaming/persistence infrastructure. Show Loop latency invariant: FocusState + Diff always renderable within one interaction cycle. Implementation sequencing note (§16): Show Loop ships on existing MCP tools; materialised Action Surface is execution-side optimisation. ViewportManifest hashing spec (SHA-256, RFC 8785 canonical JSON). FocusState as server-side shared truth updated by agent and UI. |
| 1.0 | 2026-02-19 | Implementation-ready. Five pre-implementation amendments resolved: (1) Changeset identity unified — `changeset_id == snapshot_set_id`, single UUID, no indirection. (2) `FocusChanged` event added to StewardshipEvent enum for audit replay. (3) Draft Overlay ABAC impersonation — `assume_principal` parameter on preview, recorded in ViewportManifest. (4) Draft uniqueness invariant — one Draft head per `(object_type, fqn)` per Changeset, enforced by constraint + guardrail G15. (5) Template storage resolved — stewardship-layer tables in v1, path to SemReg object type when kernel object_type enum is next extended. `PreconditionStatus.satisfied` gains `Unknown` variant for static Action Surface layer. |
| 1.0.1 | 2026-02-19 | Pre-planning surgical fixes: (1) Draft uniqueness constraint corrected to `object_id` (kernel has no `fqn` column). (2) Gate/overlay WHERE clause adds `effective_until IS NULL` on Active side to exclude superseded history. (3) `assume_principal` type standardised to `Option<Principal>` everywhere. (4) Draft snapshot mutability rule stated explicitly (Drafts are pre-release mutable; Active snapshots immutable except predecessor supersede). (5) VerbImplementationBinding pinned to stewardship-layer tables. (6) Scope bullet for templates aligned with §9.5. (7) Idempotency via `client_request_id` on mutating tools. (8) WorkbenchPacket `frame_type` discriminator specified. |

---

## 0. Executive Summary

Semantic OS execution is intentionally closed-world: the execution/intent agent reads Active registry snapshots and cannot create changesets or modify the registry.

Registry evolution is different: it needs authoring, cross-referencing, review, audit, and release discipline. The Stewardship Agent is the authoring persona operating on the registry via Changesets of Draft snapshots. Drafts are isolated from execution; publish gates remain the single enforcement boundary.

The Stewardship Workbench provides a structured review surface where non-technical governance users can inspect diffs, see impact, run gates, and approve/reject — with guardrails that shift governance failures left (before gates fire). The conversational, CLI, and MCP surfaces all operate on the same Changeset model and capability layer.

**v0.8:** Semantic OS is not merely a lookup service for the execution agent. It is the **Execution Constraint Surface** — the system that computes and publishes the agent's complete, context-scoped action surface. Three formal contracts (Action Surface, Verb Compilation Spec, Plan Validation) define the integration boundary between Semantic OS and the intent/execution pipeline, ensuring that governed verbs and nouns are not just registered but *findable, compilable, and validatable* by agentic consumers. This separation of concerns keeps Semantic OS as the source of truth for the universe of things and the functionals to interact with them, while the intent pipeline owns reasoning and the DSL runtime owns execution.

**v0.81–1.0:** The Show Loop formalises a requirement that was always implicit: **agentic governance only works if the human can continuously see what the agent believes is in scope, what changed, and what becomes executable.** The Workbench is not a convenience UI — it is the human verification mechanism. Every agent action emits a structured ShowPacket (transported via WorkbenchPacket on the same streaming infrastructure as execution-side DecisionPackets) that drives viewport rendering. Draft Overlay mode lets the human preview exactly what the execution agent will see post-publish — computed against Draft snapshots stored directly in `sem_reg.snapshots` with `status = Draft`, ensuring Draft Overlay and gate pre-check use the same resolution logic as execution, not a duplicate store. Draft Overlay supports execution-principal impersonation so the preview reflects the execution agent's ABAC scope, not the steward's. The Show Loop is closed by SuggestedActions that present the minimal next steps. Audit records capture what viewports were displayed when a decision was made (with cryptographic hashes for integrity), closing the chain for regulatory reconstruction. A hard latency invariant guarantees FocusState + Diff are always renderable within one interaction cycle — the Show Loop never blocks on gates or impact analysis.

This document is self-contained. It supersedes all prior versions.

---

## 1. Problem Statement

The Semantic OS establishes a closed, governed vocabulary: every attribute, verb, policy, entity type, and derivation is registered, snapshotted, and gate-checked before it can participate in execution. This is correct and necessary for regulated finance — you cannot allow an agent to invent compliance semantics at runtime.

But this creates a bootstrap paradox. **The vocabulary that constrains execution must itself be authored from outside that vocabulary.** New attributes, new policy rules, new entity types — these don't exist in the registry yet, so by definition no registered verb can create them. The onboarding pipeline handles the initial bulk load from the existing operational schema. But it cannot handle:

- **New regulatory requirements** that introduce attributes with no schema precedent (e.g., FATCA tax classification fields that don't exist in any current table)
- **Evolving business domains** where new entity types emerge from changing product offerings
- **Policy refinement** where existing hardcoded business rules need extraction, formalisation, and regulatory linkage
- **Governance promotion** where an operational convenience attribute needs to become a governed proof attribute because a new compliance workflow depends on it
- **Ongoing stewardship** — the steady-state operation of a living registry that must evolve with the business

The current plan treats these as manual YAML edits or CLI commands. That is unsustainable at scale and wastes the one resource that should be doing the heavy lifting: the domain knowledge embedded in a well-grounded language model.

**Additionally, a governed registry that the execution agent cannot efficiently navigate and compile against is a data dictionary — not an executable constraint system.** The gap between "registered in Semantic OS" and "actionable by the intent pipeline" must be closed with formal contracts, not ad-hoc integration.

**Finally, an agent-in-front-of-everything model is only trustworthy if the human can continuously see what the agent is doing.** Without structured visual feedback at every step, governance degrades to "trust the LLM" — which is the opposite of what regulated finance requires.

---

## 2. Vision

The Stewardship Agent is a **second agent persona** that operates at the meta level — authoring, refining, and governing the semantic vocabulary that the execution agent consumes. Where the execution agent works WITHIN the registry, the stewardship agent works ON the registry.

### 2.0 Semantic OS: Verbs and Nouns are Co‑Equal Partners

Semantic OS is not "metadata on the side". It is the **source of truth for the enterprise's executable universe**:

- **Nouns (meaning/data):** governed definitions — attributes, taxonomies, evidence requirements, policy rules, derivations, document types, security labels — versioned as snapshots.
- **Verbs (functions/actions):** governed **VerbContracts** describing what can be done, with preconditions, inputs/outputs, side‑effects, and required evidence — also versioned as snapshots.

In an MCP / agentic DSL / intent world, this partnership is the pivot from "LLMs as free spirits" to a **disciplined, testable, auditable army**:

- LLMs can *propose* actions in natural language.
- The platform can only *execute* actions that are **registered, linked, gate‑checked, and published**.
- Any gap becomes an explicit stewardship task (author/adjust the noun, the verb contract, or the link), not an ad‑hoc workaround.

### 2.0.1 Managed Links: the Verbs↔Nouns Contract Graph

Semantic OS stores **managed, versioned links** between functions and data so intent can be compiled deterministically:

- `VerbContract` links: **consumes / produces / requires / prohibits**
- `PolicyRule` links: **governs** verbs, nouns, or both (and constrains thresholds/parameters)
- `EvidenceRequirement` links: **satisfies** policy rules and verb preconditions
- `DerivationSpec` links: **depends_on** attributes/observations and yields derived nouns
- `TaxonomyMembershipRule` links: **classifies** nouns and/or verb outcomes (for scoping + discoverability)

These links travel through the same **Changeset → Gate → Publish** lifecycle, making Semantic OS a **consolidating service for deterministic, executable actions**.

### 2.0.2 Link-Graph Consistency Tiers

When registry objects evolve (promotion, deprecation, type change), the managed links between them can become semantically stale. For example, promoting an attribute to Governed/Proof may leave consuming VerbContracts with a `predicate_trust_minimum` that no longer matches the attribute's tier.

Rather than treating all consistency failures uniformly, the platform defines **consistency tiers** that govern how strictly cross-object coherence is enforced:

| Tier | Scope | Enforcement | Rationale |
|---|---|---|---|
| **Intra-Changeset** | Objects within the same Changeset | Block (mandatory) | A Changeset must be internally coherent. A PolicyRule referencing a new AttributeDef in the same Changeset must resolve. |
| **Cross-Changeset / Active** | Published Active objects affected by a new publish | Configurable per domain: Warning (default) or Block | Pragmatic: strict enforcement prevents any publish that creates cross-registry staleness; lenient enforcement creates tracked stewardship debt. |
| **Historical** | Deprecated or superseded objects | Advisory only | Deprecated objects may have stale links by design; the deprecation workflow tracks migration. |

Cross-Changeset consistency enforcement is a **platform configuration per domain**. Regulated domains (e.g., `RegDomain.FATCA`, `RegDomain.AML`) may require Block-level cross-Changeset consistency, while operational domains may tolerate Warning-level debt with a visible migration backlog.

When a publish creates cross-Changeset staleness at Warning level, the publish event (§9.8) includes a `stale_links` manifest that feeds the Coverage Report (§5.4) and creates stewardship tasks for remediation.


### 2.1 Two Agent Personas, One Registry

| | Execution / Intent Agent | Stewardship Agent |
|---|---|---|
| Operates on | Cases, evidence, decisions | Registry definitions, policies, taxonomies |
| Reads | Active snapshots only | Active + Draft snapshots |
| Writes | Case state, observations | Draft snapshots (via Changesets) |
| Cannot | Create changesets or mutate registry | Publish without gate approval |
| Epistemology | Closed-world (if not registered, doesn't exist) | Open-world propose, closed-world publish |

The two agents share infrastructure (SnapshotStore, taxonomy, security labels) but differ fundamentally in what constrains them. The execution agent is bound by the vocabulary. The stewardship agent is bound by the publish gate.

### 2.2 Open-World Propose, Closed-World Publish

Two epistemological modes co-exist:

- **Closed-world execution**: The execution agent can only reference objects that exist as Active snapshots. If it isn't registered, it doesn't exist. This is the correctness guarantee for regulated processing.
- **Open-world authoring**: The stewardship agent uses broad domain knowledge — regulatory frameworks, financial services vocabulary, common evidence artifacts, terminology — to propose new registry objects that don't yet exist. The agent's knowledge extends beyond the current registry. This is where new vocabulary comes from.

The boundary between the two modes is the **publish gate**. Open-world proposals become closed-world vocabulary only after passing deterministic gate checks and human approval. The stewardship agent cannot bypass this boundary.

### 2.3 Interaction Model

The interaction model is conversational and intent-driven:

> **Human**: "We're adding FATCA reporting to institutional client onboarding. I need the registry updated for US tax status determination."
>
> **Agent**: Proposes a cohesive set of registry objects (attributes, policies, evidence requirements, taxonomy memberships, security labels) grounded in FATCA Chapter 4 requirements and the existing entity model. Cross-references against the current registry to identify promotions, conflicts, and gaps.
>
> **Human**: Refines, accepts, or rejects. The agent adjusts.
>
> **Agent**: Creates a Changeset of Draft snapshots. The publish gate remains the hard enforcement boundary.

The stewardship agent does not bypass governance. It accelerates it. Every object it proposes is a Draft snapshot subject to the same invariants, gates, and approval workflows as any other registry change. The agent's value is in **domain inference, cross-referencing, and consistency checking** — not in unilateral mutation.

#### 2.3.1 Intent → Show → Refine (the "Show Loop")

Stewardship is not usable if it is only conversational. The agent must make the current semantic state **visible and inspectable** as the human works.

For every meaningful turn, the agent emits (or triggers) a **ShowPacket** (§9.14) that drives the Workbench UI:

- **Intent**: what the human asked for / what the agent proposes to do next
- **Show**: the *current focus* (what object(s) are being worked on) plus the relevant read-only context (taxonomies, predecessors, policies)
- **Refine**: the smallest set of next actions the human can take (accept, edit, request evidence, resolve conflicts, run gates)

This mirrors the DSL agent model already proven in ob‑poc: humans navigate CBU/UBO taxonomies and graphs in a read-only viewport while the agent drives changes. Stewardship extends this to registry authoring: "show me what we're changing, what it affects, and what becomes executable".

#### 2.3.2 What must be shown (minimum viable visibility)

To keep humans oriented and prevent "agent hallucinated state", the Workbench must always be able to show:

1. **Focus** — the current Changeset, the object(s) being edited, and the Resolution Context being used
2. **Before/After** — predecessor Active snapshot vs Draft successor (diff-first)
3. **Effect** — impact graph + action surface preview ("what verbs become available / constrained?")
4. **Trust boundary state** — gate pre-check results, missing basis, coverage gaps, review notes

#### 2.3.3 Immediate feedback is a requirement (not a UX nice-to-have)

Agentic governance fails if feedback is slow. The human must get **near-immediate visual confirmation** of:

- what the agent believes is in scope,
- what changed,
- what the consequences are.

Practically:
- Workbench viewports should render from **deterministic, precomputed registry projections** wherever possible (active views, cached surfaces).
- Updates should be **delta-based** (patch the current view rather than re-rendering "the universe").
- Expensive computations (large impact graphs, embedding refresh) may stream progress, but the Workbench must still show a stable focus + diff immediately.

#### 2.3.4 Draft Overlay Preview (trust through "what will the execution agent see?")

The Workbench must support a **Draft Overlay** mode: treat a Changeset as provisionally Active for preview.

This enables the most important human question:

> "If we publish this Changeset, what will the execution/intent agent see as executable and required?"

The Draft Overlay works because **Drafts are stored as `sem_reg.snapshots` with `status = Draft`** (§9.1), not in a separate store. This means Draft Overlay preview uses the **exact same resolution logic** as real execution — the only difference is the WHERE clause includes the Changeset's Draft snapshots alongside Active ones. If Drafts lived outside `sem_reg`, the preview would compute against a different data path than the real publish, and the preview would lie.

The Draft Overlay reuses the **Resolution Contract tools (§2.7) exactly**. `GetActionSurface` and `GetVerbCompilationSpec` accept an `overlay: OverlayMode` parameter. When set to `DraftOverlay(changeset_id)`, they resolve against `Active ∪ Changeset.Drafts` by extending the snapshot resolution scope to include the Changeset's `snapshot_set_id` (§9.2).

The preview must render **deltas, not absolute state** — what changed relative to the current Active state:

- Action Surface changes (new/removed verbs, eligibility deltas) — via `ActionSurfaceDelta` (§9.15)
- Verb compilation spec changes (inputs/evidence/preconditions)
- Policy verdict or threshold changes
- Coverage changes (new orphan/owner/classification states)

**ABAC impersonation for Draft Overlay:** The steward viewing a Draft Overlay preview has different ABAC roles than the execution agent that will consume the published state. Without impersonation, the preview would either show surfaces the execution agent won't see (steward-privileged) or hide surfaces the execution agent will see (execution-privileged). 

Draft Overlay preview accepts an optional `assume_principal: Option<Principal>` parameter (with an optional `assume_role_set: Option<Vec<Role>>` if role impersonation is also needed). When set, the Action Surface and Verb Compilation Spec are computed as if the caller had the specified principal and role set — typically the execution agent's standard identity. When not set, the preview uses the steward's own ABAC context (useful for stewardship-scoped previews).

The assumed principal is recorded in the `ViewportManifest` (§9.4) so the audit chain captures "the reviewer saw this preview computed as execution-agent-role-X, not as their own steward role."

#### 2.3.5 ShowPacket Emission Contract

Three scenarios produce ShowPackets, with clear ownership:

| Trigger | Emitter | Lifecycle |
|---|---|---|
| **Agent-initiated** | The agent completes a Suggest/Refine/CrossReference step and emits a ShowPacket telling the Workbench what to display and what actions are available next. | Primary flow. Every agent action that changes Changeset state MUST emit a ShowPacket. |
| **Tool-result-initiated** | A tool call (e.g., `GatePrecheck`, `ImpactAnalysis`) returns structured results. The agent wraps the result into a ShowPacket that updates the relevant viewport(s). The agent, not the tool, is always the emitter. | The agent is the choreographer; tools supply data. |
| **Human-navigation** | The user clicks a taxonomy node, selects a different Changeset item, or toggles overlay mode. The Workbench updates FocusState server-side and calls Show Loop tools (§6.4) directly to refresh affected viewports. No agent involvement. | UI-local. No ShowPacket emitted — the Workbench manages its own focus transitions. |

The invariant: **the agent always knows what the Workbench is showing** (because the agent emitted the last ShowPacket or can query `GetFocusState`), and **the Workbench never renders state the agent didn't produce or approve**.

**Transport:** ShowPackets are delivered as **WorkbenchPackets** — a peer payload type to execution-side DecisionPackets on the same WebSocket channel and persistence infrastructure. See §9.16 for the transport model. This avoids inventing new streaming infrastructure while keeping stewardship payloads structurally separate from execution payloads.

**Show Loop Latency Invariant:** A ShowPacket MUST include a renderable FocusState and at least the Diff Viewport data (or "new object" skeleton for Add actions) within one interaction cycle. Viewports for Impact, Gates, Coverage, and Action Surface Preview MAY be delivered with `status = Loading` and updated via subsequent ViewportDelta messages. The Show Loop MUST NEVER block on gates, impact analysis, or embedding recomputation. This is a contract the Workbench enforces, not a guideline the implementation may optimise away.


### 2.4 Stewardship Workbench as the Semantic OS Operating Console

The Stewardship Workbench is the **primary operating console** for Semantic OS evolution. It is not a collection of bespoke screens per object type. Instead, it is a single workbench that:

- Drives the full registry lifecycle (Draft → Review → Gate → Publish) through a consistent Changeset workflow
- Uses **schema-driven editing** (`GetEditSchema`) to render only the minimal structured inputs required for safe change authoring
- Keeps the human in control with explicit diffs, impact graphs, gate results, and audit-ready Basis packs

In this model, the conversational agent is the control layer, and the Workbench is the canonical review/edit surface — together they replace "100 forms" with one governed authoring environment.

### 2.5 Bridge: DSL Verbs ↔ Semantic Registry Items

A core integration goal is to **bridge the executable DSL surface** (verbs users invoke) with the **Semantic OS registry surface** (VerbContracts, AttributeDefs, EntityTypeDefs, PolicyRules, EvidenceRequirements, etc.) so that:

- Every user-facing executable verb is constrained by an **Active `VerbContract`** (already required for closed-world execution)
- Every Active `VerbContract` intended for execution has an explicit **implementation binding** (how the platform executes it)
- Publish gates can assert "no dangling semantics": a verb cannot be promoted to an executable, user-facing capability without a valid binding and resolvable dependencies

This bridge ensures the registry is not merely descriptive — it is the authoritative constraint system for both intent resolution and execution wiring.

### 2.6 Semantic OS Lifecycles as Verbs

To minimise bespoke UI, Semantic OS lifecycle operations are themselves exposed as a small, coherent verb set (surfaced via conversation/CLI/MCP but rendered in the Workbench):

- `steward.compose-changeset`, `steward.add-item`, `steward.refine-item`, `steward.attach-basis`
- `steward.run-gates`, `steward.submit-for-review`, `steward.record-review-decision`
- `steward.publish` (atomic), emitting a single publish event that triggers downstream projections

These verbs are *meta-verbs*: they operate on registry lifecycles and audit artefacts, not on case execution state.


### 2.7 Resolution Contract — Semantic OS as Execution Constraint Surface

Semantic OS should not be a passive lookup that the execution agent queries speculatively. It is the system that **computes and publishes the agent's complete action surface for any given context**. The agent does not fish in the registry hoping to find something useful. Semantic OS tells the agent: "given who you are, what you're looking at, and what state you're in, here is the complete set of things you can do, what each one needs, and what each one produces."

This is the difference between a data dictionary and an executable constraint system.

Three formal contracts define the integration boundary:

| Contract | Purpose | Consumer | Computation |
|---|---|---|---|
| **Action Surface** (§9.9) | "What can I do in this context?" | Intent pipeline (verb discovery + eligibility) | Materialised projection, refreshed on publish |
| **Verb Compilation Spec** (§9.10) | "What does this specific verb need and produce?" | Intent pipeline (slot filling + plan assembly) | On-demand query against Active registry |
| **Plan Validation** (§9.11) | "Is this proposed plan legal?" | Intent pipeline (pre-execution validation) | Deterministic validation against Active registry |

These contracts are Semantic OS's **consumer-facing API to the execution world**. They are the authoritative specification that stewards are authoring toward — a VerbContract that is structurally valid but not resolvable by the Action Surface or not compilable via the Verb Compilation Spec is stewardship debt.

#### 2.7.1 Resolution Context — The Scoping Key

Every consumption of Semantic OS by the execution agent is parameterised by a **Resolution Context** — the execution agent's declaration of where it is, who it is, and what it's working on:

```
ResolutionContext {
    // Who
    principal:              Principal,           // ABAC identity
    role_set:               Vec<Role>,           // active roles for this session

    // What
    entity_type:            EntityTypeFQN,       // e.g. "entity.institutional_client"
    entity_state:           Option<StateRef>,     // current lifecycle state if applicable

    // Where
    jurisdiction:           Vec<Jurisdiction>,    // regulatory scope (US, EU, etc.)
    domain:                 Vec<DomainFQN>,       // taxonomy domains in scope

    // When
    as_of:                  Timestamp,            // snapshot temporal reference

    // Case context (if executing within a case)
    case_id:                Option<CaseId>,
    available_evidence:     Vec<EvidenceFQN>,     // what's already been collected
    satisfied_preconditions: Vec<PredicateRef>,   // what's already true
}
```

The Resolution Context is the **execution agent's concern to construct**. Semantic OS's concern is to accept it and compute against it. There is no "give me all verbs" — only "give me what's actionable here."

#### 2.7.2 Separation of Concerns

The three-system separation is architecturally load-bearing:

**Semantic OS owns** (source of truth + projections):
- The registry — all governed nouns, verbs, links, policies, evidence specs
- Action Surface computation — "what's available in this context" as a materialised projection
- Verb Compilation Spec — "everything the agent needs to construct a valid invocation"
- Plan Validation — "is this proposed plan legal against published state"
- Embedding index — Active VerbContracts + AttributeDefs + managed links, refreshed on publish
- Publish event emission — triggers all projection/embedding/coverage refresh

**Intent Pipeline owns** (reasoning + compilation):
- Utterance parsing — NLP/LLM decomposition of natural language
- Semantic search — using Semantic OS embeddings to find candidate verbs/nouns (search logic and ranking is the pipeline's concern)
- Intent ranking and selection — LLM reasoning over the Action Surface to select verb(s)
- Slot filling / parameter binding — LLM maps utterance elements to InputSpec fields using source hints
- Plan assembly — constructing the ExecutionPlan from selected verbs + bound inputs
- Multi-step plan optimisation — ordering and data-flow logic for compound plans

**DSL Compiler / Runtime owns** (execution):
- DSL parsing and type checking — syntactic and type-level validation
- Execution dispatch — using VerbImplementationBinding to route to the actual handler
- Runtime state management — case state, transaction boundaries, compensation
- Side-effect execution — writing observations, changing states, triggering external systems

Semantic OS publishes what's legal. The intent pipeline reasons about what's useful. The DSL runtime makes it happen.

#### 2.7.3 Relationship to Implemented Context Resolution

The ob-poc implementation already provides a **12-step deterministic context resolution pipeline** (`context_resolution.rs`) that returns ranked verbs, attributes, views, precondition status, policy verdicts, governance signals, and a confidence score. The Action Surface (§9.9) is **not a parallel computation** — it is a **materialised, enriched projection built on top of context resolution**.

The relationship:

- `resolve_context()` remains the **computation engine** — the 12-step pipeline that selects views, filters/ranks verbs and attributes, evaluates preconditions, applies ABAC, computes policy verdicts, and emits governance signals.
- `ActionSurface` is the **materialised/cached projection** of `resolve_context()` output, enriched with additional fields the intent pipeline needs: pre-computed `actionable` booleans, `InputSourceHint` resolution, composition hints, and embedding references.
- `GetActionSurface` is the **tool that serves the cached projection** with optional dynamic eligibility overlay (case-specific precondition evaluation) and optional Draft Overlay (`OverlayMode`).

This ensures the Action Surface is always consistent with context resolution — no divergent computation paths.

---

## 3. Principles

**P1 — Intent over specification.** The human expresses what they need in business terms. The agent translates to registry objects. The human should never need to know the JSONB schema of a `PolicyRule.predicate` to create a compliance rule.

**P2 — Suggest, don't dictate.** Every agent output is a proposal. The human reviews, refines, accepts, or rejects. The agent explains its reasoning. No registry mutation happens without human-visible approval.

**P3 — The publish gate is the trust boundary.** The stewardship agent can create Drafts freely. Only the publish pipeline (with its full gate suite) can promote Drafts to Active. This is the single enforcement point — all other interactions are advisory.

**P4 — Changesets are atomic.** A stewardship action is a cohesive set of related changes, not isolated objects. Attributes, policies, evidence requirements, taxonomy memberships, and security labels that belong together must be created, reviewed, gate-checked, and published together.

**P5 — Cross-reference is mandatory.** Before proposing any new object, the agent must check the existing registry for conflicts, duplicates, promotable orphans, and dependent objects. A new attribute that duplicates an existing one (or an existing orphan that should be promoted) is a stewardship failure.

**P6 — Domain knowledge is the agent's primary value.** The agent's advantage over a YAML editor is knowing what FATCA requires, what evidence artifacts are standard, what governance tier is appropriate for a tax identifier, and how to link a policy rule to the regulatory basis that mandates it. The agent must be explicit about knowledge boundaries and confidence levels.

**P7 — Auditability.** Every registry object must be traceable to its authoring intent, the agent's reasoning, the alternatives considered, human approval, and publish metadata. An auditor must be able to reconstruct the decision chain for any Active object.

**P8 — Workbench guardrails over free-text.** Make invalid and unsafe edits hard to express and easy to explain before gates fire. Guardrails are the shift-left safety layer; they stop governance failures early and explain remediation, while gates remain the final trust boundary.

**P9 — Verbs and Nouns are partners, and execution is registry-gated.** The registry is the allowlist for deterministic action. Agents may *propose* anything, but runtime may only *execute* what is (a) registered, (b) correctly linked to governed nouns/evidence/policies, and (c) published via gates. This is the mechanism that turns probabilistic intent into disciplined, approved, testable outcomes.

**P10 — Semantic OS is the Execution Constraint Surface, not a lookup service.** The execution agent does not speculatively search the registry. Semantic OS computes and publishes the agent's complete, context-scoped action surface — including pre-computed eligibility, input source hints, and composition guidance. The agent reasons over a scoped menu, not an unbounded catalogue. This is the mechanism that makes LLM reasoning tractable and deterministic within the governed universe.

**P11 — Authored for resolution, not just for registration.** A VerbContract that is structurally valid but cannot be found by semantic search, compiled by the intent pipeline, or validated at plan time is stewardship debt. Resolution metadata (usage examples, parameter guidance, embedding enrichment) is a governed obligation, not optional documentation.

**P12 — Show, don't tell.** Stewardship is only trustworthy if the human can *see* what is in focus, what changed, and what becomes executable. Every agent action that changes Changeset state must yield a structured visual artifact (ShowPacket → viewport renders: taxonomy view, object inspector, diff, impact, gate result, action surface preview) — not only narrative text.

**P13 — Diff-first, immediate feedback.** The Workbench prioritises fast, stable visual feedback: show focus + diffs immediately, stream heavier impact/embedding computations with progress indicators, and update viewports via small deltas. Humans must never wonder whether the agent is acting on the correct object.

---

## 4. Scope

Semantic OS + the Stewardship Agent form a single operating model:

- **Semantic OS** defines the governed universe of **verbs and nouns**, plus the managed links between them, and **publishes the execution constraint surface** that the intent pipeline compiles against.
- **Stewardship Agent** is the Workbench operator that authors/maintains that universe through controlled Changesets.
- **Execution / Intent Agents** consume the published constraint surface (Action Surface, Verb Compilation Spec, Plan Validation) to discover, compile, and validate executable plans deterministically.
- **DSL Compiler / Runtime** dispatches validated plans to execution handlers — it consumes VerbImplementationBindings but owns parsing, type-checking, and runtime state.

This is explicitly a **consolidating service for all agentic deterministic/executable actions** — the "secret sauce" that turns LLMs from free-form actors into a disciplined, policy-bounded execution layer.

### In Scope

**Registry and governance:**
- Treat Semantic OS as the **source of truth** for the enterprise action surface: every plan/step resolves through the registry (verbs) and typed registry nouns (data)
- Maintain **managed links** between verbs and nouns (and their evidence/policy constraints) as first-class governed objects within Changesets
- Expose the registry surface via **MCP-compatible tools** (lookup/resolve/diff/gates) so every agentic integration must consult the same source of truth
- Create/extend attributes, evidence requirements, policy rules, derivations, taxonomies, security label assignments
- Promote / alias / reclassify / deprecate semantics via Changesets (Draft → Review → Gate → Publish)
- Coverage gap analysis, drift detection, impact analysis (semantic blast radius)
- Stewardship Workbench + guardrails (review flow, diffs, approvals, remediation)
- Verb contract authoring (I/O surface, preconditions, postconditions) — the contract only; implementation is a separate engineering task
- Template management — templates as first-class governed stewardship objects (versioned, gate-checked; promotable to SemReg object type when kernel enum is next extended)
- Basis / Research Pack as mandatory audit artifact

**Workbench and lifecycle:**
- Semantic OS lifecycle operations exposed through a small Stewardship verb set (compose, refine, gate, review, publish) rendered in a single Workbench
- Schema-driven editing to minimise bespoke forms/screens (`GetEditSchema` + guardrail-aware editors)
- DSL ↔ Semantic OS bridge: binding and coverage checks ensuring executable verbs are constrained by Active VerbContracts and resolvable registry dependencies
- Publish event integration: a single publish outcome emits a registry event to drive projections, embeddings refresh, coverage recomputation, and Action Surface invalidation

**Execution Constraint Surface (v0.8):**
- Resolution Contract: Action Surface, Verb Compilation Spec, and Plan Validation as the formal interface between Semantic OS and the intent/execution pipeline
- Resolution Context as the scoping key for all execution-side queries against Semantic OS
- Link-graph consistency tiers (intra-Changeset mandatory, cross-Changeset configurable per domain)
- Embedding and Resolution Projection model: what gets embedded, how managed links enrich embeddings, scoping invariants, refresh triggers
- Intent Resolution Readiness as a stewardship coverage check: VerbContracts must be findable and compilable, not just structurally valid
- Extended VerbContract fields: usage examples, parameter guidance, input source hints, composition hints, embedding enrichment template
- Action Surface built on the implemented context resolution pipeline (not a parallel computation)

**Visual Control Loop (v0.81–1.0):**
- Intent → Show → Refine closed loop with ShowPacket as the structured render contract
- Eight stable Workbench viewports (Focus, Taxonomy, Object Inspector, Diff, Impact, Action Surface Preview, Gates, Coverage/Readiness)
- ShowPacket emission contract: agent-initiated, tool-result-initiated, and human-navigation flows
- **Show Loop Latency Invariant:** FocusState + Diff always renderable within one interaction cycle; heavier viewports stream with Loading status
- WorkbenchPacket as peer transport to DecisionPacket on shared streaming/persistence infrastructure (§9.16)
- SuggestedAction model for closing the Refine step of the loop
- ViewportStatus lifecycle (Ready, Loading, Error, Stale)
- Draft Overlay threaded through Resolution Contract tools (OverlayMode parameter on Action Surface + Verb Compilation Spec) — computed against Draft snapshots in `sem_reg.snapshots`, not a separate store
- **ABAC impersonation for Draft Overlay** — `assume_principal` parameter for previewing as execution agent's ABAC context, recorded in ViewportManifest
- ActionSurfaceDelta for preview diffs showing what changes post-publish
- Viewport data shapes specified per viewport kind for stable Workbench rendering
- FocusState as server-side shared truth, updated by agent and UI, with `FocusChanged` events in audit chain
- ViewportManifest in audit chain with SHA-256 canonical hashes (RFC 8785) for cryptographic integrity, including assumed_principal
- Delta-based viewport updates for immediate feedback
- Implementation sequencing: Show Loop ships on existing MCP tools; materialised Action Surface is execution-side optimisation (§16)

### Out of Scope

- Rust implementation, DB schema changes, runtime execution internals
- Implementing new execution verb handlers (stewardship may propose contracts, implementation is separate)
- Cross-platform registry federation (future capability)
- Execution conversation history access (analytics telemetry, not conversation mining)
- Intent pipeline internals: utterance parsing, LLM prompt construction, semantic search ranking algorithms, slot-filling strategies (these consume Semantic OS projections but are owned by the intent pipeline)
- DSL compiler/runtime internals: parsing, type checking, execution dispatch, transaction management (these consume VerbImplementationBindings but are owned by the DSL engine)

---

## 5. Capabilities

### 5.1 Suggest → Draft → Refine Loop

The core conversational loop. The agent proposes a cohesive set of registry objects (Suggest), materialises them as Draft snapshots grouped in a Changeset (Draft), and accepts targeted modifications that increment the Changeset revision (Refine). The loop terminates when the steward submits for review.

### 5.2 Describe and Cross-Reference

Before proposing anything, the agent describes the current registry state relevant to the intent — existing attributes, verbs consuming them, policies referencing them, taxonomy memberships, security labels. Cross-reference identifies: duplicate FQN candidates, promotable orphans, alias collisions, dependent verbs/derivations that would be affected.

Cross-reference is mandatory. The Workbench will not allow submission for review until cross-reference has been run and findings acknowledged.

### 5.3 Impact Analysis

Given a proposed change (promotion, deprecation, type change), the agent computes the semantic blast radius: which verbs consume the affected attribute, which derivations depend on it, which policies reference it, which taxonomy memberships would be affected, and whether any existing observations would be impacted.

### 5.4 Coverage Report and Drift Detection

Produce a report of attributes used in policy predicates or proof chains that are below the required governance tier (orphan detection). Identify attributes that have drifted from their original classification intent. Surface candidates for promotion, alias, or deprecation.

Coverage now includes **Intent Resolution Readiness** — VerbContracts that are Active and bound but lack sufficient resolution metadata (usage examples, parameter guidance, input source hints) for the intent pipeline to reliably find and compile against them. See §5.11.

### 5.5 Gate Pre-Check

Answers "If I published this Changeset now, what fails?" The pre-check runs the full gate suite against the Draft snapshots in isolation mode, treating intra-Changeset Drafts as provisionally Active for the purposes of dependency resolution (see §9.2). Returns failures per item, cross-item dependency issues, and Proof Rule check results.

### 5.6 Publish (Atomic)

Promotes an approved Changeset from Draft to Active atomically. Sets predecessor `effective_until`. Activates taxonomy memberships. Marks embeddings stale. Writes audit chain entry. No partial publishes.

Publish emits the structured publish event (§9.8) that triggers Action Surface projection refresh, embedding recomputation, coverage revalidation, and intent pipeline cache invalidation. If cross-Changeset consistency enforcement is set to Warning for the affected domain, the publish event includes a `stale_links` manifest.

### 5.7 Stewardship Workbench

A structured review and edit surface for non-technical governance users. The Workbench is the governance operating model interface — not a convenience UI. It surfaces diffs, impact graphs, gate results, Basis packs, and reviewer decision workflows. All Workbench operations operate on the same Changeset model as the conversational and CLI surfaces.

### 5.8 Workbench Guardrails

Shift-left governance: guardrails fire during editing, before gates. They make invalid edits hard to express and explain remediation immediately. See §8 for the full guardrail taxonomy.

### 5.9 Schema-Driven Editing (Replace 100 Forms)

The Workbench uses `GetEditSchema` to render object-specific editors from registry schemas and role permissions, rather than implementing bespoke UI per object type. Conversation remains the intent layer; the Workbench is the structured edit/review layer. Guardrails fire during edits, producing immediate, actionable remediation.

`GetEditSchema` returns a structured schema (§9.12) with conditional field visibility, contextual defaults, and declarative cross-field constraints — not just flat field lists. This prevents the "100 forms" problem from reappearing as "100 conditional rendering rules" in Workbench UI code.

### 5.10 Contract Coverage and Binding Integrity

Stewardship maintains *coverage* between registry semantics and execution:

- **Contract coverage:** executable verbs must have Active `VerbContract` definitions with resolvable inputs/outputs and referenced registry objects
- **Binding integrity:** execution-intended contracts must have a valid implementation binding (Rust handler, BPMN process, remote API, or macro expansion)
- **Drift detection:** when implementations change, contracts/bindings are verified and any mismatch is surfaced as a stewardship issue (non-blocking in Draft, blocking at publish for user-facing verbs)

This capability is the formal bridge between DSL verbs and Semantic OS items, preventing "semantic drift" where code and registry disagree.

### 5.11 Intent Resolution Readiness

A VerbContract that is Active and bound but not findable or compilable by the intent pipeline is **stewardship debt**, not implementation debt. Intent Resolution Readiness is a coverage check that ensures published VerbContracts carry sufficient metadata for the execution constraint surface:

| Check | What it validates | Severity |
|---|---|---|
| **Usage examples present** | At least one `UsageExample` attached to user-facing VerbContracts | Warning (configurable to Block per domain) |
| **Parameter guidance complete** | Every `InputSpec` with `source_hint = UserProvided` has human-readable `parameter_guidance` | Warning |
| **Input source hints assigned** | Every `InputSpec` has a non-null `source_hint` | Block for user-facing verbs |
| **Embedding enrichment resolvable** | All FQN references in the embedding input template resolve to Active objects | Block |
| **Composition hints valid** | `typical_predecessors` and `typical_successors` reference Active VerbContracts | Advisory |

Intent Resolution Readiness is evaluated as part of gate pre-check (§5.5) and reported in the Coverage Report (§5.4). It creates a new category of stewardship task: "this verb is registered and bound, but the intent pipeline can't reliably find or compile against it."

### 5.12 Action Surface Computation

Semantic OS computes and maintains the **Action Surface** — a materialised projection of what verbs are available, eligible, and actionable for a given Resolution Context. The Action Surface is the primary interface through which the execution/intent agent discovers what it can do. See §9.9 for the data model.

The Action Surface is computed in two layers:

- **Static surface** (entity-type × jurisdiction × role → available verbs): materialised on publish, cached, invalidated by publish events. Changes only when the registry changes. Built on top of the implemented 12-step context resolution pipeline (§2.7.3).
- **Dynamic eligibility** (case-specific precondition evaluation): computed at query time by overlaying case state (available evidence, satisfied preconditions) onto the static surface.

This two-layer design ensures the hot path of intent resolution is fast (static lookup + lightweight dynamic overlay) while remaining fully consistent with published registry state.

### 5.13 Plan Validation

Before the intent pipeline hands a compiled plan to the DSL runtime, Semantic OS validates it deterministically. This is the execution-side equivalent of the publish gate — but lighter, because it validates against already-published Active state rather than Draft state. The agent cannot bypass this; it is architecturally equivalent to the publish gate but for execution rather than authoring. See §9.11 for the data model.

### 5.14 Intent → Show → Refine Visual Loop

The Stewardship Agent and Workbench operate as a **closed control loop**:

1. **Intent** (human request)
2. **Propose** (agent generates Draft items + rationale)
3. **Show** (Workbench renders focus, diffs, impact, preview surfaces via ShowPacket)
4. **Refine** (human edits/accepts via SuggestedActions, agent adjusts)
5. **Gate** (pre-check and remediation)
6. **Review** (human approval)
7. **Publish** (atomic promote + publish event)

This loop is the practical mechanism that makes governance usable and safe: the agent accelerates authoring, while the Workbench keeps the human continuously oriented.

### 5.15 Workbench Viewports (Replace 100 Screens with 8 Lenses)

The Workbench is a single surface composed of a small set of stable **viewports** ("lenses"). Each viewport is fed by deterministic Semantic OS tools and is read-only by default.

Minimum viewport set:

- **A. Focus / Breadcrumb** — current Changeset, object(s) in focus, Resolution Context, overlay mode (Active vs Draft overlay)
- **B. Taxonomy Viewport** — tree/graph navigation with membership overlays (Active vs Draft)
- **C. Object Inspector** — current snapshot + memberships + security label + consumers
- **D. Diff Viewport** — predecessor Active vs Draft successor (structured + human-readable)
- **E. Impact Viewport** — blast radius graph (verbs, policies, derivations, views)
- **F. Action Surface Preview** — available verbs + eligibility deltas for a Resolution Context (Active vs Draft overlay)
- **G. Gate Viewport** — pre-check results, blocking issues, remediation suggestions
- **H. Coverage / Readiness** — intent resolution readiness, orphan/steward coverage, drift signals

These viewports are not "nice UI". They are the **human verification mechanism** that makes agentic governance safe and scalable.

### 5.16 ShowPackets and Delta Rendering

The agent produces a structured UI instruction (a **ShowPacket**, §9.14) alongside narrative output. The Workbench uses it to:

- set focus deterministically,
- request viewport models from Semantic OS tools,
- apply small deltas to update viewports quickly,
- and present the minimal set of SuggestedActions for the next step.

This design keeps UI complexity low: the Workbench renders a small number of viewport kinds; the agent chooses what to show by emitting ShowPackets.

### 5.17 Immediate Feedback Policy

Workbench interactions are engineered around "feels immediate" responsiveness, governed by the **Show Loop Latency Invariant** (§2.3.5):

- **Hard rule:** FocusState + Diff are always renderable within one interaction cycle. The Show Loop never blocks on gates, impact, or embedding recomputation.
- Always render **focus + diff** first (even if impact/gates are still running)
- Prefer cached/projection-backed data for "show" operations
- Stream long-running computations (impact, embedding, large graph layouts) with progress and partial results — tracked via `ViewportStatus` (§9.14.5)
- Apply changes as deltas: update only affected viewport panels where possible

This matches the existing UX patterns in the ob-poc application: chat updates immediately, inspector/viewport regenerate projections asynchronously.

---

## 6. Tool Surface

The stewardship agent exposes capabilities through three surfaces (conversational, CLI, MCP tools) that all operate on the same Changeset model and capability layer.

### 6.0 Tool Classification

The tool surface is organised into four categories by function and consumer:

| Category | Consumer | Purpose | Examples |
|---|---|---|---|
| **Query** | Any agent or UI | Read-only inspection of Active/Draft registry state | `DescribeObject`, `CrossReference`, `ImpactAnalysis`, `CoverageReport` |
| **Stewardship** | Stewardship agent + Workbench | Mutate Changesets, run gates, manage lifecycle | `ComposeChangeset`, `Suggest`, `RefineItem`, `GatePrecheck`, `Publish` |
| **Resolution** | Execution/intent pipeline | Consume the published constraint surface for intent compilation and plan validation | `GetActionSurface`, `GetVerbCompilationSpec`, `ValidatePlan` |
| **Visualisation** | Workbench UI (often via ShowPackets) | Supply deterministic viewport models for the Show Loop | `GetFocusState`, `GetViewportModel`, `GetDiffModel`, `GetActionSurfacePreview` |

This classification is important because the tool surface is large (~57 tools across all categories). Query and Visualisation tools are read-only. Stewardship tools mutate Draft state. Resolution tools serve the execution pipeline. Mixing these categories risks confused ownership and security boundaries.

### 6.1 Query Tools

| Tool | Parameters | Returns |
|---|---|---|
| `DescribeObject` | `fqn: String` | Snapshot, taxonomy memberships, security labels, consuming verbs |
| `CrossReference` | `fqn: String, proposed_body: JsonValue` | Conflicts, duplicates, promotable candidates, alias suggestions |
| `ImpactAnalysis` | `changeset_id: Uuid` | Blast radius: verbs, derivations, policies, observations affected |
| `CoverageReport` | `domain: Option<String>` | Orphans, drift candidates, promotion candidates, intent resolution readiness |

### 6.2 Stewardship Tools

| Tool | Parameters | Returns |
|---|---|---|
| `ComposeChangeset` | `intent: String, template: Option<String>` | `changeset_id`, proposed items list |
| `Suggest` | `changeset_id: Uuid, refinement: String` | Updated item list, revision increment |
| `AddItem` | `changeset_id: Uuid, item: ChangesetItem` | Updated Changeset |
| `RemoveItem` | `changeset_id: Uuid, item_id: Uuid` | Updated Changeset |
| `RefineItem` | `changeset_id: Uuid, item_id: Uuid, patch: JsonValue` | Updated item, revision increment |
| `AttachBasis` | `changeset_id: Uuid, basis: Basis` | Updated Changeset with Basis attached |
| `GatePrecheck` | `changeset_id: Uuid` | `GateResult` per item + cross-item issues + intent resolution readiness |
| `SubmitForReview` | `changeset_id: Uuid` | Review queue entry, status → `UnderReview` |
| `RecordReviewDecision` | `changeset_id: Uuid, decision: ReviewDecision` | Updated Changeset, `ReviewNote` appended |
| `Publish` | `changeset_id: Uuid` | Publish metadata, audit chain entry, publish event emitted |
| `GetEditSchema` | `role: Role, object_type: String, context: Option<EditContext>` | Structured edit schema (§9.12): fields, visibility rules, defaults, cross-field constraints |
| `ValidateEdit` | `item: ChangesetItem` | Guardrail results (blocking/warning/advisory) |
| `ApplyTemplate` | `changeset_id: Uuid, template_fqn: String` | Changeset pre-populated from template |
| `ExplainConstraint` | `guardrail_id: String` | Human-readable explanation + remediation steps |
| `ResolveConflict` | `changeset_id: Uuid, conflict: Conflict` | Resolution options (merge/rebase/supersede) |
| `SuggestRemediation` | `gate_result: GateResult` | Ordered remediation actions |
| `ListUnboundVerbContracts` | `domain: Option<String>` | Active VerbContracts that lack an execution binding |
| `BindVerbImplementation` | `verb_fqn: String, binding: VerbImplementationBinding` | Binding record (draft/active depending on workflow) |
| `VerifyVerbBindings` | `scope: Option<Vec<String>>` | Coverage report: contract↔binding↔implementation status |
| `ExplainVerbCoverageGap` | `verb_fqn: String` | Human-readable explanation + remediation steps |
| `CheckIntentResolutionReadiness` | `verb_fqn: Option<String>, domain: Option<String>` | Resolution readiness per VerbContract: missing examples, guidance, source hints |

**Idempotency:** All mutating stewardship tools (`ComposeChangeset`, `AddItem`, `RefineItem`, `Publish`, etc.) accept an optional `client_request_id: Option<Uuid>` parameter. If a request with the same `client_request_id` has already been processed, the tool returns the previous result without re-executing. This prevents duplicate Draft snapshots on retries (network failures, agent re-attempts) and is essential for reliable agent-driven workflows.

### 6.3 Resolution Tools

These tools are consumed by the **execution/intent pipeline**, not by the stewardship agent. They are listed here because Semantic OS publishes them as part of its consumer-facing API.

| Tool | Parameters | Returns |
|---|---|---|
| `GetActionSurface` | `context: ResolutionContext, overlay: Option<OverlayMode>, assume_principal: Option<Principal>` | ActionSurface: available verbs with pre-computed eligibility (§9.9). When overlay = DraftOverlay, computes against Active ∪ Changeset.Drafts. When assume_principal set, computes ABAC as that principal (§2.3.4). |
| `GetVerbCompilationSpec` | `verb_fqn: String, context: ResolutionContext, overlay: Option<OverlayMode>, assume_principal: Option<Principal>` | Full compilation spec: inputs, outputs, preconditions, evidence, binding, guidance (§9.10) |
| `ValidatePlan` | `plan: ExecutionPlan` | PlanValidationResult: step-by-step validation, policy violations, missing evidence (§9.11) |
| `RefreshActionSurface` | `scope: ActionSurfaceScope` | Triggers recomputation for specified scope (admin/publish-event use) |
| `GetEmbeddingManifest` | `scope: Option<DomainFQN>` | Current embedding index metadata: what's embedded, staleness, version |

### 6.4 Visualisation Tools

These tools supply deterministic viewport models for the Show Loop. They are consumed by the Workbench UI (and often invoked indirectly via ShowPackets from the agent).

| Tool | Parameters | Returns |
|---|---|---|
| `GetFocusState` | `session_id: Uuid` | Current focus: `changeset_id`, `object_refs[]`, `resolution_context`, `overlay_mode` |
| `GetViewportModel` | `viewport: ViewportSpec` | Deterministic `ViewportModel` for a requested lens (§9.14.6) |
| `GetDiffModel` | `object_ref: ObjectRef, overlay: OverlayMode` | Structured diff: predecessor vs draft + field-level diffs + human-readable summary (§9.14.6) |
| `GetTaxonomyOverlay` | `taxonomy_fqn: String, overlay: OverlayMode` | Taxonomy tree/graph with membership overlays |
| `GetActionSurfacePreview` | `context: ResolutionContext, overlay: OverlayMode, assume_principal: Option<Principal>` | ActionSurface + ActionSurfaceDelta: eligibility deltas (Active vs Draft overlay) (§9.15). When assume_principal set, previews as that ABAC context. |
| `GetImpactPreview` | `changeset_id: Uuid` | Impact graph preview + affected objects list (streamable) |
| `GetGateViewport` | `changeset_id: Uuid` | Gate results rendered as a viewport model (blocking/warn/advisory + remediation) |

> Note: ShowPackets are a response contract emitted by the agent, not a tool. The tools above supply deterministic models the UI can render.

---

## 7. DSL Surface (Proposal Expressions)

The stewardship agent expresses proposals in a structured but semi-formal notation. This is the agent's output format — the human never writes it directly. It is rendered in the Workbench as the diff view and item editor.

```
PROPOSE AttributeDef
  fqn:                  "regulatory.fatca.us_tax_classification"
  display_name:         "US Tax Classification"
  description:          "FATCA Chapter 4 entity classification category"
  data_type:            Enum(FatcaTaxClassification)
  governance_tier:      Governed
  trust_class:          Proof
  steward:              "compliance-team"
  evidence_requirements: [W8_BEN, W8_BEN_E, W9]
  security_labels:      [PII, TaxIdentifier]
  basis_ref:            "basis:fatca-2026-q1"
  taxonomy:             [RegDomain.FATCA, KYC.TaxStatus]

PROPOSE Promotion
  fqn:                  "entity.tax_status"
  from_tier:            Operational
  to_tier:              Governed
  to_trust:             DecisionSupport
  migration_guidance:   "Map legacy enum values to FatcaTaxClassification via mapping table"
  observation_policy:   Grandfathered  // see §9.3
  basis_ref:            "basis:fatca-2026-q1"

PROPOSE PolicyRule
  fqn:                  "policy.fatca.reporting_obligation"
  predicate:            us_tax_classification IS_CLASSIFIED AND giin_number IS_PRESENT
  predicate_trust_minimum: Proof
  action:               REQUIRE_FILING(Form1042S)
  basis_ref:            "basis:fatca-2026-q1"

PROPOSE Deprecation
  fqn:                  "entity.legacy_status_code"
  replacement_fqn:      "entity.entity_lifecycle_state"
  migration_guidance:   "Map legacy codes via entity_migration_v2 procedure"
  sunset_expectation:   "2026-Q3"
  observation_policy:   PrePromotionFlagged

PROPOSE VerbContract
  fqn:                  "verb.fatca.assess_tax_status"
  display_name:         "Assess FATCA Tax Status"
  description:          "Determine US tax classification for an entity under FATCA Chapter 4"
  category:             Assessment
  entity_type:          "entity.institutional_client"
  inputs:
    - fqn: "regulatory.fatca.us_tax_classification", required: true, source_hint: FromEvidence
    - fqn: "regulatory.fatca.giin_number", required: true, source_hint: FromEvidence
    - fqn: "entity.country_of_incorporation", required: true, source_hint: FromCase
  outputs:
    - fqn: "regulatory.fatca.reporting_obligation_status", produces: true
  preconditions:
    - "entity.country_of_incorporation IS_PRESENT"
    - "regulatory.fatca.w8_ben_doc_ref IS_PRESENT OR regulatory.fatca.w9_doc_ref IS_PRESENT"
  postconditions:
    - "regulatory.fatca.reporting_obligation_status IS_CLASSIFIED"
  exec_mode:            Sync
  usage_examples:
    - utterance: "What's the FATCA status for this client?"
      slot_bindings: { entity: current_case_entity }
    - utterance: "Determine if we have FATCA reporting obligations for Acme Corp"
      slot_bindings: { entity: "Acme Corp" }
  parameter_guidance:
    - input_fqn: "regulatory.fatca.us_tax_classification"
      guidance: "The FATCA entity classification — select from the W-8 or W-9 form submitted"
  typical_predecessors:  ["verb.kyc.collect_entity_documents"]
  typical_successors:    ["verb.fatca.file_form_1042s", "verb.fatca.apply_withholding_exemption"]
  basis_ref:            "basis:fatca-2026-q1"
```

The notation is declarative and referentially transparent. All FQN references must resolve at gate pre-check time (either to Active snapshots or to other items in the same Changeset).

---

## 8. Guardrail Taxonomy

Guardrails are evaluated during editing in the Workbench (and via `ValidateEdit`). They are distinct from gate checks: guardrails fire early and explain; gates are the final enforcement boundary.

### 8.1 Guardrail Classification

| Severity | Behaviour | Override |
|---|---|---|
| **Block** | Edit cannot be saved; user must resolve | No override; some blocks can be escalated to PlatformSteward |
| **Warning** | Edit can be saved; must be acknowledged before submit-for-review | AcknowledgedBy recorded in audit |
| **Advisory** | Informational; no action required | No action |

### 8.2 Guardrail Table

| ID | Name | Trigger | Severity | Remediation |
|---|---|---|---|---|
| G01 | RolePermission | Field being edited is not permitted for the user's role | Block | Request role elevation or use permitted fields only |
| G02 | NamingConvention | FQN does not match domain naming pattern | Warning | Apply suggested FQN from template |
| G03 | TypeConstraint | Data type incompatible with governance tier | Block | Change type or adjust tier |
| G04 | ProofChainCompatibility | Attribute participates in policy predicate but tier < Proof | Block | Set tier/trust to Governed/Proof; attach evidence requirements |
| G05 | ClassificationRequired | Object in regulated domain missing taxonomy membership | Block | Add taxonomy membership before submit |
| G06 | SecurityLabelRequired | Object with PII/TaxIdentifier semantics missing security label | Block | Apply security label from suggested set |
| G07 | SilentMeaningChange | Type change attempted without migration note | Block | Attach migration guidance or deprecate + replace |
| G08 | DeprecationWithoutReplacement | Deprecation without `replacement_fqn` and `migration_guidance` | Block | Specify replacement; auto-generate migration note template |
| G09 | AIKnowledgeBoundary | Agent confidence below threshold for a claim in Basis | Advisory | Review claim; mark as OpenQuestion or supply internal reference |
| G10 | ConflictDetected | Changeset modifies an FQN also modified in another open Changeset | Warning | Resolve via merge/rebase/supersede before submit |
| G11 | StaleTemplate | Template used is below current version | Warning | Upgrade to current template version or acknowledge delta |
| G12 | ObservationImpact | Promotion would affect existing observations | Warning | Choose observation policy (Grandfathered / Invalidated / PrePromotionFlagged) |
| G13 | ResolutionMetadataMissing | User-facing VerbContract missing usage examples, parameter guidance, or input source hints | Warning (configurable to Block per domain) | Add usage examples; complete parameter guidance for UserProvided inputs; assign source hints |
| G14 | CompositionHintStale | VerbContract's `typical_predecessors` or `typical_successors` reference non-Active VerbContracts | Advisory | Update composition hints or remove stale references |
| G15 | DraftUniquenessViolation | `AddItem` or `RefineItem` would create a second current Draft head for the same `(object_type, object_id)` in the Changeset | Block | Supersede the existing Draft via `RefineItem` instead of `AddItem` |

### 8.3 Guardrail → Journey Mapping

| Guardrail | Journey A (FATCA Add) | Journey B (Promotion) | Journey C (Deprecation) |
|---|---|---|---|
| G04 ProofChainCompatibility | Fires in A5 | Fires in B3 | — |
| G05 ClassificationRequired | Fires in A6 | Fires in B4 | Fires in C4 |
| G07 SilentMeaningChange | — | Fires in B3 | — |
| G08 DeprecationWithoutReplacement | — | — | Fires in C2 |
| G12 ObservationImpact | — | Fires in B4 | — |
| G13 ResolutionMetadataMissing | Fires in A7 (new verb) | — | — |

---

## 9. Data Model

### 9.1 Changeset

A Changeset is the governance wrapper for a cohesive set of registry changes. It has two storage layers and **one identity**.

**Identity: `changeset_id == snapshot_set_id`.** A Changeset and its `sem_reg.snapshot_set` share a single UUID. There is no indirection — when you know the Changeset ID, you know the snapshot_set_id, and vice versa. This eliminates a join, simplifies every WHERE clause that bridges stewardship and registry layers, and makes the relationship unambiguous.

**Registry layer (`sem_reg` schema):** Proposed items are stored as `sem_reg.snapshots` with `status = Draft`, grouped in the `snapshot_set` identified by the Changeset's UUID. This means Drafts participate in the same typed body deserialization, resolution logic, diff infrastructure, and ABAC enforcement as Active snapshots. The execution agent never sees Drafts because it only reads `status = Active` with `effective_until IS NULL`. Draft Overlay preview and gate pre-check work by including the Changeset ID in the resolution WHERE clause — not by querying a separate store.

**Stewardship layer (stewardship tables):** The governance metadata — intent, Basis, review notes, guardrail log, conflict records, lifecycle status — lives in stewardship-specific tables keyed by the same `changeset_id`. This metadata does not belong in `sem_reg.snapshots` because it is about the *authoring process*, not the *registry content*.

On publish: gates run → Draft snapshots flip to `status = Active` → predecessors supersede (`effective_until` set) → effective windows activate → publish event emits — all in one DB transaction. On rejection: Draft snapshots are marked rejected (or deleted, per platform policy); the stewardship record preserves the audit chain regardless.

**Snapshot mutability rule:** Draft snapshots are **pre-release mutable artifacts**. They may be updated in place (status flip on publish) or superseded (effective_until set on refinement). Active snapshots are **immutable** — the only permitted mutation is setting `effective_until` on a predecessor when it is superseded by a newly published successor. This distinction is intentional: Drafts are working state; Active snapshots are the auditable, immutable record of what was published. The audit chain for Draft evolution is preserved via supersede chains (each refinement creates a new Draft head and closes the prior one), not via in-place mutation logging.

```
Changeset {
  id:             Uuid,                      // == snapshot_set_id in sem_reg (single identity)
  intent:         String,                    // human's original statement
  status:         ChangesetStatus,           // Draft | UnderReview | Approved | Published | Rejected
  items:          Vec<ChangesetItem>,
  basis_id:       Option<Uuid>,              // attached Basis entity
  template_fqn:   Option<String>,            // template applied at creation
  revision:       u32,                       // incremented on each Refine
  created_by:     Principal,
  created_at:     Timestamp,
  submitted_at:   Option<Timestamp>,
  gate_results:   Option<Vec<GateResult>>,
  review_notes:   Vec<ReviewNote>,
  conflict_log:   Vec<ConflictRecord>,
  publish_meta:   Option<PublishMeta>,
}

ChangesetItem {
  id:             Uuid,
  snapshot_id:    Uuid,                      // references sem_reg.snapshots.snapshot_id (status = Draft)
  action:         ItemAction,                // Add | Modify | Promote | Deprecate | Alias
  object_type:    RegistryObjectType,        // AttributeDef | PolicyRule | EvidenceReq | TaxonomyMembership | VerbContract | ...
  fqn:            String,
  predecessor_id: Option<Uuid>,             // for Modify / Promote / Deprecate
  reasoning:      String,                    // agent's reasoning for this item
  cross_refs:     Vec<CrossRef>,
  guardrail_log:  Vec<GuardrailResult>,
  revision:       u32,
}

ItemAction   = Add | Modify | Promote | Deprecate | Alias
ChangesetStatus = Draft | UnderReview | Approved | Published | Rejected
```

**Draft uniqueness invariant:** Within a Changeset, there is at most **one current Draft head** per `(object_type, object_id)`. When the agent refines an item (§5.1), the prior Draft snapshot is superseded (its `effective_until` is set to the refinement timestamp) and a new Draft snapshot is written as the current head. This prevents ambiguous resolution ("which Draft of this attribute is the current one in this Changeset?").

`object_id` is deterministically derived from FQN in the kernel, so uniqueness on `object_id` implies uniqueness on FQN.

Enforced by:
- A partial UNIQUE constraint on `sem_reg.snapshots`: `UNIQUE (snapshot_set_id, object_type, object_id) WHERE status = 'draft' AND effective_until IS NULL`
- Guardrail G15 (DraftUniquenessViolation) fires as Block if the constraint would be violated during `AddItem` or `RefineItem`

**Why Drafts live in `sem_reg.snapshots`:** If Drafts were stored outside the registry, the platform would need to duplicate resolution logic (Active + external Draft store), diffing logic, typed body deserialization, and audit reconstruction. The Draft Overlay preview (§2.3.4) would compute against a different data path than the real publish — exactly the lie §2.3.4 prohibits. Storing Drafts in the kernel with status-based isolation eliminates this duplication and guarantees preview fidelity.

### 9.2 Gate Pre-Check with Intra-Changeset Resolution

Because Draft snapshots are stored in `sem_reg.snapshots` (§9.1), gate pre-check resolves intra-Changeset references by including the `changeset_id` (which is the `snapshot_set_id`) in the resolution scope: Draft snapshots in the current Changeset are treated as provisionally Active for dependency resolution within the check. This is mandatory for coherent proposals: a PolicyRule referencing a new AttributeDef in the same Changeset must be validatable without requiring the AttributeDef to be published first.

Mechanically, the gate engine receives the `changeset_id` and adds a WHERE clause: `(status = 'active' AND effective_until IS NULL) OR (snapshot_set_id = $changeset_id AND status = 'draft' AND effective_until IS NULL)`. The `effective_until IS NULL` on both branches ensures only current heads are included — excluding superseded Active history and superseded Draft refinements. This is a scope extension, not a separate resolution path.

The provisional scope is limited to the pre-check invocation and does not affect the execution agent's view of Active snapshots.

**The same mechanism is used by Draft Overlay preview** (§2.3.4): the Resolution Contract tools (`GetActionSurface`, `GetVerbCompilationSpec`) accept `overlay: OverlayMode` and, when set to `DraftOverlay(changeset_id)`, compute against the same extended scope. This ensures the preview, the gate pre-check, and the eventual publish all use identical resolution logic.

### 9.3 Basis (First-Class Entity)

Basis is a mandatory audit artifact attached to a Changeset. It is a first-class registered entity — snapshotted, versioned, and preserved in the audit chain.

```
Basis {
  id:             Uuid,
  changeset_id:   Uuid,
  claims:         Vec<Claim>,
  open_questions: Vec<String>,
  references:     Vec<BasisReference>,       // internal policy docs or curated external refs
  created_by:     Principal,
  created_at:     Timestamp,
}

Claim {
  id:             Uuid,
  claim_type:     ClaimType,                 // RegulatoryFact | MarketPractice | PlatformConvention
  statement:      String,
  confidence:     Confidence,                // High | Medium | Low
  source:         Option<String>,            // internal reference or curated source
  ai_generated:   bool,                      // true if agent-authored; false if human-supplied
}

ClaimType  = RegulatoryFact | MarketPractice | PlatformConvention
Confidence = High | Medium | Low
```

**AI Knowledge Boundary (G09):** When `ai_generated = true` and `confidence = Low`, guardrail G09 fires as Advisory — the claim should be reviewed and either confirmed with an internal reference or moved to `open_questions`.

Basis references may cite internal policy documents and curated summaries. External regulatory text references are permitted when cited by document name and section. The agent must be explicit about knowledge boundaries and confidence.

### 9.4 StewardshipRecord and Audit Chain

```
StewardshipRecord {
  id:             Uuid,
  changeset_id:   Uuid,
  event_type:     StewardshipEvent,
  actor:          Principal,
  timestamp:      Timestamp,
  payload:        JsonValue,                 // event-specific data
  viewport_manifest: Option<ViewportManifest>,  // what was shown when this event occurred (v0.82)
}

StewardshipEvent = 
  | ChangesetCreated
  | ItemAdded | ItemRemoved | ItemRefined
  | BasisAttached
  | GuardrailFired { id: GuardrailId, severity: Severity, resolution: String }
  | GatePrechecked { result: GateResult }
  | SubmittedForReview
  | ReviewNoteAdded
  | ReviewDecisionRecorded { disposition: ReviewDisposition }
  | FocusChanged { from: FocusState, to: FocusState, source: FocusUpdateSource }
  | Published
  | Rejected

ReviewNote {
  id:             Uuid,
  changeset_id:   Uuid,
  reviewer:       Principal,
  note:           String,
  disposition:    ReviewDisposition,         // Approve | RequestChange | Reject
  timestamp:      Timestamp,
  viewport_manifest: Option<ViewportManifest>,  // what was shown when this review was recorded (v0.82)
}

ReviewDisposition = Approve | RequestChange | Reject
```

The audit chain is the ordered sequence of `StewardshipRecord` entries for a Changeset. It is immutable after append and must be complete for a Changeset to be publishable.

**ViewportManifest (v0.82, hashing spec v0.83, impersonation v1.0):** When a human makes a decision (review, approval, guardrail acknowledgement), the audit record captures what the Workbench was displaying. This supports regulatory reconstruction — "what did the reviewer see when they approved?"

```
ViewportManifest {
  captured_at:        Timestamp,
  focus_state:        FocusState,
  rendered_viewports: Vec<ViewportRef>,
  overlay_mode:       OverlayMode,
  assumed_principal:  Option<Principal>,      // if Draft Overlay used impersonation (§2.3.4), which principal was assumed
}

ViewportRef {
  viewport_id:    String,
  kind:           ViewportKind,
  data_hash:      String,                   // SHA-256 of canonical JSON (see hashing spec below)
  registry_version: SnapshotSetId,          // which published state this viewport was computed from
  tool_call_ref:  Option<String>,           // which tool call produced this viewport's data
}
```

**Canonical hashing for audit integrity:**
- **Algorithm:** SHA-256
- **Canonicalisation:** RFC 8785 (JSON Canonicalization Scheme — deterministic key ordering, no insignificant whitespace)
- **Hashed material:** The `ViewportModel.data` JSON, prepended with `overlay_mode`, `registry_version`, and `assumed_principal` (if present) as structured prefix fields. This ensures the hash is reconstructable: given the registry state at `registry_version`, the `overlay_mode`, and the assumed ABAC context, the tool call should reproduce identical data and therefore an identical hash.
- **Purpose:** Lightweight integrity proof. The full viewport data is not stored in the audit chain — it can be reconstructed from the tool call reference and registry state at `captured_at`. The hash proves the reconstruction matches what was originally displayed.

### 9.5 Template (Stewardship-Layer Object)

Templates are **versioned, gate-checked stewardship objects** stored in stewardship-layer tables. They are governed by the same Changeset lifecycle as registry content (propose, review, approve), but they do not require a new `sem_reg` object type in v1.

**Why stewardship-layer in v1:** The `sem_reg.snapshots` table has a fixed `object_type` enum. Adding `TemplateDef` would require a kernel schema migration, which is a breaking change across the entire resolution pipeline. Templates are consumed only by the stewardship agent (to pre-populate Changesets), never by the execution agent or intent pipeline. Storing them in stewardship tables avoids a kernel schema delta while preserving full versioning and governance.

**Path to SemReg promotion:** When the kernel `object_type` enum is next extended (e.g., to add `OrchestrationType` for verb composition, Q10), templates can be promoted to a SemReg object type in the same migration. Until then, stewardship-layer storage is sufficient and correct.

```
Template {
  id:             Uuid,
  fqn:            String,                    // e.g. "template.regulatory.fatca-starter"
  display_name:   String,
  version:        SemanticVersion,
  domain:         String,
  scope:          Vec<EntityType>,
  items:          Vec<TemplateItem>,         // pre-populated ChangesetItem skeletons
  steward:        String,
  basis_ref:      Option<Uuid>,
  status:         TemplateStatus,            // Draft | Active | Deprecated
  created_at:     Timestamp,
  created_by:     Principal,
}

TemplateStatus = Draft | Active | Deprecated
```

Templates are versioned. When a Template is updated, existing Changesets that used the prior version fire guardrail G11 (StaleTemplate) as a Warning.

### 9.6 Conflict Model

A conflict exists when two or more open Changesets (status = Draft or UnderReview) contain items that modify the same FQN.

```
ConflictRecord {
  id:             Uuid,
  changeset_id:   Uuid,
  competing_changeset_id: Uuid,
  fqn:            String,
  detected_at:    Timestamp,
  resolution:     Option<ConflictResolution>,
}

ConflictResolution {
  strategy:       ConflictStrategy,          // Merge | Rebase | Supersede
  decision_by:    Principal,
  decision_at:    Timestamp,
  rationale:      String,
}

ConflictStrategy = Merge | Rebase | Supersede
```

**Merge:** The two items are combined by a human steward; both Changesets update to reference the merged item.  
**Rebase:** One Changeset is updated to incorporate the published result of the other.  
**Supersede:** One Changeset explicitly replaces the other; the superseded Changeset is rejected with a recorded rationale.  

Conflict detection fires guardrail G10 (ConflictDetected) as Warning on the later Changeset. First-published-wins is the default if no resolution is recorded before publish — a ConflictRecord with null resolution and `Supersede` strategy is written automatically referencing the earlier-published Changeset.


### 9.7 VerbImplementationBinding (Bridge Record)

A binding links an Active `VerbContract` to an execution implementation reference. This is not "the implementation" — it is the registry's authoritative statement of **how** the platform executes the verb, enabling coverage checks and publish gating for user-facing verbs.

```
VerbImplementationBinding {
  id:             Uuid,
  verb_fqn:        String,                 // matches VerbContract.fqn
  binding_kind:    BindingKind,            // RustHandler | BpmnProcess | RemoteHttp | MacroExpansion
  binding_ref:     String,                 // function path / BPMN id / URL / macro id
  exec_modes:      Vec<ExecMode>,          // Sync | DurableStart | DurableResume (if applicable)
  status:          BindingStatus,          // Draft | Active | Deprecated
  last_verified_at: Option<Timestamp>,
  notes:           Option<String>,
}

BindingKind   = RustHandler | BpmnProcess | RemoteHttp | MacroExpansion
BindingStatus = Draft | Active | Deprecated
```

Bindings may be governed (for high-risk domains) or operational (for platform wiring). The publish pipeline may enforce "binding required" for user-facing verbs, while allowing internal/admin verbs to remain unbound during early authoring.

**Storage:** VerbImplementationBindings are stored in **stewardship-layer tables** (like Templates, §9.5), not as a `sem_reg` object type. They have their own lifecycle (Draft/Active/Deprecated) managed via stewardship tools (`BindVerbImplementation`, `VerifyVerbBindings`). Resolution tools join Active bindings by `verb_fqn` when returning `VerbCompilationSpec` (§9.10). This avoids a kernel schema delta while keeping bindings queryable for coverage checks and gating.

### 9.8 Publish Event

A successful publish emits a single structured registry event carrying the published SnapshotSet identity, metadata, and downstream processing directives. Downstream consumers subscribe to this event rather than coupling directly to registry tables. This keeps publish atomic and downstream processing deterministic.

```
PublishEvent {
  event_id:           Uuid,
  changeset_id:       Uuid,
  snapshot_set_id:    SnapshotSetId,
  published_at:       Timestamp,
  published_by:       Principal,
  
  // Items published (summary)
  published_items:    Vec<PublishedItemSummary>,
  
  // Downstream processing directives
  stale_embeddings:   Vec<FQN>,              // embeddings that must be recomputed
  stale_action_surfaces: Vec<ActionSurfaceScope>,  // Action Surface scopes to invalidate/recompute
  stale_links:        Vec<StaleLink>,        // cross-Changeset consistency debt (if Warning-level enforcement)
  
  // Coverage impact
  coverage_revalidation_scope: Vec<DomainFQN>,  // domains requiring coverage report refresh
}

PublishedItemSummary {
  fqn:            String,
  object_type:    RegistryObjectType,
  action:         ItemAction,
  snapshot_id:    SnapshotId,
}

StaleLink {
  source_fqn:     String,                   // the object that changed
  affected_fqn:   String,                   // the object whose link is now potentially stale
  link_type:      String,                   // consumes / produces / governs / etc.
  staleness_reason: String,                 // human-readable explanation
}

ActionSurfaceScope {
  entity_type:    Option<EntityTypeFQN>,
  jurisdiction:   Option<Vec<Jurisdiction>>,
  domain:         Option<Vec<DomainFQN>>,
}
```

Downstream consumers triggered by the publish event include: Action Surface projection refresh, embedding recomputation, coverage report revalidation, drift dashboard update, and intent pipeline cache invalidation.


### 9.9 Action Surface

The Action Surface is the answer to "what can I do right now?" — a **computed projection** of the registry state scoped by a Resolution Context. It is the primary interface through which the execution/intent agent discovers available, eligible verbs.

The Action Surface is built on top of the implemented 12-step context resolution pipeline (§2.7.3). It is not a parallel computation — it is a materialised, enriched projection of `resolve_context()` output.

```
ActionSurface {
  context:            ResolutionContext,       // the input scope
  computed_at:        Timestamp,
  registry_version:   SnapshotSetId,          // which published state this was computed from
  overlay_mode:       OverlayMode,            // ActiveOnly or DraftOverlay(changeset_id)

  available_verbs:    Vec<ActionableVerb>,
  available_nouns:    Vec<AccessibleNoun>,     // attributes readable/writable in context
  active_policies:    Vec<ApplicablePolicy>,   // policies that constrain this context
}

ActionableVerb {
  verb_fqn:           String,
  display_name:       String,
  description:        String,                  // for LLM intent matching
  category:           VerbCategory,            // Query | Mutation | Assessment | Filing | ...

  // Pre-computed eligibility
  preconditions_met:  Vec<PreconditionStatus>, // which preconditions are satisfied/unsatisfied
  actionable:         bool,                    // ALL required preconditions satisfied
  blocked_reason:     Option<String>,          // if not actionable, why not

  // Input surface (summary — full spec via VerbCompilationSpec)
  required_inputs:    Vec<InputSummary>,
  optional_inputs:    Vec<InputSummary>,

  // What this verb produces
  output_summary:     Vec<OutputSummary>,
  side_effects:       Vec<SideEffectSummary>,

  // Embedding reference for semantic search
  embedding_id:       EmbeddingRef,
}

PreconditionStatus {
  predicate:          String,                  // human-readable
  satisfied:          SatisfactionState,       // Yes | No | Unknown (see below)
  satisfiable:        bool,                    // CAN it be satisfied (evidence collectible vs structurally impossible)
  blocking:           bool,                    // hard block vs advisory
}

SatisfactionState = Yes | No | Unknown
```

**`SatisfactionState.Unknown`:** The static Action Surface layer (materialised on publish, cached) does not have case-specific state. Preconditions that depend on case evidence or runtime state are marked `Unknown` in the static surface. The dynamic eligibility overlay (computed at query time with case context) resolves `Unknown` to `Yes` or `No`. This prevents the static surface from making claims it can't back up — the intent pipeline only treats a verb as `actionable = true` when all required preconditions are `Yes`, never when any are `Unknown`.

```

AccessibleNoun {
  fqn:                String,
  display_name:       String,
  data_type:          DataType,
  governance_tier:    GovernanceTier,
  trust_class:        TrustClass,
  access:             NounAccess,              // Read | Write | ReadWrite
}

ApplicablePolicy {
  fqn:                String,
  description:        String,
  constrains:         Vec<String>,             // FQNs of verbs/nouns this policy governs in context
}

InputSummary {
  name:               String,
  fqn:                String,
  data_type:          DataType,
  source_hint:        InputSourceHint,
}

OutputSummary {
  name:               String,
  fqn:                String,
  data_type:          DataType,
}

SideEffectSummary {
  description:        String,
  affected_fqns:      Vec<String>,
}

NounAccess     = Read | Write | ReadWrite
VerbCategory   = Query | Mutation | Assessment | Filing | Notification | Orchestration
InputSourceHint = FromCase | FromEvidence | UserProvided | Derived | PolicyDefault
```

**Design rationale:** The Action Surface pre-computes precondition status. The intent agent does not receive a verb contract and then try to figure out if it can be invoked. The Action Surface tells it: "here are 12 verbs available in this context; 8 are actionable now; 3 need evidence you haven't collected yet; 1 is blocked by a policy constraint." This makes LLM reasoning tractable — the agent reasons over a scoped, pre-validated menu, not an unbounded catalogue.


### 9.10 Verb Compilation Spec

Once the intent agent has selected a verb (via LLM reasoning over the Action Surface), it needs the **full specification** to assemble a plan. The Verb Compilation Spec is a richer view than the ActionableVerb summary, carrying everything needed for slot filling, parameter binding, and plan assembly.

```
VerbCompilationSpec {
  verb_fqn:           String,
  contract_version:   SnapshotId,

  // Full input schema
  inputs:             Vec<InputSpec>,

  // Full output schema
  outputs:            Vec<OutputSpec>,

  // Preconditions as evaluable predicates
  preconditions:      Vec<Predicate>,

  // Postconditions (what must be true after execution)
  postconditions:     Vec<Predicate>,

  // Evidence requirements
  required_evidence:  Vec<EvidenceSpec>,

  // Policy constraints active for this verb in context
  governing_policies: Vec<PolicyConstraint>,

  // Execution binding (how the DSL runtime dispatches this)
  binding:            VerbImplementationBinding,

  // Composition hints (if this verb participates in known sequences)
  typical_predecessors: Vec<VerbFQN>,
  typical_successors:   Vec<VerbFQN>,

  // For LLM prompt construction and intent pipeline consumption
  usage_examples:     Vec<UsageExample>,       // governed, authored via stewardship
  parameter_guidance: Vec<ParameterGuidance>,  // human-readable input descriptions
}

InputSpec {
  name:               String,
  fqn:                String,                  // references an AttributeDef
  data_type:          DataType,
  required:           bool,
  source_hint:        InputSourceHint,         // FromCase | FromEvidence | UserProvided | Derived | PolicyDefault
  default_derivation: Option<DerivationFQN>,   // if source_hint = Derived, how to compute it
  current_value:      Option<JsonValue>,       // if available from case context, pre-resolved
}

OutputSpec {
  name:               String,
  fqn:                String,
  data_type:          DataType,
  produces:           bool,                    // true = this verb creates/updates this attribute
}

EvidenceSpec {
  evidence_fqn:       String,
  description:        String,
  required:           bool,
  alternatives:       Vec<String>,             // alternative evidence that also satisfies
}

PolicyConstraint {
  policy_fqn:         String,
  description:        String,
  constraint_type:    ConstraintType,          // Requires | Prohibits | ConditionallyRequires
  parameters:         JsonValue,               // policy-specific parameters
}

UsageExample {
  utterance:          String,                  // natural language example
  slot_bindings:      Map<String, String>,     // how utterance elements map to inputs
  notes:              Option<String>,          // additional context for the example
}

ParameterGuidance {
  input_fqn:          String,
  guidance:           String,                  // human-readable: "The FATCA entity classification — select from W-8 or W-9"
  valid_values:       Option<Vec<String>>,     // if enumerated
  format_hint:        Option<String>,          // "ISO 3166-1 alpha-2 country code"
}

ConstraintType = Requires | Prohibits | ConditionallyRequires
```

**`InputSourceHint` is architecturally significant.** It tells the intent agent not just what the verb needs but **where to get it**: "this input is already available from the case state" vs "this needs to be asked from the user" vs "this can be derived from these other attributes" vs "a policy provides a default." This information comes from the managed links in Semantic OS, not from the agent's own reasoning. It is the mechanism that turns parameter binding from an open-ended LLM inference problem into a structured lookup.


### 9.11 Plan Validation

Before the intent pipeline hands a compiled plan to the DSL runtime, Semantic OS validates it deterministically. This is a non-LLM validation — the agent proposes, Semantic OS validates, the DSL runtime only sees plans that pass.

```
ExecutionPlan {
  context:        ResolutionContext,
  steps:          Vec<PlannedStep>,
}

PlannedStep {
  verb_fqn:       String,
  bound_inputs:   Map<String, BoundValue>,
  evidence_refs:  Vec<EvidenceRef>,
  sequence:       u32,                       // ordering within the plan
}

BoundValue {
  source:         InputSourceHint,           // how this value was obtained
  value:          JsonValue,                 // the actual bound value (or reference)
  fqn:            String,                    // which AttributeDef this binds to
}

PlanValidationResult {
  valid:              bool,
  step_results:       Vec<StepValidation>,
  policy_violations:  Vec<PolicyViolation>,
  missing_evidence:   Vec<EvidenceGap>,
  unresolved_refs:    Vec<String>,           // FQNs that don't resolve to Active
  sequence_warnings:  Vec<SequenceWarning>,  // ordering issues based on precondition/postcondition chains
}

StepValidation {
  step_sequence:  u32,
  verb_fqn:       String,
  valid:          bool,
  issues:         Vec<ValidationIssue>,
}

ValidationIssue {
  severity:       IssueSeverity,             // Block | Warning
  category:       IssueCategory,
  description:    String,
  remediation:    Option<String>,
}

PolicyViolation {
  policy_fqn:     String,
  violated_at_step: u32,
  description:    String,
}

EvidenceGap {
  evidence_fqn:   String,
  required_by:    String,                    // which verb/policy requires it
  alternatives:   Vec<String>,
}

SequenceWarning {
  step_a:         u32,
  step_b:         u32,
  reason:         String,                    // e.g. "step B requires postcondition of step A but is sequenced before it"
}

IssueSeverity = Block | Warning
IssueCategory = UnresolvedFQN | TypeMismatch | PreconditionNotMet | PolicyViolation | EvidenceMissing | SequenceError | BindingMissing
```

**Sequence validation:** Plan Validation checks that verb ordering respects precondition/postcondition chains. If step 3 requires an attribute that step 5 produces, the plan is invalid regardless of whether each individual step is valid in isolation. This is a property of the managed links graph — the postconditions of one verb satisfy the preconditions of another, and the validator enforces that the ordering respects this dependency.


### 9.12 Edit Schema (Structural Model)

`GetEditSchema` returns a structured schema that the Workbench renders into object-specific editors. The schema carries conditional visibility, contextual defaults, and cross-field constraints — not just flat field lists. This is the mechanism that replaces "100 forms" without pushing conditional rendering logic into Workbench UI code.

```
EditSchema {
  object_type:        RegistryObjectType,
  role:               Role,
  fields:             Vec<EditFieldSpec>,
  cross_field_rules:  Vec<CrossFieldRule>,
  contextual_defaults: Vec<ContextualDefault>,
}

EditFieldSpec {
  name:               String,
  data_type:          DataType,
  required:           bool,
  permitted:          bool,                    // false = read-only for this role (G01 fires on edit attempt)
  visibility:         FieldVisibility,         // Always | Conditional
  visibility_condition: Option<VisibilityCondition>,
  validation:         Option<FieldValidation>,
  help_text:          Option<String>,
  suggested_values:   Option<Vec<SuggestedValue>>,
}

FieldVisibility = Always | Conditional
VisibilityCondition {
  depends_on:     String,                      // field name
  operator:       ConditionOperator,           // Equals | NotEquals | In | Present
  value:          JsonValue,
}

CrossFieldRule {
  id:             String,
  description:    String,                      // "If governance_tier = Governed, steward is required"
  trigger_field:  String,
  trigger_condition: VisibilityCondition,
  enforced_field: String,
  enforcement:    CrossFieldEnforcement,       // MakeRequired | MakeVisible | SetDefault | Block
  guardrail_ref:  Option<GuardrailId>,         // links to the guardrail this implements proactively
}

ContextualDefault {
  field:          String,
  condition:      DefaultCondition,            // domain, template, entity_type context
  default_value:  JsonValue,
  overridable:    bool,
}

DefaultCondition {
  context_type:   ContextType,                 // Domain | Template | EntityType | Jurisdiction
  context_value:  String,                      // e.g. "RegDomain.FATCA"
}

CrossFieldEnforcement = MakeRequired | MakeVisible | SetDefault | Block
ConditionOperator     = Equals | NotEquals | In | Present
ContextType           = Domain | Template | EntityType | Jurisdiction

SuggestedValue {
  value:          JsonValue,
  label:          String,
  source:         String,                      // "template" | "domain_convention" | "existing_registry"
}
```

**Example:** When a steward creates an AttributeDef in domain `RegDomain.FATCA`:
- `governance_tier` is visible (Always) with contextual default `Governed` (from domain convention, overridable)
- `steward` becomes Required when `governance_tier = Governed` (CrossFieldRule, linked to G05)
- `evidence_requirements` becomes visible when `trust_class = Proof` (VisibilityCondition)
- `security_labels` has suggested values `[PII, TaxIdentifier]` (from domain convention)

The Edit Schema is itself derived from registry state and role permissions. It could in principle be a governed registry object (schema policies), making the governance of governance editing recursive — but this is deferred to avoid premature complexity.

### 9.13 Embedding and Resolution Projection

Semantic OS maintains an embedding index that the intent pipeline uses for semantic search during verb/noun discovery. The embedding model is architecturally significant because it determines whether governed objects are *findable* by the intent pipeline.

#### What Gets Embedded

Each of the following Active registry object types gets its own embedding vector:

| Object Type | Embedding Input | Rationale |
|---|---|---|
| **VerbContract** | display_name + description + category + input/output FQN names + managed link context (see below) | Verbs must be findable by intent; link context ensures semantic proximity to related nouns |
| **AttributeDef** | display_name + description + data_type context + taxonomy memberships | Nouns must be discoverable for slot filling and cross-reference |
| **PolicyRule** | description + predicate in natural language + governed verb/noun FQNs | Policies must surface when the intent pipeline checks constraints |
| **EvidenceRequirement** | description + what it satisfies + document type context | Evidence must be findable when the agent determines what to collect |

#### Link-Enriched Embeddings

When embedding a VerbContract, the embedding input is **enriched with its managed links** — the consumes/produces/requires relationships are concatenated into the embedding text:

```
Embedding input for VerbContract "verb.fatca.assess_tax_status":
  "Assess FATCA Tax Status. Determines US tax classification for
   institutional clients under FATCA Chapter 4. Requires:
   us_tax_classification (Proof), giin_number (Proof). Produces:
   fatca_reporting_obligation_status. Evidence: W-8BEN, W-8BEN-E,
   W-9. Domain: RegDomain.FATCA, KYC.TaxStatus. Category: Assessment."
```

This ensures that an utterance like "what's the FATCA situation for this client" semantically matches the verb **because the embedding carries the noun and evidence context**, not just the verb's own description. The managed links in the registry become proximity in embedding space.

#### Scoping Invariants

- **Active only for execution:** Only Active snapshots are embedded in the execution-facing index. The execution/intent agent can never discover Draft objects through semantic search.
- **Draft index for stewardship:** Draft snapshots may be embedded in a separate stewardship-scoped index for discovery during authoring (e.g., "is there already a draft verb for this?"). This index is not accessible to the execution agent.
- **Partitioned by domain/entity-type:** The embedding index is partitioned so the Action Surface can pre-filter before semantic search. An intent query scoped to `entity.institutional_client` in `RegDomain.FATCA` searches only the relevant partition.
- **Publish event triggers refresh:** Embedding staleness is tracked per-object. The publish event (§9.8) includes `stale_embeddings` as a list of FQNs requiring recomputation. Refresh is asynchronous but must complete before the Action Surface projection is considered current.

#### Embedding Template (Governed, Optional)

The embedding input template — what gets concatenated to produce the vector — may itself be a governed schema per object type or per domain. This allows different domains to embed differently: a compliance verb might weight evidence requirements heavily in its embedding input, while an operational verb might weight input/output types. This is deferred as optional; a sensible default template per object type is sufficient for v1.


### 9.14 ShowPacket and Viewport Models

The Show Loop requires a stable render contract so the Workbench can render deterministic state quickly without bespoke screens.

#### 9.14.1 FocusState (Server-Side Shared Truth)

The minimal state required to keep a human oriented. **FocusState is stored server-side** (not just in UI memory) and serves as the single shared truth between agent and Workbench.

```
FocusState {
  session_id:          Uuid,
  changeset_id:        Option<Uuid>,
  overlay_mode:        OverlayMode,           // ActiveOnly | DraftOverlay(changeset_id)
  object_refs:         Vec<ObjectRef>,        // what we are editing/inspecting
  taxonomy_focus:      Option<TaxonomyFocus>, // optional navigation focus
  resolution_context:  Option<ResolutionContext>,  // for action surface preview
  updated_at:          Timestamp,             // last mutation timestamp
  updated_by:          FocusUpdateSource,     // Agent | UserNavigation
}

FocusUpdateSource = Agent | UserNavigation
```

**Update rules:**
- **Agent-emitted ShowPackets** set FocusState as part of the ShowPacket's `focus` field. This is the primary update path during stewardship workflows.
- **User navigation events** (clicking a taxonomy node, selecting a different Changeset item, toggling overlay mode) update FocusState directly via a Workbench API call. The agent is not involved but can query the current state via `GetFocusState(session_id)`.
- **Both update paths write to the same server-side record.** There is no separate "agent focus" and "UI focus" — they share one truth.
- **FocusState transitions are captured in the audit chain** (as `StewardshipEvent` entries), making the full navigation history replayable for regulatory reconstruction.

This extends the existing session context model in the ob-poc application (which already carries scope/cbu_ids on the session) with the stewardship-specific fields needed for Changeset navigation.

#### 9.14.2 OverlayMode

```
OverlayMode = ActiveOnly | DraftOverlay(changeset_id: Uuid)
```

OverlayMode is threaded through both Show Loop tools (§6.4) and Resolution Contract tools (§6.3). When `DraftOverlay(changeset_id)` is active, all tools resolve against `Active ∪ Changeset.Drafts` by extending the snapshot resolution scope to include the Changeset's `snapshot_set_id` (§9.2). Because Drafts are stored in `sem_reg.snapshots`, this is a WHERE clause change — not a separate resolution path.

#### 9.14.3 ShowPacket (agent → UI)

A structured instruction that tells the Workbench what to show next. Emitted by the agent (see emission contract §2.3.5).

```
ShowPacket {
  focus:          FocusState,
  viewports:      Vec<ViewportSpec>,         // which lenses to render/update
  deltas:         Option<Vec<ViewportDelta>>,  // optional incremental updates
  narrative:      Option<String>,            // optional human explanation
  next_actions:   Vec<SuggestedAction>,      // what the human can do next (closes the Refine step)
}
```

#### 9.14.4 SuggestedAction

The mechanism that closes the REPL loop — tells the human "here's what you can do next" with pre-validated enabled/disabled state.

```
SuggestedAction {
  action_type:    ActionType,
  label:          String,                    // human-readable: "Accept all items", "Run gate pre-check"
  target:         ActionTarget,              // what this action operates on
  enabled:        bool,                      // is this action currently valid?
  disabled_reason: Option<String>,           // if not enabled, why not
  keyboard_hint:  Option<String>,            // optional shortcut hint
}

ActionType = 
  | AcceptItem                              // accept a proposed item as-is
  | EditItem                                // open item in editor
  | RunGates                                // trigger gate pre-check
  | SubmitForReview                         // advance to review
  | RecordReview                            // approve/reject/request-change
  | Publish                                 // publish the changeset
  | ResolveConflict                         // resolve a detected conflict
  | AddEvidence                             // attach basis/evidence
  | ToggleOverlay                           // switch between ActiveOnly and DraftOverlay
  | NavigateToItem                          // jump focus to a specific item
  | Remediate                               // apply a guardrail remediation

ActionTarget {
  changeset_id:   Option<Uuid>,
  item_id:        Option<Uuid>,
  viewport_id:    Option<String>,
  guardrail_id:   Option<String>,
}
```

**Design rationale:** SuggestedActions are the Workbench's primary interaction surface. They replace free-form "what should I do next?" with a constrained, pre-validated menu. The `enabled` flag is computed from the current Changeset state — e.g., `Publish` is disabled until all gates pass and review is approved. This is the same philosophy as the Action Surface (P10): give the human a scoped menu, not an unbounded catalogue.

#### 9.14.5 ViewportStatus

Each viewport has a lifecycle state that the Workbench uses to render loading indicators, error states, and staleness warnings:

```
ViewportStatus = Ready | Loading { progress: Option<f32> } | Error { message: String } | Stale
```

ViewportStatus is carried on ViewportModel (not ViewportSpec — the spec is the request; the model is the response with its status).

#### 9.14.6 ViewportSpec / ViewportModel with Typed Data Shapes

```
ViewportSpec {
  id:             String,
  kind:           ViewportKind,              // Focus | Taxonomy | Object | Diff | Impact | ActionSurface | Gates | Coverage
  title:          String,
  params:         JsonValue,                 // deterministic tool parameters
  render_hint:    RenderHint,                // Tree | Graph | Table | Diff | Cards
}

ViewportModel {
  id:             String,
  kind:           ViewportKind,
  status:         ViewportStatus,            // Ready | Loading | Error | Stale
  data:           ViewportData,              // typed by kind (see below)
  meta:           ViewportMeta,
}

ViewportMeta {
  updated_at:     Timestamp,
  sources:        Vec<String>,               // tool call IDs that produced this data
  overlay_mode:   OverlayMode,
}
```

**Viewport data shapes per kind** (conceptual contracts for Workbench rendering):

```
// A. Focus Viewport
FocusViewportData {
  changeset_summary:   Option<ChangesetSummary>,  // id, intent, status, item_count, revision
  focused_objects:     Vec<ObjectSummary>,         // fqn, type, action, status
  resolution_context:  Option<ResolutionContext>,
  overlay_mode:        OverlayMode,
}

// B. Taxonomy Viewport
TaxonomyViewportData {
  root_fqn:           String,
  nodes:              Vec<TaxonomyNodeView>,       // id, label, children, membership_status
  membership_overlay: Vec<MembershipDelta>,        // Active vs Draft membership changes
}

// C. Object Inspector
ObjectInspectorData {
  snapshot:           SnapshotSummary,             // fqn, type, tier, trust, status, version
  memberships:        Vec<TaxonomyMembershipView>,
  security_label:     SecurityLabelView,
  consumers:          Vec<ConsumerRef>,            // verbs, derivations, policies consuming this object
  steward:            Option<String>,
}

// D. Diff Viewport
DiffViewportData {
  predecessor:        Option<SnapshotSummary>,     // null for new objects
  successor:          SnapshotSummary,
  field_diffs:        Vec<FieldDiff>,              // { field, old_value, new_value, change_type }
  human_summary:      String,                      // agent-generated plain-language diff summary
}

FieldDiff {
  field:              String,
  old_value:          Option<JsonValue>,
  new_value:          Option<JsonValue>,
  change_type:        FieldChangeType,             // Added | Removed | Modified | Unchanged
}

// E. Impact Viewport
ImpactViewportData {
  affected_verbs:     Vec<AffectedObjectRef>,      // fqn + impact description
  affected_policies:  Vec<AffectedObjectRef>,
  affected_derivations: Vec<AffectedObjectRef>,
  observation_count:  u32,                         // existing observations affected
  severity:           ImpactSeverity,              // Low | Medium | High | Critical
  stale_links:        Vec<StaleLink>,              // cross-Changeset consistency debt
}

// F. Action Surface Preview
ActionSurfacePreviewData {
  surface:            ActionSurface,               // the computed surface (Active or Draft overlay)
  delta:              Option<ActionSurfaceDelta>,   // what changed vs Active-only (§9.15)
}

// G. Gate Viewport
GateViewportData {
  results:            Vec<GateResultView>,         // guardrail_id, name, severity, passed, message
  blocking_count:     u32,
  warning_count:      u32,
  advisory_count:     u32,
  remediations:       Vec<RemediationView>,        // guardrail_id, action, one_click_available
  overall_publishable: bool,
}

// H. Coverage / Readiness
CoverageViewportData {
  resolution_readiness: Vec<ResolutionReadinessItem>,  // verb_fqn, check, status
  orphan_count:       u32,
  unsteward_count:    u32,
  drift_signals:      Vec<DriftSignal>,
  coverage_pct:       f32,                         // overall coverage metric
}
```

These shapes are the **render contracts** the Workbench builds against. They are stable across versions — new fields may be added but existing shapes are not broken.

#### 9.14.7 ViewportDelta

For "feels immediate" updates, the Workbench applies deltas when possible:

```
ViewportDelta {
  viewport_id:    String,
  op:             PatchOp,                   // Add | Remove | Replace | Move
  path:           String,                    // JSON Pointer into viewport data
  value:          Option<JsonValue>,         // the new value (for Add/Replace)
}
```

This keeps updates small and fast while maintaining determinism (all deltas derive from Semantic OS tool outputs).

### 9.15 ActionSurfaceDelta (New in v0.82)

When the Workbench is in Draft Overlay mode, the Action Surface Preview viewport must show **what changes** relative to the current Active-only surface. The delta is the mechanism:

```
ActionSurfaceDelta {
  verbs_added:        Vec<ActionableVerbSummary>,   // new verbs that become available
  verbs_removed:      Vec<VerbFQN>,                 // verbs that are no longer available (e.g., deprecated)
  eligibility_changes: Vec<EligibilityChange>,      // verbs whose actionable/blocked status changed
  policy_changes:     Vec<PolicyChange>,            // policies that gain/lose constraints
  noun_changes:       Vec<NounChange>,              // attributes added/removed/tier-changed
}

ActionableVerbSummary {
  verb_fqn:       String,
  display_name:   String,
  category:       VerbCategory,
  actionable:     bool,
}

EligibilityChange {
  verb_fqn:       String,
  was_actionable: bool,
  now_actionable: bool,
  reason:         String,                    // human-readable explanation of what changed
}

PolicyChange {
  policy_fqn:     String,
  change_type:    PolicyChangeType,          // Added | Removed | ConstraintChanged
  description:    String,
}

NounChange {
  fqn:            String,
  change_type:    NounChangeType,            // Added | Removed | TierChanged | TrustChanged | Deprecated
  description:    String,
}

PolicyChangeType = Added | Removed | ConstraintChanged
NounChangeType   = Added | Removed | TierChanged | TrustChanged | Deprecated
```

The ActionSurfaceDelta is computed by running `GetActionSurface` twice — once with `overlay: ActiveOnly` and once with `overlay: DraftOverlay(changeset_id)` — and diffing the results. This computation is performed by the `GetActionSurfacePreview` tool (§6.4).

### 9.16 WorkbenchPacket Transport Model (New in v0.83)

The stewardship Workbench needs to stream structured visual instructions (ShowPackets) to the UI. Rather than inventing a new transport, WorkbenchPackets ride on the **same WebSocket channel and persistence infrastructure** as execution-side DecisionPackets. They are a peer payload type, not a sub-type.

```
WorkbenchPacket {
  packet_id:      Uuid,
  session_id:     Uuid,
  timestamp:      Timestamp,
  kind:           WorkbenchPacketKind,
  payload:        WorkbenchPayload,
}

WorkbenchPacketKind = Show | DeltaUpdate | StatusUpdate

WorkbenchPayload =
  | ShowPayload { show_packet: ShowPacket }
  | DeltaPayload { deltas: Vec<ViewportDelta> }
  | StatusPayload { viewport_id: String, status: ViewportStatus }
```

**Why not extend DecisionPacket?**

The execution-side DecisionPacket protocol serves case execution (proposals, confirmations, clarifications) with payloads shaped around case state. Stewardship ShowPackets carry viewport specs, deltas, and SuggestedActions shaped around Changeset authoring. These are structurally different concerns:

| | DecisionPacket | WorkbenchPacket |
|---|---|---|
| **Consumer** | Chat UI (conversation stream) | Workbench UI (viewport panel/inspector) |
| **Payload** | Proposals, confirmations, clarification requests | ShowPackets, viewport deltas, status updates |
| **Lifecycle** | Case execution steps | Changeset authoring steps |
| **Rendering** | Inline in chat | Inspector panel / viewport grid |

Sharing the transport envelope (WebSocket channel, message framing, persistence layer, audit stream) gives "don't reinvent infrastructure." Separating the payload schemas gives "stewardship UI changes don't break execution-side rendering, and vice versa."

**Routing:** The shared WebSocket channel carries both `DecisionPacket` and `WorkbenchPacket` frames, distinguished by a top-level `frame_type` field on the envelope: `frame_type: "decision"` for execution-side packets, `frame_type: "workbench"` for stewardship packets. The UI routes by this discriminator: execution DecisionPackets go to the chat renderer; stewardship WorkbenchPackets go to the Workbench viewport panel. Both are persisted in the same audit stream for reconstruction.

**Persistence:** WorkbenchPackets are persisted alongside DecisionPackets in the session event stream. This means the full stewardship interaction (agent ShowPackets, viewport transitions, SuggestedAction selections) is replayable from the same infrastructure used for execution audit.

---

## 10. Observation Continuity on Promotion

When an attribute is promoted from a lower governance tier (e.g., Operational/Convenience) to a higher tier (e.g., Governed/Proof), existing observations recorded under the prior tier present a semantic integrity problem. Three policies are available, selectable at promotion time:

| Policy | Behaviour | When to Use |
|---|---|---|
| `Grandfathered` | Existing observations remain Active with a `pre_promotion_tier` annotation. Future observations must meet the new tier/trust requirements. | Low regulatory risk; practical where re-collection is infeasible |
| `Invalidated` | Existing observations are marked Stale. Re-collection is required before the attribute can participate in policy predicates. | High regulatory risk; required where evidence integrity is mandatory |
| `PrePromotionFlagged` | Existing observations remain Active but are flagged as `pre_promotion_evidence`. Policy rules referencing the attribute may optionally require post-promotion evidence only. | Best for audit transparency; allows policy writers to choose |

The observation policy must be specified in the Promotion `PROPOSE` expression. Guardrail G12 (ObservationImpact) fires as Warning if the attribute has existing observations and no policy is specified.

Gate pre-check validates that the selected policy is consistent with any policies referencing the attribute (`predicate_trust_minimum` vs `Grandfathered` creates a warning if the existing observations would fail the predicate's trust requirement).

---

## 11. Worked Workbench Journeys

### 11.1 Journey A — Add FATCA Reporting Support

**Persona:** Data Steward (non-technical) + Governance Reviewer  
**Intent:** "We're adding FATCA reporting to institutional client onboarding. Update the registry to support US tax status determination and reporting obligation."

**A1. Workbench Entry**  
User selects: Domain = Regulatory → FATCA, Scope = InstitutionalClient, Jurisdiction = US. Applies Template: `template.regulatory.fatca-starter`.  
Guardrail G11 does not fire (template is current version).

**A2. Agent Suggest**  
Proposed items: `us_tax_classification` (AttributeDef, Governed/Proof), `giin_number` (AttributeDef, Governed/Proof), `w8_ben_doc_ref`, `w9_doc_ref` (EvidenceReqs), `fatca_reporting_obligation` (PolicyRule), `assess_fatca_status` (VerbContract), TaxonomyMemberships for all items under `RegDomain.FATCA`, security label assignments (PII, TaxIdentifier).

Basis attached automatically with Claims:
- `RegulatoryFact`: FATCA obligations require classification and W-8/W-9 documentation basis (High confidence)
- `MarketPractice`: GIIN stored for entity FATCA status (Medium confidence)
- `PlatformConvention`: Use `us_tax_classification` as Proof-tier attribute; keep `tax_status` as DecisionSupport for legacy mapping (Medium confidence, ai_generated = true)

OpenQuestion: "Do you treat withholding reporting as in-scope in onboarding, or only classification capture?"

**A3. Cross-Reference**  
Finds existing `entity.tax_status` used by 3 verbs. Proposes Promotion instead of creating `tax_status_v2`. Confirms `giin_number` is not present. No alias collisions.  
Guardrail: cross-reference mandatory before submit-for-review.

**A4. Draft Materialised**  
Changeset status: Draft. Items: 11 (4 AttributeDefs + 1 PolicyRule + 1 VerbContract + 2 EvidenceReqs + 3 TaxonomyMemberships). Revision: 1.

**A5. Guardrail Fire (early)**  
User edits `us_tax_classification` and sets tier to Operational.  
G04 (ProofChainCompatibility) fires: Block. "This attribute participates in `fatca_reporting_obligation` predicate; minimum trust must satisfy Proof Rule."  
One-click remediation: Set tier/trust to Governed/Proof; attach evidence requirements W-8/W-9; set steward.

**A6. Gate Pre-Check (controlled failure)**  
- PASS: type correctness, dependency resolution, snapshot integrity
- PASS: Proof Rule (after remediation)
- FAIL: `giin_number` missing taxonomy membership `RegDomain.FATCA`
- WARN: Promotion of `tax_status` requires security label recomputation for 1 derivation

**A7. Refine**  
Click "Add taxonomy membership" for `giin_number`. Add steward assignments. Confirm evidence requirements.  
G13 (ResolutionMetadataMissing) fires: Warning. VerbContract `assess_fatca_status` has no usage examples. Steward adds two usage examples and parameter guidance for user-provided inputs.  
Revision: 2.

**A8. Review Workflow**  
Reviewer sees: diff summary, impact graph, gate results (all pass), Basis pack, intent resolution readiness (all green).  
Reviewer adds ReviewNote: "Change freshness to 3 years for W-8BEN-E per policy." Disposition: RequestChange.  
Steward refines. Second pre-check passes. Reviewer approves.

**A9. Publish Outcome**  
Items promoted to Active. Predecessor snapshots `effective_until` set. Taxonomy memberships activate. Embeddings marked stale. Action Surface projections for `entity.institutional_client` × `RegDomain.FATCA` invalidated and queued for recomputation. Publish event emitted with `stale_embeddings` and `stale_action_surfaces` manifests.  
Audit chain: intent → agent reasoning → guardrail log → refine records → gate results → approval → publish metadata.

---

### 11.2 Journey B — Promote an Orphan Attribute to Proof

**Persona:** Platform Steward + Governance Reviewer  
**Intent:** "We keep using `ubo_ownership_pct` in decision logic; it's currently Operational. Promote it so it can participate in policy predicates."

**B1. Entry: Coverage/Drift Dashboard**  
Flag: "Operational attribute used in policy predicate / proof chain risk."  
Click "Create Promotion Changeset."

**B2. Cross-Reference and Impact Graph**  
Verbs consuming `ubo_ownership_pct`, derivations using it, policies referencing it.  
Cross-reference finds alias variation "beneficial owner percentage" — suggests Alias rather than duplication.

**B3. Guardrail: Silent Meaning Change**  
User attempts type change (float → decimal) without migration note.  
G07 (SilentMeaningChange) fires: Block. "Must attach migration guidance or deprecate + replace."

**B4. Observation Continuity (G12)**  
Gate pre-check identifies 847 existing observations at Operational/Convenience trust.  
Workbench presents three options (Grandfathered / Invalidated / PrePromotionFlagged) with trade-off summary.  
Steward selects `PrePromotionFlagged`. Rationale recorded in ConflictRecord.

**B5. Gate Pre-Check**  
Validates: taxonomy membership present, steward assigned, evidence requirements appropriate for Proof, `predicate_trust_minimum` satisfied.  
WARN: 2 policies reference this attribute with `predicate_trust_minimum = Convenience` — these will need updating in a follow-on Changeset. Cross-Changeset consistency enforcement for this domain is set to Warning; the publish event will include a `stale_links` entry for these 2 policies.

**B6. Publish Outcome**  
`ubo_ownership_pct` promoted (Operational → Governed/Proof). Required metadata attached. Existing observations flagged `pre_promotion_evidence`. Publish event includes `stale_links` manifest for the 2 policies requiring follow-on updates. Coverage Report updated with stewardship debt item.  
Release note: "Promoted UBO ownership percentage to Proof-tier for policy enforcement use."

---

### 11.3 Journey C — Deprecate and Migrate

**Persona:** Data Steward + Platform Steward + Reviewer  
**Intent:** "Mark `legacy_status_code` as Deprecated — we're migrating to `entity_lifecycle_state`."

**C1. Changeset Creation**  
Atomic: Deprecation of old + ensure replacement exists + alias/migration note + taxonomy/label updates.

**C2. Guardrail: Deprecation Without Replacement**  
User attempts to deprecate without specifying `replacement_fqn`.  
G08 (DeprecationWithoutReplacement) fires: Block. "Deprecation requires replacement_fqn and migration_guidance."  
Quick action: select `entity_lifecycle_state`; auto-generate migration note template.

**C3. Impact Analysis**  
Lists 12 verbs and 3 derivations still reading `legacy_status_code`. Workbench produces "Migration Backlog" list — non-blocking for publish but recorded as visible debt.

**C4. Gate Pre-Check**  
- Deprecated object still classified ✓
- Replacement fully classified and labeled ✓
- Alias collisions resolved ✓
- No policies accidentally reference deprecated attribute ✓

**C5. Publish Outcome**  
`legacy_status_code` lifecycle_state → Deprecated. Replacement promoted (if needed) to correct tier/trust. Publish event emitted. Action Surface projections updated: verbs consuming `legacy_status_code` flagged with deprecation notice in ActionableVerb metadata.  
Release notes: replacement FQN, sunset expectation 2026-Q3, migration guidance reference.


### 11.4 Journey D — Intent Pipeline Consumes Published VerbContract

This journey illustrates the **execution-side consumption** of Semantic OS after a VerbContract has been authored via stewardship (Journey A) and published.

**Persona:** End user (via conversational interface) + Execution/Intent Agent  
**Utterance:** "What's the FATCA status for Acme Corp?"

**D1. Resolution Context Construction**  
Intent agent constructs Resolution Context: principal = current_user, entity_type = `entity.institutional_client`, entity = Acme Corp (case_id resolved), jurisdiction = [US], domain = [RegDomain.FATCA], available_evidence = [w8_ben_e_doc_ref (collected)], satisfied_preconditions = [country_of_incorporation IS_PRESENT].

**D2. Action Surface Query**  
Agent calls `GetActionSurface(context)`. Semantic OS returns Action Surface (built on context resolution pipeline):
- `verb.fatca.assess_tax_status`: **actionable = true** (all preconditions met: country_of_incorporation present, W-8BEN-E evidence collected)
- `verb.fatca.file_form_1042s`: **actionable = false** (precondition: reporting_obligation_status IS_CLASSIFIED not yet satisfied; satisfiable = true)
- 4 other verbs in scope, various eligibility states

**D3. Intent Matching**  
Intent pipeline performs semantic search against the embedding index (scoped to Action Surface partition). Utterance "What's the FATCA status" matches `verb.fatca.assess_tax_status` with high confidence (link-enriched embedding includes "FATCA", "tax classification", "status").

Agent selects `assess_tax_status` based on: semantic match score + actionable = true + category = Assessment (matches interrogative utterance).

**D4. Verb Compilation Spec Retrieval**  
Agent calls `GetVerbCompilationSpec("verb.fatca.assess_tax_status", context)`. Receives full spec including:
- InputSpec for `us_tax_classification`: source_hint = FromEvidence, current_value = resolved from W-8BEN-E
- InputSpec for `giin_number`: source_hint = FromEvidence, current_value = resolved from case
- InputSpec for `country_of_incorporation`: source_hint = FromCase, current_value = "US"
- ParameterGuidance: "The FATCA entity classification — select from the W-8 or W-9 form submitted"
- Typical successors: `file_form_1042s`, `apply_withholding_exemption`

**D5. Plan Assembly**  
Agent constructs ExecutionPlan with one PlannedStep: `assess_tax_status` with all inputs bound via source hints. No user prompting required (all inputs available from case/evidence).

**D6. Plan Validation**  
Agent calls `ValidatePlan(plan)`. Semantic OS validates:
- All FQN references resolve to Active ✓
- All bound inputs type-check ✓
- All preconditions satisfied ✓
- No policy violations ✓
- Evidence requirements met ✓

Result: `valid = true`.

**D7. DSL Dispatch**  
Validated plan is handed to the DSL compiler/runtime. The runtime uses VerbImplementationBinding (binding_kind = RustHandler, binding_ref = `fatca::assess_tax_status_handler`) to dispatch execution. This is entirely out of Semantic OS scope.

**D8. Post-Execution**  
Verb execution produces `fatca_reporting_obligation_status = Classified`. Agent notes that `verb.fatca.file_form_1042s` was previously `actionable = false` with `satisfiable = true`. The postcondition of `assess_tax_status` satisfies the precondition of `file_form_1042s`. Agent can proactively suggest: "Acme Corp has been classified. Would you like to proceed with Form 1042-S filing?"

This suggestion is grounded in `typical_successors` from the Verb Compilation Spec + the postcondition → precondition chain — not in the LLM's general knowledge.


### 11.5 Journey E — Show Loop Walkthrough (New in v0.82)

This journey traces a single stewardship interaction through the full Intent → Show → Refine cycle with explicit ShowPacket emissions, viewport transitions, and SuggestedActions. It demonstrates how the visual control loop keeps the human oriented.

**Persona:** Data Steward  
**Intent:** "Add a new evidence requirement for LEI verification to the institutional client domain."

**E1. Intent**  
Steward types: "We need LEI verification as evidence for institutional clients."

**E2. Agent Propose + ShowPacket #1**  
Agent calls `CrossReference("evidence.lei_verification", ...)` → no existing match. Agent calls `ComposeChangeset(intent)` → creates Changeset with 1 EvidenceRequirement + 1 TaxonomyMembership.

Agent emits **ShowPacket #1**:
```
ShowPacket {
  focus: { changeset_id: cs-001, object_refs: [evidence.lei_verification], overlay_mode: ActiveOnly }
  viewports: [
    { kind: Focus,   render_hint: Cards },
    { kind: Object,  params: { fqn: "evidence.lei_verification" }, render_hint: Cards },
    { kind: Diff,    params: { object_ref: "evidence.lei_verification" }, render_hint: Diff },
  ]
  deltas: null    // first render — no deltas, full viewport models requested
  narrative: "I've proposed a new LEI verification evidence requirement. No existing match found in registry."
  next_actions: [
    { action_type: AcceptItem, label: "Accept proposal", target: { item_id: item-001 }, enabled: true },
    { action_type: EditItem,   label: "Edit details",    target: { item_id: item-001 }, enabled: true },
    { action_type: RunGates,   label: "Run gate pre-check", target: { changeset_id: cs-001 }, enabled: true },
    { action_type: ToggleOverlay, label: "Preview execution impact", target: {}, enabled: true },
  ]
}
```

Workbench renders: **Focus** (Changeset cs-001, 2 items, Draft), **Object Inspector** (LEI verification details), **Diff** (no predecessor — new object, all fields shown as Added).

**E3. Human Toggles Draft Overlay**  
Steward clicks "Preview execution impact" (SuggestedAction: ToggleOverlay).

This is a **human-navigation** event (§2.3.5) — no agent involvement. The Workbench updates FocusState locally to `overlay_mode: DraftOverlay(cs-001)` and calls `GetActionSurfacePreview(context, DraftOverlay(cs-001))`.

Workbench renders: **Action Surface Preview** showing `ActionSurfaceDelta`:
- `verbs_added: []` (no new verbs — this is an evidence requirement, not a verb)
- `eligibility_changes: [{ verb_fqn: "verb.kyc.verify_entity_identity", was_actionable: true, now_actionable: true, reason: "New LEI evidence adds alternative satisfaction path for identity_evidence precondition" }]`
- `noun_changes: [{ fqn: "evidence.lei_verification", change_type: Added }]`

Steward can see: "this evidence requirement will give the identity verification verb an additional way to satisfy its precondition." The preview is computed via the same context resolution pipeline as the real execution agent will use.

**E4. Human Runs Gates + ShowPacket #2**  
Steward clicks "Run gate pre-check" (SuggestedAction: RunGates).

Agent calls `GatePrecheck(cs-001)`. Result: G05 (ClassificationRequired) fires as Block — missing taxonomy membership for `RegDomain.KYC`.

Agent emits **ShowPacket #2** (delta update):
```
ShowPacket {
  focus: { changeset_id: cs-001, object_refs: [evidence.lei_verification], overlay_mode: DraftOverlay(cs-001) }
  viewports: [
    { kind: Gates, params: { changeset_id: "cs-001" }, render_hint: Table },
  ]
  deltas: [
    { viewport_id: "focus", op: Replace, path: "/status_summary", value: "1 blocking issue" }
  ]
  narrative: "Gate pre-check found 1 blocking issue: missing taxonomy membership."
  next_actions: [
    { action_type: Remediate, label: "Add KYC taxonomy membership (one-click)", target: { guardrail_id: "G05", item_id: item-001 }, enabled: true },
    { action_type: EditItem,  label: "Edit manually", target: { item_id: item-001 }, enabled: true },
  ]
}
```

Workbench renders: **Gate Viewport** (1 Block: G05, with one-click remediation available). Focus viewport updates via delta (status summary patched).

**E5. Human Remediates + ShowPacket #3**  
Steward clicks "Add KYC taxonomy membership (one-click)" (SuggestedAction: Remediate).

Agent calls `RefineItem(cs-001, item-001, { add_taxonomy: "RegDomain.KYC" })`. Then re-runs `GatePrecheck(cs-001)` — all pass.

Agent emits **ShowPacket #3**:
```
ShowPacket {
  focus: { changeset_id: cs-001, object_refs: [evidence.lei_verification], overlay_mode: DraftOverlay(cs-001) }
  viewports: [
    { kind: Gates, params: { changeset_id: "cs-001" }, render_hint: Table },
    { kind: Object, params: { fqn: "evidence.lei_verification" }, render_hint: Cards },
  ]
  deltas: [
    { viewport_id: "focus", op: Replace, path: "/revision", value: 2 },
    { viewport_id: "focus", op: Replace, path: "/status_summary", value: "All gates pass" },
  ]
  narrative: "Taxonomy membership added. All gates pass."
  next_actions: [
    { action_type: SubmitForReview, label: "Submit for review", target: { changeset_id: cs-001 }, enabled: true },
    { action_type: EditItem, label: "Continue editing", target: { item_id: item-001 }, enabled: true },
  ]
}
```

Workbench renders: **Gate Viewport** updated (all green), **Object Inspector** updated (taxonomy membership added), Focus delta applied (revision 2, all gates pass).

**E6. Review + ViewportManifest**  
Steward clicks "Submit for review". Reviewer opens the Changeset. Reviewer sees ShowPacket-driven viewports. On approval, the `ReviewNote` captures a `ViewportManifest`:
```
ViewportManifest {
  captured_at: 2026-02-19T15:30:00Z,
  focus_state: { changeset_id: cs-001, overlay_mode: DraftOverlay(cs-001) },
  rendered_viewports: [
    { viewport_id: "diff", kind: Diff, data_hash: "a3f2...", tool_call_ref: "GetDiffModel-001" },
    { viewport_id: "gates", kind: Gates, data_hash: "b7c1...", tool_call_ref: "GetGateViewport-002" },
    { viewport_id: "impact", kind: Impact, data_hash: "d4e5...", tool_call_ref: "GetImpactPreview-001" },
  ],
  overlay_mode: DraftOverlay(cs-001),
}
```

This records: "when the reviewer approved, they were viewing the diff, gate results, and impact analysis, all computed with Draft Overlay active."

---

## 12. Grounding Model and Knowledge Boundaries

The stewardship agent operates with domain knowledge across three tiers:

**Tier 1 — Registry knowledge (closed-world):** The current Active and Draft snapshots. This is precise and authoritative. The agent reads this directly.

**Tier 2 — Domain knowledge (open-world, high confidence):** Regulatory frameworks (FATCA, CDD, AML, GDPR), standard evidence artifacts (W-8, W-9, Articles of Incorporation, LEI), financial services terminology, common governance patterns. The agent uses this to propose registry objects. Claims derived from this tier are marked `RegulatoryFact` or `MarketPractice` with High or Medium confidence.

**Tier 3 — Platform conventions (open-world, medium confidence):** How this specific platform has implemented patterns — tier assignments, naming conventions, specific policy predicate structures. The agent infers these from the existing registry but must be explicit that inferences are platform-specific. Claims from this tier are marked `PlatformConvention` and flagged `ai_generated = true`.

When the agent is uncertain (Low confidence), it surfaces the uncertainty as an OpenQuestion in the Basis rather than making a low-confidence assertion. G09 (AIKnowledgeBoundary) fires Advisory on low-confidence ai_generated claims.

---

## 13. Security and ABAC Integration

The stewardship agent operates within the platform's ABAC model. Role permissions govern which fields can be edited, which guardrails can be overridden, and which stages of the Changeset lifecycle a principal can advance.

| Role | Can Author | Can Submit for Review | Can Approve | Can Publish | Can Override Guardrails |
|---|---|---|---|---|---|
| DataSteward | ✓ | ✓ | — | — | G09, G11 |
| PlatformSteward | ✓ | ✓ | — | — | G09, G11, G12 |
| GovernanceReviewer | — | — | ✓ | — | — |
| PublishAuthority | — | — | ✓ | ✓ | — |
| Admin | ✓ | ✓ | ✓ | ✓ | All |

No role can override Block guardrails G01, G04, G07, G08 — these represent invariant violations. G10 (ConflictDetected) requires explicit ConflictResolution to be recorded; it cannot be silently overridden. G13 (ResolutionMetadataMissing) override policy is configurable per domain — regulated domains may escalate to Block.

---

## 14. Open Questions (Resolved and Remaining)

### Resolved

**Q1: Execution agent conversation history access** — Deferred. The stewardship agent reads registry state and operational schema, not execution conversation history. Usage patterns should come from structured telemetry (verb invocation counts, attribute access logs), not conversation mining. Future analytics capability.

**Q2: Intra-changeset cross-references** — Resolved: Yes. Gate pre-check resolves intra-Changeset references by extending the snapshot resolution scope to include the Changeset's `snapshot_set_id` — Draft snapshots in `sem_reg.snapshots` are treated as provisionally Active within the check (§9.2). This is a WHERE clause extension, not a separate resolution path.

**Q3: Verb contract authoring** — Resolved: Yes, contract first. The stewardship agent proposes VerbContracts (I/O surface, preconditions, postconditions, exec_mode). The contract's structural correctness is gate-validated (attribute references resolve, entity types exist). In addition, Stewardship may maintain an explicit **VerbImplementationBinding** for execution-intended verbs; publish policy can require a valid binding for user-facing verbs while allowing early Draft authoring without one.

**Q4: Changeset branching** — Deferred. The conflict model (§9.6) handles the primary concurrency concern. Branching adds data model complexity for a use case that can be served by two independent Changesets compared manually.

**Q5: Dry-run publish / sandbox** — Substantially addressed (v0.82–0.83). The Draft Overlay preview (§2.3.4) provides execution-context preview by computing `GetActionSurface` and `GetVerbCompilationSpec` with `overlay: DraftOverlay(changeset_id)`. Because Draft snapshots are stored in `sem_reg.snapshots` (§9.1), this uses the exact same resolution logic as the execution agent — the only difference is the WHERE clause includes the Changeset's Drafts. Full execution-context sandbox mode ("what would the DSL runtime return for Case X if this Changeset were Active?") remains deferred — the preview shows what becomes *available* but does not simulate *execution*.

**Q6: Cross-platform registry federation** — Deferred, out of scope. The stewardship agent is scoped to the ob-poc registry. Cross-platform coordination is an operational process. `external_impact_notes` could extend the Changeset model if needed, but the agent would not modify external systems.

### Remaining Open

**Q7: Template governance process** — §9.5 defines templates as versioned stewardship-layer objects (storage resolved — stewardship tables in v1, SemReg promotion path when kernel object_type enum is next extended). Governance question remains: is there a dedicated "template steward" role, or does GovernanceReviewer cover template authoring and approval?

**Q8: Deprecation consumer policy** — Do you allow publishing deprecations when consumers still exist (with visible debt / migration backlog), or require "consumer zero" before publish? Journey C assumes the former (non-blocking but visible). This should be a platform configuration.

**Q9: Basis external references** — The Research Pack permits internal policy doc references and curated external summaries. The boundary for acceptable external citation needs explicit policy — particularly for regulatory text (permitted by section reference) vs third-party analysis (restricted to curated sources).

**Q10: Verb composition model** — Real financial workflows are composed sequences (e.g., `open_account` invokes `verify_identity` → `classify_entity` → `apply_regulatory_hold` → `create_custody_agreement`). Open questions: (a) Do orchestration sequences get their own VerbContract subtype (e.g., `OrchestrationType` with sub-steps), or is composition expressed as a PolicyRule linking multiple verbs? (b) Does precondition inheritance apply — if `verify_identity` requires `identity_doc IS_PRESENT`, does the parent `open_account` automatically inherit that precondition? (c) What are the compensation semantics when a mid-sequence step fails? The current model handles individual verbs well; the composition model is a known future pressure point because the DSL will need to express it and the stewardship agent will need to author composition contracts. The `typical_predecessors` / `typical_successors` fields (§9.10) are advisory hints, not structural composition — they serve the intent pipeline's suggestion logic but do not enforce sequencing.

**Q11: Action Surface materialisation strategy** — The two-layer design (§5.12) — static materialised surface + dynamic eligibility overlay — requires a caching and invalidation strategy. Options: (a) Full materialisation per entity-type × jurisdiction × role combination (predictable but potentially large matrix); (b) Lazy computation with publish-event invalidation (lower storage, higher latency on first access); (c) Hybrid with hot-path pre-materialisation for high-traffic combinations and lazy for long-tail. The choice depends on the cardinality of entity-type × jurisdiction × role in the deployment and the acceptable latency for intent resolution. This is an implementation decision but the architectural invariant is: the Action Surface must be **consistent with the most recent publish** — stale Action Surfaces are a correctness risk, not merely a performance issue.

**Q12: Embedding template governance** — §9.13 notes that the embedding input template could be a governed registry object per object type or per domain. This is deferred as optional for v1, but the question is: when a domain-specific embedding template changes, does it trigger full re-embedding of all objects in that domain? If so, this is a potentially expensive operation that needs rate limiting or batching policy.

**Q13: ShowPacket persistence and replay** — The ViewportManifest (§9.4) records *what was shown* at decision points. Should full ShowPacket sequences be persisted for regulatory replay ("show me exactly what the steward saw at each step")? This would enable audit reconstruction beyond the data-hash approach but increases storage and raises questions about ShowPacket versioning across Workbench UI updates.

**Q14: Draft snapshot cleanup policy** — Now that Drafts are stored as `sem_reg.snapshots` (§9.1), rejected or abandoned Changesets leave Draft snapshots in the kernel table. Options: (a) Mark as rejected/deleted but retain for audit (row stays, status changes); (b) Hard delete after a retention period; (c) Move to an archive partition. The choice affects storage growth, audit reconstructability, and query performance on the snapshots table. For regulated environments, option (a) is safest. This is a platform configuration.

---

## 15. Success Criteria

1. A non-technical governance user can author a complete FATCA attribute set, pass gate pre-check, complete the review workflow, and publish to Active — without writing YAML or knowing the snapshot schema.
2. The agent surfaces all existing cross-references before any new object is proposed. No duplicate FQNs reach Draft status.
3. Every published object has a complete audit chain traceable to its authoring intent, agent reasoning, Basis claims, guardrail log, reviewer decisions, and publish metadata.
4. Gate pre-check with intra-Changeset provisional resolution correctly validates PolicyRules referencing new AttributeDefs in the same Changeset.
5. Ongoing stewardship (alias, reclassify, promote, deprecate) is achievable in a single conversational turn with guardrail-guided remediation.
6. Observation continuity policy is recorded for every promotion that affects existing observations.
7. Contract coverage and binding integrity are visible: user-facing verbs are never Active without an associated VerbContract and a valid implementation binding (or an explicit exception policy).
8. **The execution/intent agent can discover all actionable verbs for a given Resolution Context via the Action Surface, without speculative registry search.** The Action Surface is consistent with the most recent publish event and is built on the implemented context resolution pipeline.
9. **A published VerbContract with complete resolution metadata (usage examples, parameter guidance, input source hints) is findable by semantic search and compilable by the intent pipeline within one publish-event refresh cycle.**
10. **Plan Validation deterministically rejects any ExecutionPlan that references unresolved FQNs, violates active policies, or sequences verbs inconsistently with precondition/postcondition chains.**
11. **Intent Resolution Readiness is reported for all Active user-facing VerbContracts, and resolution metadata gaps are tracked as stewardship debt in the Coverage Report.**
12. **Every agent action that changes Changeset state emits a ShowPacket (via WorkbenchPacket). The Workbench renders focus + diff within one interaction cycle (Show Loop Latency Invariant). The human is never disoriented about what the agent is operating on.**
13. **Draft Overlay preview accurately reflects the execution agent's post-publish view — computed via the same resolution logic against Draft snapshots stored in `sem_reg.snapshots`, not a separate store. ABAC impersonation (`assume_principal`) ensures the preview reflects the execution agent's access scope, not the steward's.**
14. **Audit records for human decisions (review, approval, guardrail acknowledgement) include a ViewportManifest with SHA-256 canonical hashes (including assumed_principal), enabling cryptographic proof of what was displayed at decision time. FocusChanged events are captured in the audit chain for navigation replay.**
15. **Within any Changeset, at most one current Draft head exists per `(object_type, object_id)` — enforced by UNIQUE constraint and guardrail G15. Refinements supersede prior Drafts; no ambiguous resolution within a Changeset.**

---

## 16. Implementation Sequencing

The architecture in this document is the target state. It does not all need to ship at once. This section defines the implementation phases in order of dependency and value, mapped to what already exists in the ob-poc Semantic OS.

### Phase 0 — Changeset Layer (prerequisite for everything)

**Build:** Stewardship tables (Changeset, ChangesetItem, Basis, ReviewNote, ConflictRecord, StewardshipRecord) referencing `sem_reg.snapshot_sets`. Changeset creation writes Draft snapshots into `sem_reg.snapshots` with `status = Draft`.

**Depends on:** Existing `sem_reg.snapshots` table (Migration 078) and `snapshot_sets`. No schema changes to the kernel — only new stewardship-layer tables and the use of existing `status = Draft`.

**Delivers:** The Suggest → Draft → Refine → Gate → Review → Publish lifecycle. All subsequent phases build on this.

### Phase 1 — Show Loop (minimal viable Workbench)

**Build:** FocusState (server-side), ShowPacket emission from agent, WorkbenchPacket transport on existing WebSocket, Workbench viewport panel rendering 4 of 8 viewports:

- **Focus** — Changeset + object refs (trivial render from FocusState)
- **Object Inspector** — compose from existing `sem_reg_describe_*` MCP tools
- **Diff** — server-side diff of Active predecessor vs Draft successor snapshots (both in `sem_reg.snapshots`)
- **Gates** — gate pre-check result model (needs implementing but well-specified)

**Depends on:** Phase 0 (Changeset layer). Existing MCP tools for object inspection.

**Delivers:** The core Intent → Show → Refine loop. Human can see focus, inspect objects, view diffs, and run gates. This is the minimum to make agentic stewardship trustworthy.

### Phase 2 — Impact + Coverage + Taxonomy Viewports

**Build:** Remaining 4 viewports:

- **Taxonomy** — compose from existing `sem_reg_taxonomy_tree` + `sem_reg_taxonomy_members` tools
- **Impact** — blast radius computation from managed links graph
- **Coverage/Readiness** — Intent Resolution Readiness checks, orphan detection, drift signals
- **Action Surface Preview** — initially implemented as `resolve_context()` output reformatted into ActionSurface shape (no materialisation needed)

**Depends on:** Phase 1.

**Delivers:** Full 8-viewport Workbench. Draft Overlay mode for Action Surface Preview uses `resolve_context()` with Changeset's Draft scope — no materialised/cached Action Surface projection required at this phase.

### Phase 3 — Materialised Action Surface (execution-side optimisation)

**Build:** The materialised/cached Action Surface projection (§5.12) with publish-event-driven invalidation and refresh. Two-layer design: static pre-computed surface + dynamic eligibility overlay.

**Depends on:** Phase 2 (which proves the Action Surface data model via `resolve_context()` composition).

**Delivers:** Execution-side performance: the hot path of intent resolution reads a cached projection instead of running the full 12-step pipeline. This is an optimisation for the execution agent's latency requirements — the stewardship Workbench does not need it because a steward editing one Changeset can tolerate the full resolution latency.

### Phase 4 — Embedding + Semantic Search Integration

**Build:** Link-enriched embedding generation (§9.13), publish-event-triggered refresh, embedding index partitioned by domain/entity-type.

**Depends on:** Phase 3 (Action Surface projection provides the scope for embedding partitioning).

**Delivers:** Semantic search for intent resolution (Journey D). VerbContracts are findable by natural language utterances because embeddings carry managed-link context.

### Key principle

**The Show Loop (Phase 1) is the highest-value, lowest-dependency deliverable after the Changeset layer.** It can ship with existing MCP tools composed into viewport models. The materialised Action Surface (Phase 3) and embedding integration (Phase 4) are execution-side optimisations — they make the execution agent faster and more capable, but the stewardship Workbench can function without them.

---

*This document is a vision and capability specification. It does not prescribe Rust implementation, DB schema, or MCP tool JSON schemas. It is the authoritative conceptual reference for the Stewardship Agent, the Execution Constraint Surface, the Visual Control Loop, and the Resolution Contract, and supersedes all prior versions.*
