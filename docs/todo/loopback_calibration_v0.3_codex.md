# Loopback Calibration for Semantic OS / DSL v0.3
**Subtitle:** Governed Synthetic Boundary Calibration — Vision, Scope, and Implementation Plan

**Status:** Codex-Ready — Self-Contained Architecture + Implementation Document  
**Repo:** `ob-poc-main`  
**Companion:** Semantic Traceability Kernel v0.3.2  
**Author:** Adam  
**Date:** March 2026

**Document structure:** §1–§14 and §16–§17 define the vision, scope, and architectural model. §15 is the phased implementation plan with complete code for Codex execution. This document is self-contained — Codex does not need to reference the Traceability Kernel paper or any other document to execute the implementation phases.

---

## 0. Prerequisites (assumed complete before implementation begins)

This implementation plan assumes the following are fully implemented and operational. **Do not start §15 until all prerequisites are verified.**

| Prerequisite | Source | What "complete" means |
|-------------|--------|----------------------|
| Constellation remediation pack (all 5 phases) | `TODO_constellation_remediation_pack.md` | 23 broken macro-verb references resolved, 845 missing SemOS footprints populated, 69 invocation phrase collisions eliminated, `verb_concepts.yaml` at 30%+ coverage, 34 orphan footprints removed |
| `utterance_traces` table | Kernel §13 | Schema deployed with hoisted canonical columns per Kernel §13 storage schema |
| `is_synthetic` column on `utterance_traces` | This document §7.4 | `ALTER TABLE "ob-poc".utterance_traces ADD COLUMN is_synthetic BOOLEAN NOT NULL DEFAULT false;` |
| Phases 0–4 instrumented | Kernel §3 | Each phase emits trace segments; `UtteranceTrace` records persist to DB on every utterance |
| Constellation maps formalised | SemOS runtime | `ConstellationMapDef` / `ValidatedConstellationMap` implemented in `sem_os_runtime::constellation_runtime` with slot validation, verb validation, bulk macro validation, topo sort, state machine loading, and 18 built-in maps under `rust/config/sem_os_seeds/constellation_maps/` |
| Constellation map revisioning | SemOS runtime | `compute_map_revision()` provides stable revision IDs for built-in YAML map definitions |
| Constellation recovery from UUID | Live Phase 2 / SemOS grounding | Given a grounded entity/session context, the live SemOS/traceability path produces linked entity context through `Phase2Evaluation` and `trace_payload.phase_2` rather than a standalone `ConstellationSnapshot` type |
| Situation signature computation | Traceability payloads | `compute_phase2_situation_signature_hash()` is live and the canonical form is persisted under `trace_payload.phase_2.situation_signature.canonical_form` |
| Verb metadata classification | Live verb metadata | Harm/action/phase-tag metadata is live via verb config / SemOS adapter contracts; there is no standalone `EntityVerb` / `StateVerb` taxonomy function exposed under that paper name |
| ECIR narrowing | Kernel §3 Phase 3 | Phase 3 operational with plane filter, taxonomy filter, pattern filter, action-category filter |
| Fallback escape hatch | Kernel §3 Phase 4 | `FallbackEscapeHatch` traced; `fallback_invoked` hoisted column populated |
| BGE embeddings via Candle | `ob-semantic-matcher` / `CandleEmbedder` | BGE model loaded, `embed_query()` / `embed_target()` operational for Phase 4 |
| Replay engine (basic) | Kernel §12 | Can re-execute Phases 0–4 against a trace, produce `ReplayResult` |
| Loop 3 aggregation excludes synthetics | This document §7.4 | Any production-learning aggregation over utterance traces must enforce `WHERE is_synthetic = false`; no specific `situation_verb_distributions` object is assumed to already exist |

**If any prerequisite is incomplete**, the calibration harness will produce misleading results — it will calibrate the quality of incomplete metadata rather than stress-test pipeline discrimination. Do not start.

---

## 1. Vision

The OB-POC platform converts operational user intent into deterministic, governed, state-legal DSL execution across onboarding, KYC, UBO, document, product/service, and related operational domains.

The core problem is not "did the model understand the utterance?" The core problem is:

- did the platform recover the correct constellation (Phase 2),
- did it apply the correct state and predicate legality ceiling (Phase 2 + DAG 1),
- did it narrow correctly to the intended DSL verb (Phase 3 ECIR),
- did it resolve without fallback (Phase 4),
- and did it produce a traceable, replayable, deterministic execution path into the REPL (Phase 5).

Loopback Calibration is a governed Semantic OS capability to harden that path.

### 1.1 What Loopback Calibration Is

Loopback Calibration uses the platform's own Semantic OS and DSL metadata as the canonical semantic seed, generates synthetic utterances intended to express that seed, runs them through the production six-phase utterance-to-REPL pipeline (Traceability Kernel v0.3.2 §2–§3), and analyses the resulting `UtteranceTrace` records to locate failure, ambiguity, drift, and missing coverage.

Instead of waiting for real users to discover weakness, the platform uses its own semantic model to stress-test itself.

### 1.2 Core Architectural Idea

The platform already has the ingredients:

| Ingredient | Kernel Reference | Role in Calibration |
|-----------|-----------------|-------------------|
| Constellation templates (DAG 1) | Kernel §6b | Canonical seed source — entity types, prerequisite edges, state propagation |
| Entity FSMs with state-verb classification | Kernel §3 Phase 2d | Target verb selection + entity-verb/state-verb distinction |
| Situation signatures | Kernel §3 Phase 2e | Scenario grouping — calibrate per operational phase |
| Legal verb set computation | Kernel §3 Phase 2c | Expected ceiling — the correct answer space |
| ECIR narrowing with prune/rank/demote | Kernel §3 Phase 3 | Narrowing quality measurement |
| Fallback escape hatch with error budget | Kernel §3 Phase 4 | Anti-pattern detection |
| `UtteranceTrace` with per-phase trace structs | Kernel §4 | Diagnostic output — the thing we read |
| Version-pinned replay | Kernel §12 | Regression detection across calibration runs |
| Three feedback loops | Kernel §10 | Where calibration results feed back |
| Three DAGs (structural, macro, runtime) | Kernel §6 | Cross-entity scenario coverage |
| Three execution shapes | Kernel §9 | Singleton, Batch, CrossEntityPlan scenario coverage |

Loopback Calibration combines these into a controlled loop:

1. Select a governed constellation seed (from DAG 1 + entity state + situation signature)
2. Generate synthetic utterance families (positive, negative, boundary)
3. Run them through the actual six-phase pipeline
4. Read the `UtteranceTrace` records
5. Classify outcomes using `HaltReason` variants and phase-specific trace data
6. Feed results into governed improvement workflows

**The vision is not "AI testing AI." The vision is deterministic boundary calibration for Semantic OS / DSL execution.**

---

## 2. Problem Statement

Today, the quality of an utterance-driven platform is judged informally: "it seems to resolve most requests," "the embeddings look good," "the demo worked." That is insufficient for OB-POC.

In this platform, a user utterance may ultimately drive onboarding progression, KYC/UBO investigation actions, document solicitation, evidence requests, product/service provisioning, entity state transitions, and remediation workflows. The platform needs stronger guarantees than generic chatbot quality.

### 2.1 Current Risk

Without a governed loopback mechanism:

- Semantic gaps remain undiscovered until encountered by real users
- Regressions in resolution quality are detected late
- Boundary drift between neighbouring DSL verbs is invisible
- Legality failures may be misdiagnosed as "model weakness" when they are constellation predicate failures
- Missing verbs or weak `verb_concepts.yaml` coverage (currently 3.6%) remains hidden
- Changes to embeddings, thresholds, constellation templates, or DAG 1 edges may silently degrade reliability
- Phase 3's prune/rank/demote behaviour may drift without the narrowing drift detection from Kernel §12

### 2.2 Why This Matters in OB-POC

The final target is not a natural-language answer. The final target is a legal, traceable, deterministic DSL action in a governed operational domain. That makes synthetic calibration unusually valuable because the destination is explicit and testable — every calibration scenario has a known-correct `resolved_verb` and a known-correct `HaltReason` for utterances that should not resolve.

---

## 3. Strategic Intent

Loopback Calibration has five strategic purposes.

### 3.1 Stress-Test Semantic Boundaries

Probe the edge of a constellation or verb family by generating utterances with controlled variance: polite vs abrupt, operational vs colloquial, precise vs vague, complete vs referential, direct vs euphemistic. Measure whether linguistic variation causes unwanted drift into neighbouring verbs or illegal interpretations.

### 3.2 Detect Missing DSL or Semantic OS Coverage

If the generator produces plausible utterances that feel operationally valid within a constellation, but the pipeline halts with `HaltReason::NoViableVerb`, the result may indicate concept coverage weakness, weak metadata, missing synonym support, or a genuinely missing DSL capability. These surface as `GAP`-coded entries in the constellation remediation pipeline (Kernel §10 Loop 1).

### 3.3 Create a Deterministic Regression Harness

Any change to code, concept registries, embedding models, thresholds, constellation templates, DAG 1 edges, or DSL verb surfaces should be measurable against a governed benchmark corpus tied to known constellation seeds. The replay engine (Kernel §12) runs historical calibration traces against the new surface and diffs resolution outcomes, narrowing behaviour, and fallback frequency.

### 3.4 Strengthen Traceability as an Operational Asset

Calibration only has value if every run produces first-class `UtteranceTrace` output (Kernel §4). The harness tells the platform whether failure occurred in referential binding (Phase 1), constellation recovery (Phase 2), legality gating (Phase 2 predicates), Phase 3 narrowing (ECIR over-pruning), final resolution (Phase 4), fallback usage (Phase 4 escape hatch), or execution protection (Phase 5).

### 3.5 Probe Near-Neighbour Verb Boundaries (new in v0.2)

The hardest calibration problem is not linguistic variation — it is near-neighbour verb confusion. If the target is `cbu.suspend` and the nearest neighbour is `cbu.terminate`, the generator should produce utterances that sit exactly on the boundary: "put the account on hold," "freeze it temporarily," "stop all activity." The calibration value is in measuring the **margin** between the top-1 and top-2 candidates in Phase 4. A thin margin means the verb boundary is fragile — regardless of whether top-1 is currently correct.

---

## 4. Conceptual Model

Loopback Calibration is a governed Semantic OS workflow, not a test script.

### 4.1 Canonical Seed (wired to Kernel)

Each calibration scenario begins with a governed seed derived from Semantic OS metadata:

```rust
struct CalibrationScenario {
    // ─── Identity ───
    scenario_id: Uuid,
    scenario_name: String,
    created_by: String,
    governance_status: GovernanceStatus,  // Draft | Reviewed | Admitted | Deprecated

    // ─── Constellation Context (from DAG 1) ───
    constellation_template_id: String,     // e.g., "lu-ucits-sicav"
    constellation_template_version: String,
    active_dag1_edges: Vec<DependencyEdge>,  // which prerequisite edges are in play
    situation_signature: SituationSignature,  // from Kernel §3 Phase 2e
    operational_phase: OperationalPhase,

    // ─── Entity Context ───
    target_entity_type: String,            // e.g., "cbu"
    target_entity_state: StateNode,        // e.g., ACTIVE
    linked_entity_states: Vec<(String, StateNode)>,  // e.g., [("kyc-case", OPEN), ("ubo", VERIFIED)]

    // ─── Verb Context ───
    target_verb: VerbId,                   // the verb that SHOULD resolve
    legal_verb_set_snapshot: Vec<VerbId>,   // Phase 2 ceiling at scenario definition time
    verb_taxonomy_tag: VerbCategory,        // EntityVerb | StateVerb
    cross_entity_couplings: Vec<CrossEntityCoupling>,  // from Kernel §3 Phase 2d

    // ─── Boundary Context ───
    excluded_neighbours: Vec<ExcludedNeighbour>,
    near_neighbour_verbs: Vec<NearNeighbourVerb>,  // verbs the generator should target the boundary against
    expected_margin_threshold: f32,         // minimum acceptable top-1 vs top-2 margin

    // ─── Execution Shape (new in v0.2) ───
    execution_shape: CalibrationExecutionShape,

    // ─── Generation Profile ───
    generation_profile: GenerationProfile,
    
    // ─── Gold Standard ───
    gold_utterances: Vec<GoldUtterance>,   // human-written reference utterances
    admitted_synthetic_set_id: Option<Uuid>,  // curated synthetic corpus
}

enum CalibrationExecutionShape {
    /// Single entity, single verb — most common
    Singleton,
    /// Same verb across multiple entities via filter
    Batch {
        filter_expression: String,
        expected_entity_count: usize,
    },
    /// Multiple entities, different verbs, ordering + exclusions
    CrossEntityPlan {
        plan_nodes: Vec<CalibrationPlanNode>,
        expected_dag3_edges: Vec<(usize, usize)>,
        exclusion_predicates: Vec<ExclusionPredicate>,
    },
}

struct CalibrationPlanNode {
    entity_type: String,
    entity_state: StateNode,
    target_verb: VerbId,
}

struct NearNeighbourVerb {
    verb_id: VerbId,
    expected_embedding_distance: f32,  // how close is this to the target
    confusion_risk: ConfusionRisk,      // High | Medium | Low
    distinguishing_signals: Vec<String>, // what SHOULD differentiate them
}

enum ConfusionRisk {
    /// These verbs are routinely confused — high calibration value
    High,
    /// Occasionally confused — moderate calibration value
    Medium,
    /// Rarely confused but worth monitoring
    Low,
}

struct ExcludedNeighbour {
    verb_id: VerbId,
    reason: String,  // why this verb should NOT match
}

struct GoldUtterance {
    text: String,
    expected_outcome: ExpectedOutcome,
    authored_by: String,
    admitted_at: Option<DateTime<Utc>>,
}

enum GovernanceStatus {
    Draft,
    Reviewed,
    Admitted,
    Deprecated,
    Superseded { by: Uuid },
}
```

The seed is the truth anchor. It is derived from governed Semantic OS metadata, not inferred by the generator.

### 4.2 Synthetic Utterance Family

A high-reasoning model generates synthetic candidate utterances intended to express the target action under the defined seed. Utterances are classified into three calibration modes:

#### 4.2a Positive Calibration (should resolve to target verb)

Utterances that vary linguistically but should all resolve to the same target verb:

| Variation Dimension | Example for `cbu.suspend` |
|-------------------|--------------------------|
| Formal/operational | "Please suspend the Acme Corp custody account" |
| Colloquial | "Freeze the Acme account for now" |
| Indirect | "We need to put a hold on this CBU" |
| Urgent | "Stop all activity on 7742 immediately" |
| Referential | "Suspend it" (with session context) |
| Vague | "Can we pause things on the Acme account?" |
| Euphemistic | "Let's take this one offline temporarily" |

#### 4.2b Negative Calibration — new in v0.2

Equally important: utterances that a user might plausibly say in the target constellation but that should NOT resolve to the target verb. Negative calibration covers two distinct failure surfaces that must be kept separate because they test different pipeline behaviours:

**Negative Type A — Resolves Elsewhere (should resolve to a different valid verb):**

These test that the pipeline correctly distinguishes the target verb from its neighbours. The utterance is valid and should resolve — just not to the target.

| Example | Expected Resolution | What It Tests |
|---------|-------------------|--------------|
| "Terminate the Acme account" | `cbu.terminate` (not `cbu.suspend`) | Phase 4 embedding discrimination between near-neighbours |
| "Suspend the KYC case" | `kyc.suspend` (not `cbu.suspend`) | Phase 2 entity-type resolution — correct entity, correct domain |
| "What's the status of the account?" | `cbu.get-status` (observation verb) | Phase 0 plane classification — Observation, not Mutation |

**Negative Type B — Should NOT resolve to any verb (halt, clarify, or fall to Sage):**

These test that the pipeline correctly refuses to resolve utterances that are outside the DSL surface, ambiguous beyond discrimination, or illegal in the current constellation state.

