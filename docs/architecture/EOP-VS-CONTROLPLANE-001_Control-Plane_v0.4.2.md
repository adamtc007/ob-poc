# EOP Control Plane
### Vision & Scope for Governed AI-Led Execution

| | |
|---|---|
| **Document** | EOP-VS-CONTROLPLANE-001 |
| **Version** | v0.4.2 (Draft) |
| **Type** | Vision & Scope |
| **Status** | Draft — fit for peer review |
| **Purpose** | Define the Control Plane as a first-class capability for AI-led, governed, deterministic execution in the Enterprise Onboarding Platform, mapped to the NIST AI Risk Management Framework as a control crosswalk. |
| **Audience** | Architecture, Engineering, Product, Risk, Controls, Audit |
| **Related** | Verb-Governed State; SemOS; DSL; DAG; Sage; REPL; Runbook Runtime |

## Change Log

| Version | Change |
|---|---|
| v0.1 | Initial draft as "EOP Control Pack" (EOP-VS-CONTROLPACK-001). |
| v0.2 | Capability renamed **Control Plane** (see §1.1). Added: NIST AI RMF mapping (Appendix A); decision snapshot semantics (§6.15); envelope single-use, expiry and pre-state pinning (§6.10); type-level envelope provenance and proof-carrying construction (§9.4); pipeline evaluation strategy (§6.16); Phase 0 control inventory (§13); write-set attestation ownership (§6.7); resolution of probabilistic-signal handling (§6.13.1). Tightened repetition between §7 and §12. |
| v0.3 | Review hardening. NIST language softened from "formalised against" to design **crosswalk** with an explicit pre-Phase-5 caveat (Appendix A). Write-set attestation failure semantics sharpened: abort-before-commit default, post-durability quarantine (§6.7.1). Evidence ownership deduplicated between authority gate and evidence gate (§6.5/§6.6). Durable/queued execution note added: `EnvelopeHandle` rehydration through Control Plane verification (§6.10.4). Gate dependencies declared, with dependency table (§6.16.1). Version-pin precision note for legacy non-versioned (Mode-1) state (§6.10.1). Terminology: `ControlPlaneProof` used consistently; "approved runtime admission path" replaces "runtime execution path" (§6.9). |
| v0.4 | Amendment 1: Clearing-House Mandate. New §15 (mediation topology; leakproof L1–L3; coverage C1–C3; pack universality K1–K3; read-lens decision §15.5; migration posture). §8 relationship directions inverted. §12 criteria 13–15 added. v0.3 checkpoint topology reclassified as transitional. |
| v0.4.1 | Micro-amendment: C3 constitutive clarification (ruling on MCA-002 escalation E-4). §15.3's C3 ("every agent-originated state transition has a Control Plane decision record reachable from its audit trail") is **constitutive at mediation completion (T12), not owed during checkpoint topology (T0–T11)**. Best-effort, non-blocking shadow-decision persistence is the conformant posture through T11; C3 graduates to a hard guarantee only when enforce-mode/mediation is live. C3 is reclassified MIGRATION-PENDING, sharing AB1's T12/mediation terminus, not a T11 exit criterion. |
| v0.4.2 | Micro-amendment: new §7.1, definitional vs judgmental clauses within the Binary Tollgate (T11.F.2 implementation work). Three of the nine §7 clauses (intent recognition, active pack, DAG legality) each conflate an unconditional structural fact with a judgmental policy/authority sub-case; §7.1 names the split for those three and states its enforcement posture (definitional core unconditional from T11.F onward, independent of shadow-vs-enforce mode; judgmental sub-cases stay shadow-first/graduated, unchanged terminus at T12). The remaining six §7 clauses are unamended — architect ruling: they are "more technical fails," not conflations requiring the same split. |

---

# 1. Executive Summary

The Enterprise Onboarding Platform already contains the major primitives required for AI-led straight-through processing: a DSL for semantic business intent, a DAG/state-machine model for lawful state transitions, Semantic OS for binding data to verbs and authority, Sage for agent interpretation, REPL/compiler validation, and a deterministic runtime.

What is missing is a first-class **Control Plane** capability.

The Control Plane is the layer that turns those primitives into an executable assurance framework. It answers the question:

> Is this AI-proposed operational action allowed to execute, against these entity instances, in this state, under this authority, with this evidence, through this governed path, producing this bounded write-set and audit record?

The Control Plane is not a policy document beside the system. It is policy made executable.

Its job is to convert AI-led intent into a deterministic, governed execution plan — or reject it.

Today this capability exists only in ad-hoc form, distributed across Sage, the REPL and the compiler. This document defines it as a first-class capability, implemented as a dedicated crate, with a proposed crosswalk to the NIST AI Risk Management Framework (Appendix A).

## 1.1 Naming note

v0.1 of this document named the capability the "Control Pack". That name is retired. **Pack** is reserved exclusively for SemOS domain packs (§6.3); overloading the term invites conflation of the governance artefact (a SemOS pack) with the governance layer (the Control Plane) in both prose and code. The capability is the **Control Plane**; the crate is `ob-poc-control-plane`.

---

# 2. Problem Statement

AI can assist operators today: draft, summarise, classify, search, recommend and prepare work.

That is not the same as AI executing regulated operational work.

For AI to lead execution against systems of record, the platform needs a binary tollgate between:

1. **AI-assisted work**, where the model supports a human who remains the execution authority; and
2. **AI-executed operational work**, where a governed platform permits a state transition to occur because the proposed action has passed all required controls.

Prompt engineering and review workflow are not sufficient for regulated execution. They may improve output quality and reduce risk, but they do not define deterministic authority, bounded write-scope, replayable state transition, or accountable control.

The platform already has the component parts. Portions of the required control behaviour already run in production paths — but as an **ad-hoc control plane**: checks embedded in Sage's interpretation loop, validation logic in the REPL, and gating inside the compiler, with no single owner, no unified decision object, and no guarantee that every execution path passes through every gate.

What the platform lacks is a first-class capability that assembles these controls into a single, owned, provable execution decision.

That capability is the Control Plane.

---

# 3. Vision

The Control Plane is the governed execution-control layer for AI-led onboarding.

It ensures that no AI-originated action can move operational state unless it has been:

