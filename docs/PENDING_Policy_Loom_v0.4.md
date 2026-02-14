# The Policy Loom

**Weaving Legal Text into Executable Operations via a Semantic OS**

Version: 0.4 — February 2026
Status: Position Paper — Final Draft
Author: Adam / Lead Solution Architect, BNY Mellon Enterprise Onboarding Platform
Lineage: v0.1 vision → v0.2 structural definition → v0.3 action contract generalisation, merge semantics, disputed lifecycle → v0.4 identity model, section addressing, merge formalism, ActionRef, fixture inheritance, editorial tightening
Audience: KYC Policy Owners, Compliance, Operations SMEs, Platform Architecture, Workflow Engineering, Agent Runtime Teams

---

## 1. Vision

KYC policy is not a "document problem." It is a meaning problem.

Banks already have comprehensive policy documents — carefully authored, reviewed, and published by policy teams. Yet those documents are written for legal defensibility and regulatory alignment, not for deterministic operational execution.

The Policy Loom is the capability to turn policy from static text into a first-class, versioned, testable, explainable control plane that can drive:

- evidence requirements ("passport + tax return under X conditions")
- security handling (masking / export / residency / dual control)
- case progression parameters (deadlines, thresholds, escalation levels)
- agentic runbook compilation (multi-step plans that close evidence gaps)

This is not about building a new catalog product. It is about making policy addressable, queryable, enforceable, and audit-grade — integrated into a Semantic Operating System (Semantic OS) as a structural capability, not a bolt-on layer.

### 1.1 The central thesis: actions are policy execution surfaces

Every executable action in the onboarding platform — resolving UBO ownership, requesting evidence, running sanctions screening, validating proof — exists because a policy requires it. The action is the operational expression of policy intent. The Loom makes this relationship explicit, traceable, and auditable:

- **Policy** says what must happen and under what conditions
- **Action contracts** define the executable shape of those actions
- **The Loom** is the provenance chain that links them: legal text → operational interpretation → executable rules → action preconditions + parameters + evidence requirements

In ob-poc, actions are surfaced as **DSL verbs** (`ubo.resolve`, `evidence.request`, `screening.check`, `proof.validate`) with typed contracts, preconditions, and postconditions. In other stacks, they may be workflow tasks, service calls, or Spring-managed operations — the Loom's structural model is the same regardless of the execution substrate. This paper uses "action contract" for the general concept and "verb contract" / "DSL verb" where referring specifically to ob-poc's implementation.

When an agent (or a workflow engine, or a service orchestrator) executes an action, the decision record must trace back through the Loom to the specific policy version, interpretation, and section that directed the action. Without this chain, the system's decisions are operationally correct but legally ungrounded.

---

## 2. Problem Statement

### 2.1 The policy-to-execution gap

Policy exists in published documents, but execution happens in many places: workflows (BPMN gateways, process forks), decision tables (DMN), application code (Java services, Rust DSL verbs, Spring controllers), and analyst judgment and procedural playbooks.

That fragmentation causes:

- **policy drift** — different interpretations across systems
- **workflow sprawl** — dozens of KYC workflows encoding the same policy in different ways
- **audit weakness** — "why did we do this?" becomes narrative reconstruction, not provenance
- **change pain** — a policy update becomes multi-team recoding with no regression safety

### 2.2 What's broken is not compliance — it's operational semantics

The bank is compliant on paper. The gap is that policy meaning is not captured as a first-class artifact that systems can evaluate consistently. The Semantic OS provides the runtime substrate — immutable snapshots, ABAC, gates, action contracts — but the upstream chain from legal text to executable rule is missing. The Loom fills that gap.

### 2.3 The action linkage gap

The platform's action contracts already carry preconditions, postconditions, and evidence requirements. But these are currently authored directly on the contracts without traceable provenance to the policies that mandate them. This means:

- A precondition like "entity must have verified UBO declaration" exists because some policy requires it — but the action contract doesn't say *which* policy
- When policy changes, there is no mechanism to identify which action contracts are affected
- When an auditor asks "why did the system require a certified UBO declaration for this Saudi High-risk case?", the answer is "because the precondition says so" — not "because Policy bny.kyc.policy.ubo_discovery@v12 §4.3 requires certified UBO declarations for High-risk Saudi-linked entities"

The Loom closes this gap by making every precondition, every evidence requirement, and every operational parameter traceable to a specific policy section.

---

## 3. Scope

### 3.1 In scope

A first-class policy capability consisting of:

- Policy document cataloging with stable identities and version control
- Two views of policy:
  1. **Legal/Published View** (authoritative text — the document as issued)
  2. **Operational/Interpretation View** (what it means in practice — structured, reviewable, compilable)
- Coverage/applicability metadata with conflict resolution and deterministic merge semantics
- Executable policy rules (gates + parameters) compiled from interpretations and linked to Semantic OS objects
- Action contract linkage (policy → preconditions, evidence requirements, operational parameters)
- Explainability and traceability (policy → interpretation → rules → action execution → decision record)
- Policy test harness with regression safety as a publish gate
- Runbook compilation (policy-driven agent plan generation)

### 3.2 Out of scope (for this paper)

- The specific rule language / authoring UI (implementation detail — the Loom defines the structural contract, not the editor)
- Full workflow orchestration design (BPMN/DMN are consumers of Loom outputs, not part of the Loom)
- Full enterprise-wide policy federation across all banking domains (the Loom is designed for KYC/onboarding first; the architecture generalises)
- Execution substrate choice (Rust DSL, Java 21 / Spring, BPMN tasks — the Loom is substrate-agnostic)

### 3.3 Non-goals

- **LLM-generated policy rules**: LLMs may propose candidate directives or suggest mappings during authoring, but only human-reviewed, Approved structured directives compile to executable RuleSets. The compilation chain is deterministic; there is no generative step between Approved interpretation and Active RuleSet.
- **DMN as a second truth source**: DMN decision tables, if used, must be derived from PolicyRuleSet outputs — never authored independently. There is one source of policy truth: the Loom.

---

## 4. Core Concepts

### 4.1 Policy as a first-class document family

Policies must be managed as families with versions:

- **Policy Family ID**: stable identity for the policy concept (e.g., `bny.kyc.policy.ubo_discovery`). The family ID is immutable once assigned. It does not contain volatile facts (dates, org structure, version numbers). Those belong in metadata.
- **Policy Version**: a specific published revision with effective dates (e.g., `@v12` effective 2026-01-01)

### 4.2 Identity model

Every Loom artifact follows a consistent identity scheme:

| Identity layer | Meaning | Example | Mutability |
|---|---|---|---|
| `*_family_id` | Stable conceptual identity across all versions | `bny.kyc.policy.ubo_discovery` | Immutable once assigned |
| `*_version_id` | Human-readable version label | `@v12`, `@2026-01` | Immutable once published |
| `snapshot_id` | Immutable registry record ID (UUID) in the Semantic OS snapshot store | `a3f8c1...` | Immutable; one per published revision |
| `effective_from` | Date this version becomes applicable | `2026-01-01` | Set at publish time |
| `effective_until` | Date this version is superseded (null if current) | `2026-07-01` or null | Set when successor activates |
| `status` | Lifecycle state | Draft → Active → Superseded → Retired | Transitions per lifecycle rules |