| Example | Expected Outcome | What It Tests |
|---------|-----------------|--------------|
| "Can you email the client about this?" | `HaltReason::NoViableVerb` or fall to Sage | Outside DSL surface entirely |
| "Deal with the Acme situation" | `HaltReason::AmbiguousResolution` or `BelowConfidenceThreshold` | Too vague to resolve — should trigger clarification |
| "Suspend the terminated account" | `HaltReason::StateConflict` | Entity FSM correctly blocking illegal verb |
| "Freeze it" (no session context) | `HaltReason::MissingReferentialContext` | No antecedent for pronoun |

The distinction matters because Type A failures indicate discrimination weakness (the pipeline resolved, but to the wrong verb), while Type B failures indicate boundary recognition weakness (the pipeline resolved when it should have refused, or refused when it should have resolved). These are different defect categories with different remediation paths.

#### 4.2c Boundary Calibration (near-neighbour margin testing) — new in v0.2

Utterances designed to sit exactly on the boundary between the target verb and its nearest neighbours. These test whether the Phase 3 narrowing and Phase 4 resolution can reliably distinguish verbs that are semantically close.

For target `cbu.suspend` with nearest neighbour `cbu.terminate`:

| Utterance | Desired Resolution | Why It's a Boundary Case |
|-----------|--------------------|--------------------------|
| "Put the account on hold" | `cbu.suspend` | "hold" could mean temporary (suspend) or permanent (terminate) |
| "Stop all activity" | `cbu.suspend` | "stop" is ambiguous between suspend and terminate |
| "Freeze temporarily" | `cbu.suspend` | "temporarily" is the discriminating signal — but what if it's absent? |
| "Freeze" | **Ambiguous** | Without "temporarily," this genuinely could be either verb |
| "Shut down the account" | `cbu.terminate` | "shut down" implies permanence — should NOT resolve to suspend |

The calibration value is the **margin**: the difference between the top-1 and top-2 candidate scores in Phase 4. If `cbu.suspend` scores 0.87 and `cbu.terminate` scores 0.85, the margin is 0.02 — fragile. If the margin is 0.15+, the boundary is stable.

### 4.3 Embedding Pre-Screening (new in v0.2)

Before running the full six-phase pipeline, the harness optionally pre-screens generated utterances through the same Candle/BGE embedding path used in Phase 4. This stratifies the synthetic corpus:

**Advisory only (non-negotiable):** Pre-screening is a stratification tool for prioritising calibration effort — it helps identify which utterances are boundary cases worth the most diagnostic attention. Pre-screening must never suppress full-pipeline execution for admitted benchmark utterances. Every admitted utterance runs through all six phases regardless of its pre-screen stratum. Otherwise someone could later optimise the harness by short-circuiting the expensive runs and quietly lose the traceability that gives calibration its value.

```rust
struct EmbeddingPreScreen {
    utterance: String,
    target_verb_distance: f32,       // BGE embedding distance to target
    nearest_neighbour_distance: f32,  // distance to closest non-target verb
    margin: f32,                      // target - nearest_neighbour
    stratum: PreScreenStratum,
}

enum PreScreenStratum {
    /// High similarity to target — should definitely resolve
    /// Useful for regression baselines
    ClearMatch { distance: f32 },

    /// Medium similarity — boundary case, highest calibration value
    /// These test the discrimination power of Phase 3 + Phase 4
    BoundaryCase { margin: f32 },

    /// Low similarity — should NOT resolve to target
    /// Tests false-positive resistance
    ClearNonMatch { distance: f32 },

    /// Closer to a non-target verb than to the target
    /// Tests whether Phase 3/4 correctly prefer the non-target
    NeighbourPreferred { preferred_verb: VerbId, preferred_distance: f32 },
}
```

This pre-screening uses the same `CandleEmbedder` + BGE model that the production pipeline uses (Kernel §3 Phase 4), ensuring that the stratification reflects the actual embedding geometry the pipeline will encounter.

### 4.4 Pipeline Execution

Synthetic utterances run through the actual six-phase pipeline, not a simplified shim:

| Pipeline Phase | Kernel Reference | What Calibration Tests |
|---------------|-----------------|----------------------|
| Phase 0: Plane Classification | Kernel §3 Phase 0 | Does linguistic variation change plane/polarity? |
| Phase 1: Linguistic Decomposition | Kernel §3 Phase 1 | Do referential/vague utterances parse correctly? |
| Phase 2: Constellation Recovery | Kernel §3 Phase 2 | Does the correct constellation recover? Is the legal verb set correct? |
| Phase 2: DAG 1 Predicate Check | Kernel §6b | Do constellation predicates correctly block/allow? |
| Phase 2: Verb Taxonomy | Kernel §3 Phase 2d | Are entity-verbs/state-verbs classified correctly? |
| Phase 2: Situation Signature | Kernel §3 Phase 2e | Does the signature match the scenario's expected phase? |
| Phase 3: ECIR Narrowing | Kernel §3 Phase 3 | Does narrowing eliminate the right verbs? Over-prune? Under-prune? |
| Phase 3: Pattern Matching | Kernel §5 | Does the constellation pattern filter help or hurt? |
| Phase 4: DSL Resolution | Kernel §3 Phase 4 | Correct verb? Correct strategy? Margin to runner-up? |
| Phase 4: Fallback | Kernel §3 Phase 4 | Was fallback invoked? (Anti-pattern budget) |
| Phase 5: Execution | Kernel §3 Phase 5 | Did the DSL command execute cleanly? |

For `CrossEntityPlan` scenarios, the pipeline also tests:
- DAG 3 compilation from user ordering + DAG 1 prerequisites (Kernel §6d)
- Exclusion predicate materialisation (Kernel §6f)
- Mid-plan constellation re-check (Kernel §6e)
- `DagOrderingConflict` and `ExclusionMakesPlanInfeasible` detection

### 4.5 Trace-Based Diagnosis

Each run reads the `UtteranceTrace` record (Kernel §4) and classifies the outcome using the exact same structs and enums:

```rust
struct CalibrationOutcome {
    utterance: String,
    calibration_mode: CalibrationMode,   // Positive | Negative | Boundary
    pre_screen: Option<EmbeddingPreScreen>,
    
    // ─── Expected ───
    expected_outcome: ExpectedOutcome,
    
    // ─── Actual (from UtteranceTrace) ───
    trace_id: Uuid,
    actual_outcome: TraceOutcome,         // from Kernel §4c
    actual_resolved_verb: Option<VerbId>,
    actual_halt_reason: Option<HaltReason>, // from Kernel §4c — exact enum variant
    
    // ─── Diagnosis ───
    verdict: CalibrationVerdict,
    failure_phase: Option<u8>,            // 0–5, which phase went wrong
    failure_detail: Option<FailureDetail>,
    
    // ─── Margin (for boundary calibration) ───
    top1_score: Option<f32>,
    top2_score: Option<f32>,
    margin: Option<f32>,
    margin_stable: Option<bool>,          // margin >= expected_margin_threshold
}

enum CalibrationMode {
    Positive,   // should resolve to target verb
    Negative,   // should NOT resolve to target verb (or should resolve elsewhere)
    Boundary,   // near-neighbour margin test
}

enum ExpectedOutcome {
    /// Should resolve to exactly this verb
    ResolvesTo(VerbId),
    /// Should resolve to one of these verbs (acceptable alternatives)
    ResolvesToOneOf(Vec<VerbId>),
    /// Should halt with this specific reason
    HaltsWithReason(ExpectedHaltReason),
    /// Should halt at this phase (any reason)
    HaltsAtPhase(u8),
    /// Should trigger clarification
    TriggersClarification,
    /// Should fall to Sage (outside DSL surface)
    FallsToSage,
}

enum ExpectedHaltReason {
    NoViableVerb,
    StateConflict,
    ConstellationBlock,
    AmbiguousResolution,
    BelowConfidenceThreshold,
    DagOrderingConflict,
    ExclusionMakesPlanInfeasible,
    MidPlanConstellationBlock,
    // Maps to HaltReason variants from Kernel §4c
}

enum CalibrationVerdict {
    /// Correct: outcome matches expected
    Pass,
    /// Wrong verb resolved
    WrongVerb { expected: VerbId, actual: VerbId },
    /// Should have resolved but didn't
    FalseNegative { expected: VerbId, actual_halt: HaltReason },
    /// Should NOT have resolved but did
    FalsePositive { unexpected_verb: VerbId, expected_halt: ExpectedHaltReason },
    /// Right verb but margin too thin — boundary fragile
    PassWithFragileMargin { margin: f32, threshold: f32 },
    /// Halted at correct phase but wrong reason
    CorrectPhaseWrongReason { expected: ExpectedHaltReason, actual: HaltReason },
    /// Halted at wrong phase entirely
    WrongPhase { expected_phase: u8, actual_phase: u8 },
    /// Fallback invoked when it shouldn't have been
    UnnecessaryFallback,
    /// Cross-entity plan compiled incorrectly
    WrongPlanShape { expected_shape: String, actual_shape: String },
}
```

### 4.6 Failure Taxonomy (wired to Kernel HaltReasons)

The harness classifies every failure to the specific pipeline phase and `HaltReason` variant:

| Failure Category | Pipeline Phase | HaltReason (Kernel §4c) | Diagnostic Value |
|-----------------|---------------|--------------------------|-----------------|
| Referential binding failure | Phase 1 | `NoParsableIntent` | Generator produced unparseable utterance, or NLCI parser gap |
| Constellation recovery failure | Phase 2 | `NoEntityFound` | NounIndex gap — entity type not resolved |
| Ambiguous entity | Phase 2 | `AmbiguousEntity` | Disambiguation signal insufficient |
| State legality mismatch | Phase 2 | `StateConflict` | Entity FSM constraint correctly blocking (if expected) or incorrectly blocking (if not) |
| Constellation predicate block | Phase 2 | `ConstellationBlock` | DAG 1 prerequisite edge correctly/incorrectly blocking |
| Referential context missing | Phase 2 | `MissingReferentialContext` | Session context not carried into synthetic run |
| ECIR over-pruning | Phase 3 | `NoViableVerb` | Action-category classifier or taxonomy filter too aggressive |
| Phase 4 wrong resolution | Phase 4 | (resolved wrong verb) | Embedding similarity or concept match pointed to wrong target |
| Phase 4 neighbour drift | Phase 4 | (correct verb but thin margin) | Verb boundary fragile — near-neighbour too close |
| Fallback overuse | Phase 4 | `FallbackWidened` strategy | Phase 3 candidate set insufficient — ECIR anti-pattern budget breach |
| Ambiguity inflation | Phase 4 | `AmbiguousResolution` | Two candidates within epsilon — verb boundary indistinct |
| Confidence too low | Phase 4 | `BelowConfidenceThreshold` | Embedding distance or concept coverage insufficient |
| DAG ordering conflict | Phase 2/§9 | `DagOrderingConflict` | User-stated ordering contradicts DAG 1 prerequisite |
| Exclusion infeasibility | Phase 2/§9 | `ExclusionMakesPlanInfeasible` | Exclusion contradicts DAG 1 prerequisite |
| Mid-plan block | Phase 5 | `MidPlanConstellationBlock` | Earlier node caused constellation change that blocks later node |
| Execution failure | Phase 5 | `ValidationError` / `StateTransitionError` | DSL command itself failed (rare in calibration) |

---

## 5. Calibration Run as a Governed Artifact

Each execution of a scenario produces a governed run artifact:

```rust
struct CalibrationRun {
    // ─── Identity ───
    run_id: Uuid,
    scenario_id: Uuid,
    triggered_by: String,           // manual | CI | scheduled
    run_start: DateTime<Utc>,
    run_end: DateTime<Utc>,

    // ─── Version Pins (from Kernel §4b — same struct) ───
    surface_versions: SurfaceVersions,

    // ─── Input ───
    utterance_count: usize,
    positive_count: usize,
    negative_count: usize,
    boundary_count: usize,
    pre_screened: bool,

    // ─── Aggregate Metrics ───
    metrics: CalibrationMetrics,

    // ─── Per-Utterance Outcomes ───
    outcomes: Vec<CalibrationOutcome>,

    // ─── Drift Detection (if prior run exists) ───
    prior_run_id: Option<Uuid>,
    drift: Option<CalibrationDrift>,

    // ─── Trace References ───
    trace_ids: Vec<Uuid>,           // links to UtteranceTrace records
}

struct CalibrationMetrics {
    // ─── Hit Rates ───
    positive_hit_rate: f32,          // % of positive utterances that resolved correctly
    negative_rejection_rate: f32,    // % of negative utterances that correctly did NOT resolve to target
    boundary_correct_rate: f32,      // % of boundary utterances that resolved to correct verb
    overall_accuracy: f32,

    // ─── Phase-Specific ───
    phase2_legality_compliance: f32, // % where Phase 2 legal verb set was correct
    phase3_overprune_rate: f32,      // % where correct verb was eliminated by ECIR
    phase3_candidate_set_avg: f32,   // average Phase 3 output set size
    phase4_fallback_rate: f32,       // % where fallback escape hatch was invoked
    phase4_avg_margin: f32,          // average top-1 vs top-2 margin

    // ─── Boundary Quality ───
    fragile_boundary_count: usize,   // boundaries where margin < threshold
    margin_histogram: Vec<(f32, usize)>,  // (margin_bucket, count)

    // ─── Constellation ───
    constellation_recovery_rate: f32, // % where correct constellation was recovered
    dag1_predicate_accuracy: f32,     // % where DAG 1 predicates fired correctly

    // ─── Latency ───
    avg_total_latency_ms: f32,
    avg_phase_latency_ms: [f32; 6],  // per-phase average
    p95_total_latency_ms: f32,

    // ─── Execution Shape (for cross-entity scenarios) ───
    plan_compilation_success_rate: Option<f32>,
    exclusion_enforcement_accuracy: Option<f32>,
}
```

---

## 6. Drift Detection

Comparing calibration runs over time detects systematic degradation:

```rust
struct CalibrationDrift {
    prior_run_id: Uuid,
    current_run_id: Uuid,
    version_deltas: Vec<String>,     // which SurfaceVersions changed

    // ─── Rate Deltas ───
    positive_hit_rate_delta: f32,     // negative = regression
    negative_rejection_rate_delta: f32,
    fallback_rate_delta: f32,        // positive = more fallback = worse
    avg_margin_delta: f32,           // negative = thinner margins = worse

    // ─── Specific Regressions ───
    newly_failing_utterances: Vec<DriftedUtterance>,
    newly_passing_utterances: Vec<DriftedUtterance>,
    margin_degraded_utterances: Vec<DriftedUtterance>,

    // ─── Narrowing Drift (from Kernel §12) ───
    narrowing_drift: Option<NarrowingDrift>,

    // ─── Flags ───
    drift_flags: Vec<DriftFlag>,
}

struct DriftedUtterance {
    utterance: String,
    prior_verdict: CalibrationVerdict,
    current_verdict: CalibrationVerdict,
    prior_resolved_verb: Option<VerbId>,
    current_resolved_verb: Option<VerbId>,
    prior_margin: Option<f32>,
    current_margin: Option<f32>,
}

enum DriftFlag {
    HitRateRegression { delta: f32 },
    FallbackRateIncrease { delta: f32 },
    MarginDegradation { avg_delta: f32, fragile_count_delta: i32 },
    NewFalsePositives { count: usize },
    NewFalseNegatives { count: usize },
    NarrowingWeakened,
    ConstellationPredicateRegression,
    PatternInfluenceDrift,
}
```

---

## 7. Relationship to the Three Feedback Loops

Loopback Calibration is a fourth capability that reads from and writes to the existing three loops, but is architecturally separate.

### 7.1 Relationship to Loop 1 (DSL Discovery)

**Reads from:** Calibration scenarios should prioritise verb families where Loop 1 has already detected `NoViableVerb` gaps — these are known weak spots.

