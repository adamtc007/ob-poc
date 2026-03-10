# Sage/Coder Pipeline Failure Analysis

## Abstract

The current Sage -> Coder -> DSL pipeline is underperforming at a level that is no longer plausibly explained by ordinary tuning gaps. The latest end-to-end comparative harness shows Sage+Coder resolving only `11/134` utterances correctly (`8.21%`), versus `58/134` (`43.28%`) for the existing pipeline. This paper argues that the dominant cause is multiplicative error across too many lossy semantic stages, not isolated classifier weakness.

The core claim is simple: a multi-stage pipeline with moderate per-stage fidelity collapses catastrophically when each stage compresses or distorts the user's intent before execution planning. The measured gate results are numerically consistent with that collapse. This suggests that the current strategy should be reconsidered at the level of pipeline shape, not just further tuned at the level of verb metadata, clash cleanup, or domain hints.

## 1. Background

The implemented architecture is conceptually:

1. User utterance enters the unified session input path
2. Sage classifies the utterance into an `OutcomeIntent`
3. Coder resolves a target verb from that structured intent
4. Coder assembles arguments
5. Coder generates DSL
6. Orchestrator applies serve/delegate/confirmation policy
7. DSL or runbook execution is performed by the deterministic REPL/runbook layer

This is not a model-executes-code architecture. Sage does not execute DSL. Sage infers intent and desired outcome, and Coder maps that inferred outcome into a registry-constrained DSL target.

That design is coherent in principle. However, the measured results indicate that the current realization of that design loses too much signal between stages.

## 2. Observed Results

### 2.1 End-to-End Comparative Harness

Latest head-to-head run:

- Existing pipeline: `58/134 = 43.28%`
- Sage+Coder: `11/134 = 8.21%`

Previous recent run before the latest declash work:

- Sage+Coder: `10/134 = 7.46%`

This means the recent declash refactor produced only a marginal absolute improvement of `0.75` percentage points.

### 2.2 Sage Coverage Harness

Latest Sage-only classification results:

- Plane: `169/176 = 96.0%`
- Polarity: `148/176 = 84.1%`
- Domain: `119/176 = 67.6%`

These figures show that some upstream components are respectable in isolation, especially plane and polarity. Yet end-to-end output remains extremely poor. That gap is the key signal.

## 3. Multiplicative Error Model

A chained semantic pipeline can be approximated as:

`P(success) = P(S1) * P(S2 | S1) * P(S3 | S1,S2) * ... * P(Sn | S1..Sn-1)`

If we simplify and assume each stage retains a roughly equal fraction `r` of the relevant signal, then:

`r^n = P(end_to_end_success)`

Using the current measured Sage+Coder rate:

`P(end_to_end_success) = 0.0821`

### 3.1 Equal-Retention Approximation

If there are 4 materially lossy stages:

`r^4 = 0.0821`

`r ≈ 0.535`

So each stage would preserve only about `53.5%` of useful signal on average.

If there are 5 materially lossy stages:

`r^5 = 0.0821`

`r ≈ 0.607`

If there are 6 materially lossy stages:

`r^6 = 0.0821`

`r ≈ 0.660`

These are not absurd numbers. They are entirely plausible for a pipeline that repeatedly compresses natural language into intermediate abstractions and then tries to reconstruct an exact executable action.

### 3.2 Practical Interpretation

Even moderately good stages fail badly when chained:

- `0.8^5 = 0.32768` -> `32.8%`
- `0.7^5 = 0.16807` -> `16.8%`
- `0.6^5 = 0.07776` -> `7.8%`

The observed `8.21%` is almost exactly what a 5-stage pipeline looks like when each stage is only around `60%` faithful.

That matters because the current architecture plausibly has at least five opportunities for semantic loss:

1. utterance -> Sage preclassification
2. preclassification -> compressed outcome representation
3. outcome -> verb resolution
4. verb -> argument assembly
5. args -> DSL proposal
6. post-resolution fallbacks and policy overlays

Not every stage is equally lossy. But the architecture does not need catastrophic failure at any one step. It only needs repeated moderate loss.

## 4. Evidence from Current Metrics

A crude product of the three measured Sage sub-metrics is already revealing:

`0.96 * 0.841 * 0.676 ≈ 0.546`

