# ADR 042: Macro Intent Refinement Plan

> **Status:** PROPOSED (2026-03-29)
> **Context:** 83.7% two-attempt hit rate (337 cases), 80% macro two-attempt (64/80), 16 macro misses remaining
> **Constraint:** Any change must not regress existing hit rates. The pipeline has ~1,400 verbs and 93 macros competing for the same utterance space.

---

## Current State

| Metric | Value |
|--------|-------|
| Overall first-attempt | 62.6% |
| Overall two-attempt | 83.7% |
| MacroIndex accuracy (when fired) | 86.0% (49/57) |
| macro_match first-attempt | 66.2% (53/80) |
| macro_match two-attempt | 80.0% (64/80) |
| Remaining macro misses | 16/80 |

The 16 misses fall into three categories:
1. **Embedding gap** (~10): The BGE embeddings don't connect the utterance phrase to the macro label or its underlying verbs
2. **Collision** (~4): A nearby verb or macro wins because its embedding is closer
3. **Novel domain** (~2): Governance/attribute macros with no training signal in the embedding space

---

## Refinement Options (Ranked by ROI / Regression Risk)

### R1: Phrase Enrichment (LOW risk, HIGH ROI)

**What:** Add invocation phrases to verb YAML definitions for the 16 failing macros' underlying verbs, then re-embed.

**Why it's safe:** Invocation phrases are training data for the embedder. They don't change the pipeline logic — they give the existing Tier 0+ embedding search better signal. The MacroIndex (Tier -2B) is unaffected since it uses its own scorer.

**Steps:**
1. For each of the 16 failing macro utterances, identify the underlying verb(s)
2. Add the failing utterance text (or a close variant) as an `invocation_phrase` on the verb YAML
3. `cargo x verbs compile && populate_embeddings`
4. Re-run hit rate — expect +5-8% on macro_match two-attempt

**Regression risk:** Minimal. New phrases only add signal, they don't remove existing matches. The only risk is a new phrase colliding with an existing verb's embedding space — but that's already visible in the hit rate report as a wrong-verb-selected case.

**Recommendation:** DO THIS FIRST. It's the lowest-cost, highest-impact change.

---

### R2: UI Macro Label Enrichment (LOW risk, MEDIUM ROI)

**What:** When the verb surface returns a primitive verb to the UI, check if a macro in the active pack wraps that verb. If so, show the macro's UI label instead of the raw verb FQN.

**Why it's safe:** This is a presentation-layer change only. It doesn't modify intent resolution, verb search, or execution. The operator sees "Assign Role" instead of `cbu.assign-role` — better UX, no pipeline change.

**Steps:**
1. In `response_adapter.rs`, after verb surface computation, cross-reference `allowed_verbs` from the active pack against macro registry
2. Build a `verb_to_macro_label` lookup (primitive FQN → macro UI label)
3. Enrich `suggested_utterance` and `forward_verbs` with macro labels where available
4. No search pipeline changes

**Regression risk:** Zero for intent resolution. UI-only.

**Recommendation:** DO THIS. Quick win for operator experience. Decoupled from search accuracy.

---

### R3: Constellation-Aware Macro Promotion (MEDIUM risk, LOW ROI)

**What:** When a primitive verb resolves via embedding search (Tier 0+) and the verb lives in a constellation slot that a macro also targets, promote the macro as the primary result.

**Why it's concerning:**
- Adds a post-resolution rewrite step — the pipeline says "verb X" and then we override it with "macro Y"
- The macro might have prereqs that aren't met (state DAG), so promoting it could offer an action the operator can't take
- It creates a hidden dependency between constellation maps and intent resolution — changing a constellation slot could change which verb utterances resolve to
- The MacroIndex already handles this at Tier -2B with explicit scoring. If it didn't fire, the utterance probably isn't a macro invocation.