**Writes to:** When positive calibration utterances produce `HaltReason::NoViableVerb`, the harness generates **proposed** `GAP`-coded entries in the same `macro_verb_corrections.yaml` format used by Loop 1. These are draft artifacts for review — they enter the existing remediation pipeline as candidates, not as admitted corrections. This is consistent with the governance posture elsewhere in the document: calibration may suggest, not self-authorise.

### 7.2 Relationship to Loop 2 (User Clarification)

**Reads from:** Clarification patterns from Loop 2 indicate which verb boundaries are confusing in production. These should inform which near-neighbour pairs to target in boundary calibration.

**Writes to:** When boundary calibration reveals thin margins between verb pairs, the harness can generate suggested clarification prompts for those specific pairs. "When the user says 'freeze', ask whether they mean temporarily (suspend) or permanently (terminate)."

### 7.3 Relationship to Loop 3 (Operational Pattern Learning)

**Non-negotiable boundary:** Calibration traces do NOT feed into Loop 3's verb-frequency distributions. Loop 3 must learn from real user behaviour, not from synthetic probes. If synthetic traces polluted Loop 3, the pattern catalogue would reflect generator language patterns, not operational language patterns.

**Reads from:** Loop 3's verb-frequency distributions per situation signature tell the harness which signatures have sparse data. Sparse signatures are high-priority calibration targets — the system has the least empirical knowledge there and is most likely to misresolve.

**Writes to:** Nothing directly. But calibration results may inform which situation signatures deserve more production observation (e.g., by suggesting that Sage proactively prompts in those situations to build trace data faster).

### 7.4 Separation Enforcement

```sql
-- Calibration traces are tagged and excluded from Loop 3 aggregation
ALTER TABLE "ob-poc".utterance_traces ADD COLUMN is_synthetic BOOLEAN DEFAULT false;

-- Loop 3 aggregation query filters:
-- WHERE is_synthetic = false
```

---

## 8. Benchmark Corpus Lifecycle

Synthetic utterances are not automatically valid. They require governed lifecycle management.

### 8.1 Lifecycle States

```
Generated → Screened → Curated → Admitted → Active → Deprecated → Superseded
```

| State | Meaning | Who Decides |
|-------|---------|-------------|
| Generated | Raw output from the LLM generator | Automated |
| Screened | Embedding pre-screened, stratified by distance | Automated |
| Curated | Reviewed by domain expert for operational plausibility | Domain reviewer |
| Admitted | Approved for inclusion in benchmark corpus | Scenario owner |
| Active | Used in calibration runs | Automated |
| Deprecated | No longer representative (DSL changed, verb renamed) | Scenario owner |
| Superseded | Replaced by a newer generation for the same scenario | Automated |

### 8.2 Admission Criteria

A synthetic utterance is admitted to the benchmark corpus only if:

1. **Operational plausibility:** A domain reviewer confirms that a real operations user could plausibly say this in the target operational context.
2. **Linguistic distinctiveness:** The utterance tests a meaningfully different linguistic dimension from existing admitted utterances for the same scenario.
3. **Expected outcome is unambiguous:** The correct resolution (or correct halt reason) is agreed by the reviewer, not just asserted by the generator.
4. **Pre-screen stratum is recorded:** The embedding distance to the target verb is known, so the utterance can be classified as ClearMatch, BoundaryCase, or ClearNonMatch.

Utterances that fail admission criteria are retained as `Screened` for potential future use but are not included in calibration runs.

---

## 9. Scope

### 9.1 In Scope

- Governed calibration scenario definition anchored to DAG 1 constellation templates
- Synthetic utterance generation with positive, negative, and boundary modes
- Embedding pre-screening via the same Candle/BGE path as production
- Execution through the production six-phase pipeline (Kernel §2–§3)
- Per-utterance `UtteranceTrace`-based diagnosis using Kernel halt reason variants
- Near-neighbour boundary margin measurement
- Cross-entity scenario coverage (Singleton, Batch, CrossEntityPlan)
- DAG 1 prerequisite edge testing (ordering conflicts, exclusion infeasibility)
- Version-pinned drift detection across calibration runs
- Integration with Loop 1 (gap discovery) and Loop 2 (clarification patterns)
- Governed benchmark corpus lifecycle with admission review
- Latency profiling per phase per utterance
- Feedback into constellation remediation pipeline

### 9.2 Explicitly Out of Scope for First Iteration

- Autonomous production prompt mutation
- Autonomous legality rule changes
- Autonomous state predicate changes
- Autonomous DSL boundary redefinition
- Direct production self-healing without review
- Uncontrolled synthetic data generation without governed seed definitions
- Replacing user-driven evaluation with synthetic-only evaluation
- Feeding synthetic traces into Loop 3 verb-frequency distributions

The first delivery is a diagnostic and calibration capability, not a self-governing execution engine.

---

## 10. Architectural Principles

### 10.1 Seed Truth Is Governed Truth

The calibration seed is derived from governed Semantic OS / DSL metadata (DAG 1 templates, entity FSMs, legal verb sets). The generator may propose utterances, but it does not define truth.

### 10.2 Legality Remains Authoritative

No amount of linguistic plausibility or synthetic confidence may override the legality ceiling defined by Phase 2 constellation recovery and predicate/state gating. Loopback Calibration reinforces this rule, not weakens it.

### 10.3 Pattern Learning May Inform, Not Authorise

Any learning derived from calibration may suggest metadata updates, benchmark additions, threshold reviews, concept expansions, or negative constraints — but it must not silently become policy or legality. (Same principle as Kernel §10 Loop 3 non-negotiable boundary.)

### 10.4 Traceability Is First-Class

Every synthetic run produces a full `UtteranceTrace` (Kernel §4) suitable for replay, comparison, and failure localisation. If a calibration result cannot be traced, it is not trustworthy.

### 10.5 Generated Utterances Are Candidates, Not Axioms

Synthetic utterances are benchmark candidates subject to the governed corpus lifecycle (§8). Some may be semantically leaky, ambiguous, or invalid. The platform supports validation, curation, and admission control.

### 10.6 Regression Must Be Version-Pinned

Calibration results are interpretable against the same `SurfaceVersions` struct (Kernel §4b): code, concept registries, embedding models, threshold policies, parser logic, constellation templates, DAG 1 edges, pattern catalogue, and macro compiler. Without this, drift analysis becomes anecdotal.

### 10.7 Negative Calibration Is First-Class (new in v0.2)

Testing that the pipeline correctly refuses to resolve utterances that should not match is as important as testing that it resolves utterances that should match. Every scenario should include negative and boundary utterances alongside positive ones.

---

## 11. Storage Schema

```sql
-- ─── Scenarios ───
CREATE TABLE "ob-poc".calibration_scenarios (
    scenario_id         UUID PRIMARY KEY,
    scenario_name       TEXT NOT NULL,
    created_by          TEXT NOT NULL,
    governance_status   TEXT NOT NULL,     -- Draft | Reviewed | Admitted | Deprecated
    constellation_template_id TEXT NOT NULL,
    constellation_template_version TEXT NOT NULL,
    situation_signature TEXT,
    operational_phase   TEXT,
    target_entity_type  TEXT NOT NULL,
    target_entity_state TEXT NOT NULL,
    target_verb         TEXT NOT NULL,
    execution_shape     TEXT NOT NULL,     -- Singleton | Batch | CrossEntityPlan
    seed_data           JSONB NOT NULL,    -- full CalibrationScenario as JSON
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_cal_scenario_verb ON "ob-poc".calibration_scenarios (target_verb);
CREATE INDEX idx_cal_scenario_template ON "ob-poc".calibration_scenarios (constellation_template_id);
CREATE INDEX idx_cal_scenario_phase ON "ob-poc".calibration_scenarios (operational_phase);

-- ─── Benchmark Corpus ───
CREATE TABLE "ob-poc".calibration_utterances (
    utterance_id        UUID PRIMARY KEY,
    scenario_id         UUID NOT NULL REFERENCES "ob-poc".calibration_scenarios(scenario_id),
    text                TEXT NOT NULL,
    calibration_mode    TEXT NOT NULL,     -- Positive | Negative | Boundary
    lifecycle_status    TEXT NOT NULL,     -- Generated | Screened | Curated | Admitted | ...
    expected_outcome    JSONB NOT NULL,
    pre_screen          JSONB,            -- EmbeddingPreScreen data
    pre_screen_stratum  TEXT,             -- ClearMatch | BoundaryCase | ClearNonMatch | NeighbourPreferred
    reviewed_by         TEXT,
    admitted_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_cal_utt_scenario ON "ob-poc".calibration_utterances (scenario_id, lifecycle_status);
CREATE INDEX idx_cal_utt_mode ON "ob-poc".calibration_utterances (calibration_mode);
CREATE INDEX idx_cal_utt_stratum ON "ob-poc".calibration_utterances (pre_screen_stratum)
                                 WHERE pre_screen_stratum IS NOT NULL;

-- ─── Runs ───
CREATE TABLE "ob-poc".calibration_runs (
    run_id              UUID PRIMARY KEY,
    scenario_id         UUID NOT NULL REFERENCES "ob-poc".calibration_scenarios(scenario_id),
    triggered_by        TEXT NOT NULL,     -- manual | CI | scheduled
    surface_versions    JSONB NOT NULL,    -- SurfaceVersions from Kernel §4b
    utterance_count     INTEGER NOT NULL,
    metrics             JSONB NOT NULL,    -- CalibrationMetrics
    drift               JSONB,            -- CalibrationDrift (if prior run exists)
    prior_run_id        UUID REFERENCES "ob-poc".calibration_runs(run_id),
    run_start           TIMESTAMPTZ NOT NULL,
    run_end             TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_cal_run_scenario ON "ob-poc".calibration_runs (scenario_id, run_start DESC);

-- ─── Per-Utterance Outcomes ───
CREATE TABLE "ob-poc".calibration_outcomes (
    outcome_id          UUID PRIMARY KEY,
    run_id              UUID NOT NULL REFERENCES "ob-poc".calibration_runs(run_id),
    utterance_id        UUID NOT NULL REFERENCES "ob-poc".calibration_utterances(utterance_id),
    trace_id            UUID NOT NULL,     -- references "ob-poc".utterance_traces.trace_id
    calibration_mode    TEXT NOT NULL,
    expected_outcome    JSONB NOT NULL,
    verdict             TEXT NOT NULL,     -- Pass | WrongVerb | FalseNegative | ...
    actual_resolved_verb TEXT,
    actual_halt_reason   TEXT,
    failure_phase       SMALLINT,
    top1_score          REAL,
    top2_score          REAL,
    margin              REAL,
    margin_stable       BOOLEAN,
    latency_ms          INTEGER,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_cal_out_run ON "ob-poc".calibration_outcomes (run_id, verdict);
CREATE INDEX idx_cal_out_trace ON "ob-poc".calibration_outcomes (trace_id);
CREATE INDEX idx_cal_out_verb ON "ob-poc".calibration_outcomes (actual_resolved_verb)
                               WHERE actual_resolved_verb IS NOT NULL;
CREATE INDEX idx_cal_out_fragile ON "ob-poc".calibration_outcomes (margin_stable)
                                 WHERE margin_stable = false;
```

---

## 12. Worked Example — End-to-End Calibration Run

### Scenario: `cbu.suspend` in KYCBlocked Constellation

**Seed:**
```
scenario_id: cal-001
constellation_template: lu-ucits-sicav
situation_signature: cbu:ACTIVE|kyc:OPEN|si:APPROVED|tp:PENDING_VALIDATION|ubo:VERIFIED
operational_phase: KYCBlocked
target_entity_type: cbu
target_entity_state: ACTIVE
target_verb: cbu.suspend
near_neighbours: [cbu.terminate (High), cbu.hold (Medium), cbu.annotate (Low)]
execution_shape: Singleton
```

**Generated utterances (after admission):**

| # | Utterance | Mode | Pre-Screen Stratum |
|---|-----------|------|-------------------|
| 1 | "Please suspend the Acme Corp custody account" | Positive | ClearMatch (0.91) |
| 2 | "Freeze it for now" | Positive | ClearMatch (0.87) |
| 3 | "Can we pause things?" | Positive | BoundaryCase (0.76) |
| 4 | "Shut down the Acme account permanently" | Negative | NeighbourPreferred (terminate: 0.89) |
| 5 | "What's the status of the account?" | Negative | ClearNonMatch (0.34) |
| 6 | "Stop all activity" | Boundary | BoundaryCase (margin: 0.04) |
| 7 | "Put a temporary hold on 7742" | Boundary | BoundaryCase (margin: 0.11) |
| 8 | "Terminate CBU-7742" | Negative | NeighbourPreferred (terminate: 0.94) |

**Run results:**

| # | Verdict | Resolved Verb | Top-1 | Top-2 | Margin | Notes |
|---|---------|---------------|-------|-------|--------|-------|
| 1 | Pass | `cbu.suspend` | 0.93 | 0.78 | 0.15 | Clean resolution, ExactMatch strategy |
| 2 | Pass | `cbu.suspend` | 0.88 | 0.82 | 0.06 | Resolved but margin thin — "freeze" is ambiguous |
| 3 | Pass | `cbu.suspend` | 0.79 | 0.71 | 0.08 | Constellation pattern boosted — KYCBlocked phase helped |
| 4 | Pass | `cbu.terminate` | 0.91 | 0.74 | 0.17 | Correctly resolved to terminate, not suspend |
| 5 | Pass | `cbu.get-status` | 0.94 | 0.42 | 0.52 | Observation verb, wide margin — no confusion |
| 6 | **PassWithFragileMargin** | `cbu.suspend` | 0.82 | 0.80 | **0.02** | Correct but dangerously thin — "stop" is a weak discriminator |
| 7 | Pass | `cbu.suspend` | 0.86 | 0.73 | 0.13 | "temporary" is the key signal — drives correct resolution |
| 8 | Pass | `cbu.terminate` | 0.95 | 0.71 | 0.24 | Clear negative — correctly rejected as not suspend |

**Metrics:**
```
positive_hit_rate: 100% (3/3)
negative_rejection_rate: 100% (3/3 correctly resolved to non-target or non-target domain)
boundary_correct_rate: 100% (2/2)
fragile_boundary_count: 1 (utterance #6, margin 0.02)
phase4_fallback_rate: 0%
avg_margin: 0.17
```

**Diagnostic insight:** Utterance #6 ("Stop all activity") resolves correctly today but with margin 0.02. A small embedding model change, threshold adjustment, or `verb_concepts.yaml` update could flip it to `cbu.terminate`. This is a concrete signal: the verb boundary between `suspend` and `terminate` needs strengthening for utterances containing "stop" without a temporal qualifier. Options: add "stop activity" to `verb_concepts.yaml` under `cbu.suspend`, or add a disambiguation prompt for "stop" without temporal context.

---

## 13. First Delivery — Benchmark Portfolio Prioritisation (new in v0.3)

The framework supports arbitrary scenario creation, but the first delivery should target scenarios with the highest calibration value — the places where misresolution is most likely, most consequential, or most poorly covered by existing data.

### 13.1 Priority Categories