1. expressed as a recognised semantic intent;
2. bound to concrete entity instances;
3. resolved inside an active SemOS pack;
4. validated against the relevant DAG/state-machine constraints;
5. checked against actor, authority and policy;
6. compiled into a deterministic runbook;
7. assigned a bounded write-set;
8. classified as STP, human-gated or rejected;
9. executed only through approved runtime paths;
10. recorded with full audit, evidence, version and replay context.

The Control Plane makes the distinction explicit:

> The AI may propose intent.
> The Control Plane decides whether that intent is executable.
> The runtime, not the AI, moves state.
> …and the Control Plane is the only party that can ask it to.

---

# 4. Core Principle

The Control Plane exists because verbs alone are not enough.

A verb catalogue names possible work. It does not, by itself, prove that work is executable.

The executable unit is:

> **an entity-bound governed semantic command**

That means:

- a named verb;
- bound to concrete entity instances;
- inside a known domain pack;
- checked against current state;
- validated against the DAG;
- authorised against policy;
- compiled into a bounded execution plan;
- recorded for audit and replay.

The Control Plane therefore binds together:

| Primitive | Control role |
|---|---|
| **DSL** | Expresses semantic business intent |
| **Entity instances** | Define concrete execution scope |
| **DAG/state machine** | Defines legal transition paths |
| **Semantic OS** | Defines meaning, authority, pack context and verb/data binding |
| **REPL/compiler** | Proves the plan before execution |
| **Runbook** | Captures the executable plan |
| **Runtime** | Performs deterministic state transition |
| **Audit stream** | Records what happened, why, by whom and under which authority |

The Control Plane is the capability that assembles those into a single execution decision.

A corollary of this principle: the Control Plane is a **decision assembler over borrowed proofs**, not a re-implementation of validation logic. DAG legality lives in the state-graph layer; entity resolution lives in the compiler; authority lives in SemOS. The Control Plane invokes those validators and owns the composition of their outputs into one decision. Duplicating validator logic inside the Control Plane is a design violation (§11).

---

# 5. Scope

## 5.1 In scope

The Control Plane covers the pre-execution, execution-authorisation and post-execution assurance boundary for AI-led operational work.

It includes:

- semantic intent validation;
- entity binding validation;
- active SemOS pack resolution;
- DAG reachability and state-slot enforcement;
- authority and policy checks;
- bounded write-set derivation;
- STP eligibility classification;
- human gate determination;
- runbook proof generation;
- execution envelope creation;
- decision snapshot pinning;
- version pinning;
- audit/replay evidence;
- exception routing;
- assurance metrics.

## 5.2 Out of scope

The Control Plane does not:

- replace the DSL;
- replace the DAG;
- replace SemOS;
- replace the runtime;
- perform probabilistic intent classification itself (it consumes deterministic attestations of probabilistic signals — see §6.13.1);
- decide business policy outside configured authority;
- allow direct system-of-record mutation;
- provide a generic workflow engine;
- act as an LLM agent.

It is not the AI layer.

It is the execution-control layer that makes AI-led execution governable.

---

# 6. Required Capabilities

## 6.1 Intent Admission

The Control Plane must admit only recognised semantic intents.

It must reject:

- unknown verbs;
- deprecated verbs;
- hallucinated actions;
- verbs outside the active pack;
- verbs not available to the current actor/context;
- free-form tool requests masquerading as business actions;
- candidate intents lacking a valid interpretation attestation (§6.13.1).

Output:

> `IntentAdmissionDecision`

Possible outcomes:

- `admitted`;
- `rejected_unknown_intent`;
- `rejected_outside_pack`;
- `rejected_deprecated`;
- `rejected_unauthorised_surface`;
- `rejected_attestation_insufficient`.

---

## 6.2 Entity Binding

The Control Plane must require verbs to bind to concrete entity instances before execution.

A verb without entity scope is not executable.

The binder must validate:

- entity existence;
- entity type;
- entity lifecycle state;
- entity relationship to other bound entities;
- entity membership in the active domain pack;
- entity visibility versus write authority;
- whether the entity is locked, suspended, archived or otherwise unavailable.

Example:

> `cbu.suspend(CBU-123)` is executable only if `CBU-123` exists, is in a suspendable state, belongs to the active pack, and the actor/context has authority to suspend it.

Output:

> `EntityBindingReport`

---

## 6.3 Semantic Pack Resolution

The Control Plane must resolve the active SemOS pack for the proposed execution.

The pack defines:

- domain context;
- permitted verb families;
- governed nouns/entities;
- authority model;
- role bindings;
- lifecycle lenses;
- applicable policies;
- evidence requirements;
- audit requirements.

No active pack means no execution.

Output:

> `PackResolution`

Possible outcomes:

- `resolved`;
- `ambiguous_pack`;
- `missing_pack`;
- `pack_denies_intent`;
- `pack_denies_entity`.

---

## 6.4 DAG and State-Slot Enforcement

The Control Plane must validate that a proposed state transition is legal in the current DAG/state-machine context.

It must check:

- current state;
- required from-state;
- allowed to-state;
- transition reachability;
- lifecycle slot ownership;
- mutually exclusive state axes;
- required substates;
- forbidden transitions;
- terminal states;
- wire-later or unimplemented transitions;
- cross-domain guards.

This is where the platform proves:

> The proposed action is legal for this entity, in this state, through this governed state slot.

Output:

> `StateTransitionProof`

Possible outcomes:

- `legal`;
- `illegal_from_state`;
- `illegal_to_state`;
- `unreachable`;
- `wrong_lifecycle_axis`;
- `transition_unimplemented`;
- `guard_failed`.

---

## 6.5 Authority and Policy Gate

The Control Plane must decide whether the actor/context has authority to execute the proposed command.

Authority is not the same as authentication.

The gate must check:

- actor identity;
- agent identity;
- human sponsor where applicable;
- role;
- delegation;
- approval threshold;
- segregation of duties;
- policy constraints;
- jurisdiction constraints;
- product/service constraints;
- risk tier;
- the evidence decision result from §6.6, where policy requires it;
- manual approval requirement.

Evidence readiness itself is owned by the evidence gate (§6.6). The authority gate consumes that gate's outcome; it does not re-evaluate evidence. Every control has exactly one owner (§13, Phase 0 exit criterion).