**Relationships**: a `family_id` has many `version_id`s. Each `version_id` corresponds to exactly one `snapshot_id`. The `snapshot_id` is the canonical reference used in all machine linkage (provenance chains, decision records, fixture references). The `version_id` is the human-facing label. The `family_id` is for discovery and grouping.

This model applies uniformly to PolicyDocumentFamily/Version, PolicyInterpretationVersion, PolicyRuleSet, and PolicyTestFixture.

### 4.3 Active version constraint

**Activation scope key**: `(family_id, jurisdiction_scope)`

At most one version of a given policy family may be Active for a given activation scope key at a given time. When version v13 becomes Active for jurisdiction SA, version v12 is Superseded for that scope.

The same family may have different active versions in different jurisdictions during a transition period (e.g., v13 active for SA, v12 still active for UK pending UK-specific review). If no jurisdiction scope is specified on the version, the activation scope is global: one active version per family.

**Overlap is resolved at evaluation-time, not activation-time.** The activation constraint keeps the version lifecycle simple. CoverageSpec predicates (which may include risk_tier, product, entity_type, and other dimensions beyond jurisdiction) are evaluated at case-resolution time using the merge semantics defined in §6.3.1. This separation prevents the activation model from becoming as complex as the coverage model.

### 4.4 Two views of the same policy: "what it says" vs "what it means"

Policy is both:

- **a human-authoritative artifact** (legal/published view) — the text as authored by policy teams, stored immutably with hash verification
- **a machine-operational artifact** (interpretation and executable meaning) — structured records that capture the operational intent in a form that compiles to executable rules

Both must exist, both must be versioned, and both must be linked. The interpretation is always subordinate to the legal text — it is a *reading* of the policy, not a replacement for it. When the legal text and the interpretation diverge, the legal text is authoritative and the interpretation must be corrected.

**Disputed interpretations**: if policy owners or legal counsel believe an interpretation misrepresents the policy, the interpretation enters a `Disputed` state. Disputed interpretations **cannot compile to Active RuleSets** — this is a structural constraint, not a process guideline. The dispute must be resolved (interpretation corrected and re-approved, or policy text clarified) before the RuleSet can activate. This gives legal/policy teams a clean veto mechanism without requiring them to understand the compilation machinery.

### 4.5 The compilation chain

The Loom defines a deterministic compilation chain from legal text to executable operations:

```
PolicyDocumentVersion          (immutable legal text)
        │
        ▼
PolicyInterpretationVersion    (structured operational reading, SME-reviewed)
        │
        ▼
PolicyRuleSet                  (compiled executable rules — gates + parameters)
        │
        ├──▶ PolicyRule snapshots           (individual predicates + enforcement)
        ├──▶ EvidenceRequirement snapshots  (what proof is needed)
        ├──▶ Action contract linkages       (which actions execute this policy)
        └──▶ PolicyTestFixture snapshots    (regression safety)
        │
        ▼
Runtime evaluation             (case-specific policy application + merge)
        │
        ├──▶ AgentPlan + PlanSteps          (runbook compiled from evidence gaps)
        ├──▶ DecisionRecord                 (snapshot-pinned, traceable to policy)
        └──▶ Gate verdicts                  (Permit / Deny / Escalate)
```

Every link in this chain is snapshot-pinned and audit-queryable. At any point in time, you can reconstruct: "this decision was made because PolicyDocumentVersion X was interpreted as InterpretationVersion Y, which compiled to RuleSet Z containing Rule R, which was evaluated against Case C and produced Verdict V."

### 4.6 Actions as policy execution surfaces

An executable action does not exist in isolation. Every action that participates in regulated KYC operations is the execution surface for one or more policies. The Loom makes this relationship structural:

| Policy element | Action contract element | Example (ob-poc DSL) | Example (Java / Spring) |
|---|---|---|---|
| "Require certified UBO declaration" | `preconditions[]` | `ubo.confirm` verb precondition | `UboConfirmationService` guard |
| "Passport + tax return for High-risk" | linked `EvidenceRequirement` | `evidence.request` verb | `EvidenceRequestTask` in workflow |
| "14-day deadline for Saudi cases" | `continuation.timeout_policy` | Durable verb timeout | `@DurableTask(timeout="14d")` |
| "Dual control for PEP approval" | `security_label.handling` | `pep.approve` verb label | Spring Security `@DualControl` |
| "Escalate to compliance on timeout" | `preconditions[]` + PolicyRule scope | `case.escalate` verb | `EscalationService.trigger()` |

This linkage means that when a policy changes, the impact is traceable regardless of execution substrate: "Policy bny.kyc.policy.ubo_discovery changed from v12 to v13. The following action contracts are affected: `ubo.resolve` (precondition change), `evidence.request` (evidence set change), `proof.validate` (threshold change)."

**Impact analysis is computed from three data sources**: ActionPolicyLinkage records (which actions are linked to the changing policy), PolicyProvenance fields on individual PolicyRule snapshots (which rules derive from the policy), and CoverageSpec domain_tags (which domain areas are affected). No codebase search is required — impact is a graph traversal over Loom artifacts.

---

## 5. Indexing & Cataloguing Strategy: Dot Notation Namespaces

### 5.1 Why dot notation

A dotted namespace strategy provides: stable, readable addressing for humans and agents; prefix filtering for discovery (`bny.kyc.policy.ubo.*`); alignment with domain mental models; and compatibility with Semantic OS taxonomies and Venn-style overlap.

### 5.2 Policy identifiers

Recommended canonical format:

- **policy_family_id**: `bny.kyc.policy.ubo_discovery`
- **policy_version_id**: `bny.kyc.policy.ubo_discovery@v12` (or `@2026-01`)

Rule: the family ID must not contain volatile facts (dates, org structure). Those belong in metadata.

### 5.3 Dot notation is not the whole truth

Dot prefixes support discovery, but applicability is expressed via explicit `CoverageSpec` (predicate + scopes) and taxonomy membership across multiple dimensions. This prevents brittle coupling where naming equals meaning. The agent uses dot notation for *discovery* ("load all `bny.kyc.policy.ubo.*`") and CoverageSpec for *selection* ("which of these actually apply to this High-risk Saudi Fund case?").

---

## 6. Capabilities

### 6.1 Policy Document Catalog (Authoritative Published View)

**Goal**: manage policy as official artifacts with immutable published versions.

**Capabilities**:

- Ingest/upload policy documents with provenance (issuer, approval chain, effective date)
- Issuer/owner stewardship (e.g., BNY KYC Policy team)
- Versioning with effective dates, status lifecycle (Draft → Active → Superseded → Retired)
- Active version constraint per activation scope key `(family_id, jurisdiction_scope)` (§4.3)
- Immutability of published versions (content hash + storage reference)
- Section addressing via `PolicySectionRef` (§6.1.1) for fine-grained traceability

**Artifacts**: `PolicyDocFamily`, `PolicyDocVersion`

#### 6.1.1 Policy section addressing