| Priority | Category | Rationale | Example Scenarios |
|----------|----------|-----------|-------------------|
| **P1** | High-risk near-neighbour pairs | These are the verb boundaries most likely to break under linguistic variation. Thin margins here cause real misresolution. | `cbu.suspend` vs `cbu.terminate`, `kyc.close` vs `kyc.withdraw`, `entity.deactivate` vs `entity.archive` |
| **P2** | High-volume operational verbs | Verbs that fire most frequently in production. Even a small accuracy drop affects many users. | `cbu.get-status`, `kyc.assign-analyst`, `cbu.create`, `ubo.discover` |
| **P3** | Sparse situation signatures | Signatures where Loop 3 has < 10 historical traces — the system has the least empirical knowledge here and is flying on static catalogue + ECIR alone. | Cross-border hedge fund constellations, multi-feeder structures, early-onboarding with incomplete entity set |
| **P4** | Recently changed concept surfaces | Any verb family where `verb_concepts.yaml`, embedding model, threshold policy, or constellation template changed in the last release. Regression risk is highest immediately after change. | Whatever surfaces changed in the most recent Codex execution |
| **P5** | Verbs with high production clarification rate | Verbs that frequently trigger `ClarificationTriggered` in production traces (Loop 2 data). These are known-ambiguous — boundary calibration can measure whether the ambiguity is inherent or fixable. | Read from `utterance_traces WHERE outcome = 'ClarificationTriggered'` grouped by `resolved_verb` |
| **P6** | Cross-entity DAG scenarios | At least 1–2 scenarios covering `CrossEntityPlan` execution shape, testing DAG ordering conflict detection, exclusion infeasibility, and mid-plan constellation re-check. These are the hardest execution paths. | "Close KYC then terminate CBU" (consistent ordering), "Terminate CBU but leave KYC untouched" (infeasible exclusion) |

### 13.2 Recommended First Portfolio

For the first delivery, target 10–15 scenarios:

- 4–5 P1 near-neighbour pair scenarios (CBU domain — highest verb count, most confusion risk)
- 2–3 P2 high-volume observation verb scenarios (ensure the fast path works reliably)
- 1–2 P3 sparse signature scenarios (cross-border or early-onboarding)
- 1 P4 recently-changed scenario (whatever changed last)
- 1 P5 high-clarification scenario (from production Loop 2 data)
- 1–2 P6 cross-entity DAG scenarios

Each scenario should include at least 15–25 admitted utterances across all three modes: approximately 8–10 positive, 4–6 negative (split between Type A and Type B), and 3–5 boundary cases targeting the identified near-neighbours.

This gives a first benchmark corpus of roughly 150–375 admitted utterances — large enough to produce meaningful metrics, small enough to curate properly and run within a CI cycle.

---

## 14. Component Map

| Component | Description | Status |
|-----------|-------------|--------|
| CalibrationScenario definition | Governed seed objects anchored to DAG 1 | **New — not yet implemented** |
| Synthetic utterance generator | LLM-based generation from scenario seeds | **New — not yet implemented** |
| Embedding pre-screener | Candle/BGE stratification of synthetic corpus | **New — not yet implemented** |
| Pipeline execution harness | Runs synthetic utterances through production 6-phase pipeline | **New — not yet implemented** |
| Trace reader and outcome classifier | Reads `UtteranceTrace`, classifies against expected outcome | **New — not yet implemented** |
| CalibrationRun persistence | Run artifact with metrics, drift, trace references | **New — not yet implemented** |
| Benchmark corpus lifecycle | Generation → screening → curation → admission workflow | **New — not yet implemented** |
| Drift detector | Run-over-run comparison with version-pinned deltas | **New — not yet implemented** |
| Loop 1 gap report integration | `GAP`-coded entries from calibration false negatives | **New — wires to existing pipeline** |
| Loop 2 clarification suggestions | Suggested prompts from thin-margin boundary pairs | **New — not yet implemented** |
| Synthetic trace tagging | `is_synthetic` flag to exclude from Loop 3 | **New — trivial schema change** |
| Latency profiler | Per-phase latency tracking per utterance | **New — reads existing trace timestamps** |
| Cross-entity scenario support | Batch + CrossEntityPlan calibration modes | **New — not yet implemented** |
| DAG ordering conflict scenarios | Tests `DagOrderingConflict` detection | **New — not yet implemented** |
| Exclusion infeasibility scenarios | Tests `ExclusionMakesPlanInfeasible` detection | **New — not yet implemented** |

---

## 15. Implementation Plan

This section is the Codex execution plan. Seven phases, each with verification steps. All types, schemas, and logic from §4–§11 are implemented here in full, but they must align to the live repo contracts before coding begins.

All new library code goes in `rust/src/calibration/` (new module). Operator workflow should prefer `rust/xtask/` integration over a new standalone runtime binary unless a dedicated binary is strictly required. All migrations go in `rust/migrations/` using the repo's existing `YYYYMMDD_name.sql` convention.

---

### Phase 1: Schema & Rust Types

**Goal:** Deploy storage tables and implement all Rust types for the calibration domain.

#### Task 1.1: Create migration — calibration tables

**File:** `rust/migrations/YYYYMMDD_calibration_tables.sql`

```sql
CREATE TABLE "ob-poc".calibration_scenarios (
    scenario_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_name       TEXT NOT NULL,
    created_by          TEXT NOT NULL,
    governance_status   TEXT NOT NULL DEFAULT 'Draft',
    constellation_template_id TEXT NOT NULL,
    constellation_template_version TEXT NOT NULL,
    situation_signature TEXT,
    operational_phase   TEXT,
    target_entity_type  TEXT NOT NULL,
    target_entity_state TEXT NOT NULL,
    target_verb         TEXT NOT NULL,
    execution_shape     TEXT NOT NULL DEFAULT 'Singleton',
    seed_data           JSONB NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_cal_scenario_verb ON "ob-poc".calibration_scenarios (target_verb);
CREATE INDEX idx_cal_scenario_template ON "ob-poc".calibration_scenarios (constellation_template_id);
CREATE INDEX idx_cal_scenario_phase ON "ob-poc".calibration_scenarios (operational_phase);
CREATE INDEX idx_cal_scenario_status ON "ob-poc".calibration_scenarios (governance_status);

CREATE TABLE "ob-poc".calibration_utterances (
    utterance_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_id         UUID NOT NULL REFERENCES "ob-poc".calibration_scenarios(scenario_id),
    text                TEXT NOT NULL,
    calibration_mode    TEXT NOT NULL,     -- Positive | Negative | Boundary
    negative_type       TEXT,             -- TypeA | TypeB (NULL for Positive/Boundary)
    lifecycle_status    TEXT NOT NULL DEFAULT 'Generated',
    expected_outcome    JSONB NOT NULL,
    pre_screen          JSONB,
    pre_screen_stratum  TEXT,
    reviewed_by         TEXT,
    admitted_at         TIMESTAMPTZ,
    deprecated_at       TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_cal_utt_scenario ON "ob-poc".calibration_utterances (scenario_id, lifecycle_status);
CREATE INDEX idx_cal_utt_mode ON "ob-poc".calibration_utterances (calibration_mode);
CREATE INDEX idx_cal_utt_stratum ON "ob-poc".calibration_utterances (pre_screen_stratum)
                                 WHERE pre_screen_stratum IS NOT NULL;
CREATE INDEX idx_cal_utt_admitted ON "ob-poc".calibration_utterances (scenario_id)
                                  WHERE lifecycle_status = 'Admitted';

CREATE TABLE "ob-poc".calibration_runs (
    run_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_id         UUID NOT NULL REFERENCES "ob-poc".calibration_scenarios(scenario_id),
    triggered_by        TEXT NOT NULL,
    surface_versions    JSONB NOT NULL,
    utterance_count     INTEGER NOT NULL,
    positive_count      INTEGER NOT NULL DEFAULT 0,
    negative_count      INTEGER NOT NULL DEFAULT 0,
    boundary_count      INTEGER NOT NULL DEFAULT 0,
    metrics             JSONB NOT NULL,
    drift               JSONB,
    prior_run_id        UUID REFERENCES "ob-poc".calibration_runs(run_id),
    run_start           TIMESTAMPTZ NOT NULL,
    run_end             TIMESTAMPTZ
);

CREATE INDEX idx_cal_run_scenario ON "ob-poc".calibration_runs (scenario_id, run_start DESC);
CREATE INDEX idx_cal_run_prior ON "ob-poc".calibration_runs (prior_run_id)
                                WHERE prior_run_id IS NOT NULL;

CREATE TABLE "ob-poc".calibration_outcomes (
    outcome_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id              UUID NOT NULL REFERENCES "ob-poc".calibration_runs(run_id),
    utterance_id        UUID NOT NULL REFERENCES "ob-poc".calibration_utterances(utterance_id),
    trace_id            UUID NOT NULL,
    calibration_mode    TEXT NOT NULL,
    negative_type       TEXT,
    expected_outcome    JSONB NOT NULL,
    verdict             TEXT NOT NULL,
    actual_resolved_verb TEXT,
    actual_halt_reason  TEXT,
    failure_phase       SMALLINT,
    failure_detail      JSONB,
    top1_score          REAL,
    top2_score          REAL,
    margin              REAL,
    margin_stable       BOOLEAN,
    latency_total_ms    INTEGER,
    latency_per_phase   JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_cal_out_run ON "ob-poc".calibration_outcomes (run_id, verdict);
CREATE INDEX idx_cal_out_trace ON "ob-poc".calibration_outcomes (trace_id);
CREATE INDEX idx_cal_out_fragile ON "ob-poc".calibration_outcomes (margin_stable)
                                 WHERE margin_stable = false;
CREATE INDEX idx_cal_out_failures ON "ob-poc".calibration_outcomes (verdict, failure_phase)
                                  WHERE verdict != 'Pass';
```

#### Task 1.2: Implement all Rust types

**File:** `rust/src/calibration/types.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Core Enums ───

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CalibrationMode {
    Positive,
    Negative,
    Boundary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NegativeType {
    /// Should resolve to a different valid verb (discrimination test)
    TypeA,
    /// Should halt / clarify / fall to Sage (boundary recognition test)
    TypeB,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GovernanceStatus {
    Draft,
    Reviewed,
    Admitted,
    Deprecated,
    Superseded { by: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfusionRisk {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectedOutcome {
    ResolvesTo(String),
    ResolvesToOneOf(Vec<String>),
    HaltsWithReason(ExpectedHaltReason),
    HaltsAtPhase(u8),
    TriggersClarification,
    FallsToSage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExpectedHaltReason {
    NoViableVerb,
    StateConflict,
    ConstellationBlock,
    AmbiguousResolution,
    BelowConfidenceThreshold,
    DagOrderingConflict,
    ExclusionMakesPlanInfeasible,
    MidPlanConstellationBlock,
    MissingReferentialContext,
    NoParsableIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalibrationVerdict {
    Pass,
    WrongVerb { expected: String, actual: String },
    FalseNegative { expected: String, actual_halt: String },
    FalsePositive { unexpected_verb: String, expected_halt: ExpectedHaltReason },
    PassWithFragileMargin { margin: f32, threshold: f32 },
    CorrectPhaseWrongReason { expected: ExpectedHaltReason, actual: String },
    WrongPhase { expected_phase: u8, actual_phase: u8 },
    UnnecessaryFallback,
    WrongPlanShape { expected_shape: String, actual_shape: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreScreenStratum {
    ClearMatch { distance: f32 },
    BoundaryCase { margin: f32 },
    ClearNonMatch { distance: f32 },
    NeighbourPreferred { preferred_verb: String, preferred_distance: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalibrationExecutionShape {
    Singleton,
    Batch {
        filter_expression: String,
        expected_entity_count: usize,
    },
    CrossEntityPlan {
        plan_nodes: Vec<CalibrationPlanNode>,
        expected_dag3_edges: Vec<(usize, usize)>,
        exclusion_predicates: Vec<String>, // serialised ExclusionPredicate
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DriftFlag {
    HitRateRegression { delta: f32 },
    FallbackRateIncrease { delta: f32 },
    MarginDegradation { avg_delta: f32, fragile_count_delta: i32 },
    NewFalsePositives { count: usize },
    NewFalseNegatives { count: usize },
    NarrowingWeakened,
    ConstellationPredicateRegression,
    PatternInfluenceDrift,
}

// ─── Structs ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearNeighbourVerb {
    pub verb_id: String,
    pub expected_embedding_distance: f32,
    pub confusion_risk: ConfusionRisk,
    pub distinguishing_signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludedNeighbour {
    pub verb_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldUtterance {
    pub text: String,
    pub expected_outcome: ExpectedOutcome,
    pub authored_by: String,
    pub admitted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationPlanNode {
    pub entity_type: String,
    pub entity_state: String,
    pub target_verb: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationScenario {
    pub scenario_id: Uuid,
    pub scenario_name: String,
    pub created_by: String,
    pub governance_status: GovernanceStatus,

    // Constellation context
    pub constellation_template_id: String,
    pub constellation_template_version: String,
    pub situation_signature: String,
    pub operational_phase: String,

    // Entity context
    pub target_entity_type: String,
    pub target_entity_state: String,
    pub linked_entity_states: Vec<(String, String)>,

    // Verb context
    pub target_verb: String,
    pub legal_verb_set_snapshot: Vec<String>,
    pub verb_taxonomy_tag: String,  // "EntityVerb" | "StateVerb"

    // Boundary context
    pub excluded_neighbours: Vec<ExcludedNeighbour>,
    pub near_neighbour_verbs: Vec<NearNeighbourVerb>,
    pub expected_margin_threshold: f32,

    // Execution shape
    pub execution_shape: CalibrationExecutionShape,

    // Gold standard
    pub gold_utterances: Vec<GoldUtterance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingPreScreen {
    pub utterance: String,
    pub target_verb_distance: f32,
    pub nearest_neighbour_distance: f32,
    pub nearest_neighbour_verb: String,
    pub margin: f32,
    pub stratum: PreScreenStratum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationOutcome {
    pub utterance_id: Uuid,
    pub utterance_text: String,
    pub calibration_mode: CalibrationMode,
    pub negative_type: Option<NegativeType>,
    pub pre_screen: Option<EmbeddingPreScreen>,
    pub expected_outcome: ExpectedOutcome,

    pub trace_id: Uuid,
    pub actual_resolved_verb: Option<String>,
    pub actual_halt_reason: Option<String>,

    pub verdict: CalibrationVerdict,
    pub failure_phase: Option<u8>,
    pub failure_detail: Option<String>,

    pub top1_score: Option<f32>,
    pub top2_score: Option<f32>,
    pub margin: Option<f32>,
    pub margin_stable: Option<bool>,

    pub latency_total_ms: Option<i64>,
    pub latency_per_phase: Option<Vec<(u8, i64)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationMetrics {
    pub positive_hit_rate: f32,
    pub negative_type_a_rejection_rate: f32,
    pub negative_type_b_rejection_rate: f32,
    pub boundary_correct_rate: f32,
    pub overall_accuracy: f32,

    pub phase2_legality_compliance: f32,
    pub phase3_overprune_rate: f32,
    pub phase3_candidate_set_avg: f32,
    pub phase4_fallback_rate: f32,
    pub phase4_avg_margin: f32,

    pub fragile_boundary_count: usize,
    pub margin_histogram: Vec<(f32, usize)>,

    pub constellation_recovery_rate: f32,
    pub dag1_predicate_accuracy: f32,

    pub avg_total_latency_ms: f32,
    pub avg_phase_latency_ms: [f32; 6],
    pub p95_total_latency_ms: f32,

    pub plan_compilation_success_rate: Option<f32>,
    pub exclusion_enforcement_accuracy: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftedUtterance {
    pub utterance_id: Uuid,
    pub utterance_text: String,
    pub prior_verdict: String,
    pub current_verdict: String,
    pub prior_resolved_verb: Option<String>,
    pub current_resolved_verb: Option<String>,
    pub prior_margin: Option<f32>,
    pub current_margin: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationDrift {
    pub prior_run_id: Uuid,
    pub current_run_id: Uuid,
    pub version_deltas: Vec<String>,

    pub positive_hit_rate_delta: f32,
    pub negative_rejection_rate_delta: f32,
    pub fallback_rate_delta: f32,
    pub avg_margin_delta: f32,

    pub newly_failing_utterances: Vec<DriftedUtterance>,
    pub newly_passing_utterances: Vec<DriftedUtterance>,
    pub margin_degraded_utterances: Vec<DriftedUtterance>,

    pub drift_flags: Vec<DriftFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRun {
    pub run_id: Uuid,
    pub scenario_id: Uuid,
    pub triggered_by: String,
    pub run_start: DateTime<Utc>,
    pub run_end: Option<DateTime<Utc>>,

    pub surface_versions: serde_json::Value, // SurfaceVersions as JSON
    pub utterance_count: usize,
    pub positive_count: usize,
    pub negative_count: usize,
    pub boundary_count: usize,

    pub metrics: CalibrationMetrics,
    pub outcomes: Vec<CalibrationOutcome>,

    pub prior_run_id: Option<Uuid>,
    pub drift: Option<CalibrationDrift>,

    pub trace_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedUtterance {
    pub text: String,
    pub calibration_mode: CalibrationMode,
    pub negative_type: Option<NegativeType>,
    pub expected_outcome: ExpectedOutcome,
    pub generation_rationale: String,
}

pub struct AdmissionCheckResult {
    pub pass: bool,
    pub warnings: Vec<String>,
}
```

