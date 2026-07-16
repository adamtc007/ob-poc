# EOP-DESIGN-CONTROLPLANE-T11.1b/slice2 — `agent/orchestrator.rs` Interpretation/Adjudication Split

### Basis: EOP-DESIGN-CONTROLPLANE-T11.1a-BOUNDARY-MAP-001 §0, ratified answer 3 ("needs splitting, not a single tier verdict" — decompose along the CP-adjacent/utterance-parsing seam)
### Status: RATIFIED (2026-07-12) — design only, no code. Unblocks the orchestrator.rs sub-pass of T11.1b slice 2.

## 0. Ratification (2026-07-12)

Architect ruling, verbatim law for this split:

> Interpretation answers "what does the user mean" — a linguistic, probabilistic claim. Adjudication answers "is that legal for this pack, this entity, in this state" — a deterministic verdict. The first is agent-tier; the second is CP-tier; no code evaluates both.

This is the separation law for `agent/orchestrator.rs` (4,890 lines, flagged MIXED in T11.1a §2) and, by extension, any future file that mixes utterance handling with legality computation.

## 1. Why the entanglement happened (not just where)

The AB5/E-3 trace already demonstrated the cost: session state feeding `VerbSurfaceContext` → `compute_session_verb_surface` → `surface_allowed` means legality computation runs *inside* the interpretation loop, on agent-owned data, with no CP provenance. That is a verdict being manufactured where only a claim should exist.

But the split must not destroy the one real reason the entanglement happened: **interpretation quality genuinely benefits from legality context.** Constrained matching against the allowed surface is what stops Sage hallucinating verbs. Discarding that signal to achieve a clean tier split would be a regression, not a fix.

v0.4's inverted §8 already contains the correct resolution: **the CP invokes Sage with a granted context.** Legality data may flow *into* interpretation, but only as a **CP-minted grant**, and its epistemic role changes on the way through:

- To Sage, the granted surface is a **hint** — advisory, staleness-tolerant, used to rank and constrain candidates. Sage cannot compute it (post-split it cannot even import the registry/DAG/policy types to try), only receive it.
- To the CP, the same data is a **verdict** — recomputed at decision time against the pinned snapshot, by the floor and gates. The hint is never trusted; a candidate that slipped past a stale hint dies at G1/G3/G4 exactly as it should.

Hint drift is therefore harmless by construction. This also retires the two-touchpoint drift concern raised in the T11.F design: the interpretation-side surface and the floor ask different questions with different authority, and only one of them binds.

## 2. Rules for the split

1. **Agent tier:** utterance intake, clarification loop, candidate assembly, attestation.
2. **CP tier, full stop:** `compute_session_verb_surface`, `surface_allowed`, and every legality predicate. No exceptions, no "just this one read."
3. **The surface reaches Sage only as a per-invocation CP grant** — provenance-carrying, read-only, labeled advisory, never persisted as authority. (The AB5 field-split follows this same line: `scope`/`stage_focus` are CP-side data *because* they feed a verdict.)
4. **MCA gains a clause:** *no legality predicate evaluates in agent-tier code; grants are CP-minted and advisory.* Provable mechanically by (a) the L1 dependency graph and (b) a grep for verdict-type imports (`VerbSurfaceContext`, registry/DAG/policy types) in agent-tier crates returning zero hits.

## 3. Payoff

Interpretation becomes swappable (different model, different matcher, better Sage) without a control review. The control model becomes auditable without reading a line of NLP code. Meaning and law sit in separate rooms with one supervised door between them.

## 4. Status / next step

This is the design ruling for the split, not the file-level plan. Before any code moves: a dedicated boundary-tracing pass over `orchestrator.rs`'s 4,890 lines, sorted into the two piles per the rules above, with the CP-grant shape (what a "hint" struct looks like, how it's minted, how staleness is labeled) specified before extraction — that shape is new (T11.2-adjacent), not a mechanical rename like T11.1b slice 1's crate move. Not started; awaiting explicit "proceed" to begin the file-level trace.