A PolicyRule, directive, or linkage can reference a specific section of a PolicyDocumentVersion. The address format is `PolicySectionRef`:

```
PolicySectionRef {
    // ── Primary address (use one) ──
    anchor_id: Option<String>
        // Preferred. A stable anchor assigned at document ingestion time.
        // Format: "sec-4.3", "def-ubo", "table-evidence-matrix".
        // Anchors are extracted during ingestion and stored in the
        // PolicyDocVersion's section_index.

    // ── Fallback address (when anchoring is not possible) ──
    positional: Option<PositionalRef>
        // { doc_content_hash, page: u32, paragraph_index: u32 }
        // The doc_content_hash ties the position to a specific
        // content version — if the hash doesn't match, the
        // positional ref is stale and must be re-resolved.

    // ── Drift detection ──
    quote_excerpt_hash: Option<String>
        // SHA-256 of a short excerpt (first 100 chars of the referenced
        // section). Used to detect when the legal text has been
        // re-published with changes that affect this section.
        // If the hash doesn't match current text, the system flags
        // the referencing directive as "section drift detected —
        // interpretation review required."

    // ── Human label ──
    display_label: String        // "§4.3", "Table 2", "Definition: UBO"
}
```

**Ingestion contract**: when a PolicyDocumentVersion is published, the system extracts a `section_index` — a map of `anchor_id → { display_label, page, paragraph_index, quote_excerpt_hash }`. Subsequent directives and linkages reference sections via `anchor_id`. If anchoring is not feasible (legacy PDFs without structure), the positional fallback is used with the content hash as a staleness guard.

**Drift detection**: when a PolicyDocumentVersion is superseded, all `PolicySectionRef.quote_excerpt_hash` values referencing the old version are checked against the new version's text. Mismatches flag the referencing directives for interpretation review. This catches the case where §4.3 in v12 says "25% threshold" but §4.3 in v13 says "10% threshold" — the system doesn't silently carry forward the old interpretation.

### 6.2 Operational Interpretation View

**Goal**: capture the operational meaning of a policy in a structured form that humans can review and systems can compile to executable rules.

**The interpretation is a structured artifact, not free-form prose.** This is the critical design decision. An interpretation consists of typed records — evidence requirement declarations, threshold tables, handling directives, exception predicates, deadline specifications — not narrative paragraphs. Prose rationale accompanies each structured element as explanation but is not the compilation input.

**Structural definition of an interpretation**:

```
PolicyInterpretationVersion {
    interpretation_id           // stable identity
    snapshot_id                 // immutable version (§4.2)
    policy_doc_version_ref      // linked PolicyDocVersion snapshot_id
    policy_section_refs[]       // PolicySectionRef[] — which sections covered

    // ── Structured operational elements (typed directives) ──

    evidence_directives[]:      EvidenceDirective[]
    threshold_directives[]:     ThresholdDirective[]
    deadline_directives[]:      DeadlineDirective[]
    handling_directives[]:      HandlingDirective[]
    escalation_directives[]:    EscalationDirective[]
    exception_clauses[]:        ExceptionClause[]
    definitions[]:              PolicyDefinition[]

    // ── Governance ──
    reviewed_by
    reviewed_at
    review_status               // Draft | Reviewed | Approved | Disputed
    dispute_reason              // required when status = Disputed
    rationale_notes

    // ── Snapshot metadata ──
    governance_tier, trust_class, security_label
    steward, status, version, effective_from, effective_until
}
```

**Directive types** (each carries an `applicability_predicate`, a `PolicySectionRef`, and an optional `rationale` field):

| Directive | Key fields | Example |
|---|---|---|
| `EvidenceDirective` | required_doc_types[], substitute_doc_types[], freshness_window, confidence_minimum | Passport + TaxReturn (or NationalID + BankRef). Freshness: 180d. §4.3 |
| `ThresholdDirective` | parameter_name, value, unit, comparison_op (Gte\|Gt\|Lte\|Lt\|Eq\|InSet) | ownership_pct_trigger Gte 25%. §3.2 |
| `DeadlineDirective` | action_category, deadline_days, reminder_days, timeout_action (Escalate\|Cancel\|Extend) | evidence_request: 14d, remind 10d, Escalate. §7.1 |
| `HandlingDirective` | handling_controls[] | [MaskByDefault, SecureViewerOnly, NoExport]. §5.1 |
| `EscalationDirective` | trigger_condition, escalation_target, severity, required_context[] | compliance_reviewer on timeout or PEP. §8.2 |
| `ExceptionClause` | exception_predicate, effect (Exclude\|Override\|Substitute), substitute_directive?, rationale | "LU RegulatedFund excluded — see lu_funds". §2.4 |
| `PolicyDefinition` | term, definition_text, semantic_mapping? | "UBO" = natural person ≥25% ownership. Maps to entity.ubo_threshold |

**Interpretation lifecycle**: Draft → Reviewed → Approved → Disputed

- **Draft**: being authored. Cannot compile.
- **Reviewed**: SME has reviewed. Awaiting formal approval.
- **Approved**: authorised for compilation. PolicyRuleSet can be generated.
- **Disputed**: legal/policy team has challenged the interpretation. **Cannot compile to Active RuleSets** (structural constraint). `dispute_reason` is required. Must be resolved to Approved or replaced before compilation proceeds.

**Why structured, not prose**: structured directives give you deterministic compilation to PolicyRule snapshots, meaningful diffs across versions ("v13 changed the ownership threshold from 25% to 10% for EU jurisdictions"), and machine-readable test fixture generation. Prose rationale explains *why* each directive exists but does not drive compilation.

**Capabilities**:

- Create an interpretation linked to a specific PolicyDocVersion
- Represent each operational element as a typed directive with a `PolicySectionRef`
- Support SME review and approval with the full lifecycle including Disputed state
- Diff interpretations across versions — structural diff on typed directives, not text diff on prose
- Detect interpretation drift via `PolicySectionRef.quote_excerpt_hash` when the legal text is superseded

### 6.3 Applicability / Coverage Model

**Goal**: allow the Semantic OS to select the relevant policies for a case, and resolve conflicts when multiple policies apply.

**CoverageSpec** defines the multi-dimensional applicability of a policy:

```
CoverageSpec {
    // ── Positive applicability ──
    domain_tags[]               // e.g., ["kyc.ubo", "kyc.proofs"]
    entity_type_scopes[]        // e.g., ["Person", "Company", "Fund"]
    evidence_type_scopes[]      // e.g., ["Passport", "TaxReturn"]
    action_categories[]         // e.g., ["request_doc", "screening"]
    jurisdiction_scopes[]       // empty = all
    risk_tier_scopes[]          // empty = all
    product_scopes[]            // empty = all

    // ── Negative applicability (exclusions) ──
    exclusion_predicates[]: ExclusionPredicate[]
        // Each: { predicate, rationale, superseded_by? }

    // ── Default / fallback ──
    is_default_for_domain: bool // at most one per domain_tag

    // ── Precedence ──
    specificity_rank: i32       // higher = more specific; wins on conflict
}
```

**Conflict resolution model**:

1. **Exclusion check**: if a policy's CoverageSpec has an exclusion predicate that matches, the policy does not apply
2. **Specificity rank**: higher `specificity_rank` wins for conflicting outputs
3. **Non-conflicting outputs merge**: different output types from different policies both apply
4. **Conflicting outputs** are resolved by type-specific merge semantics (§6.3.1)
5. **Equal-rank conflict on the same output** is a governance finding — the system **must** emit a structured `GovernanceFinding` and the agent escalates with a `DisambiguationPrompt` referencing both policies. No silent resolution.

#### 6.3.1 Output merge semantics

When multiple applicable policies produce outputs of the same type, the merge rule depends on the output class. These defaults apply unless a policy's `ExceptionClause` with `effect = Override` or `effect = Substitute` declares an explicit override.

| Output class | Merge rule | Definition |
|---|---|---|
| **Evidence requirements** | **Union** | Case must satisfy all evidence sets from all applicable policies. If Policy A requires {Passport} and Policy B requires {TaxReturn}, the case needs {Passport, TaxReturn}. No policy's evidence requirements are silently dropped. Override: `ExceptionClause` with `effect = Substitute` can declare one evidence set replaces another. |
| **Deadlines / timeouts** | **Most stringent** | Shortest deadline governs. If Policy A says 14d and Policy B says 7d, result is 7d. Override: `ExceptionClause` with `effect = Override` on the more-specific policy can relax. |
| **Thresholds** | **Most stringent** (direction-aware) | See threshold merge formalism below. |
| **Security / handling** | **Most restrictive** | Union of all handling controls. Highest confidentiality classification. Consistent with Semantic OS security inheritance. Policy cannot relax security set by another policy. |
| **Gate verdicts** | **Severity ordering** | Deny > Escalate > PermitWithMasking > Permit. Highest severity wins. Equal-severity with different targets: governance finding. |
| **Escalation targets** | **Union** | All triggered escalations fire. Case gets the union of required reviewers. |

**Threshold merge formalism**:

"Most stringent" depends on the comparison operator's direction — the merge selects the value that captures *more* cases (triggers more often):

| Comparison operator | Merge function | Example | Rationale |
|---|---|---|---|
| `Gte` (≥) | `min(values)` | Policy A: ≥25%, Policy B: ≥10% → result ≥10% | Lower threshold triggers on more entities |
| `Gt` (>) | `min(values)` | Same logic | |
| `Lte` (≤) | `max(values)` | Policy A: ≤100k, Policy B: ≤50k → result ≤100k | Higher ceiling captures more cases |
| `Lt` (<) | `max(values)` | Same logic | |
| `Eq` (=) | Conflict if values differ | Must be governance finding | Cannot merge two exact-match requirements |
| `InSet` | `union(sets)` | Policy A: {SA, AE}, Policy B: {SA, QA} → {SA, AE, QA} | Broader set triggers on more jurisdictions |

**Multiple thresholds for the same parameter**: if two policies both set `ownership_pct_trigger` with the same comparison operator, apply the merge function. If they set the same parameter with *different* comparison operators (one says ≥25%, another says ≤75%), this is a structural conflict — governance finding, not silent merge.

**Equal-rank conflict**: when two policies at the same `specificity_rank` produce conflicting values for the same output (after applying the merge function above), the system **must**:
1. Emit a `GovernanceFinding` with both policy references, the conflicting values, and the output class
2. Generate a `DisambiguationPrompt` for the agent/analyst: "Policy A (§4.3) sets deadline=14d, Policy B (§5.2) sets deadline=14d via different reasoning. Both have specificity_rank=20. Please confirm which applies or escalate to governance."
3. Block automated execution of the conflicting parameter until resolved

**Audit trail**: when merge is applied, the `PolicyEvaluationTrace` records which policies contributed to each output, which merge rule was applied, and the pre-merge values. The auditor sees: "evidence requirements are the union of Policy A §4.3 {Passport, TaxReturn} and Policy B §6.1 {SourceOfFunds}. Deadline is 7d from Policy B §7.1 (most-stringent, overriding Policy A's 14d)."

### 6.4 Executable Policy Rules (Gates + Parameters)

**Goal**: compile interpretations into enforceable rules without hardcoding them into workflows or application code.

**Relationship to existing `PolicyRule` in the Semantic OS**: the existing `PolicyRule` is the **atomic executable unit** — an individual predicate + enforcement level + evidence requirement + regulatory reference. The Loom does not replace it. The Loom adds the **upstream provenance chain**: legal text → interpretation → compiled rule set → individual PolicyRule snapshots.

A `PolicyRuleSet` is a **compilation artifact**: a versioned bundle of `PolicyRule` snapshots generated from a single `PolicyInterpretationVersion`. The compilation is deterministic — the same Approved interpretation always produces the same rule set.

**Compilation constraint**: a PolicyRuleSet can only be compiled from an interpretation with `review_status = Approved`. Draft, Reviewed, and Disputed interpretations cannot produce Active RuleSets.

```
PolicyRuleSet {
    ruleset_id, snapshot_id     // per §4.2 identity model
    interpretation_ref          // PolicyInterpretationVersion snapshot_id
    policy_doc_version_ref      // transitively linked

    rule_snapshot_ids[]         // individual PolicyRule snapshots
    action_linkages[]           // ActionPolicyLinkage records (§6.4.2)
    fixture_snapshot_ids[]      // PolicyTestFixture snapshots (§6.6)

    compiled_at, compiled_by
    compilation_warnings[]

    // Snapshot metadata (per §4.2)
    governance_tier, trust_class, security_label
    steward, status, version, effective_from, effective_until
}
```

Policy rules produce two types of outputs:

**A) Gate outputs (compliance decisions)**: Permit / Deny / Escalate / PermitWithMasking, required controls, prohibited exports/processing

**B) Parameter outputs (configuration decisions)**: evidence requirements, deadlines, thresholds, escalation levels, routing metadata

#### 6.4.1 Policy provenance on PolicyRule

Every `PolicyRule` snapshot gains a `policy_provenance` field:

```
PolicyProvenance {
    policy_doc_family_id        // stable family reference
    policy_doc_version_ref      // PolicyDocVersion snapshot_id
    interpretation_ref          // PolicyInterpretationVersion snapshot_id
    ruleset_ref                 // PolicyRuleSet snapshot_id
    source_directive_type       // Evidence | Threshold | Deadline | Handling | Escalation
    source_directive_index      // index within the interpretation's directive array
    policy_section_ref          // PolicySectionRef (§6.1.1) — full section address
}
```

This closes the audit loop. Decision → rule → ruleset → interpretation → legal text + section. Complete. Reconstructable. Snapshot-pinned at every level.

#### 6.4.2 Action-policy linkages

Each action contract that executes policy-directed work carries explicit linkages via `ActionPolicyLinkage`:

```
ActionPolicyLinkage {
    action_ref: ActionRef       // canonical action identity (§6.4.3)
    linkage_type                // Precondition | EvidenceSource |
                                // ParameterSource | SecurityDirective |
                                // TimeoutSource | EscalationTrigger
    affected_elements[]         // which contract elements are policy-driven
    policy_section_ref          // PolicySectionRef (§6.1.1)
    interpretation_directive    // "evidence_directives[0]"
}
```