**File:** `rust/src/calibration/mod.rs`

```rust
pub mod types;
pub mod seed;
pub mod generator;
pub mod pre_screen;
pub mod harness;
pub mod classifier;
pub mod runner;
pub mod metrics;
pub mod drift;
pub mod loop1_integration;
pub mod loop2_integration;
pub mod admission;
pub mod governance;
pub mod report;
pub mod portfolio;
pub mod db;
```

**Verification:** `cargo check` passes with all types.

→ IMMEDIATELY proceed to Phase 2. Progress: 15%.

---

### Phase 2: Scenario Seed Builder & First Scenarios

**Goal:** Build the function that constructs a `CalibrationScenario` from live SemOS metadata, then define 10–15 scenarios.

#### Task 2.1: Build scenario seed builder

**File:** `rust/src/calibration/seed.rs`

```rust
use crate::calibration::types::*;
use sqlx::PgPool;
use anyhow::Result;
use uuid::Uuid;

/// Build a calibration scenario from live SemOS metadata.
/// Reads constellation template, computes legal verb set,
/// classifies verb taxonomy, identifies near-neighbours via BGE embeddings.
pub async fn build_scenario_seed(
    pool: &PgPool,
    embedder: &crate::agent::learning::embedder::CandleEmbedder,
    scenario_name: &str,
    template_id: &str,
    target_entity_type: &str,
    target_entity_state: &str,
    target_verb: &str,
    linked_entity_states: Vec<(String, String)>,
    execution_shape: CalibrationExecutionShape,
    margin_threshold: f32,
) -> Result<CalibrationScenario> {
    // 1. Load built-in SemOS constellation map (DAG 1 seed source)
    let template = load_builtin_constellation_map(template_id)?;
    let template_revision = compute_map_revision(
        &std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("config/sem_os_seeds/constellation_maps")
                .join(format!("{}.yaml", template_id.replace(['.', '-'], "_"))),
        )?,
    );

    // 2. Compute situation signature from entity states
    let signature = compute_situation_signature(
        target_entity_type, target_entity_state, &linked_entity_states
    );
    let phase = derive_operational_phase(&signature);

    // 3. Compute legal verb set via the live SemOS/session-verb-surface path.
    //    Reuse the same legality/governance ceiling the production pipeline applies.
    let legal_verbs = compute_live_legal_verb_set(
        pool, template_id, target_entity_type, target_entity_state, &linked_entity_states
    ).await?;

    // 4. Classify the target verb from live verb metadata / contract summaries.
    let taxonomy_tag = classify_live_verb_metadata(pool, target_verb).await?;

    // 5. Find near-neighbour verbs by BGE embedding distance
    let target_embedding = embedder.embed_target(target_verb).await?;
    let mut neighbours = Vec::new();
    for verb in &legal_verbs {
        if verb == target_verb { continue; }
        let verb_embedding = embedder.embed_target(verb).await?;
        let distance = cosine_distance(&target_embedding, &verb_embedding);
        if distance < 0.40 {  // only verbs within embedding proximity
            let risk = if distance < 0.15 {
                ConfusionRisk::High
            } else if distance < 0.25 {
                ConfusionRisk::Medium
            } else {
                ConfusionRisk::Low
            };
            neighbours.push(NearNeighbourVerb {
                verb_id: verb.clone(),
                expected_embedding_distance: distance,
                confusion_risk: risk,
                distinguishing_signals: derive_distinguishing_signals(pool, target_verb, verb).await?,
            });
        }
    }
    neighbours.sort_by(|a, b| a.expected_embedding_distance.partial_cmp(&b.expected_embedding_distance).unwrap());

    Ok(CalibrationScenario {
        scenario_id: Uuid::new_v4(),
        scenario_name: scenario_name.to_string(),
        created_by: "seed_builder".to_string(),
        governance_status: GovernanceStatus::Draft,
        constellation_template_id: template_id.to_string(),
        constellation_template_version: template_revision,
        situation_signature: signature,
        operational_phase: phase,
        target_entity_type: target_entity_type.to_string(),
        target_entity_state: target_entity_state.to_string(),
        linked_entity_states,
        target_verb: target_verb.to_string(),
        legal_verb_set_snapshot: legal_verbs,
        verb_taxonomy_tag: taxonomy_tag,
        excluded_neighbours: Vec::new(), // populated during review
        near_neighbour_verbs: neighbours,
        expected_margin_threshold: margin_threshold,
        execution_shape,
        gold_utterances: Vec::new(), // populated during review
    })
}

fn compute_situation_signature(
    entity_type: &str, entity_state: &str,
    linked: &[(String, String)]
) -> String {
    let mut parts = vec![format!("{}:{}", entity_type, entity_state)];
    for (t, s) in linked {
        parts.push(format!("{}:{}", t, s));
    }
    parts.sort();
    parts.join("|")
}

fn derive_operational_phase(signature: &str) -> String {
    // Match signature patterns to operational phases
    if signature.contains("cbu:DRAFT") || signature.contains("cbu:DISCOVERED") {
        "EarlyOnboarding".to_string()
    } else if signature.contains("kyc:OPEN") && signature.contains("cbu:ACTIVE") {
        "KYCBlocked".to_string()
    } else if signature.contains("cbu:VALIDATED") {
        "PreActivation".to_string()
    } else if signature.contains("cbu:ACTIVE") && !signature.contains("kyc:OPEN") {
        "Active".to_string()
    } else if signature.contains("cbu:TERMINATED") {
        "Terminated".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - (dot / (norm_a * norm_b))
}

// TODO: implement these by wiring to the live repo contracts:
// - load_builtin_constellation_map() / compute_map_revision()
// - compute_live_legal_verb_set() through the real SemOS / SessionVerbSurface path
// - classify_live_verb_metadata() via runtime verb metadata / SemOS adapter contracts
// - derive_distinguishing_signals() from live verb metadata / concepts where available
// These should call the SAME functions the production pipeline uses,
// not paper-era placeholder names.
```

#### Task 2.2: Define first 10–15 scenarios

Build and persist scenarios per §13.2 portfolio prioritisation. Use `build_scenario_seed()` for each:

**P1 — Near-neighbour pairs (4–5 scenarios):**

| Scenario Name | Target Verb | State | Neighbours | Shape |
|--------------|------------|-------|------------|-------|
| `cbu-suspend-kyc-blocked` | `cbu.suspend` | ACTIVE (KYCBlocked) | `cbu.terminate` (H), `cbu.hold` (M) | Singleton |
| `cbu-terminate-active` | `cbu.terminate` | ACTIVE | `cbu.suspend` (H), `cbu.deactivate` (M) | Singleton |
| `kyc-close-vs-withdraw` | `kyc.close` | OPEN | `kyc.withdraw` (H) | Singleton |
| `cbu-validate-preactivation` | `cbu.validate` | PENDING_VALIDATION | `cbu.approve` (H) | Singleton |

**P2 — High-volume observation verbs (2–3 scenarios):**

| Scenario Name | Target Verb | State | Shape |
|--------------|------------|-------|-------|
| `cbu-get-status-any` | `cbu.get-status` | ACTIVE | Singleton |
| `cbu-get-status-with-blockers` | `cbu.get-status-with-blockers` | ACTIVE (KYCBlocked) | Singleton |

**P3 — Sparse signatures (1–2 scenarios):**

Use this query to find sparse signatures, then build scenarios for the most operationally interesting:

```sql
SELECT
    trace_payload #>> '{phase_2,situation_signature,canonical_form}' AS situation_signature,
    COUNT(*) as trace_count
FROM "ob-poc".utterance_traces
WHERE is_synthetic = false
  AND trace_payload #>> '{phase_2,situation_signature,canonical_form}' IS NOT NULL
GROUP BY situation_signature
HAVING COUNT(*) < 10
ORDER BY COUNT(*) ASC
LIMIT 10;
```

**P6 — Cross-entity DAG scenarios (1–2 scenarios):**

| Scenario Name | Shape | Verbs | DAG 1 Edge |
|--------------|-------|-------|-----------|
| `cross-terminate-with-kyc` | CrossEntityPlan | `kyc.close` → `cbu.terminate` | KYC TerminationPrerequisite |
| `cross-terminate-excluded-kyc` | CrossEntityPlan | `cbu.terminate` + exclusion on KYC | ExclusionMakesPlanInfeasible |

**Verification:** `SELECT COUNT(*) FROM "ob-poc".calibration_scenarios;` returns 10–15 rows.

→ IMMEDIATELY proceed to Phase 3. Progress: 30%.

---

### Phase 3: Synthetic Utterance Generation & Pre-Screening

**Goal:** Build the LLM generation prompt, embedding pre-screener, and generate utterance families for each scenario.

#### Task 3.1: Build the generation prompt

**File:** `rust/src/calibration/generator.rs`

```rust
use crate::calibration::types::*;
use anyhow::Result;

/// Build the LLM prompt for generating a synthetic utterance family.
pub fn build_generation_prompt(scenario: &CalibrationScenario) -> String {
    let near_neighbours_desc = scenario.near_neighbour_verbs.iter()
        .map(|n| format!(
            "  - {} (confusion risk: {:?}, distance: {:.2})\n    Distinguishing signals: {}",
            n.verb_id, n.confusion_risk, n.expected_embedding_distance,
            n.distinguishing_signals.join(", ")
        ))
        .collect::<Vec<_>>()
        .join("\n");

    format!(r#"You are generating synthetic test utterances for a governed DSL calibration harness.

## Context
- Entity type: {entity_type}
- Entity state: {entity_state}
- Operational phase: {phase}
- Constellation: {signature}
- Target DSL verb: {target_verb}
- Legal verb set: {legal_verbs}

## Near-neighbour verbs (the ones most likely to be confused with the target):
{neighbours}

## Task
Generate exactly:
- 8–10 POSITIVE utterances (should resolve to {target_verb})
- 3–4 NEGATIVE TYPE A utterances (should resolve to a DIFFERENT verb — specify which)
- 2–3 NEGATIVE TYPE B utterances (should NOT resolve to any verb — should halt or clarify)
- 3–5 BOUNDARY utterances (sit on the margin between {target_verb} and its nearest neighbour)

## Variation dimensions for POSITIVE utterances
Vary across: formal/operational, colloquial, indirect, urgent, referential ("it", "that account"),
vague, euphemistic. Each utterance should test a meaningfully different linguistic dimension.

## Rules for NEGATIVE TYPE A
These must be plausible things a user would say in this constellation, but they should resolve
to a DIFFERENT verb. State which verb each should resolve to and why.

## Rules for NEGATIVE TYPE B
These must be plausible but should trigger one of: NoViableVerb (outside DSL surface),
AmbiguousResolution (too vague), StateConflict (illegal in current state),
MissingReferentialContext (pronoun with no antecedent). State which halt reason each should trigger.

## Rules for BOUNDARY utterances
These must sit exactly on the boundary between {target_verb} and its nearest neighbour.
Use words that could go either way. The distinguishing signals above tell you what
SHOULD differentiate the verbs — test what happens when those signals are weak or absent.

## Output format
Respond with ONLY a JSON array. No preamble, no markdown backticks.
Each element:
{{
  "text": "the utterance",
  "calibration_mode": "Positive" | "Negative" | "Boundary",
  "negative_type": null | "TypeA" | "TypeB",
  "expected_outcome": {{
    "type": "ResolvesTo" | "HaltsWithReason" | "TriggersClarification" | "FallsToSage",
    "verb": "verb_id or null",
    "halt_reason": "reason or null"
  }},
  "generation_rationale": "why this utterance tests what it tests"
}}
"#,
        entity_type = scenario.target_entity_type,
        entity_state = scenario.target_entity_state,
        phase = scenario.operational_phase,
        signature = scenario.situation_signature,
        target_verb = scenario.target_verb,
        legal_verbs = scenario.legal_verb_set_snapshot.join(", "),
        neighbours = near_neighbours_desc,
    )
}

/// Call the LLM to generate utterances from a scenario seed.
pub async fn generate_utterance_family(
    scenario: &CalibrationScenario,
    // Use whatever LLM client the repo has — Anthropic API, etc.
    // The prompt is the important part; the client is a detail.
) -> Result<Vec<GeneratedUtterance>> {
    let prompt = build_generation_prompt(scenario);

    // Call the LLM with the prompt
    // Parse the JSON array response
    // Map each element to GeneratedUtterance

    // Example parsing (adjust to actual LLM client):
    // let response = llm_client.complete(&prompt).await?;
    // let cleaned = response.trim().trim_start_matches("```json").trim_end_matches("```");
    // let utterances: Vec<GeneratedUtterance> = serde_json::from_str(cleaned)?;

    todo!("Wire to actual LLM client in the repo")
}
```

#### Task 3.2: Build the embedding pre-screener

**File:** `rust/src/calibration/pre_screen.rs`

```rust
use crate::calibration::types::*;
use anyhow::Result;
use std::collections::HashMap;

/// Pre-screen generated utterances using the SAME Candle/BGE embeddings
/// as the production Phase 4 pipeline.
///
/// ADVISORY ONLY: pre-screening stratifies utterances for diagnostic insight.
/// It does NOT filter utterances out of pipeline runs.
/// Every admitted utterance runs through all six phases regardless of stratum.
pub async fn pre_screen_utterances(
    utterances: &[GeneratedUtterance],
    scenario: &CalibrationScenario,
    embedder: &crate::agent::learning::embedder::CandleEmbedder,
) -> Result<Vec<EmbeddingPreScreen>> {
    // 1. Embed the target verb (as a target, not a query — same as production)
    let target_embedding = embedder.embed_target(&scenario.target_verb).await?;

    // 2. Embed all near-neighbour verbs
    let mut neighbour_embeddings: HashMap<String, Vec<f32>> = HashMap::new();
    for n in &scenario.near_neighbour_verbs {
        let emb = embedder.embed_target(&n.verb_id).await?;
        neighbour_embeddings.insert(n.verb_id.clone(), emb);
    }

    // 3. Pre-screen each utterance
    let mut results = Vec::new();
    for utterance in utterances {
        // Embed the utterance as a QUERY (with retrieval prefix — same as production)
        let utt_embedding = embedder.embed_query(&utterance.text).await?;

        // Distance to target verb
        let target_distance = cosine_distance(&utt_embedding, &target_embedding);

        // Distance to each near-neighbour
        let mut nearest_neighbour_verb = String::new();
        let mut nearest_neighbour_distance = f32::MAX;
        for (verb_id, emb) in &neighbour_embeddings {
            let dist = cosine_distance(&utt_embedding, emb);
            if dist < nearest_neighbour_distance {
                nearest_neighbour_distance = dist;
                nearest_neighbour_verb = verb_id.clone();
            }
        }

        let margin = nearest_neighbour_distance - target_distance;

        // Stratify
        let stratum = if target_distance < 0.15 && margin > 0.10 {
            PreScreenStratum::ClearMatch { distance: target_distance }
        } else if nearest_neighbour_distance < target_distance {
            PreScreenStratum::NeighbourPreferred {
                preferred_verb: nearest_neighbour_verb.clone(),
                preferred_distance: nearest_neighbour_distance,
            }
        } else if margin.abs() < 0.08 {
            PreScreenStratum::BoundaryCase { margin }
        } else if target_distance > 0.40 {
            PreScreenStratum::ClearNonMatch { distance: target_distance }
        } else {
            PreScreenStratum::BoundaryCase { margin }
        };

        results.push(EmbeddingPreScreen {
            utterance: utterance.text.clone(),
            target_verb_distance: target_distance,
            nearest_neighbour_distance,
            nearest_neighbour_verb,
            margin,
            stratum,
        });
    }

    Ok(results)
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - (dot / (norm_a * norm_b))
}
```

