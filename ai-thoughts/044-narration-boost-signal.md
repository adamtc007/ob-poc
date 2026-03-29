# ADR 044: Narration Boost Signal — Context-Aware Intent Bias

> **Status:** PROPOSED (2026-03-29)
> **Depends on:** ADR 043 Phase 0 (NarrationEngine — implemented)
> **Risk:** Low — additive score bias, no pipeline restructure

---

## Problem

After NarrationEngine suggests 3 actions ("assign Management Company", "open KYC case",
"assign Auditor"), the operator types "assign the manco". The intent pipeline doesn't know
about the narration context — it searches the full 1,442-verb embedding space and may return
`manco.primary-controller` instead of `cbu.assign-role`.

The narration already identified the most likely next verbs. The pipeline should use that signal.

---

## Design

### Mechanism

Store the `suggested_next` verb FQNs from the last `NarrationPayload` on the session.
Before verb search, pass them to `HybridVerbSearcher` as a **boost set**.

```rust
pub struct NarrationBoost {
    /// Verb FQNs from the last narration's suggested_next
    pub hot_verbs: Vec<String>,
    /// Boost score to add when a hot verb appears in search results
    pub boost: f32,  // Default: +0.05
}
```

### Where it applies

In `HybridVerbSearcher::search()`, after embedding search returns candidates:

```rust
for result in &mut candidates {
    if boost.hot_verbs.contains(&result.verb_fqn) {
        result.score += boost.boost;
    }
}
```

That's it. No new tier, no new search path, no reranking. Just a nudge.

### Why +0.05

Current disambiguation band is typically 0.02-0.03 between the top two candidates.
A +0.05 boost is enough to tip the balance when the utterance is genuinely about one
of the suggested verbs, but not enough to override a strong embedding match for an
unrelated verb.

If the operator says "assign the manco" and `cbu.assign-role` scores 0.88 vs
`manco.primary-controller` at 0.90, the boost tips it to 0.93 vs 0.90. Correct answer wins.

If the operator says "compute ownership chains" (nothing to do with the suggestion),
`ownership.compute` at 0.95 vs boosted `cbu.assign-role` at 0.88+0.05=0.93. Correct
answer still wins.

### Session State

```rust
// In WorkspaceFrame or ReplSessionV2:
pub narration_hot_verbs: Vec<String>,
```

Updated after every `compute_narration()` call:
```rust
session.narration_hot_verbs = narration.suggested_next
    .iter()
    .map(|s| s.verb_fqn.clone())
    .collect();
```

Cleared when:
- Workspace changes (hot verbs are workspace-specific)
- Session scope changes (different entity context)
- Operator explicitly changes topic

### Integration Points

| Component | Change | Risk |
|-----------|--------|------|
| `ReplSessionV2` | Add `narration_hot_verbs: Vec<String>` | Low — additive field |
| `HybridVerbSearcher::search()` | Accept optional `&[String]` boost set, apply +0.05 | Low — 3 lines |
| Response adapter | After `compute_narration()`, store hot verbs on session | Low |
| `WorkspaceFrame` | Clear hot verbs on push/pop | Low |

### What it does NOT do

- Does not change MacroIndex scoring (Tier -2B). Macros have their own scorer.
- Does not change ScenarioIndex scoring (Tier -2A). Scenarios are compound utterances.
- Does not change ECIR (Tier -1). Deterministic resolution is unaffected.
- Does not filter results. All verbs remain searchable. The boost is additive, not exclusive.

---

## Regression Analysis

**Expected improvements:**
- Utterances that follow narration suggestions should resolve 5-10% more accurately
- "assign the manco" after "Next: assign Management Company" → `cbu.assign-role` wins over `manco.primary-controller`

**Regression risk:**
- A boosted verb could beat a legitimately better match if the operator ignores the narration
- Mitigation: +0.05 is small enough that a 0.10+ score gap still wins for the non-boosted verb
- Mitigation: hot verbs are cleared on workspace/scope change

**Measurement:**
- Add ~10 "contextual follow-up" test cases to intent_test_utterances.toml
- Mark them with `category = "narration_boost"` and `expected_boost_verbs = [...]`
- Run hit rate with and without boost to measure delta

---

## Implementation Estimate

~30 lines of Rust:
- 5 lines: `narration_hot_verbs` field on session
- 5 lines: populate from `suggested_next` after narration
- 5 lines: clear on workspace/scope change
- 10 lines: boost application in `HybridVerbSearcher`
- 5 lines: thread boost through `search()` signature

---

## Decision

**DEFER until Phase 1-2 of ADR 043 are complete.** The boost signal requires the
narration engine to be actively producing `suggested_next` on every response, which
requires Phase 1 (React rendering for feedback) and Phase 2 (contextual routing).
Building the boost before the narration is live would add dead code.

When ready, implement as a single PR with the 10 contextual test cases. Run hit rate
before/after. If two-attempt drops below 92%, revert.