**When it might help:**
- Operator is mid-workflow, constellation is hydrated, and the verb surface already knows the macro is valid
- At that point it's not search — it's contextual narrowing (the verb surface already does this via pack `allowed_verbs`)

**Regression risk:** Medium. Any verb that shares a constellation slot with a macro could get silently rewritten. Hard to predict which utterances are affected without running the full test suite.

**Recommendation:** DEFER. The verb surface + pack `allowed_verbs` already provides contextual narrowing. Adding another promotion path creates a second channel for the same decision, which is where regressions hide.

---

### R4: Macro Namespace Separation (MEDIUM risk, LOW ROI)

**What:** Rename macros that share FQN with their underlying verb to use a distinct operator namespace. E.g., `tollgate.evaluate` (verb) vs `op.tollgate.evaluate` (macro).

**Why it's concerning:**
- 93 macros already deployed, referenced in packs, scenarios, search overrides, test fixtures, and the macro annex
- The 1:1 name collision only affects ~5 macros (tollgate.evaluate, tollgate.override, red-flag.raise, red-flag.close, red-flag.set-blocking)
- The collision is benign — both resolve to the same operation
- Renaming creates a migration burden with no functional benefit

**Regression risk:** High (migration scope). Low (if done correctly).

**Recommendation:** DON'T DO THIS. The collision is intentional for single-step wrappers. The operator overlay convention (party ≠ entity, case ≠ kyc-case) already handles the cases where disambiguation matters.

---

### R5: Embedding Space Partitioning (HIGH risk, MEDIUM ROI)

**What:** Train separate embedding indices for macros vs primitives, with tier-specific similarity thresholds.

**Why it's concerning:**
- The current single embedding space (BGE-small-en-v1.5, 384-dim) works because macros and verbs share vocabulary ("assign role", "create fund")
- Partitioning would require either two embedding models or two separate vector indices
- The MacroIndex already provides deterministic macro matching without embeddings
- Adding a second embedding index doubles the vector search cost (currently 5-15ms)

**Regression risk:** High. Two indices means two sets of similarity scores that need to be calibrated against each other. The score comparison between tiers (macro at 0.96 vs embedding at 0.92) is already fragile.

**Recommendation:** DON'T DO THIS. The MacroIndex deterministic scorer is the right answer for macro matching. Embeddings are for verb discovery, not macro discovery.

---

## Recommended Execution Order

| Priority | Refinement | Risk | Expected Impact | Effort |
|----------|-----------|------|-----------------|--------|
| 1 | R1: Phrase enrichment | Low | +5-8% macro two-attempt | 1-2 hours |
| 2 | R2: UI macro label enrichment | Low | UX improvement, no accuracy change | 2-3 hours |
| — | R3: Constellation promotion | Medium | Marginal accuracy, hidden couplings | DEFER |
| — | R4: Namespace separation | Medium | Zero functional benefit | DON'T DO |
| — | R5: Embedding partitioning | High | Marginal, high calibration cost | DON'T DO |

---

## Guardrails

Before implementing any refinement:
1. **Baseline snapshot:** Run `intent_hit_rate` and record all metrics before the change
2. **Regression gate:** After the change, overall two-attempt must not drop below 82%. macro_match two-attempt must not drop below 78%
3. **Collision audit:** Check the "Wrong verb selected" count — it must not increase by more than 2
4. **PACK001 clean:** `cargo x verbs lint-macros` must pass with zero errors

---

## Non-Goals

- **100% macro hit rate** is not a goal. Some utterances are genuinely ambiguous between a macro and its underlying verb. If "approve the case" resolves to `kyc-case.update-status` (the primitive) instead of `case.approve` (the macro), the operator gets the same outcome.
- **LLM-assisted macro matching** is not planned. The MacroIndex deterministic scorer + embedding fallback is fast (5-15ms) and predictable. Adding an LLM call for macro disambiguation would add 200-500ms and introduce non-determinism.