#### Task 3.3: Generate, pre-screen, and persist

For each scenario: call `generate_utterance_family()`, call `pre_screen_utterances()`, insert into `calibration_utterances` with `lifecycle_status = 'Generated'`, `pre_screen` as JSONB, and `pre_screen_stratum` as the enum variant name.

**Verification:** Each scenario has 15–25 generated utterances. `SELECT scenario_id, calibration_mode, COUNT(*) FROM "ob-poc".calibration_utterances GROUP BY scenario_id, calibration_mode;`

→ IMMEDIATELY proceed to Phase 4. Progress: 45%.

---

### Phase 4: Pipeline Execution Harness & Outcome Classifier

**Goal:** Build the harness that runs utterances through the production pipeline and classifies outcomes.

#### Task 4.1: Calibration fixture entities

**File:** `rust/src/calibration/fixtures.rs`

The harness needs entities in known states to provide context for synthetic utterances. Use dedicated calibration fixture entities — pre-seeded test data entities maintained in known states by the harness.

```rust
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Calibration fixture entity set.
/// These are real entities in the DB with controlled states.
/// The harness creates them once and resets states between runs.
pub struct CalibrationFixtures {
    pub session_id: Uuid,
    pub entities: HashMap<String, FixtureEntity>,
}

pub struct FixtureEntity {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub current_state: String,
}

/// Ensure calibration fixture entities exist and are in the required states.
/// Creates them if missing, resets states if they've drifted.
pub async fn ensure_fixtures(
    pool: &PgPool,
    scenario: &CalibrationScenario,
) -> Result<CalibrationFixtures> {
    // 1. Check if fixture entities exist for this constellation template
    //    Convention: fixture entities have names like "CAL_FIXTURE_{template_id}_{entity_type}"

    // 2. If missing, create them:
    //    - Create the target entity in the specified state
    //    - Create linked entities in their specified states
    //    - Create structure links between them

    // 3. If states have drifted, reset them:
    //    - UPDATE entity SET lifecycle_state = $target WHERE id = $fixture_id

    // 4. Create a calibration session
    //    - Session context pointing at the fixture entity
    //    - So that referential utterances ("freeze it") have an antecedent

    todo!("Wire to actual entity creation/update functions in the repo")
}

/// Reset fixture states after a calibration run.
/// Ensures the next run starts from a clean baseline.
pub async fn reset_fixtures(
    pool: &PgPool,
    fixtures: &CalibrationFixtures,
    scenario: &CalibrationScenario,
) -> Result<()> {
    // Reset each entity to its scenario-defined state
    todo!("Wire to actual entity state reset functions")
}
```

#### Task 4.2: Pipeline execution function

**File:** `rust/src/calibration/harness.rs`

```rust
use crate::calibration::types::*;
use crate::calibration::fixtures::CalibrationFixtures;
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

/// Execute a single utterance through the PRODUCTION pipeline
/// and return the trace ID.
///
/// CRITICAL: This must call the SAME entry point as a real user utterance.
/// No simplified shims, no test-only code paths.
/// The value of calibration comes from exercising the real pipeline.
pub async fn execute_calibration_utterance(
    pool: &PgPool,
    fixtures: &CalibrationFixtures,
    utterance_text: &str,
) -> Result<Uuid> {
    // 1. Construct the pipeline input:
    //    - session_id from fixtures
    //    - utterance text
    //    - entity context from fixtures
    //    - is_synthetic = true

    // 2. Call the ACTUAL production pipeline entry point
    //    This is likely something like:
    //    handle_utterance(&orch_ctx, utterance_text).await
    //
    //    Find the actual entry point in the repo and call it.
    //    DO NOT reimplement the pipeline.

    // 3. The pipeline persists an UtteranceTrace.
    //    Use the returned OrchestratorOutcome.trace_id rather than polling "most recent".

    // 4. Return the trace_id

    todo!("Wire to actual pipeline entry point — find handle_utterance() or equivalent")
}

/// Load an UtteranceTrace from the DB by trace_id.
pub async fn load_trace(pool: &PgPool, trace_id: Uuid) -> Result<crate::traceability::UtteranceTraceRecord> {
    let repo = crate::traceability::UtteranceTraceRepository::new(pool.clone());
    repo.get(trace_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("trace not found: {}", trace_id))
}
```

#### Task 4.3: Outcome classifier — full implementation

**File:** `rust/src/calibration/classifier.rs`

```rust
use crate::calibration::types::*;
use anyhow::Result;

/// Classify a calibration outcome from a trace against the expected outcome.
/// Handles ALL seven ExpectedOutcome variants.
pub fn classify_outcome(
    trace: &crate::traceability::UtteranceTraceRecord,
    utterance: &CalibrationUtteranceRow,
    scenario: &CalibrationScenario,
) -> CalibrationOutcome {
    let actual_verb = trace.resolved_verb.clone();
    let actual_halt = trace.halt_reason_code.clone();
    let actual_outcome = trace.outcome;
    let halt_phase = trace.halt_phase.map(|p| p as u8);
    let fallback_invoked = trace.fallback_invoked;

    // Trace payloads currently expose a single Phase 4 confidence scalar and
    // alternative verb IDs, but not per-alternative scores. In v1 calibration,
    // margin_stable should default to the pre-screen margin unless richer Phase 4
    // ranking telemetry is added.
    let top1_score = trace.trace_payload.pointer("/phase_4/confidence")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let top2_score = trace.trace_payload.pointer("/phase_4/alternative_verbs/0/score")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let margin = match (top1_score, top2_score) {
        (Some(t1), Some(t2)) => Some(t1 - t2),
        _ => None,
    };

    // Classify verdict based on expected outcome
    let expected: ExpectedOutcome = serde_json::from_value(
        utterance.expected_outcome.clone()
    ).unwrap();

    let verdict = match &expected {
        ExpectedOutcome::ResolvesTo(target) => {
            classify_resolves_to(target, &actual_verb, &actual_halt, margin, scenario)
        }
        ExpectedOutcome::ResolvesToOneOf(targets) => {
            match &actual_verb {
                Some(v) if targets.contains(v) => {
                    check_margin_stability(margin, scenario)
                }
                Some(v) => CalibrationVerdict::WrongVerb {
                    expected: targets.join("|"),
                    actual: v.clone(),
                },
                None => CalibrationVerdict::FalseNegative {
                    expected: targets.join("|"),
                    actual_halt: actual_halt.clone().unwrap_or_default(),
                },
            }
        }
        ExpectedOutcome::HaltsWithReason(expected_halt) => {
            match &actual_verb {
                Some(v) => CalibrationVerdict::FalsePositive {
                    unexpected_verb: v.clone(),
                    expected_halt: expected_halt.clone(),
                },
                None => {
                    if halt_reason_matches(&actual_halt, expected_halt) {
                        CalibrationVerdict::Pass
                    } else {
                        CalibrationVerdict::CorrectPhaseWrongReason {
                            expected: expected_halt.clone(),
                            actual: actual_halt.clone().unwrap_or_default(),
                        }
                    }
                }
            }
        }
        ExpectedOutcome::HaltsAtPhase(expected_phase) => {
            match halt_phase {
                Some(p) if p == *expected_phase => CalibrationVerdict::Pass,
                Some(p) => CalibrationVerdict::WrongPhase {
                    expected_phase: *expected_phase,
                    actual_phase: p,
                },
                None if actual_verb.is_some() => CalibrationVerdict::FalsePositive {
                    unexpected_verb: actual_verb.clone().unwrap(),
                    expected_halt: ExpectedHaltReason::NoViableVerb,
                },
                None => CalibrationVerdict::Pass, // halted, just don't know the phase
            }
        }
        ExpectedOutcome::TriggersClarification => {
            if actual_outcome == crate::traceability::TraceOutcome::ClarificationTriggered {
                CalibrationVerdict::Pass
            } else if actual_verb.is_some() {
                CalibrationVerdict::FalsePositive {
                    unexpected_verb: actual_verb.clone().unwrap(),
                    expected_halt: ExpectedHaltReason::AmbiguousResolution,
                }
            } else {
                CalibrationVerdict::CorrectPhaseWrongReason {
                    expected: ExpectedHaltReason::AmbiguousResolution,
                    actual: actual_halt.clone().unwrap_or_default(),
                }
            }
        }
        ExpectedOutcome::FallsToSage => {
            // "Falls to Sage" means no DSL verb resolved and the system
            // handled it conversationally. Check that no verb resolved.
            match &actual_verb {
                Some(v) => CalibrationVerdict::FalsePositive {
                    unexpected_verb: v.clone(),
                    expected_halt: ExpectedHaltReason::NoParsableIntent,
                },
                None => CalibrationVerdict::Pass,
            }
        }
    };

    if actual_outcome == crate::traceability::TraceOutcome::InProgress {
        return CalibrationOutcome {
            utterance_id: utterance.utterance_id,
            utterance_text: utterance.text.clone(),
            calibration_mode: serde_json::from_str(&utterance.calibration_mode).unwrap_or(CalibrationMode::Positive),
            negative_type: utterance.negative_type.as_ref().map(|n| serde_json::from_str(n).unwrap_or(NegativeType::TypeA)),
            pre_screen: utterance.pre_screen.as_ref().map(|p| serde_json::from_value(p.clone()).unwrap()),
            expected_outcome: expected,
            trace_id: trace.trace_id,
            actual_resolved_verb: actual_verb,
            actual_halt_reason: actual_halt,
            verdict: CalibrationVerdict::FalseNegative {
                expected: "trace_incomplete".to_string(),
                actual_halt: "in_progress".to_string(),
            },
            failure_phase: halt_phase,
            failure_detail: Some(serde_json::json!({"error": "trace still in_progress"})),
            top1_score,
            top2_score,
            margin,
            margin_stable: None,
            latency_total_ms: extract_latency_ms(trace),
            latency_per_phase: extract_per_phase_latency(trace),
        };
    }

    // Check for unnecessary fallback on Pass verdicts
    let final_verdict = if matches!(verdict, CalibrationVerdict::Pass) && fallback_invoked {
        CalibrationVerdict::UnnecessaryFallback
    } else {
        verdict
    };

    CalibrationOutcome {
        utterance_id: utterance.utterance_id,
        utterance_text: utterance.text.clone(),
        calibration_mode: serde_json::from_str(&utterance.calibration_mode).unwrap_or(CalibrationMode::Positive),
        negative_type: utterance.negative_type.as_ref().map(|n| serde_json::from_str(n).unwrap_or(NegativeType::TypeA)),
        pre_screen: utterance.pre_screen.as_ref().map(|p| serde_json::from_value(p.clone()).unwrap()),
        expected_outcome: expected,
        trace_id: trace.trace_id,
        actual_resolved_verb: actual_verb,
        actual_halt_reason: actual_halt,
        verdict: final_verdict,
        failure_phase: halt_phase,
        failure_detail: None,
        top1_score,
        top2_score,
        margin,
        margin_stable: margin.map(|m| m >= scenario.expected_margin_threshold),
        latency_total_ms: extract_latency_ms(trace),
        latency_per_phase: extract_per_phase_latency(trace),
    }
}

fn classify_resolves_to(
    target: &str, actual_verb: &Option<String>, actual_halt: &Option<String>,
    margin: Option<f32>, scenario: &CalibrationScenario,
) -> CalibrationVerdict {
    match actual_verb {
        Some(v) if v == target => check_margin_stability(margin, scenario),
        Some(v) => CalibrationVerdict::WrongVerb {
            expected: target.to_string(),
            actual: v.clone(),
        },
        None => CalibrationVerdict::FalseNegative {
            expected: target.to_string(),
            actual_halt: actual_halt.clone().unwrap_or_default(),
        },
    }
}

fn check_margin_stability(margin: Option<f32>, scenario: &CalibrationScenario) -> CalibrationVerdict {
    match margin {
        Some(m) if m < scenario.expected_margin_threshold => {
            CalibrationVerdict::PassWithFragileMargin {
                margin: m,
                threshold: scenario.expected_margin_threshold,
            }
        }
        _ => CalibrationVerdict::Pass,
    }
}

fn halt_reason_matches(actual: &Option<String>, expected: &ExpectedHaltReason) -> bool {
    let expected_str = format!("{:?}", expected);
    actual.as_ref().map(|a| a == &expected_str).unwrap_or(false)
}

fn extract_latency_ms(trace: &crate::traceability::UtteranceTraceRecord) -> Option<i64> {
    // Phase 5 currently exposes execution_start/execution_end placeholders, but
    // trace-level total latency is not yet a first-class persisted field.
    None // TODO: wire to actual trace timestamp fields
}

fn extract_per_phase_latency(trace: &crate::traceability::UtteranceTraceRecord) -> Option<Vec<(u8, i64)>> {
    None // TODO: wire to actual per-phase timing if available in trace
}

// Placeholder for DB row type — adjust to actual sqlx model
pub struct CalibrationUtteranceRow {
    pub utterance_id: uuid::Uuid,
    pub text: String,
    pub calibration_mode: String,
    pub negative_type: Option<String>,
    pub expected_outcome: serde_json::Value,
    pub pre_screen: Option<serde_json::Value>,
    pub trace_id_placeholder: uuid::Uuid,
}
```

#### Task 4.4: Metrics computation

**File:** `rust/src/calibration/metrics.rs`