These linkages are stored on the `PolicyRuleSet` and materialized in the Semantic OS taxonomy system — specifically in a "Policy Traceability" taxonomy that classifies actions by the policies that drive them.

**Impact analysis** is computed from three data sources: `ActionPolicyLinkage` records (which actions are linked to the policy), `PolicyProvenance` fields on individual `PolicyRule` snapshots (which rules derive from the policy), and `CoverageSpec.domain_tags` (which domain areas are affected). Impact is a graph traversal over Loom artifacts — no codebase search required.

#### 6.4.3 ActionRef: canonical action identity

Actions are identified by a stable, substrate-aware reference type:

```
ActionRef {
    // ── One of ──
    variant: ActionRefVariant
}

enum ActionRefVariant {
    Dsl { fqn: String }
        // e.g., "ubo.resolve"
        // The FQN from the ob-poc verb dictionary.

    Java { class_fqn: String, method: String, version: Option<String> }
        // e.g., class_fqn = "com.bny.kyc.ubo.UboResolutionService",
        //        method = "resolveUbo", version = "v1"
        // Versioned via metadata, not name changes.

    Bpmn { process_id: String, task_id: String }
        // e.g., process_id = "kyc-onboarding", task_id = "request-evidence"
        // BPMN process definition + service task within it.

    Custom { namespace: String, identifier: String }
        // Escape hatch for other substrates.
}
```

