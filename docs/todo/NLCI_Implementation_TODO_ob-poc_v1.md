# NLCI Implementation TODO for ob-poc

> **Date:** 2026-03-12
> **Status:** Review draft
> **Governing Spec:** `/Users/adamtc007/Downloads/nlci-architecture-v1.2.md`
> **Instruction:** Implement from Sections 5, 6, 9, and 10 only. Do not redesign the architecture unless a gate review exposes a structural defect.

---

## 1. Objective

Implement the Natural Language Compiler Interface in `ob-poc` as a spec-faithful pipeline that:

1. extracts structured intent from natural language
2. compiles that intent deterministically into DSL-ready operations
3. enforces the architecture invariants in code
4. proves behavior with a harness before broad domain rollout

This TODO is repo-specific and maps the paper onto the current codebase.

---

## 2. Scope Boundary

This implementation pass is limited to the architecture defined in:

- Section 5: Structured Intent Extraction
- Section 6: Deterministic Intent Compiler
- Section 9: Failure Taxonomy
- Section 10: Implementation Proposal

Out of scope for this pass:

- opportunistic redesign of Sage/Coder/orchestrator semantics
- broad domain rollout before the harness is in place
- replacing existing runtime governance or execution layers unless required by the spec

---

## 3. Current Repo Landing Zones

The most relevant existing code locations are:

- `rust/src/sage/`
  - current intent-understanding types and structured handoff
- `rust/src/agent/orchestrator.rs`
  - current utterance pipeline and phase control
- `rust/src/semtaxonomy_v2/`
  - candidate home for compiler-facing semantic selection/binding logic
- `rust/src/agent/harness/`
  - deterministic end-to-end harness foundation
- `rust/src/api/agent_service.rs`
  - externally visible chat/pipeline boundary
- `rust/src/dsl_v2/prompts/general_intent_extraction.md`
  - existing prompt surface for structured extraction
- `rust/config/lexicon/`, `rust/config/noun_index.yaml`, `rust/config/verb_schemas/`
  - metadata surfaces that act like SemOS inputs

The goal is to extend these surfaces coherently rather than create a parallel architecture.

---

## 4. Delivery Rules

The implementation must follow these rules:

1. define schemas, Rust types, and interfaces first
2. enforce invariants in code before broad feature rollout
3. build the test harness before multi-domain expansion
4. preserve the paper’s staged delivery discipline
5. treat the paper as authoritative unless a gate review proves a structural defect

---

## 5. Work Package Matrix

### WP1. Structured Intent Schema

Derived from:

- Section 5.2
- Section 5.2A
- Section 5.2B
- Section 5.3

Deliverables:

- canonical Rust types for the structured intent plan
- canonical Rust types for Semantic IR
- binding mode enum/types
- serde-compatible wire schema
- validation rules for required/optional fields

Repo landing zone:

- new module under `rust/src/semtaxonomy_v2/` or `rust/src/sage/`
- shared public types if needed in `rust/crates/ob-poc-types/`

Required types:

- `StructuredIntentPlan`
- `IntentStep`
- `IntentTarget`
- `IntentQualifier`
- `IntentParameter`
- `SemanticIr`
- `BindingMode`
- `CompilerInputEnvelope`

Acceptance gate:

- all compiler-facing intent data has one canonical typed representation
- no ad hoc JSON maps at phase boundaries

### WP2. Compiler Phase Interfaces

Derived from:

- Section 6.2
- Section 6.3

Deliverables:

- explicit interfaces for each compiler phase
- typed inputs/outputs for:
  - surface object resolution
  - operation resolution
  - binding resolution
  - candidate selection
  - discrimination
  - composition/parameter binding

Repo landing zone:

- `rust/src/semtaxonomy_v2/`
- thin orchestration integration in `rust/src/agent/orchestrator.rs`

Required interfaces:

- `SurfaceObjectResolver`
- `OperationResolver`
- `BindingResolver`
- `CandidateSelector`
- `Discriminator`
- `CompositionBinder`
- `IntentCompiler`

Acceptance gate:

- phase output types are explicit and not reused ambiguously
- orchestrator can call the compiler through a single typed entrypoint

### WP3. Failure Taxonomy Types

Derived from:

- Section 9

Deliverables:

- typed normalized failure taxonomy
- mapping from internal failures to user-safe compiler failures
- deterministic failure codes for tests and telemetry

Repo landing zone:

- `rust/src/semtaxonomy_v2/`
- integration adapters in `rust/src/agent/orchestrator.rs`
- outward projection in `rust/src/api/agent_service.rs`

Required types:

- `CompilerFailureKind`
- `CompilerFailure`
- `AmbiguityReason`
- `BindingFailure`
- `ResolutionFailure`
- `DiscriminationFailure`

Acceptance gate:

- all compiler-stage failures normalize into the Section 9 taxonomy
- no free-form stringly-typed failure handling across phase boundaries

### WP4. Architectural Invariant Enforcement

Derived from:

- Sections 5 and 6
- delivery gates in Section 10

Deliverables:

- code-level guards for architectural invariants
- unit/static tests enforcing boundaries

Invariant set to enforce:

1. LLM output is structured intent, not DSL text
2. compiler phases are deterministic after structured intent extraction
3. phase boundaries use typed contracts only
4. binding mode is explicit and validated
5. failure taxonomy is normalized before API/output projection
6. governed execution remains downstream of compilation, not mixed into intent extraction
7. session grounding is input to resolution, not a side-effectful hidden shortcut

Repo landing zone:

- `rust/src/agent/orchestrator.rs`
- `rust/src/sage/`
- `rust/src/semtaxonomy_v2/`
- test guards under existing module tests and `rust/tests/`

Acceptance gate:

- invariant violations fail tests
- orchestrator cannot bypass the typed compiler path once enabled

### WP5. Structured Extraction Prompt and Adapter

Derived from:

- Section 5.4
- Section 10.1

Deliverables:

- prompt/spec refresh for structured intent extraction
- adapter that converts provider output into `StructuredIntentPlan`
- validation and rejection path for malformed extraction output

Repo landing zone:

- `rust/src/dsl_v2/prompts/general_intent_extraction.md`
- `rust/crates/ob-agentic/`
- `rust/src/api/agent_service.rs` or a dedicated extraction adapter module

Acceptance gate:

- prompt output shape matches the canonical schema exactly
- malformed extraction output is rejected before compiler phases run

### WP6. Deterministic Intent Compiler Prototype

Derived from:

- Section 10.3

Deliverables:

- first compiler prototype implementing the Section 6 phase model
- single typed compile function from structured intent to selected operation/composition output

Repo landing zone:

- `rust/src/semtaxonomy_v2/`
- orchestrator bridge in `rust/src/agent/orchestrator.rs`

Acceptance gate:

- prototype compiles at least two archetype flows deterministically
- candidate ranking/discrimination is explainable from typed inputs

### WP7. End-to-End Harness

Derived from:

- Section 10.4

Deliverables:

- scenario harness that drives:
  - utterance
  - structured intent extraction
  - compiler phases
  - selected DSL/runbook output
- golden fixtures for success, ambiguity, and failure cases

Repo landing zone:

- extend `rust/src/agent/harness/`
- add fixture corpus under `rust/tests/fixtures/`

Harness coverage must include:

- straightforward single-step resolution
- ambiguous action/object resolution
- binding ambiguity
- session grounding cases
- multi-step composition cases
- failure taxonomy assertions

Acceptance gate:

- harness exists before broad domain rollout begins
- every new domain/archetype case enters via harness first

### WP8. Composition and Session Grounding

Derived from:

- Section 5.6
- Section 10.5

Deliverables:

- typed session-grounding input model
- compiler support for multi-step composition
- explicit handling of reference-based binding vs identifier-based binding

Repo landing zone:

- `rust/src/semtaxonomy_v2/`
- `rust/src/session/`
- `rust/src/agent/orchestrator.rs`

Acceptance gate:

- session grounding is explicit compiler input
- multi-step resolution does not rely on hidden mutable coupling

### WP9. Archetype Rollout

Derived from:

- Section 10.2
- Section 10.6

Deliverables:

- implement two initial archetypes only after WP1-WP8 gates pass
- publish the metadata needed for those archetypes across the repo’s SemOS-like surfaces

Suggested first archetypes for `ob-poc`:

1. CBU mutation/read lifecycle path
2. KYC/document requirement path

Acceptance gate:

- no broad domain expansion until the two archetypes pass the harness and gate review

---

## 6. Suggested Implementation Sequence

The implementation order should be:

1. WP1 `Structured Intent Schema`
2. WP2 `Compiler Phase Interfaces`
3. WP3 `Failure Taxonomy Types`
4. WP4 `Architectural Invariant Enforcement`
5. WP5 `Structured Extraction Prompt and Adapter`
6. WP7 `End-to-End Harness`
7. WP6 `Deterministic Intent Compiler Prototype`
8. WP8 `Composition and Session Grounding`
9. WP9 `Archetype Rollout`

Rationale:

- types and invariants come first
- the harness must exist before broad resolver expansion
- rollout comes last

---

## 7. Gate Reviews

### Gate A: Schema and Interface Freeze

Review after:

- WP1
- WP2
- WP3

Questions:

- do the typed contracts fully cover Sections 5, 6, and 9
- are any phase boundaries still stringly-typed or ambiguous

### Gate B: Invariant Enforcement

Review after:

- WP4
- WP5

Questions:

- can malformed or off-spec extraction output bypass validation
- can the orchestrator still bypass the compiler contracts

### Gate C: Harness Before Rollout

Review after:

- WP7

Questions:

- does the harness prove success, ambiguity, and failure paths
- are session-grounding and composition cases covered

### Gate D: Archetype Rollout

Review after:

- WP6
- WP8
- WP9

Questions:

- are the first two archetypes implemented without violating the architecture
- is broad rollout still justified

---

## 8. Concrete File-Level TODO

### Types and schemas

- add `rust/src/semtaxonomy_v2/intent_schema.rs`
- add `rust/src/semtaxonomy_v2/semantic_ir.rs`
- add `rust/src/semtaxonomy_v2/binding.rs`
- add `rust/src/semtaxonomy_v2/failure.rs`
- export those modules from `rust/src/semtaxonomy_v2/mod.rs`
- add shared outward-facing types to `rust/crates/ob-poc-types/` only if API serialization requires them

### Compiler interfaces

- add `rust/src/semtaxonomy_v2/compiler.rs`
- add `rust/src/semtaxonomy_v2/phases/`
- define typed traits or structs for each phase
- wire one entrypoint from orchestrator into the compiler

### Prompt/extraction adapter

- update `rust/src/dsl_v2/prompts/general_intent_extraction.md`
- add extraction response validator/parser
- reject non-conforming provider output

### Invariant enforcement

- add tests in `rust/src/agent/orchestrator.rs`
- add tests in `rust/src/semtaxonomy_v2/`
- add static guard tests where architectural bypass is possible

### Harness

- extend `rust/src/agent/harness/runner.rs`
- extend `rust/src/agent/harness/assertions.rs`
- add NLCI scenario fixtures under `rust/tests/fixtures/`
- add integration tests for success/ambiguity/failure/session-grounding/composition

---

## 9. Risks to Watch

1. existing Sage/Coder types may overlap awkwardly with the new canonical schema
2. current orchestrator shortcuts may violate the intended compiler phase model
3. provider-oriented prompt logic may leak non-determinism past Layer 1
4. SemOS-like metadata in config may be incomplete for clean discrimination
5. broad rollout pressure may arrive before the harness is ready

The correct response to these risks is:

- surface them at gate review
- do not silently redesign the architecture in-flight

---

## 10. Review Questions

Before implementation, confirm:

1. should `semtaxonomy_v2` be the canonical home for NLCI compiler types and phases
2. should the first two rollout archetypes be `CBU` and `document/KYC`
3. should the harness operate through the existing orchestrator end-to-end, or through a compiler-first lower-level test layer plus orchestrator wrappers
4. should structured intent wire types be exposed in `ob-poc-types`, or kept internal until the external API requires them

---

## 11. Bottom Line

The implementation plan for `ob-poc` should be:

- freeze the architecture from the paper
- define the typed contracts first
- enforce invariants in code
- build the harness before broad rollout
- then implement only the first two archetypes through the governed compiler path

That is the safest path to improve utterance-to-DSL quality without letting the repo drift into another partially implicit pipeline.