```rust
use crate::calibration::types::*;

pub fn compute_metrics(
    outcomes: &[CalibrationOutcome],
    scenario: &CalibrationScenario,
) -> CalibrationMetrics {
    let positive: Vec<_> = outcomes.iter().filter(|o| o.calibration_mode == CalibrationMode::Positive).collect();
    let negative_a: Vec<_> = outcomes.iter().filter(|o| o.negative_type == Some(NegativeType::TypeA)).collect();
    let negative_b: Vec<_> = outcomes.iter().filter(|o| o.negative_type == Some(NegativeType::TypeB)).collect();
    let boundary: Vec<_> = outcomes.iter().filter(|o| o.calibration_mode == CalibrationMode::Boundary).collect();

    let positive_pass = positive.iter().filter(|o| matches!(o.verdict,
        CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
    )).count();
    let neg_a_pass = negative_a.iter().filter(|o| matches!(o.verdict, CalibrationVerdict::Pass)).count();
    let neg_b_pass = negative_b.iter().filter(|o| matches!(o.verdict, CalibrationVerdict::Pass)).count();
    let boundary_pass = boundary.iter().filter(|o| matches!(o.verdict,
        CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
    )).count();

    let total = outcomes.len();
    let total_pass = outcomes.iter().filter(|o| matches!(o.verdict,
        CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }
    )).count();

    // Fallback rate
    let fallback_count = outcomes.iter().filter(|o| matches!(o.verdict, CalibrationVerdict::UnnecessaryFallback)).count();

    // Fragile margins
    let fragile = outcomes.iter().filter(|o| matches!(o.verdict, CalibrationVerdict::PassWithFragileMargin { .. })).count();

    // Average margin (for outcomes that have margin data)
    let margins: Vec<f32> = outcomes.iter().filter_map(|o| o.margin).collect();
    let avg_margin = if margins.is_empty() { 0.0 } else { margins.iter().sum::<f32>() / margins.len() as f32 };

    // Margin histogram (buckets: 0.00-0.05, 0.05-0.10, 0.10-0.15, 0.15-0.20, 0.20+)
    let buckets = vec![0.05, 0.10, 0.15, 0.20];
    let mut histogram = vec![(0.05_f32, 0_usize), (0.10, 0), (0.15, 0), (0.20, 0), (1.0, 0)];
    for m in &margins {
        for (i, &threshold) in buckets.iter().enumerate() {
            if *m < threshold {
                histogram[i].1 += 1;
                break;
            }
        }
        if *m >= 0.20 { histogram[4].1 += 1; }
    }

    // Phase 3 overprune rate: outcomes where correct verb was available in Phase 2
    // but ECIR eliminated it (verdict = FalseNegative and phase = 3)
    let phase3_overprune = outcomes.iter().filter(|o|
        matches!(o.verdict, CalibrationVerdict::FalseNegative { .. }) && o.failure_phase == Some(3)
    ).count();

    // Latency
    let latencies: Vec<f32> = outcomes.iter().filter_map(|o| o.latency_total_ms.map(|l| l as f32)).collect();
    let avg_latency = if latencies.is_empty() { 0.0 } else { latencies.iter().sum::<f32>() / latencies.len() as f32 };
    let p95_latency = if latencies.is_empty() { 0.0 } else {
        let mut sorted = latencies.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted[(sorted.len() as f32 * 0.95) as usize]
    };

    CalibrationMetrics {
        positive_hit_rate: if positive.is_empty() { 0.0 } else { positive_pass as f32 / positive.len() as f32 },
        negative_type_a_rejection_rate: if negative_a.is_empty() { 0.0 } else { neg_a_pass as f32 / negative_a.len() as f32 },
        negative_type_b_rejection_rate: if negative_b.is_empty() { 0.0 } else { neg_b_pass as f32 / negative_b.len() as f32 },
        boundary_correct_rate: if boundary.is_empty() { 0.0 } else { boundary_pass as f32 / boundary.len() as f32 },
        overall_accuracy: if total == 0 { 0.0 } else { total_pass as f32 / total as f32 },
        phase2_legality_compliance: 1.0, // TODO: compute from trace Phase 2 data
        phase3_overprune_rate: if total == 0 { 0.0 } else { phase3_overprune as f32 / total as f32 },
        phase3_candidate_set_avg: 0.0, // TODO: extract from trace ECIR data
        phase4_fallback_rate: if total == 0 { 0.0 } else { fallback_count as f32 / total as f32 },
        phase4_avg_margin: avg_margin,
        fragile_boundary_count: fragile,
        margin_histogram: histogram,
        constellation_recovery_rate: 1.0, // TODO: compute from trace Phase 2 data
        dag1_predicate_accuracy: 1.0, // TODO: compute from trace Phase 2 predicate data
        avg_total_latency_ms: avg_latency,
        avg_phase_latency_ms: [0.0; 6], // TODO: aggregate per-phase latency
        p95_total_latency_ms: p95_latency,
        plan_compilation_success_rate: None, // populated for CrossEntityPlan scenarios
        exclusion_enforcement_accuracy: None,
    }
}
```

#### Task 4.5: Run orchestrator

**File:** `rust/src/calibration/runner.rs`

```rust
use crate::calibration::types::*;
use crate::calibration::fixtures;
use crate::calibration::harness;
use crate::calibration::classifier;
use crate::calibration::metrics;
use crate::calibration::drift;
use crate::calibration::db;
use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn execute_calibration_run(
    pool: &PgPool,
    scenario_id: Uuid,
    triggered_by: &str,
) -> Result<CalibrationRun> {
    let run_start = Utc::now();

    // 1. Load scenario
    let scenario = db::load_scenario(pool, scenario_id).await?;

    // 2. Load admitted utterances
    let utterances = db::load_admitted_utterances(pool, scenario_id).await?;
    anyhow::ensure!(!utterances.is_empty(), "No admitted utterances for scenario {}", scenario_id);

    // 3. Ensure fixture entities are in correct states
    let fixture_context = fixtures::ensure_fixtures(pool, &scenario).await?;

    // 4. Capture current SurfaceVersions
    let versions = db::capture_surface_versions(pool).await?;

    // 5. Find prior run for drift detection
    let prior_run = db::find_most_recent_run(pool, scenario_id).await?;

    // 6. Execute each utterance through the production pipeline
    let mut outcomes = Vec::new();
    let mut trace_ids = Vec::new();

    for utterance in &utterances {
        // Reset fixture state between utterances to prevent cross-contamination
        // (only for mutation verbs — observation verbs don't change state)
        if scenario.verb_taxonomy_tag == "StateVerb" {
            fixtures::reset_fixtures(pool, &fixture_context, &scenario).await?;
        }

        let trace_id = harness::execute_calibration_utterance(
            pool, &fixture_context, &utterance.text
        ).await?;

        let trace = harness::load_trace(pool, trace_id).await?;

        let mut outcome = classifier::classify_outcome(&trace, utterance, &scenario);
        outcome.trace_id = trace_id;
        trace_ids.push(trace_id);
        outcomes.push(outcome);
    }

    // 7. Compute aggregate metrics
    let run_metrics = metrics::compute_metrics(&outcomes, &scenario);

    // 8. Compute drift if prior run exists
    let run_drift = if let Some(ref prior) = prior_run {
        Some(drift::compute_drift(prior, &outcomes, &run_metrics, &versions))
    } else {
        None
    };

    // 9. Assemble and persist the run
    let run = CalibrationRun {
        run_id: Uuid::new_v4(),
        scenario_id,
        triggered_by: triggered_by.to_string(),
        run_start,
        run_end: Some(Utc::now()),
        surface_versions: serde_json::to_value(&versions)?,
        utterance_count: utterances.len(),
        positive_count: outcomes.iter().filter(|o| o.calibration_mode == CalibrationMode::Positive).count(),
        negative_count: outcomes.iter().filter(|o| o.calibration_mode == CalibrationMode::Negative).count(),
        boundary_count: outcomes.iter().filter(|o| o.calibration_mode == CalibrationMode::Boundary).count(),
        metrics: run_metrics,
        outcomes: outcomes.clone(),
        prior_run_id: prior_run.map(|r| r.run_id),
        drift: run_drift,
        trace_ids,
    };

    db::persist_run(pool, &run).await?;
    db::persist_outcomes(pool, &run).await?;

    // 10. Reset fixtures to clean state
    fixtures::reset_fixtures(pool, &fixture_context, &scenario).await?;

    Ok(run)
}
```

**Verification:** Run one scenario end-to-end. Check:
- `calibration_runs` has 1 row
- `calibration_outcomes` has N rows
- `utterance_traces` has N rows with `is_synthetic = true`
- At least one non-Pass verdict exists

→ IMMEDIATELY proceed to Phase 5. Progress: 65%.

---

### Phase 5: Curation Workflow

**Goal:** CLI for corpus management and admission criteria enforcement.

#### Task 5.1: Build CLI

**File:** `rust/src/bin/calibration_cli.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "calibration")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate utterance family for a scenario
    Generate { scenario_id: String },
    /// Pre-screen generated utterances via BGE embeddings
    PreScreen { scenario_id: String },
    /// List pending utterances awaiting review
    ListPending { scenario_id: String },
    /// Review an utterance (admit or reject)
    Review {
        utterance_id: String,
        #[arg(long)]
        admit: bool,
        #[arg(long)]
        reject: bool,
        #[arg(long)]
        reviewer: Option<String>,
    },
    /// Execute a full calibration run
    Run { scenario_id: String },
    /// Print run metrics and drift flags
    Report { run_id: String },
    /// Compare last two runs for a scenario
    Drift { scenario_id: String },
    /// Summary of all scenarios and their last run metrics
    Portfolio,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let pool = /* connect to DB — use same connection logic as the main server */;

    match cli.command {
        Commands::Generate { scenario_id } => {
            let id = uuid::Uuid::parse_str(&scenario_id)?;
            let scenario = db::load_scenario(&pool, id).await?;
            let utterances = generator::generate_utterance_family(&scenario).await?;
            db::persist_generated_utterances(&pool, id, &utterances).await?;
            eprintln!("Generated {} utterances for scenario {}", utterances.len(), scenario_id);
        }
        Commands::PreScreen { scenario_id } => {
            let id = uuid::Uuid::parse_str(&scenario_id)?;
            let scenario = db::load_scenario(&pool, id).await?;
            let utterances = db::load_utterances_for_screening(&pool, id).await?;
            let embedder = CandleEmbedder::new()?;
            let screens = pre_screen::pre_screen_utterances(&utterances, &scenario, &embedder).await?;
            db::update_pre_screens(&pool, &screens).await?;
            eprintln!("Pre-screened {} utterances", screens.len());
        }
        Commands::Review { utterance_id, admit, reject, reviewer } => {
            let id = uuid::Uuid::parse_str(&utterance_id)?;
            if admit {
                db::admit_utterance(&pool, id, reviewer.as_deref().unwrap_or("unknown")).await?;
            } else if reject {
                db::reject_utterance(&pool, id).await?;
            }
        }
        Commands::Run { scenario_id } => {
            let id = uuid::Uuid::parse_str(&scenario_id)?;
            let run = runner::execute_calibration_run(&pool, id, "cli").await?;
            eprintln!("Run complete: {}", run.run_id);
            report::print_run_report(&run);
        }
        Commands::Report { run_id } => {
            let id = uuid::Uuid::parse_str(&run_id)?;
            let run = db::load_run(&pool, id).await?;
            report::print_run_report(&run);
        }
        Commands::Drift { scenario_id } => {
            let id = uuid::Uuid::parse_str(&scenario_id)?;
            let runs = db::load_last_two_runs(&pool, id).await?;
            if runs.len() == 2 {
                report::print_drift_report(&runs[0], &runs[1]);
            } else {
                eprintln!("Need at least 2 runs for drift comparison");
            }
        }
        Commands::Portfolio => {
            let summary = portfolio::build_portfolio_summary(&pool).await?;
            portfolio::print_portfolio(&summary);
        }
    }

    Ok(())
}
```

#### Task 5.2: Admission criteria checks

**File:** `rust/src/calibration/admission.rs`

```rust
use crate::calibration::types::*;

pub fn check_admission_criteria(
    utterance: &GeneratedUtterance,
    pre_screen: Option<&EmbeddingPreScreen>,
    existing_admitted: &[String],  // texts of already-admitted utterances
) -> AdmissionCheckResult {
    let mut warnings = Vec::new();

    // 1. Expected outcome must be specific
    match &utterance.expected_outcome {
        ExpectedOutcome::HaltsAtPhase(_) => {
            warnings.push("Expected outcome is phase-only — reviewer should specify exact halt reason".to_string());
        }
        _ => {}
    }

    // 2. Pre-screen must be present
    if pre_screen.is_none() {
        warnings.push("No embedding pre-screen — run pre-screening first".to_string());
    }

    // 3. Negative utterances must specify type
    if utterance.calibration_mode == CalibrationMode::Negative && utterance.negative_type.is_none() {
        warnings.push("Negative utterance must specify TypeA or TypeB".to_string());
    }

    // 4. Boundary utterances should actually be boundary cases in pre-screen
    if utterance.calibration_mode == CalibrationMode::Boundary {
        if let Some(ps) = pre_screen {
            if !matches!(ps.stratum, PreScreenStratum::BoundaryCase { .. }) {
                warnings.push("Boundary utterance but pre-screen says it's not a boundary case".to_string());
            }
        }
    }

    // 5. Check for near-duplicates
    for existing in existing_admitted {
        // Simple check: exact prefix match or very high string overlap
        if strsim::jaro_winkler(&utterance.text, existing) > 0.90 {
            warnings.push(format!("Very similar to existing admitted utterance: '{}'", existing));
            break;
        }
    }

    AdmissionCheckResult {
        pass: warnings.is_empty(),
        warnings,
    }
}
```

Add `strsim` to `Cargo.toml`: `strsim = "0.11"`

**Verification:** CLI can generate, pre-screen, review, admit, and run a scenario.

→ IMMEDIATELY proceed to Phase 6. Progress: 80%.

---

### Phase 6: Drift Detection & Loop Integration

**Goal:** Drift comparison between runs, Loop 1 gap entries, Loop 2 clarification suggestions.

#### Task 6.1: Drift comparator

**File:** `rust/src/calibration/drift.rs`

```rust
use crate::calibration::types::*;

pub fn compute_drift(
    prior_run: &CalibrationRun,
    current_outcomes: &[CalibrationOutcome],
    current_metrics: &CalibrationMetrics,
    current_versions: &serde_json::Value,
) -> CalibrationDrift {
    let version_deltas = diff_versions(&prior_run.surface_versions, current_versions);

    let hit_delta = current_metrics.positive_hit_rate - prior_run.metrics.positive_hit_rate;
    let neg_delta = current_metrics.negative_type_a_rejection_rate - prior_run.metrics.negative_type_a_rejection_rate;
    let fallback_delta = current_metrics.phase4_fallback_rate - prior_run.metrics.phase4_fallback_rate;
    let margin_delta = current_metrics.phase4_avg_margin - prior_run.metrics.phase4_avg_margin;

    // Find changed verdicts
    let mut newly_failing = Vec::new();
    let mut newly_passing = Vec::new();
    let mut margin_degraded = Vec::new();

    for current in current_outcomes {
        if let Some(prior) = prior_run.outcomes.iter().find(|p| p.utterance_id == current.utterance_id) {
            let prior_pass = matches!(prior.verdict, CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. });
            let current_pass = matches!(current.verdict, CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. });

            if prior_pass && !current_pass {
                newly_failing.push(make_drifted(current, prior));
            } else if !prior_pass && current_pass {
                newly_passing.push(make_drifted(current, prior));
            }

            // Margin degradation
            if let (Some(pm), Some(cm)) = (prior.margin, current.margin) {
                if cm < pm - 0.03 {  // margin dropped by more than 0.03
                    margin_degraded.push(make_drifted(current, prior));
                }
            }
        }
    }

    // Compute flags
    let mut flags = Vec::new();
    if hit_delta < -0.05 { flags.push(DriftFlag::HitRateRegression { delta: hit_delta }); }
    if fallback_delta > 0.03 { flags.push(DriftFlag::FallbackRateIncrease { delta: fallback_delta }); }
    if margin_delta < -0.02 {
        flags.push(DriftFlag::MarginDegradation {
            avg_delta: margin_delta,
            fragile_count_delta: current_metrics.fragile_boundary_count as i32 - prior_run.metrics.fragile_boundary_count as i32,
        });
    }
    if !newly_failing.is_empty() {
        let fp_count = newly_failing.iter().filter(|d| d.current_verdict.contains("FalsePositive")).count();
        let fn_count = newly_failing.len() - fp_count;
        if fp_count > 0 { flags.push(DriftFlag::NewFalsePositives { count: fp_count }); }
        if fn_count > 0 { flags.push(DriftFlag::NewFalseNegatives { count: fn_count }); }
    }

    CalibrationDrift {
        prior_run_id: prior_run.run_id,
        current_run_id: uuid::Uuid::nil(), // set by caller
        version_deltas,
        positive_hit_rate_delta: hit_delta,
        negative_rejection_rate_delta: neg_delta,
        fallback_rate_delta: fallback_delta,
        avg_margin_delta: margin_delta,
        newly_failing_utterances: newly_failing,
        newly_passing_utterances: newly_passing,
        margin_degraded_utterances: margin_degraded,
        drift_flags: flags,
    }
}

fn make_drifted(current: &CalibrationOutcome, prior: &CalibrationOutcome) -> DriftedUtterance {
    DriftedUtterance {
        utterance_id: current.utterance_id,
        utterance_text: current.utterance_text.clone(),
        prior_verdict: format!("{:?}", prior.verdict),
        current_verdict: format!("{:?}", current.verdict),
        prior_resolved_verb: prior.actual_resolved_verb.clone(),
        current_resolved_verb: current.actual_resolved_verb.clone(),
        prior_margin: prior.margin,
        current_margin: current.margin,
    }
}

fn diff_versions(prior: &serde_json::Value, current: &serde_json::Value) -> Vec<String> {
    let mut deltas = Vec::new();
    if let (Some(p), Some(c)) = (prior.as_object(), current.as_object()) {
        for (key, prior_val) in p {
            if let Some(current_val) = c.get(key) {
                if prior_val != current_val {
                    deltas.push(format!("{}: {} → {}", key, prior_val, current_val));
                }
            }
        }
    }
    deltas
}
```