**Stability rule**: `ActionRef` must be stable across deployments. Version changes are expressed via metadata (the `version` field or the action contract's own version), never by changing the reference identity. A renamed action requires a new `ActionRef` and migration of all linkages.

### 6.5 Traceability & Explainability

**Goal**: answer "why" in audit-grade terms, from decision back to legal text.

Every evaluation returns a complete provenance chain:

```
PolicyEvaluationTrace {
    // ── What was decided ──
    verdict                     // Permit / Deny / Escalate / PermitWithMasking
    parameter_outputs           // evidence sets, deadlines, thresholds, handling
    merge_applied[]             // which merge rules resolved overlap, with
                                // pre-merge values and winning policy per output

    // ── Which policies ──
    applicable_policies[]       // all policies that matched via CoverageSpec
    winning_policy_per_output[] // { output_key, policy_ref, merge_rule, value }
    policy_doc_version_refs[]
    policy_section_refs[]       // PolicySectionRef[] with full addressing
    interpretation_refs[]
    ruleset_refs[]
    rule_snapshot_ids[]

    // ── Which actions were directed ──
    action_linkages[]
    action_contract_snapshot_ids[]

    // ── Evidence ──
    evidence_used[]
    evidence_missing[]
    negative_evidence[]

    // ── Escalation / masking rationale ──
    escalation_reasons[]
    masking_plan?

    // ── Governance findings (if any) ──
    governance_findings[]       // equal-rank conflicts, coverage gaps

    // ── Temporal ──
    evaluated_at, as_of_time
}
```

**Point-in-time reconstruction**: "as of date T, Policy bny.kyc.policy.ubo_discovery@v12 was active for jurisdiction SA. Interpretation v12.3 was the Approved operational reading. RuleSet rs-4821 was compiled from that interpretation. Rule R-7 required a certified UBO declaration for High-risk Saudi entities per §4.3 (anchor: sec-4.3, excerpt hash: a8f3...). The agent executed action `evidence.request` with those parameters. The deadline of 7 days came from Policy bny.kyc.policy.fast_track@v2 §7.1 (most-stringent merge). The decision record DR-9932 pins all these snapshot IDs."

### 6.6 Policy Test Harness (Configuration with code-grade rigor)

**Goal**: policy changes are safe and predictable. **The test harness is the publish gate.**

The test harness operates at the publish boundary: a PolicyRuleSet cannot become Active unless its fixtures pass.

**Artifact**: `PolicyTestFixture`

```
PolicyTestFixture {
    fixture_id, snapshot_id     // per §4.2 identity model
    ruleset_ref                 // which PolicyRuleSet this fixture tests

    // ── Test input ──
    scenario_name               // "High-risk Saudi Fund UBO case"
    case_profile {
        entity_type, jurisdiction, risk_tier, product
        // ... any CoverageSpec dimension
    }
    evidence_present[]
    entity_state

    // ── Expected outputs ──
    expected_gate_verdict
    expected_evidence_reqs[]
    expected_deadlines          // { action_category → days }
    expected_handling[]
    expected_escalation?
    expected_parameters

    // ── Change tracking ──
    intentional_change: bool
    change_rationale            // required when intentional_change = true

    // ── Fixture lifecycle ──
    superseded_by_fixture_id: Option<UUID>
        // When a fixture is replaced (not just updated), the old fixture
        // is marked superseded. Superseded fixtures do not run.
        // Prevents fixture sprawl across many policy versions.
    deprecated: bool
        // Explicitly removed fixture. Must have rationale.
    deprecation_rationale: Option<String>
}
```

**Publish gate integration**:

1. When a `PolicyRuleSet` is submitted for publish, the gate runner loads all linked fixtures (excluding superseded and deprecated fixtures)
2. Each fixture is evaluated against the compiled rules
3. **All fixtures must pass** unless marked `intentional_change = true` with rationale
4. Failures without `intentional_change` **block the publish** with structured remediation
5. Published PolicyRuleSets carry their fixture pass/fail summary as governance metadata

**Fixture inheritance across versions**:

When publishing a new PolicyRuleSet (v13) compiled from a new InterpretationVersion:

1. All non-superseded, non-deprecated fixtures from the predecessor RuleSet (v12) are loaded
2. Each v12 fixture is evaluated against the v13 rules
3. Any fixture that changes outcome must have either:
   - An `intentional_change` fixture in the v13 set with rationale, or
   - A superseding fixture (`superseded_by_fixture_id` pointing to a v13 fixture) that tests the new expected behaviour
4. Fixtures not relevant to v13 can be marked `deprecated = true` with rationale
5. This gives policy owners the same "green build" confidence that developers have with unit tests

**Coverage matrix**: for each active PolicyRuleSet, the harness computes which (jurisdiction × risk_tier × entity_type × product) combinations have test fixtures. Untested combinations are governance signals surfaced in coverage reports.

### 6.7 Runbook Compilation (Policy → Agent Plan)

**Goal**: the agent doesn't guess what to do — it compiles a plan from structured policy directives and action contract gates.

**Determinism constraint**: LLMs may propose candidate directives or suggest mappings during the authoring phase, but only human-reviewed, Approved structured directives compile to executable RuleSets. There is no generative step between Approved interpretation and Active RuleSet. The compilation chain is deterministic; the runbook is a computed artifact, not an improvisation.

Given a case, the agent:

1. Selects applicable policies via CoverageSpec
2. Evaluates the PolicyRuleSets against the case's current state, applying merge semantics (§6.3.1)
3. Identifies **evidence gaps**: required evidence (from policy, after union merge) minus available evidence (from observations)
4. For each gap, identifies the **action** that can close it (via ActionPolicyLinkage):
   - Missing UBO declaration → `evidence.request` action (linked to evidence_directives[0])
   - Missing sanctions screening → `screening.check` action
   - Expired passport → `evidence.request` with freshness_window from policy
5. Assembles an `AgentPlan` with steps, each carrying:
   - The `ActionRef` + contract snapshot_id (pinned)
   - Parameters sourced from PolicyRuleSet (deadlines from most-stringent merge, thresholds, handling from most-restrictive merge)
   - The `PolicyProvenance` reference (which directive generated this step)
   - `expected_postconditions` (the evidence gap this step closes)
   - `fallback_steps` (substitute evidence from ExceptionClause directives)
6. Validates the plan — proof rule compliance, ABAC clearance, contract compatibility
7. Executes steps — each execution records the snapshot_manifest including all PolicyRuleSet, PolicyRule, and PolicyDocVersion snapshot IDs

**BPMN handoff**: for steps requiring durable waits (proof solicitation with 14-day timeout, human review tasks), the agent hands off to the BPMN engine with the action's `ContinuationContract`. BPMN handles wait/retry/correlate/timeout. When the continuation resumes, the agent picks up the plan from the next step. BPMN never contains domain logic — it is a durable transport layer executing policy-compiled parameters.

---

## 7. High-Level Approach

### 7.1 Lifecycle flow

1. **Publish** policy legal text (PolicyDocVersion becomes Active on effective date; section_index extracted)
2. **Attach** CoverageSpec (explicit applicability with exclusions and specificity rank)
3. **Create** Operational Interpretation (structured directives linked to policy version via PolicySectionRef)
4. **Approve** Interpretation (Draft → Reviewed → Approved; Disputed blocks compilation)
5. **Compile** Executable RuleSet (PolicyRules + EvidenceRequirements + ActionLinkages, deterministically generated)
6. **Test** via fixture harness (all inherited + new fixtures must pass; regression gate at publish)
7. **Activate** RuleSet (becomes available to Context Resolution and agent planning)
8. **Runtime evaluation** (policy applied to cases with merge semantics and snapshot-pinned verdicts)
9. **Runbook compilation** (agent compiles plan steps from evidence gaps using action-policy linkages)
10. **Durable orchestration** (BPMN executes waits/resume with policy-sourced parameters)
11. **Decision recording** (snapshot_manifest captures full Loom provenance chain)

### 7.2 How the Semantic OS fits

| Semantic OS capability | Policy Loom contribution |
|---|---|
| Immutable snapshots | Policy objects are snapshots with full version history and identity model (§4.2) |
| ABAC / security handling | Handling directives compile to SecurityLabel fields; most-restrictive merge |
| Action contracts | Preconditions and parameters sourced from compiled PolicyRuleSets via ActionPolicyLinkage |
| Evidence requirements | Structured evidence_directives compile to EvidenceRequirement snapshots; union merge |
| Context Resolution | CoverageSpec + merge semantics enable policy-aware case resolution |
| Publish gates | Fixture harness + provenance completeness + interpretation status are first-class gates |
| DecisionRecord / snapshot_manifest | Policy provenance chain carried on every decision |
| Taxonomy | Policy Traceability taxonomy enables impact analysis |

### 7.3 BPMN/DMN reducer stance

Policies should not be implemented as workflow sprawl:

- **BPMN stays for durable primitives** (wait, retry, correlate, human task, timeout)
- **XOR gateways switch on already-decided outcomes** (success/fail/timeout) — they do not encode policy logic
- **DMN is optional and must be derived from PolicyRuleSet outputs** — DMN decision tables, if used, are parameter selection driven by Loom-compiled parameters. DMN is never a second source of policy truth; it is a rendering of Loom outputs into a DMN-compatible format
- **Policy truth is centralized and versioned** in the Semantic OS via the Loom
- **The number of BPMN processes does not grow with policy count** — the same durable skeleton handles all policies; only the parameters change

### 7.4 Execution substrate independence

The Loom is deliberately substrate-agnostic:

- **ob-poc (Rust DSL)**: action contracts are verb contracts. `ActionRef::Dsl { fqn }`. Compilation produces ActionPolicyLinkage records with DSL FQNs. The DSL runtime evaluates preconditions and pins snapshot IDs. The MCP agent layer calls `sem_reg_compile_runbook`.
- **Java 21 / Spring**: action contracts are service operation contracts. `ActionRef::Java { class_fqn, method, version }`. Compilation produces equivalent linkage records. The Semantic OS service exposes the same wire contract regardless of consumer language.
- **BPMN tasks**: action contracts map to service tasks. `ActionRef::Bpmn { process_id, task_id }`. Compilation produces task parameter bindings sourced from policy.

The Loom's artifacts (PolicyDocVersion, InterpretationVersion, RuleSet, Fixtures, Provenance) are language-neutral structures stored as immutable snapshots. The compilation target is a deployment choice — the audit chain is the same.

---

## 8. Practical Operating Model

### 8.1 "50+ policies in issue" is normal

The system must support many active policies concurrently across domains and jurisdictions. Applicability selection must be: explicit (CoverageSpec with positive and negative predicates), queryable (prefix + taxonomy + predicates), deterministic (snapshot-pinned, specificity-ranked, merge-resolved per §6.3.1), and testable (every PolicyRuleSet has fixtures; every fixture is a publish gate).

### 8.2 Agents and analysts use the same contract

An agentic manager loads policies by namespace ("Load all active `bny.kyc.policy.ubo.*` applicable to case X"). The same selection logic (CoverageSpec evaluation + specificity ranking + merge semantics) supports: UI checklists, workflow decisions, audit export, and agent planning.

### 8.3 Policy change workflow

When a policy document is updated (v12 → v13):

1. New PolicyDocVersion published (v13 Active; v12 Superseded for affected activation scope)
2. System runs drift detection: `PolicySectionRef.quote_excerpt_hash` values from v12-linked directives are checked against v13 text. Mismatches flagged.
3. System flags all PolicyInterpretationVersions linked to v12 as "requires review"
4. SME creates new interpretation linked to v13 (may start from v12 interpretation as baseline)
5. Interpretation goes through Draft → Reviewed → Approved lifecycle
6. System compiles new PolicyRuleSet from v13 Approved interpretation
7. System runs v12 fixtures against v13 rules — **structural diff** shows what changed
8. SME reviews diff, creates intentional_change fixtures or superseding fixtures with rationale
9. New PolicyRuleSet passes all fixtures → published
10. Context Resolution now selects v13 rules for new cases; in-flight cases continue on v12 (snapshot-pinned)

---

## 9. Benefits

- **Consistency**: one interpretation pipeline; fewer contradictory implementations across systems
- **Reduced workflow sprawl**: fewer BPMN processes, fewer bespoke DMN tables — policy parameters drive a common skeleton
- **Auditability**: snapshot-pinned "why" with evidence, policy section references (§6.1.1), merge rationale, and full provenance chain from decision to legal text
- **Safe change**: policy edits run through structural diffs, section drift detection, and fixture regression gates before publish
- **Configuration over code**: policy meaning becomes managed data, not scattered application logic
- **Agent-ready**: policy becomes deterministic input for runbook compilation — the agent doesn't guess, it compiles from structured directives and merge gates
- **Action traceability**: every platform action is traceable to the policy that directed it via ActionPolicyLinkage + PolicyProvenance
- **Impact visibility**: when a policy changes, the affected actions, attributes, and entity types are immediately identifiable via graph traversal (§4.6)
- **Substrate independence**: the same Loom architecture supports Rust DSL, Java 21 / Spring, and BPMN execution — the audit chain doesn't change when the runtime does

---

## 10. Summary

The Policy Loom makes policy a first-class platform capability:

- **indexed and discoverable** via dot-notation namespaces
- **governed** via document families, version lifecycle, identity model (§4.2), and active-version constraints per activation scope
- **section-addressable** via PolicySectionRef with anchor IDs, positional fallback, and drift detection
- **interpreted operationally** in a structured, typed, reviewable layer with dispute resolution (not prose → code, but prose → structured directives → compiled rules)
- **enforced** as executable rules (gates + parameters) in the Semantic OS, with full provenance to legal text and section
- **linked to actions** via ActionRef and ActionPolicyLinkage — every execution traces back to the policy that mandated it, regardless of substrate
- **merge-resolved** — overlapping policies produce deterministic outputs via type-specific, direction-aware merge rules (§6.3.1)
- **explainable and testable** with fixture regression (including cross-version inheritance) as a publish gate
- **workflow-reducing** by keeping BPMN as durable transport and DMN as a derived rendering, not domain logic or a second truth source
- **agent-compilable** — runbooks are compiled from policy-driven evidence gaps, not LLM improvisation

---

## Appendix A — Illustrative Example

**Policy family**: `bny.kyc.policy.ubo_discovery`
**Version**: `@v12` (effective 2026-01-01)

**CoverageSpec**:
- domains: `kyc.ubo`, `kyc.proofs`
- entity_types: `["Person", "Company", "Fund"]`
- jurisdiction_scopes: `["SA"]`
- risk_tier_scopes: `["High", "Enhanced"]`
- exclusion: `entity_type = RegulatedFund AND jurisdiction = LU` (superseded by `bny.kyc.policy.lu_funds@v3`)
- specificity_rank: `20`

**Interpretation (structured directives)**:
- evidence_directives[0]: {Passport, TaxReturn} (substitutes: {NationalID, BankRef}). Freshness: 180d. PolicySectionRef: anchor=sec-4.3, excerpt_hash=a8f3...
- threshold_directives[0]: ownership_pct_trigger Gte 25%, jurisdiction InSet {EU, UK, SA}. PolicySectionRef: anchor=sec-3.2
- deadline_directives[0]: evidence_request 14d, remind 10d, timeout_action=Escalate. PolicySectionRef: anchor=sec-7.1
- handling_directives[0]: [MaskByDefault, SecureViewerOnly, NoExport]. PolicySectionRef: anchor=sec-5.1
- escalation_directives[0]: compliance_reviewer on timeout or PEP. PolicySectionRef: anchor=sec-8.2

**Compiled PolicyRuleSet** (rs-4821):
- Rule R-7: `risk_tier = High AND jurisdiction = SA` → evidence={Passport, TaxReturn}, deadline=14d, handling=[Mask, SecureViewer, NoExport]. Provenance: sec-4.3, sec-5.1, sec-7.1
- Rule R-8: `pep_match = true` → Escalate to compliance_reviewer. Provenance: sec-8.2
- ActionPolicyLinkage: ActionRef::Dsl{fqn="ubo.resolve"} ← Precondition (threshold)
- ActionPolicyLinkage: ActionRef::Dsl{fqn="evidence.request"} ← EvidenceSource + TimeoutSource

**Merge example** (two policies, same case):
- Policy A (`ubo_discovery@v12`, rank=20): evidence={Passport, TaxReturn}, deadline=14d
- Policy B (`fast_track@v2`, rank=25): deadline=7d
- **Merged**: evidence={Passport, TaxReturn} (union), deadline=7d (most-stringent from B), handling=[Mask, SecureViewer, NoExport] (most-restrictive from A)
- **Trace**: "Evidence: union of A§4.3 + B§3.1. Deadline: 7d from B§5.2 (most-stringent). Handling: A§5.1 (most-restrictive)."

**Fixtures**:
- F-1: "High-risk Saudi Fund UBO" → Escalate(timeout), evidence={Passport, TaxReturn}, 14d, [Mask, SecureViewer, NoExport]
- F-2: "Standard-risk UK Company UBO" → Permit, evidence={Passport}, 7d, []
- F-3: "Luxembourg RegulatedFund" → not applicable (excluded by CoverageSpec)
- F-4 (v13, supersedes F-1): "High-risk Saudi Fund UBO" → Escalate(timeout), evidence={Passport, TaxReturn, SourceOfFunds}, 14d — intentional_change=true, rationale="v13 adds SourceOfFunds requirement per §4.3 revision"

---

## Appendix B — Reference Implementation Notes

*This appendix contains Semantic OS implementation details. It is intended for platform engineers and can be skipped by policy/governance readers.*

### B.1 New object types

| Object Type | Body Type | Purpose |
|---|---|---|
| `PolicyDocFamily` | `PolicyDocFamilyBody` | Stable identity for a policy concept |
| `PolicyDocVersion` | `PolicyDocVersionBody` | Immutable published legal text (by reference) with section_index |
| `PolicyInterpretation` | `PolicyInterpretationBody` | Structured operational reading with typed directives |
| `PolicyRuleSet` | `PolicyRuleSetBody` | Compiled bundle of rules + action linkages + fixtures |
| `PolicyTestFixture` | `PolicyTestFixtureBody` | Regression test scenario with inheritance support |

The existing `PolicyRule` and `EvidenceRequirement` object types remain unchanged. The Loom adds `PolicyProvenance` to `PolicyRule`.

### B.2 ObjectType enum extension

**Rust (ob-poc)**:
```rust
pub enum ObjectType {
    // ... existing types ...
    PolicyDocFamily,
    PolicyDocVersion,
    PolicyInterpretation,
    PolicyRuleSet,
    PolicyTestFixture,
}
```

**Java 21 (Spring service)**:
```java
public enum ObjectType {
    // ... existing types ...
    POLICY_DOC_FAMILY,
    POLICY_DOC_VERSION,
    POLICY_INTERPRETATION,
    POLICY_RULE_SET,
    POLICY_TEST_FIXTURE;
}
```

### B.3 ActionRef enum

**Rust**:
```rust
pub enum ActionRef {
    Dsl { fqn: String },
    Java { class_fqn: String, method: String, version: Option<String> },
    Bpmn { process_id: String, task_id: String },
    Custom { namespace: String, identifier: String },
}
```

**Java 21**:
```java
public sealed interface ActionRef {
    record Dsl(String fqn) implements ActionRef {}
    record JavaMethod(String classFqn, String method, String version) implements ActionRef {}
    record BpmnTask(String processId, String taskId) implements ActionRef {}
    record Custom(String namespace, String identifier) implements ActionRef {}
}
```

### B.4 ThresholdMerge function

```rust
fn merge_thresholds(op: ComparisonOp, values: &[f64]) -> MergeResult {
    match op {
        Gte | Gt => MergeResult::Value(values.iter().cloned().fold(f64::MAX, f64::min)),
        Lte | Lt => MergeResult::Value(values.iter().cloned().fold(f64::MIN, f64::max)),
        Eq => if values.windows(2).all(|w| w[0] == w[1]) {
            MergeResult::Value(values[0])
        } else {
            MergeResult::GovernanceFinding
        },
        InSet => MergeResult::UnionOfSets,
    }
}
```

### B.5 PolicySectionRef storage

Stored as JSONB within directive bodies. The `section_index` on `PolicyDocVersionBody` is a `Vec<SectionIndexEntry>`:

```rust
pub struct SectionIndexEntry {
    pub anchor_id: String,
    pub display_label: String,
    pub page: Option<u32>,
    pub paragraph_index: Option<u32>,
    pub quote_excerpt_hash: String, // SHA-256 of first 100 chars
}
```

### B.6 Taxonomy integration

The Loom introduces a canonical **Policy Traceability** taxonomy. Membership is materialized during PolicyRuleSet compilation from ActionPolicyLinkage records.

### B.7 Context Resolution integration

Phase 7 Context Resolution gains policy-aware behaviour: CoverageSpec matching during case resolution, merge semantics on overlapping policy outputs, policy verdicts in `ContextResolutionResponse`, and policy-driven action ranking.

### B.8 MCP tool extensions (Phase 8)

| Tool | Purpose |
|---|---|
| `sem_reg_describe_policy_doc` | Load PolicyDocVersion with section_index |
| `sem_reg_describe_interpretation` | Load structured interpretation with directives |
| `sem_reg_policy_coverage` | "Which policies apply to this case?" |
| `sem_reg_policy_evaluate` | Evaluate rules against case with merge semantics |
| `sem_reg_policy_evidence_gaps` | Policy-driven gap analysis |
| `sem_reg_compile_runbook` | Full runbook compilation → AgentPlan |
| `sem_reg_policy_impact` | Impact analysis via ActionPolicyLinkage + Provenance + CoverageSpec graph traversal |
| `sem_reg_policy_test_run` | Execute all fixtures (with inheritance), return pass/fail/coverage |
| `sem_reg_policy_diff` | Structural diff of two interpretations |
| `sem_reg_policy_drift_check` | Check PolicySectionRef excerpt hashes against current doc version |

### B.9 Publish gate extensions (Phase 6)

- **Fixture gate**: all non-superseded, non-deprecated fixtures must pass (Hard)
- **Interpretation coverage gate**: must reference valid active PolicyDocVersion (Hard)
- **Interpretation status gate**: linked interpretation must be Approved, not Disputed (Hard)
- **Action linkage consistency gate**: all ActionPolicyLinkages must reference active contracts (Hard)
- **Provenance completeness gate**: every compiled PolicyRule must have non-empty PolicyProvenance with valid PolicySectionRef (Hard)
- **Coverage gap warning**: <80% fixture coverage → governance warning (ReportOnly, promote to Hard)
- **Section drift warning**: any PolicySectionRef with stale quote_excerpt_hash → governance warning (ReportOnly)

### B.10 Implementation phasing

| Loom Component | Depends On | Integrates With |
|---|---|---|
| PolicyDocFamily + PolicyDocVersion + PolicySectionRef | Phase 0 | Phase 1 (registry) |
| PolicyInterpretationBody + directive types | Phase 3 | Phase 3 |
| PolicyRuleSet + compilation + ActionPolicyLinkage + ActionRef | Phase 6 | Phase 6 |
| PolicyTestFixture + fixture gate + inheritance | Phase 6 | Phase 6 |
| CoverageSpec + merge semantics (§6.3.1) | Phase 7 | Phase 7 |
| Action linkage materialization + ActionRef resolution | Phase 8 | Phase 8 |
| Runbook compilation | Phase 8 | Phase 8 |
| Policy MCP tools + drift check | Phase 8 | Phase 8 |
| Policy impact analysis (graph traversal) | Phase 9 | Phase 9 |
| Policy coverage metrics | Phase 9 | Phase 9 |
| End-to-end traceability test | Phase 10 | Phase 10 |

---

## Appendix C — Version History

| Version | Date | Key Changes |
|---------|------|-------------|
| v0.1 | Feb 2026 | Vision, scope, capabilities, approach — position paper |
| v0.2 | Feb 2026 | Structural interpretation layer, conflict resolution, test harness as publish gate, runbook compilation, Semantic OS integration, DSL verb linkage |
| v0.3 | Feb 2026 | Action contract generalisation (substrate-agnostic). Output merge semantics by type (§6.3.1). Disputed interpretation lifecycle. Active version constraint. Implementation to appendix. |
| v0.4 | Feb 2026 | Identity model (§4.2). PolicySectionRef with anchor IDs, positional fallback, drift detection (§6.1.1). Activation scope key clarification (§4.3). Threshold merge formalism with direction-aware functions (§6.3.1). ActionRef union type with substrate variants (§6.4.3). Fixture inheritance and supersession (§6.6). Impact analysis data sources explicit (§4.6, §6.4.2). LLM/determinism constraint in non-goals and §6.7. DMN as derived rendering only (§7.3). Editorial tightening. |

## Appendix D — Glossary

| Term | Definition |
|---|---|
| **Policy Loom** | The capability to turn policy from static text into executable operations via the Semantic OS |
| **PolicyDocFamily** | Stable identity for a policy concept across versions |
| **PolicyDocVersion** | An immutable published revision of a policy document with section_index |
| **PolicyInterpretationVersion** | A structured operational reading of a policy version (typed directives, SME-approved) |
| **PolicyRuleSet** | A compiled bundle of PolicyRule snapshots generated from an Approved interpretation |
| **CoverageSpec** | Multi-dimensional applicability predicate with exclusions and specificity ranking |
| **ActionPolicyLinkage** | Explicit reference from a PolicyRuleSet to the action contracts it directs |
| **ActionRef** | Canonical substrate-aware action identity (DSL FQN, Java method, BPMN task, custom) |
| **PolicyProvenance** | Provenance chain on each PolicyRule: rule → ruleset → interpretation → legal text + section |
| **PolicySectionRef** | Address for a specific section of a PolicyDocVersion (anchor, positional fallback, drift hash) |
| **PolicyTestFixture** | A scenario input/output pair with inheritance and supersession, serving as a publish gate |
| **Merge semantics** | Type-specific, direction-aware rules for resolving overlapping policy outputs (§6.3.1) |
| **Runbook compilation** | Deterministic process of compiling an AgentPlan from policy-driven evidence gaps |
| **Action contract** | Generalised term for an executable operation definition (DSL verb, Java service, BPMN task) |
| **Disputed** | Interpretation lifecycle state: legal/policy team has challenged the reading; blocks compilation |
| **Activation scope key** | `(family_id, jurisdiction_scope)` — the key against which the active-version constraint is enforced |