Output:

> `AuthorityDecision`

Possible outcomes:

- `authorised`;
- `requires_human_approval`;
- `requires_second_line_review`;
- `rejected_unauthorised`;
- `rejected_segregation_of_duties`;
- `rejected_policy`.

---

## 6.6 Evidence and Obligation Check

The Control Plane must verify that required evidence and obligations exist before execution.

For onboarding/KYC this includes:

- required documents;
- source provenance;
- proof status;
- beneficial ownership/control evidence;
- screening status;
- risk classification;
- approvals;
- open obligations;
- expired evidence;
- conflicting evidence;
- missing attestations.

Output:

> `EvidenceReadiness`

Possible outcomes:

- `sufficient`;
- `missing_required_evidence`;
- `expired_evidence`;
- `conflicting_evidence`;
- `pending_approval`;
- `obligation_open`.

---

## 6.7 Bounded Write-Set Derivation

The Control Plane must derive the exact write-set permitted for the command.

A command may only write:

- declared state slots;
- declared intent/event tables;
- declared projection targets;
- declared audit records;
- declared downstream invocation records.

It must not be possible for a command to mutate arbitrary fields because it holds an entity.

The write-set must be known before execution and verified after execution.

Output:

> `WriteSetProof`

The proof must include:

- entity IDs;
- state slots;
- tables/projections/events;
- allowed columns/fields;
- forbidden adjacent state;
- downstream systems/resources;
- idempotency key;
- transaction scope;
- lock scope.

### 6.7.1 Write-set attestation (post-execution)

Post-execution verification has a named owner and a defined mechanism.

Because versioned rows are the system of record, every write in the execution transaction produces version rows. **Write-set attestation** is the check, performed by the **runtime in its commit path**, that the set of version rows written by the transaction is a subset of the `WriteSetProof`.

Failure semantics are explicit:

- **Attestation is performed before commit wherever technically possible.** If the observed write-set exceeds the proof, the runtime **must abort the transaction** and emit a control-breach event. The out-of-scope write never becomes durable.
- **If a breach is detected only after durability** (e.g. via asynchronous verification of downstream projections), the affected execution is **quarantined**: the breach is emitted to the audit stream, the case is routed to exception handling (§6.13), and exception handling owns remediation. A post-durability breach is never merely logged.

In both cases the breach counts against the post-execution defect metrics (§6.14). The default posture for regulated execution is abort, not record-and-continue.

The Control Plane defines the attestation contract; the runtime enforces it. The Control Plane does not perform the attestation itself (it is not a second runtime — §11).

---

## 6.8 STP Eligibility Classification

The Control Plane must classify each proposed execution as one of:

| Classification | Meaning |
|---|---|
| **STP executable** | Valid, authorised, evidence complete, no human gate required |
| **Human-gated** | Valid plan, but approval/review required before execution |
| **Rejected** | Invalid, ambiguous, unauthorised, incomplete or outside scope |

This classification must be deterministic: a pure function of the candidate intent, the actor/context, and the decision snapshot (§6.15). Given the same inputs, the classification is identical on replay.

The AI must not self-certify STP eligibility.

Output:

> `StpEligibilityDecision`

---

## 6.9 Runbook Proof Generation

Before execution, the Control Plane must produce a reviewable `ControlPlaneProof`.

The `ControlPlaneProof` must show:

- proposed semantic intent;
- actor and authority context;
- active SemOS pack;
- bound entities;
- current state;
- proposed transition;
- DAG proof;
- policy decision;
- evidence decision;
- write-set;
- expected post-state;
- required approvals;
- audit envelope;
- idempotency and correlation keys;
- decision snapshot pins (§6.15);
- approved runtime admission path.

Output:

> `ControlPlaneProof`

This is the pre-execution artefact that allows the platform, operator, reviewer or auditor to understand exactly what will happen.

---

## 6.10 Runtime Execution Envelope

The Control Plane must create the execution envelope passed to the runtime.

The envelope must include:

- approved runbook ID;
- compiled command set;
- active governance versions;
- actor/agent/human sponsor;
- authority decision;
- write-set proof;
- lock scope;
- idempotency keys;
- audit requirements;
- exception path;
- timeout/retry policy;
- compensation path where applicable;
- **expected pre-state pins** (§6.10.1);
- **validity window** (§6.10.2).

The runtime must reject any execution without a valid envelope.

Output:

> `ExecutionEnvelope`

### 6.10.1 Pre-state pinning (TOCTOU protection)

The Control Plane's proofs are computed against a snapshot (§6.15). Between envelope creation and runtime execution, another writer may advance the entity. The envelope therefore carries the **expected pre-state version** for every bound entity — the monotonic sequence number of the version row against which the proofs were computed.

At execution time, the runtime compares the current version sequence of each bound entity against the pinned expectation. On mismatch, the runtime **must not execute**: the envelope is voided and the case is routed to the `stale_state` exception (§6.13). This is optimistic concurrency at the control boundary, using the platform's existing last-insert-wins monotonic sequence as the check value.

**Precision note — non-versioned state.** For any entity admitted to Control Plane execution, a comparable version pin must exist. Where legacy Mode-1 guarded-update state is not yet versioned, the migration must provide a synthetic pin — a version surrogate, row hash or lock token — before that entity may participate in AI-executed STP. An entity with no comparable pin is not eligible for STP execution; it may only proceed human-gated, and the absence of a pin is recorded in the `ControlPlaneProof`.

### 6.10.2 Single-use and expiry

The audit record is replayable; the envelope, as an execution authorisation, is not.