#### Task 6.2: Loop 1 — proposed gap entries

**File:** `rust/src/calibration/loop1_integration.rs`

```rust
use crate::calibration::types::*;
use serde::Serialize;

#[derive(Serialize)]
pub struct ProposedGapEntry {
    pub code: String,
    pub source: String,
    pub utterance: String,
    pub entity_type: String,
    pub entity_state: String,
    pub nearest_verb: Option<String>,
    pub status: String,
}

/// Generate PROPOSED gap entries from false-negative calibration outcomes.
/// These enter the remediation pipeline as candidates for review — NOT as admitted corrections.
pub fn generate_proposed_gaps(
    run: &CalibrationRun,
    outcomes: &[CalibrationOutcome],
) -> Vec<ProposedGapEntry> {
    outcomes.iter()
        .filter(|o| matches!(o.verdict, CalibrationVerdict::FalseNegative { .. }))
        .filter(|o| o.actual_halt_reason.as_deref() == Some("NoViableVerb"))
        .map(|o| ProposedGapEntry {
            code: "GAP".to_string(),
            source: format!("calibration:{}:{}", run.run_id, o.trace_id),
            utterance: o.utterance_text.clone(),
            entity_type: run.outcomes.first().map(|x| x.utterance_text.clone()).unwrap_or_default(), // TODO: get from scenario
            entity_state: String::new(), // TODO: get from scenario
            nearest_verb: o.actual_resolved_verb.clone(),
            status: "proposed".to_string(),
        })
        .collect()
}
```

#### Task 6.3: Loop 2 — suggested clarification prompts

**File:** `rust/src/calibration/loop2_integration.rs`

```rust
use crate::calibration::types::*;

pub struct SuggestedClarification {
    pub trigger_phrase: String,
    pub verb_a: String,
    pub verb_b: String,
    pub suggested_prompt: String,
}

/// Generate suggested clarification prompts from fragile-margin boundary outcomes.
pub fn generate_suggested_clarifications(
    scenario: &CalibrationScenario,
    outcomes: &[CalibrationOutcome],
) -> Vec<SuggestedClarification> {
    outcomes.iter()
        .filter(|o| matches!(o.verdict, CalibrationVerdict::PassWithFragileMargin { .. }))
        .filter_map(|o| {
            // Find the nearest-neighbour verb from pre-screen data
            let neighbour = o.pre_screen.as_ref()
                .map(|ps| ps.nearest_neighbour_verb.clone())
                .unwrap_or_else(|| "unknown".to_string());

            Some(SuggestedClarification {
                trigger_phrase: o.utterance_text.clone(),
                verb_a: scenario.target_verb.clone(),
                verb_b: neighbour.clone(),
                suggested_prompt: format!(
                    "Did you mean '{}' or '{}'? The phrase '{}' could match either.",
                    scenario.target_verb, neighbour, o.utterance_text
                ),
            })
        })
        .collect()
}
```

#### Task 6.4: Verify Loop 3 exclusion

Verify that any production-learning aggregation over utterance traces explicitly excludes synthetic traces.

If a source-trace materialization table exists, assert that no synthetic trace IDs appear in it. If no such table exists yet, treat this as a code-review gate on the aggregation SQL itself rather than a runnable query.

```sql
-- Example verification when a source-trace materialization exists:
SELECT COUNT(*)
FROM "ob-poc".utterance_traces
WHERE is_synthetic = true
  AND trace_id IN (
    SELECT DISTINCT trace_id FROM "ob-poc".some_learning_source_trace_table
  );
```

If the Loop 3 aggregation doesn't have a source trace table, verify the aggregation query includes `WHERE is_synthetic = false`.

→ IMMEDIATELY proceed to Phase 7. Progress: 92%.

---

### Phase 7: Reports & CI Integration

**Goal:** Human-readable reports and CI binary.

#### Task 7.1: Run report

**File:** `rust/src/calibration/report.rs`

```rust
use crate::calibration::types::*;

pub fn print_run_report(run: &CalibrationRun) {
    println!("# Calibration Report: {}", run.scenario_id);
    println!("Run: {} | Triggered: {} | Utterances: {}", run.run_start, run.triggered_by, run.utterance_count);
    println!();

    println!("## Metrics");
    println!("  Positive hit rate:     {:.1}%", run.metrics.positive_hit_rate * 100.0);
    println!("  Negative Type A:       {:.1}%", run.metrics.negative_type_a_rejection_rate * 100.0);
    println!("  Negative Type B:       {:.1}%", run.metrics.negative_type_b_rejection_rate * 100.0);
    println!("  Boundary correct:      {:.1}%", run.metrics.boundary_correct_rate * 100.0);
    println!("  Fragile margins:       {}", run.metrics.fragile_boundary_count);
    println!("  Fallback rate:         {:.1}%", run.metrics.phase4_fallback_rate * 100.0);
    println!("  Avg margin:            {:.3}", run.metrics.phase4_avg_margin);
    println!("  Avg latency:           {:.0}ms", run.metrics.avg_total_latency_ms);
    println!();

    // Drift flags
    if let Some(ref drift) = run.drift {
        if !drift.drift_flags.is_empty() {
            println!("## Drift Flags");
            for flag in &drift.drift_flags {
                println!("  ⚠️  {:?}", flag);
            }
            println!();
        }

        if !drift.newly_failing_utterances.is_empty() {
            println!("## Newly Failing");
            for u in &drift.newly_failing_utterances {
                println!("  '{}' → was {:?}, now {:?}", u.utterance_text, u.prior_verdict, u.current_verdict);
            }
            println!();
        }
    }

    // Failures
    let failures: Vec<_> = run.outcomes.iter()
        .filter(|o| !matches!(o.verdict, CalibrationVerdict::Pass | CalibrationVerdict::PassWithFragileMargin { .. }))
        .collect();
    if !failures.is_empty() {
        println!("## Failures");
        for o in &failures {
            println!("  '{}' → {:?}", o.utterance_text, o.verdict);
            if let Some(phase) = o.failure_phase {
                println!("    Phase: {}, Halt: {:?}", phase, o.actual_halt_reason);
            }
        }
        println!();
    }

    // Fragile boundaries
    let fragile: Vec<_> = run.outcomes.iter()
        .filter(|o| matches!(o.verdict, CalibrationVerdict::PassWithFragileMargin { .. }))
        .collect();
    if !fragile.is_empty() {
        println!("## Fragile Boundaries");
        for o in &fragile {
            println!("  '{}' → margin {:.3} (threshold: {:.3})",
                o.utterance_text, o.margin.unwrap_or(0.0),
                match &o.verdict {
                    CalibrationVerdict::PassWithFragileMargin { threshold, .. } => *threshold,
                    _ => 0.0,
                }
            );
        }
    }
}
```

#### Task 7.2: Portfolio summary

**File:** `rust/src/calibration/portfolio.rs`

```rust
use crate::calibration::db;
use sqlx::PgPool;

pub struct PortfolioEntry {
    pub scenario_name: String,
    pub last_run: String,
    pub hit_rate: f32,
    pub neg_reject: f32,
    pub boundary: f32,
    pub fragile: usize,
    pub drift_status: String,
}

pub async fn build_portfolio_summary(pool: &PgPool) -> anyhow::Result<Vec<PortfolioEntry>> {
    // For each admitted scenario, load the most recent run and extract key metrics
    let scenarios = db::load_all_admitted_scenarios(pool).await?;
    let mut entries = Vec::new();

    for scenario in &scenarios {
        if let Some(run) = db::find_most_recent_run(pool, scenario.scenario_id).await? {
            let drift_status = match &run.drift {
                Some(d) if d.drift_flags.iter().any(|f| matches!(f, DriftFlag::HitRateRegression { .. })) => "⚠️ regression",
                Some(d) if d.drift_flags.iter().any(|f| matches!(f, DriftFlag::MarginDegradation { .. })) => "⚠️ margin",
                Some(_) => "✅ stable",
                None => "— first run",
            };

            entries.push(PortfolioEntry {
                scenario_name: scenario.scenario_name.clone(),
                last_run: format!("{}", run.run_start.format("%Y-%m-%d %H:%M")),
                hit_rate: run.metrics.positive_hit_rate,
                neg_reject: run.metrics.negative_type_a_rejection_rate,
                boundary: run.metrics.boundary_correct_rate,
                fragile: run.metrics.fragile_boundary_count,
                drift_status: drift_status.to_string(),
            });
        }
    }

    Ok(entries)
}

pub fn print_portfolio(entries: &[PortfolioEntry]) {
    println!("# Calibration Portfolio");
    println!();
    println!("| Scenario | Last Run | Hit Rate | Neg Reject | Boundary | Fragile | Drift |");
    println!("|----------|----------|----------|------------|----------|---------|-------|");
    for e in entries {
        println!("| {} | {} | {:.0}% | {:.0}% | {:.0}% | {} | {} |",
            e.scenario_name, e.last_run,
            e.hit_rate * 100.0, e.neg_reject * 100.0,
            e.boundary * 100.0, e.fragile, e.drift_status
        );
    }
}
```

#### Task 7.3: CI binary

**File:** `rust/src/bin/calibration_ci.rs`

```rust
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    all: bool,
    #[arg(long)]
    scenario: Option<String>,
    #[arg(long)]
    portfolio_report: bool,
    #[arg(long)]
    fail_on_regression: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let pool = /* connect to DB */;

    if cli.portfolio_report {
        let summary = portfolio::build_portfolio_summary(&pool).await?;
        portfolio::print_portfolio(&summary);
        return Ok(());
    }

    let scenario_ids = if cli.all {
        db::load_all_admitted_scenario_ids(&pool).await?
    } else if let Some(ref id) = cli.scenario {
        vec![uuid::Uuid::parse_str(id)?]
    } else {
        anyhow::bail!("Specify --all or --scenario <id>");
    };

    let mut any_regression = false;

    for scenario_id in &scenario_ids {
        let run = runner::execute_calibration_run(&pool, *scenario_id, "CI").await?;
        report::print_run_report(&run);

        if let Some(ref drift) = run.drift {
            if drift.drift_flags.iter().any(|f| matches!(f, DriftFlag::HitRateRegression { .. })) {
                any_regression = true;
            }
        }
    }

    if cli.fail_on_regression && any_regression {
        eprintln!("❌ Regression detected — failing CI");
        std::process::exit(1);
    }

    Ok(())
}
```

**Verification:** Full CI pipeline runs end-to-end. Portfolio report renders. `--fail-on-regression` exits 1 when a regression is detected.

→ Progress: 100%. E-invariant satisfied.

---

### Implementation File Summary

| Phase | Files Created |
|-------|-------------|
| 1 | `migrations/NNN_calibration_tables.sql`, `rust/src/calibration/types.rs`, `rust/src/calibration/mod.rs` |
| 2 | `rust/src/calibration/seed.rs`, `rust/src/calibration/db.rs` |
| 3 | `rust/src/calibration/generator.rs`, `rust/src/calibration/pre_screen.rs` |
| 4 | `rust/src/calibration/fixtures.rs`, `rust/src/calibration/harness.rs`, `rust/src/calibration/classifier.rs`, `rust/src/calibration/metrics.rs`, `rust/src/calibration/runner.rs` |
| 5 | `rust/src/bin/calibration_cli.rs`, `rust/src/calibration/admission.rs`, `rust/src/calibration/governance.rs` |
| 6 | `rust/src/calibration/drift.rs`, `rust/src/calibration/loop1_integration.rs`, `rust/src/calibration/loop2_integration.rs` |
| 7 | `rust/src/calibration/report.rs`, `rust/src/calibration/portfolio.rs`, `rust/src/bin/calibration_ci.rs` |

### Execution Rules

- **E-invariant:** Progress must reach 100%. Do not stop after Phase 1.
- **→ IMMEDIATELY proceed** to the next phase at each gate.
- **Do not commit** — Adam will review the diff first.
- Each phase has a verification step. Run it before proceeding.
- All new code goes in `rust/src/calibration/` module.
- All new binaries go in `rust/src/bin/`.
- All migrations go in `migrations/` with sequential numbering.
- Functions marked `todo!("Wire to actual...")` require finding the real entry point in the existing codebase and calling it — do NOT reimplement pipeline logic.

---

## 16. Success Measures

The first success criterion is not a headline percentage. The first success criterion is that the platform can reliably answer:

- What was the intended canonical seed?
- What utterances were tested?
- How each utterance travelled through the pipeline (which phases, which trace data)?
- Where failures occurred (which phase, which `HaltReason`)?
- What the near-neighbour margin was?
- What changed between runs (which `SurfaceVersions` differed)?
- And what remediation is suggested (which Loop 1 gaps, which Loop 2 clarifications)?

Operational measures (KPI targets — tracked, trended, improved over time):

| Metric | Target | Why |
|--------|--------|-----|
| Positive hit rate per scenario | > 90% | Baseline resolution quality |
| Negative rejection rate (Type A — resolves elsewhere) | > 95% | Discrimination quality |
| Negative rejection rate (Type B — halts/clarifies correctly) | > 90% | Boundary recognition quality |
| Boundary correct rate | > 80% | Near-neighbour discrimination |
| Fragile margin count (< 0.05) | < 10% of boundary cases | Boundary stability |
| Phase 4 fallback rate | < 5% | ECIR narrowing effectiveness |
| Drift detection latency | < 24 hours | Regressions caught within one CI cycle |

Correctness requirements (defect-severity expectations — any violation is a bug, not KPI drift):

| Requirement | Expected | Failure Treatment |
|-------------|----------|------------------|
| Constellation predicate accuracy | Correct by construction | Any non-correct result is treated as defect severity — the legality logic is wrong, not merely underperforming |
| Trace completeness | Every run produces a full `UtteranceTrace` | Missing traces invalidate the calibration run — results cannot be trusted without traceability |
| DAG 1 acyclicity | All constellation templates pass validation | A cycle in DAG 1 is a schema-level defect, not a calibration finding |
| Exclusion predicate enforcement | Exclusions never silently violated | A violated exclusion that isn't caught is a runtime safety defect |

---

## 17. Summary

Loopback Calibration extends the Semantic OS / DSL architecture from passive traceability into active semantic hardening. It gives OB-POC a governed way to:

- Generate synthetic utterance pressure against deterministic targets (positive, negative, boundary)
- Measure how the real six-phase pipeline behaves using `UtteranceTrace` records
- Localise failures to specific phases and `HaltReason` variants
- Measure near-neighbour verb boundary stability via margin analysis
- Test cross-entity execution shapes (Batch, CrossEntityPlan, exclusions, DAG ordering)
- Detect drift early via version-pinned run-over-run comparison
- Feed results into Loop 1 (gap discovery) and Loop 2 (clarification) without polluting Loop 3

Its value is not that it makes the platform "smarter." Its value is that it makes the platform's semantic boundaries visible, testable, and calibratable — using the platform's own governed metadata as the seed and its own traceability infrastructure as the diagnostic.

---

*"While others are stress-testing prompts, we are stress-testing deterministic execution boundaries. The generator doesn't define truth — the Semantic OS does. The generator just finds out where truth stops being sharp."*