This suggests that after plane, polarity, and domain abstraction alone, only about `54.6%` of usable signal may remain aligned with the correct downstream path.

That estimate is simplistic, but useful.

Now assume downstream retention factors of `0.7`, `0.6`, and `0.5` for:

- verb resolution
- arg assembly
- DSL/fallback realization

Then:

`0.546 * 0.7 * 0.6 * 0.5 ≈ 0.115`

That lands near the actual observed end-to-end rate.

This does not prove the exact stage percentages. It does show that the measured outcomes are entirely consistent with multiplicative degradation rather than a single isolated bug.

## 5. Information Loss Framing

Another way to express the problem is in terms of retained information.

Let the raw utterance contain semantic information `I(U)`. Each transformation preserves only a fraction `k_i` of the relevant information for final execution planning.

Then after `n` transformations:

`I_final = I(U) * Π k_i`

This is not just accuracy loss. It is irrecoverable compression.

Examples of distinctions likely lost too early in the current shape:

- inventory/list vs search/query nuance
- workflow lane intent vs entity noun coincidence
- whether the user is asking for current state vs mutating state
- whether a noun is subject, scope anchor, filter, or desired output
- whether context is structural, operational, investigatory, or confirmatory

Once these distinctions are collapsed into a narrow intermediate summary, later deterministic stages cannot reconstruct them reliably.

## 6. Why Incremental Tuning Has Not Produced Uplift

Several substantial improvements have already been made:

- domain hint and domain scoring improvements
- richer Sage context and handoff payloads
- safer serve/delegate routing
- read-only bias and confirmation gating
- verb declashing and action-encoded renames in `agent`, `deal`, `view`, and `client-group`
- `harm_class`, `action_class`, and clash diagnostics

These were reasonable and often necessary. However, they produced only marginal improvement in the head-to-head metric.

That pattern strongly suggests one of two things:

1. the wrong layer is being optimized
2. the architecture contains too many loss-inducing boundaries for local improvements to matter

The second explanation is more consistent with the measurements.

## 7. Structural Failure Modes in the Current Strategy

### 7.1 Verb Selection Happens Too Early

The pipeline still becomes verb-centric before the user's intended outcome is fully preserved. That forces the system to choose among a large registry surface before enough disambiguating state has been retained.

### 7.2 Intermediate Representations Are Too Compressed

If Coder does not operate on a sufficiently rich preserved view of the utterance and session context, then Coder is resolving from a lossy surrogate rather than from the original intent signal.

### 7.3 Fallback Layers Distort Measurement and Behavior

Even after cleanup, there are still multiple serve/delegate, legacy/fallback, and policy interactions. These can suppress correct candidates or produce wrong-but-plausible outputs that are difficult to distinguish from genuine resolution success.

### 7.4 Exact-Verb Evaluation Is Harsh but Legitimate

The current harness expects the pipeline to land on the correct canonical executable verb. That is a strict target, but it is also the correct target for a deterministic REPL/runbook system. A pipeline that cannot reliably produce the correct canonical action is not ready, regardless of whether upstream summaries look reasonable.

## 8. Quantitative Threshold for Concern

A useful engineering heuristic is:

- if there are more than two lossy semantic transforms before execution planning,
- then either the model quality must be very high,
- or the intermediate typed state must preserve almost all of the original semantics,
- otherwise the chain will collapse.

The current system appears to exceed that threshold.

## 9. Implication

The implication is not that Sage or Coder are conceptually wrong.

The implication is that the current implementation shape likely has too many compression boundaries between utterance and executable outcome.

That is why the result feels "awful" despite many sensible local improvements. The math says this should happen when moderate loss compounds across stages.

## 10. Conclusion

The present Sage+Coder implementation is underperforming in a way that is numerically consistent with multiplicative semantic loss across too many stages.

The important conclusion is not merely that one classifier is weak or one scorer needs tuning. The more serious conclusion is that the current pipeline shape may be over-complex for the accuracy demanded of a deterministic DSL/runbook system.

This does not invalidate the high-level goal of:

- Sage understanding the user's intent and desired outcome
- Coder producing deterministic DSL or runbook snippets to achieve that outcome

It does suggest that the number and nature of intermediate abstractions should be reconsidered.

If the system is to improve materially, the next phase should likely focus on reducing lossy boundaries or preserving raw intent signal farther downstream, rather than continuing local tuning in isolation.