- **Single-use.** One envelope authorises at most one execution attempt (idempotency-keyed retries within the runtime's own retry policy count as one attempt). A consumed envelope cannot be resubmitted. Replay of an approved envelope would constitute a duplicated regulated action; the runtime must reject it as a control breach.
- **Expiry.** Every envelope carries a bounded validity window (TTL). An expired envelope is void regardless of pre-state pins. Expired-envelope submission routes to exception handling, not silent re-evaluation.

Replaying the *decision* (re-running `evaluate` against the pinned snapshot to confirm the same outcome) is always permitted and is a required assurance capability (§6.14). Replaying the *execution* is forbidden.

### 6.10.3 Envelope provenance

The runtime must be able to trust that an envelope originated from the Control Plane and passed every gate. Within the workspace this is enforced at the type level rather than cryptographically:

- `ExecutionEnvelope` is constructible **only** inside `ob-poc-control-plane` (private fields, no public constructor, no deserialisation implementation on the type the runtime accepts);
- construction requires the full set of gate proofs by signature (§9.4);
- the workspace-wide `#![deny(unreachable_pub)]` discipline and the public-API surface gate guarantee the constructor cannot leak.

If the runtime holds an `ExecutionEnvelope`, it provably came through the gates. Should the runtime and Control Plane ever deploy as separate processes, this guarantee must be re-established cryptographically (signed envelopes); that is a deployment-topology decision out of scope for this document, but the requirement is recorded here.

### 6.10.4 Durable and queued execution

Long-running, queued or resumable execution requires envelopes to survive process boundaries, which is in tension with "no deserialisation on the runtime-accepted type". The resolution:

- durable or queued execution persists an **`EnvelopeHandle`** — or a signed/sealed envelope record — never a directly reconstructible `ExecutionEnvelope`;
- rehydration of a handle into the runtime-accepted type occurs **only through Control Plane verification**: the Control Plane re-validates single-use status, validity window and pre-state pins against the persisted record, then reconstructs the envelope through its private constructor;
- a handle that fails rehydration verification is voided and routed to exception handling, exactly as a stale or expired envelope would be.

The type-level provenance guarantee (§6.10.3) is therefore preserved across persistence: at no point does a deserialisation path exist that yields an `ExecutionEnvelope` without passing back through the Control Plane.

---

## 6.11 Audit and Replay Record

The Control Plane must ensure every approved execution produces an audit record connecting:

- original utterance or trigger;
- interpreted intent;
- selected verb;
- bound entities;
- active pack;
- DAG/state proof;
- authority decision;
- evidence decision;
- write-set;
- write-set attestation result (§6.7.1);
- execution result;
- pre-state;
- post-state;
- model/prompt version where AI was involved;
- interpretation attestation where AI was involved (§6.13.1);
- human approval where required;
- correlation IDs;
- decision snapshot pins;
- timestamps;
- exceptions.

The audit record must allow reconstruction of:

> why state changed, who/what caused it, under what authority, and through which governed path.

---

## 6.12 Version Pinning

The Control Plane must pin all versions involved in the execution decision.

At minimum:

- DSL version;
- verb catalogue version;
- SemOS pack version;
- DAG version;
- data model/schema version;
- policy/authority version;
- prompt/model version;
- compiler version;
- runtime version;
- control-plane version.

Version identity must be **content-addressed or governance-sequenced** — a hash or monotonic governance sequence number, not a human-assigned label alone. Replay requires that a pinned version resolves to exactly one artefact; a mutable tag cannot satisfy that.

No replayable control means no regulated execution.

---

## 6.13 Exception Routing

The Control Plane must route non-executable cases into controlled exception states.

Exception classes include:

- unknown intent;
- ambiguous intent;
- missing entity;
- ambiguous entity;
- invalid lifecycle state;
- missing evidence;
- unauthorised actor;
- policy conflict;
- failed approval gate;
- failed downstream invocation;
- stale state (including pre-state pin mismatch, §6.10.1);
- expired or replayed envelope (§6.10.2);
- write-set attestation breach (§6.7.1);
- lock conflict;
- interpretation attestation failure;
- prompt-injection suspicion.

The result must be a controlled work item, not an uncontrolled failure.

### 6.13.1 Probabilistic signals as deterministic inputs

The Control Plane performs no probabilistic classification itself (§5.2). Model confidence and prompt-injection signals originate in the interpretation layer (Sage), which attaches an **interpretation attestation** to every candidate intent it proposes: model/prompt version, confidence score, and safety-screen outcomes.

The Control Plane treats the attestation as data:

- an absent attestation on an AI-originated intent is a deterministic rejection at intent admission (§6.1);
- a sub-threshold confidence score is a deterministic rejection or human-gate trigger, per configured policy;
- a raised injection-suspicion flag routes to exception handling.

The threshold comparison is deterministic and snapshot-pinned; the probability behind it is Sage's, not the Control Plane's. This preserves the boundary: Sage proposes and attests, the Control Plane enforces.

---

## 6.14 Assurance Metrics

The Control Plane must emit metrics for governance and continuous assurance.

Metrics should include:

- intent admission rate;
- rejected unknown intent count;
- hallucinated verb rejection rate;
- entity binding failure rate;
- DAG rejection rate;
- authority rejection rate;
- evidence failure rate;
- STP eligibility rate;
- human-gated rate;
- post-execution defect rate;
- write-set attestation breach count;
- replay success rate;
- audit completeness;
- model/version drift indicators;
- attestation threshold failure rate;
- override rate;
- exception ageing.

These metrics support architecture review, control review, model-risk review and operational governance. They map to the **MEASURE** function of the NIST AI RMF (Appendix A).

Note the dependency on §6.16: per-gate rejection metrics are only meaningful under an evaluation strategy that does not censor downstream gates behind the first upstream failure.

---

## 6.15 Decision Snapshot

Determinism requires defined inputs. The authority, evidence and state gates read live operational data; "deterministic" is only meaningful **relative to a snapshot**.

Every Control Plane evaluation therefore executes against a **decision snapshot**:

- all reads performed during evaluation are pinned to a single as-of point — a bitemporal coordinate and/or the monotonic version sequence, consistent with the platform's uniform bitemporality;
- the snapshot pins are recorded in the `ControlPlaneProof`;
- the decision is a pure function of `(candidate intent, actor/context, snapshot)`;
- replaying the decision means re-evaluating against the pinned snapshot and obtaining an identical outcome — not re-evaluating against current state.

Without the snapshot, "replay" silently degrades into re-evaluation against drifted data, and the audit claim of §6.11 is unprovable.

---

## 6.16 Pipeline Evaluation Strategy

The gate sequence in §3 could be evaluated fail-fast (stop at first failure) or collect-all (evaluate every gate and aggregate failures). The Control Plane uses **collect-where-independent**:

- every gate whose inputs do not depend on the output of a failed predecessor is evaluated;
- gates that structurally depend on a failed predecessor (e.g. DAG proof requires a successful entity binding) are recorded as `not_evaluated` with the blocking predecessor named;
- all failures are aggregated into the `ControlPlaneRejection`;
- the STP classifier (§6.8) is the single reduction point from the aggregated gate results to the final three-way decision.

Rationale: fail-fast censors downstream gate metrics behind upstream rejections, degrading the assurance metrics (§6.14), and produces thinner rejection artefacts for operators and auditors. A rejection that reports *every* failed control is a better work item than one that reports the first.

### 6.16.1 Declared gate dependencies

Gate dependencies are **declared, not inferred procedurally**. The dependency graph is a governance artefact of the Control Plane itself — versioned (§6.12), testable, and identical across every implementation of the evaluation loop. Without declaration, "independent" becomes an ad-hoc per-implementation judgement and the evaluation strategy is untestable.

The baseline dependency declaration:

| Gate | Depends on |
|---|---|
| Intent admission | interpretation attestation (where AI-originated) |
| Entity binding | admitted intent |
| Pack resolution | admitted intent + bound entities |
| DAG/state proof | bound entities + resolved pack |
| Authority gate | admitted intent + resolved pack (+ evidence decision where policy requires) |
| Evidence gate | bound entities + resolved pack |
| Write-set proof | legal transition + compiled runbook |
| STP classifier | aggregate of all gate results |

A gate is evaluated when all of its declared predecessors succeeded; otherwise it is recorded as `not_evaluated` with the blocking predecessors named. The dependency declaration is subject to the same regression suite as the gates themselves (§13, Phase 5).

---

# 7. The Binary Tollgate

The Control Plane defines the binary gate for AI execution:

> No recognised intent — no execution.
> No bound entity — no execution.
> No active SemOS pack — no execution.
> No DAG legality — no execution.
> No authority — no execution.
> No evidence readiness — no execution.
> No bounded write-set — no execution.
> No compiled runbook — no execution.
> No execution envelope — no execution.
> No audit/replay path — no execution.

This is the line between AI-assisted work and AI-executed regulated work. The success criteria in §12 are the testable form of this gate; the two sections state one rule.

## 7.1 Definitional vs judgmental clauses within the Tollgate

**(v0.4.2 — T11.F.2 implementation finding, ratified.)** Each bullet above states an unconditional rule, but the underlying gate's real outcome space is not uniformly unconditional. Three of the nine clauses conflate a **definitional** core (a structural fact no legitimate traffic can produce — e.g. the named verb does not exist in the runtime registry at all) with **judgmental** sub-cases (policy/authority/evidence-shaped determinations — e.g. the verb exists but ABAC denies this actor):

- **"No recognised intent"** (Intent Admission, §6.1): definitional = the verb_fqn is absent from the runtime registry. Judgmental = the verb exists but is pruned by ABAC/entity-kind/agent-mode/policy (`PruneReason`'s four variants).
- **"No active SemOS pack"** (Pack Resolution, §6.3): definitional = no candidate pack resolves, or more than one does (`MissingPack`/`AmbiguousPack`). Judgmental = a resolved pack's own authored rule denies the intent or entity (`PackDeniesIntent`/`PackDeniesEntity`).
- **"No DAG legality"** (DAG/State Proof, §6.4): definitional = the transition does not exist in the declared DAG topology, including a violated `CrossWorkspaceConstraint` (v1.3 Mode A). Judgmental = a lifecycle fail-open/fail-closed policy setting.

The remaining six clauses (Entity Binding, Authority, Evidence, Write-Set, Runbook, Execution Envelope/Audit) are **not** reclassified by this amendment. Ratified as **technical failure modes, not definitional/judgmental conflations** — each already fails on a single, uniform kind of check (a binding either succeeds or it doesn't; a runbook either compiles or it doesn't), so this subsection's split does not apply to them.

**Enforcement posture (§15.6 migration posture applies unchanged):** the definitional core of each of these three clauses is enforced unconditionally, independent of shadow-vs-enforce mode, from T11.F onward — this is not itself full mediation (§15.1) and does not change AB1/C3's T12 terminus. The judgmental sub-cases remain shadow-first/graduated per existing policy, unaffected by this amendment.

---

# 8. Relationship to Sage, REPL, DSL, DAG and SemOS

## 8.1 Sage

**(v0.4 — direction inverted per §15.1; see Amendment 1.)** The Control Plane receives the utterance and invokes Sage as its interpretation capability, granting the interpretation context (per §15.5). Sage returns candidate intents with attestations; Sage holds no capability keys and cannot dispatch.

Sage does not execute.

## 8.2 DSL

The DSL expresses semantic business intent in a typed, structured form.

The DSL does not, by itself, authorise execution.

## 8.3 SemOS

SemOS provides the semantic authority layer: packs, nouns, verbs, domain scopes, entity meaning and governance context.

SemOS tells the Control Plane what a command means and where it is allowed to operate.

## 8.4 DAG

The DAG provides lifecycle topology and transition legality.

The DAG tells the Control Plane whether a proposed transition is reachable and lawful for the current state.

## 8.5 REPL / Compiler

The REPL/compiler validates, resolves and compiles the command into an executable runbook.

The Control Plane consumes compiler proof and decides whether the runbook may proceed to execution.

**(v0.4 — direction inverted per §15.1; see Amendment 1.)** The compiler is invoked by, and only by, the clearing house on agent-originated flows — it is a delegated call from the hub, not an upstream stage the Control Plane is inserted after.

## 8.6 Runtime

The runtime executes only approved envelopes, verifies pre-state pins before execution (§6.10.1), enforces envelope single-use and expiry (§6.10.2), and performs write-set attestation in its commit path (§6.7.1).

It does not accept direct AI instructions.

**(v0.4 — direction inverted per §15.1; see Amendment 1.)** The runtime is invoked by, and only by, the clearing house on agent-originated flows — it is a delegated call from the hub, not an upstream stage the Control Plane is inserted after.

## 8.7 Migration from the ad-hoc control plane

Sage, the REPL and the compiler currently perform overlapping subsets of these controls in situ. The Control Plane crate does not add a parallel set of checks beside them — it becomes the single owner of the execution decision, and the existing in-situ checks are inventoried and dispositioned (Phase 0, §13). A control enforced in two places with subtly different semantics is worse than a control enforced once.

---

# 9. Proposed First-Class Crate

The Control Plane should exist as a first-class crate:

```text
ob-poc-control-plane
```

The name follows the workspace `ob-poc-*` convention. The v0.1 candidates `ob-poc-control-pack` and `eop-control-pack` are retired with the capability rename (§1.1).

## 9.1 Crate responsibility

The crate owns the execution-control decision.

It should not own:

- LLM prompting;
- DSL parsing;
- DAG authoring;
- SemOS authoring;
- runtime state mutation;
- re-implementations of validator logic owned by other crates (§4).

It should own:

- assembling the control context and decision snapshot;
- invoking the required validators;
- aggregating gate results under the collect-where-independent strategy;
- producing the proof;
- classifying STP eligibility;
- creating the execution envelope;
- emitting control/audit events and assurance metrics.

## 9.2 Conceptual modules

```text
control_plane/
  intent_admission
  entity_binding
  pack_resolution
  dag_proof
  authority_gate
  evidence_gate
  write_set
  stp_classifier
  snapshot
  proof
  envelope
  audit
  metrics
  exceptions
  versioning
```

## 9.3 Core API shape

Conceptual API:

```text
evaluate(candidate_intent, context) -> ControlPlaneDecision
```

Where `ControlPlaneDecision` is one of:

```text
ApprovedStp(ExecutionEnvelope)
RequiresHumanGate(ControlPlaneProof)
Rejected(ControlPlaneRejection)
```

The runtime accepts only:

```text
ExecutionEnvelope
```

not raw agent output.

## 9.4 Proof-carrying construction

The tollgate of §7 is enforced by the type system, not by a runtime checklist. Each gate returns a distinct proof type, and only the success forms are accepted by the envelope constructor — parse-don't-validate, applied at the control boundary:

```rust
impl ExecutionEnvelope {
    /// The only constructor. Private to the crate; requires every
    /// gate's success proof by signature. There is no code path from
    /// a rejection to an envelope.
    pub(crate) fn seal(
        intent:    AdmittedIntent,        // not a raw candidate intent
        binding:   BoundEntities,         // not an unchecked entity list
        pack:      ResolvedPack,
        dag:       LegalTransition,       // success form of StateTransitionProof
        authority: Authorised,            // success form of AuthorityDecision
        evidence:  EvidenceSufficient,
        write_set: WriteSetProof,
        runbook:   CompiledRunbook,
        snapshot:  SnapshotPins,
        validity:  ValidityWindow,
    ) -> ExecutionEnvelope { /* ... */ }
}
```

Design consequences:

- success proofs are distinct types, not enum variants inspected at runtime — it is a **compile error** to construct an envelope from a rejection path;
- each proof type is constructible only by its own gate module (module-private constructors under the workspace `unreachable_pub` discipline);
- `ExecutionEnvelope` has private fields, no public constructor, and no `Deserialize` on the type the runtime accepts (§6.10.3);
- the failure forms carry structured rejection reasons and feed `ControlPlaneRejection` and the metrics plane.

The gate list in `seal`'s signature *is* the tollgate. Adding a gate to the platform means adding a parameter; forgetting to run it becomes unrepresentable rather than undetected.

---

# 10. Control Plane Decision Model

The core decision object should answer:

| Question | Answered by |
|---|---|
| What is being requested? | Intent admission |
| What concrete entities are in scope? | Entity binding |
| Which domain pack governs this? | SemOS pack resolution |
| Is the transition legal? | DAG/state proof |
| Is the actor authorised? | Authority gate |
| Is evidence sufficient? | Evidence gate |
| What can be written? | Write-set proof |
| Against which state was this decided? | Decision snapshot |
| Can this go STP? | STP classifier |
| What will execute? | Runbook proof |
| How is execution contained? | Execution envelope |
| How will it be audited? | Audit envelope |

---

# 11. Non-Goals

The Control Plane must not become:

- a general-purpose workflow engine;
- a replacement for SemOS;
- a replacement for the DSL compiler;
- a re-implementation of validators owned elsewhere;
- a hidden rule engine;
- a prompt orchestration layer;
- a generic policy dumping ground;
- a second runtime;
- a bypass route around the governed execution model.

Its purpose is narrow:

> decide whether a proposed AI-led action may become governed execution.

---

# 12. Success Criteria

The Control Plane is successful when the platform can prove, before execution:

1. the intent is known and attested;
2. the entities are bound;
3. the active pack is known;
4. the transition is legal;
5. the actor/context is authorised;
6. required evidence exists;
7. the write-set is bounded;
8. the execution is classified as STP, gated or rejected — deterministically, against a pinned snapshot;
9. the runbook is compiled;
10. the runtime envelope is valid, single-use, in-window, and pre-state-pinned;
11. replay of the decision reproduces the decision; replay of the envelope is rejected;
12. audit reconstruction (§6.11) is guaranteed.

**(v0.4 additions — Amendment 1, §15):**

13. the dependency-graph gate (L1) is green with zero agent→capability edges outside the Control Plane;
14. `capability_invocations_without_cp_provenance` ≡ 0 over a full graduation window, on all packs, attested on the assurance plane;
15. a newly authored pack demonstrates K3: full coverage with zero pack-specific coverage work, evidenced at onboarding.

It is also successful when the platform can reject unsafe AI output deterministically.

A rejected AI action should be a normal, auditable control outcome — not an exception to the architecture.

---

# 13. Implementation Direction

## Phase 0 — Control Inventory (extraction from the ad-hoc plane)

- Enumerate every control check currently performed in Sage, the REPL and the compiler.
- Disposition each check as one of:
  - **moves** into `ob-poc-control-plane` (the crate becomes the owner);
  - **stays** in its current crate but is **invoked by** the Control Plane as a validator (§4);
  - **retired** (redundant or superseded).
- Record the inventory as a governance artefact; it defines the "all direct execution paths" set that Phase 4 retires.
- Exit criterion: no control check exists without exactly one owner.

## Phase 1 — Control Proof Skeleton

- Define `ControlPlaneDecision`, `ControlPlaneProof`, `ExecutionEnvelope` and the per-gate proof types (§9.4).
- Define the decision snapshot mechanism (§6.15).
- Integrate intent admission (including interpretation attestation), entity binding and SemOS pack resolution.
- Emit proof without executing.

## Phase 2 — DAG and Write-Set Enforcement

- Add DAG/state proof.
- Add bounded write-set derivation.
- Add transition-slot enforcement.
- Add collect-where-independent aggregation and structured rejection reasons (§6.16).
- Add audit event for rejected execution.

## Phase 3 — STP Classifier and Human Gates

- Add deterministic STP eligibility over the aggregated gate results.
- Add human-gate decision model.
- Add approval envelope.
- Add exception routing, including attestation-failure and injection-suspicion classes.

## Phase 4 — Runtime Integration

- Runtime accepts only `ExecutionEnvelope`.
- Pre-state pin verification, single-use enforcement and validity-window enforcement in the runtime admission path (§6.10).
- Write-set attestation in the runtime commit path (§6.7.1).
- Direct execution paths retired against the Phase 0 inventory.
- Audit/replay envelope enforced.
- Idempotency and lock scope included.

## Phase 5 — Assurance Plane

- Metrics.
- Control dashboard.
- Decision-replay checks (replay against pinned snapshots).
- Drift/version reporting.
- Control-plane regression suite.

---

# 14. Final Position

The Control Plane is the missing first-class capability in the AI-led execution architecture.

The existing platform primitives are necessary but not sufficient on their own:

- DSL gives expression.
- Entity binding gives scope.
- DAG gives legality.
- SemOS gives meaning and authority.
- REPL/compiler gives proof.
- Runtime gives deterministic execution.
- Audit gives recoverability.

The Control Plane gives the platform its execution decision.

It is the capability that says:

> This AI-proposed action is executable.
> This one requires human approval.
> This one is rejected.
> And here is the proof.

That is the difference between AI-assisted operations and governed AI-executed operations.

The Control Plane does not make AI safe by trusting the model. It makes AI-led work executable only when the proposed action can be proven, bounded, authorised, snapshotted, enveloped and audited before runtime state moves.

---

# 15. Target Topology: The Clearing House

**(Amendment 1, ratified.)** The architect directive this section converts into enforceable architecture:

> The Control Plane is the ONLY clearing house between the agents (Sage, REPL, and any future agent surface) and the guts of the solution — DSL, DAG, SemOS, runtime, stores, and every other capability. To be auditable it must be a leakproof, 100%-coverage gateway, and it MUST cover all packs.

Everything in v0.3 — gates, proofs, envelope lifecycle, snapshot semantics, shadow→enforce graduation — is unchanged. What changes is **topology ownership**: v0.3 described a control plane inserted into an existing pipeline (checkpoint); v0.4 makes the control plane the pipeline (mediator).

## 15.1 Mediation, not interception

The Control Plane intercepts at the **utterance** — before parsing, before pack/domain selection — and owns the sequence from that point. The former pipeline stages become capabilities it invokes:

- Sage is not upstream proposing intents to the Control Plane; Sage is an **interpretation capability** the Control Plane invokes with the utterance and the context it grants.
- Pack/domain selection happens **inside** the clearing house (G3 is not consulted about a resolution made elsewhere; it IS the resolution point).
- Compilation, DAG proof, authority, evidence, execution: delegated calls from the hub, returning borrowed proofs (§4/§9.1 unchanged — the decision-assembler law was always hub-shaped; this section makes the call topology match the decision topology).

No capability calls another capability laterally on an agent-originated flow. The point-to-point mesh is not gated; it is **retired**.

## 15.2 Leakproof, defined structurally

"Leakproof" is a compile-time property, not an audit finding:

- **L1 — Dependency direction lock.** Agent crates carry zero dependency edges to capability crates except via `ob-poc-control-plane`. Enforced by a CI dependency-graph gate (the `cargo tree` companion to the pub-surface ratchet). A new lateral edge fails CI the way a new pub item does.
- **L2 — Keyed doors.** Each capability crate exposes exactly one entry surface, and that surface requires a **`CapabilityInvocation` context type constructible only by the Control Plane** — the seal pattern (§9.4) applied to invocation, not just execution. Holding the type proves clearing-house provenance; code that didn't cross the clearing house cannot type-check a capability call.
- **L3 — Lateral surface deletion.** Existing pub items that enabled point-to-point calls are deleted, not deprecated — each deletion a one-commit baseline reduction the ratchet locks (the FIA-4B shrink list is this section's opening backlog).

## 15.3 100% coverage, defined measurably

Coverage is continuously attested, not periodically reviewed:

- **C1 — Compile-time coverage**: L1's graph gate green ⇒ no alternative route exists to compile.
- **C2 — Runtime coverage attestation**: every capability entry point counts invocations by provenance. The metric `capability_invocations_without_cp_provenance` is on the assurance plane (§6.14) with an alert threshold of **zero**. During migration this number is the honest measure of remaining mesh; at completion it is the standing proof of the directive.
- **C3 — Audit closure**: every agent-originated state transition has a Control Plane decision record reachable from its audit trail (§6.11 unchanged; C1+C2 are what make its universality provable rather than asserted). **(v0.4.1 — ruling on MCA-002 E-4.)** This is **constitutive of mediation completion, not an obligation of checkpoint topology**: during T0–T11 (§15.6), best-effort, non-blocking shadow-decision persistence is the conformant posture — a decision record NOT being written on a given turn (a failed insert, a dispatch path that doesn't yet call the shadow evaluator) does not itself make that turn nonconformant, because checkpoint topology never promised universal reachability in the first place. C3 becomes a hard guarantee only when mediation (§15.1) is live and every agent-originated flow provably passes through the clearing house — at that point "reachable from its audit trail" is true by construction (the CP is the only path in), not by a persistence-layer promise that could silently fail. Treat C3, through T11, the same way §15.6 already treats the rest of checkpoint topology: real, valuable, explicitly transitional.

## 15.4 Pack universality (the ALL-packs clause)

Coverage is inherited by construction, never wired per pack:

- **K1** — Pack resolution executes inside the clearing house; there is no pack-scoped entry that precedes or bypasses it.
- **K2** — A pack cannot register verbs, routes, handlers, or tools that dispatch outside the Control Plane: the registration surfaces themselves require the L2 context type, so an out-of-house dispatch is unregistrable, not merely forbidden.
- **K3** — Pack onboarding requires no coverage work and permits no coverage exemption: pack N+1 is covered by the same compile-time proof as pack 1. Any proposed pack-scoped exception to the clearing house is a V&S amendment, not a configuration.

## 15.5 Reads — ratified: R-a, typed read-only lenses

**(Ratified 2026-07-11.)** Two conformant designs for agent read access (e.g. Sage's interpretation-context reads against SemOS) were considered:

- **(R-a, RATIFIED) CP-issued read lenses**: the clearing house grants session-scoped, **typed read-only lenses** — capability views that provably cannot reach a write surface (no write types importable through the lens; enforced by the same visibility discipline). Interpretation stays hot-path-fast; leakproofness holds because a lens cannot move state and cannot be minted outside the clearing house.
- (R-b, not selected) Full mediation: every read brokered through the hub. Purest form; adds a hop inside interpretation loops; reserved as the fallback if lens discipline proves unenforceable.

Under R-a, C1–C3 and K1–K3 apply unchanged to all invocations and writes. MCA-001 (2026-07-11) found two live read-path violations of this section prior to ratification — `ob-poc-sage::session_context`'s direct `sqlx` access (AB4) and the session-checkpoint read-back feeding `SessionVerbSurface` (AB5) — both are the R-a conformance target these findings now have a concrete remedy against; see the ownership ledger's T11 mesh-retirement backlog.

## 15.6 Migration posture

Checkpoint topology (v0.3 as built, T0–T10) is the **transitional state**, not a rival design: everything built relocates into the hub unchanged — gates, envelope lifecycle, admission scope, shadow telemetry. The migration ratchet is: C2's without-provenance count falls as lateral surfaces are deleted (L3), tranche by tranche, and CI locks each fall; the terminal state is C2 ≡ 0 locked by C1. The multi-path graduation story of the runbook collapses, at completion, to a single ingress — a simplification of the enforce-mode endgame, not an extension of it.

**Ratification record:** §15.1 mediation topology ratified as the target state; §15.5 ratified R-a (typed read-only lenses); §15.6 checkpoint work (T0–T10) confirmed as transitional, relocating not discarded. Applied to the repo copy same session as ratification, per Amendment 1's own authorization.

---

# Appendix A — NIST AI Risk Management Framework Mapping

This capability is structured using the NIST AI RMF (AI 100-1) Govern / Map / Measure / Manage functions, expressed as executable platform controls. The mapping below is the **proposed crosswalk** for Risk, Controls and Audit review: each RMF function is intended to be realised by named, testable capabilities with named output artefacts.

**Status caveat.** This appendix states the intended control realisation. Until Phase 5 (assurance metrics, decision-replay checks, regression suite) is complete, the mapping is a **design crosswalk, not an operating attestation**. The NIST AI RMF is a voluntary framework; this document does not assert formal conformance, and no conformance claim should be made externally until the crosswalk has been accepted by Risk and the Phase 5 assurance plane is operational.

## A.1 Function mapping

| NIST AI RMF function | Control Plane capability | Output artefact |
|---|---|---|
| **GOVERN** — policies, accountability, roles, risk culture | Intent admission (§6.1); SemOS pack resolution (§6.3); authority & policy gate (§6.5); version pinning (§6.12); envelope provenance (§6.10.3) | `IntentAdmissionDecision`, `PackResolution`, `AuthorityDecision`, pinned version set |
| **MAP** — context, scope, impact of the AI action | Entity binding (§6.2); DAG/state-slot enforcement (§6.4); bounded write-set derivation (§6.7); decision snapshot (§6.15) | `EntityBindingReport`, `StateTransitionProof`, `WriteSetProof`, `SnapshotPins` |
| **MEASURE** — quantitative assurance, tracking, drift | Assurance metrics (§6.14); decision-replay checks (§13 Phase 5); write-set attestation results (§6.7.1); model/version drift indicators | Metrics plane, replay reports, attestation records |
| **MANAGE** — risk response, prioritisation, incident handling | STP classification (§6.8); human gates (§6.8); exception routing (§6.13); envelope single-use/expiry/pre-state voiding (§6.10) | `StpEligibilityDecision`, exception work items, voided-envelope events |

## A.2 Trustworthy-AI characteristics

The RMF's trustworthy-AI characteristics map onto the architecture as follows:

| Characteristic | Realised by |
|---|---|
| **Valid & reliable** | Deterministic classification over a pinned snapshot (§6.8, §6.15); compiler proof (§8.5); decision replay (§12) |
| **Safe** | Binary tollgate (§7); bounded write-set (§6.7); pre-state pinning (§6.10.1) |
| **Secure & resilient** | Envelope provenance, single-use and expiry (§6.10); prompt-injection routing (§6.13.1); no direct AI instruction path to the runtime (§8.6) |
| **Accountable & transparent** | Runbook proof (§6.9); audit reconstruction (§6.11); human sponsor and approval records (§6.5) |
| **Explainable & interpretable** | `ControlPlaneProof` as pre-execution reviewable artefact (§6.9); aggregated rejection reasons (§6.16) |
| **Privacy-enhanced** | Write-set restriction to declared fields (§6.7); pack-scoped entity visibility (§6.2, §6.3) |
| **Fair, with harmful bias managed** | Deterministic policy gates replace model self-certification (§6.8); attestation thresholds as configured policy, not model discretion (§6.13.1) |

## A.3 Generative-AI profile

Where AI-originated intent is involved, the controls additionally address the NIST Generative AI Profile (AI 600-1) concerns most relevant to this platform: **confabulation** (hallucinated-verb rejection, §6.1), **information integrity** (interpretation attestation and version pinning, §6.13.1, §6.12), and **human-AI configuration** (explicit human sponsor, human gates and second-line review, §6.5, §6.8).

## A.4 Design intent

The design intent is that every RMF function above is exercised on **every** proposed AI-led execution, produces a persisted artefact, and is measurable through the assurance metrics plane. On completion of Phase 5, framework alignment becomes a property of the execution path rather than a periodic attestation exercise. That is the target state this crosswalk exists to reach — subject to the status caveat above.

---

*End of document — EOP-VS-CONTROLPLANE-001 v0.4.1*
