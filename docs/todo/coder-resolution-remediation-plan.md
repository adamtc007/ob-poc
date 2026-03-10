# Coder Resolution Remediation Plan

## Goal

Improve `OutcomeIntent -> verb -> DSL` resolution materially without changing the core architecture.

This plan keeps the current flow intact:
- utterance -> Sage intent/outcome
- outcome -> Coder resolution
- resolved verb -> deterministic DSL/runbook
- execution remains on the existing REPL/runbook side

The objective here is to reduce false matches and `<none>` outcomes in the Coder layer by tightening the discriminators available to the resolver.

## Why This Is The Right Next Step

Recent state:
- Sage plane accuracy: `96.0%`
- Sage polarity accuracy: `84.1%`
- Sage domain accuracy improved from `57.4%` to `67.6%`
- End-to-end Sage+Coder coverage remains `13/134` = `9.70%`

Interpretation:
- Sage domain classification was a real issue and is now materially better.
- End-to-end verb accuracy did not move enough.
- That means the next bottleneck is the Coder layer, not the Sage architecture.

## Guiding Principle

Exploit cheap discriminators before expensive scoring.

In particular:
- `harm_class` is a powerful first-pass filter in a read-first chat architecture.
- If the user is in a safe/read path, most mutating verbs should never be considered.
- This reduces clash surface before trying to solve ambiguity with ranking heuristics.

## Non-Goals

- No Sage/Coder architecture rewrite
- No LLM-first resolution strategy
- No execution engine redesign
- No broad verb renaming sweep as the first move
- No UI redesign

## Phase C1 — Add Safety / Harm Metadata To The Verb Surface

### C1.1 Add `harm_class` to verb metadata

Introduce an explicit verb metadata dimension:
- `ReadOnly`
- `Reversible`
- `Irreversible`
- `Destructive`

This should live in the canonical verb metadata/config path that the Coder index already reads.

### C1.2 Load `harm_class` into `VerbMetadataIndex`

Extend the index so each `VerbMeta` carries:
- `harm_class`
- existing `side_effects`
- existing polarity/plane/action tags

### C1.3 Use `harm_class` as a pre-filter

Resolution rules:
- read-only serve path: prefer `ReadOnly`; reject higher-harm classes unless explicitly forced
- explicit mutation path: allow mutating classes, but keep `Destructive` isolated behind stronger confirmation rules
- ambiguous path: bias toward the lowest-harm candidate set

Expected effect:
- a large portion of write-heavy verb collisions disappear from the hot path immediately

## Phase C2 — Export A Real Clash Matrix

### C2.1 Build clash extraction over the actual registry/config surface

Produce a machine-readable artifact listing verb pairs that share:
- same domain
- same or near-identical required entity signature
- similar action tags or argument shape

The exact implementation may be:
- SQL against registry tables, if the metadata is persisted there
- or a Rust/export tool over the loaded runtime registry, if that is the authoritative source

### C2.2 Write artifacts

Emit:
- CSV clash matrix
- Markdown summary grouped by domain

Each row should include:
- `verb_a`
- `verb_b`
- `domain`
- required entity signature
- required param signature
- action class
- harm class
- side_effects

Expected effect:
- turns “Coder feels fuzzy” into a bounded concrete work list

## Phase C3 — Classify Clash Pairs By Discriminator Type

For each clash pair, classify the real differentiator:

### Bucket A: Action-class differentiable

Examples:
- list vs create
- read vs update
- describe vs delete

Fix:
- explicit `action_class` tag on verb metadata
- stronger utterance/action extraction in the Coder scorer

### Bucket B: State differentiable

Examples:
- submit vs amend
- approve vs reopen
- verify vs reject

Fix:
- state/precondition metadata in the candidate signature
- session/entity-state-aware filtering before scoring

### Bucket C: Synonymous / alias candidates

Examples:
- two verbs that effectively do the same thing under different names

Fix:
- alias one to the other
- or merge in a later vocabulary cleanup slice

### Bucket D: Context differentiable

Examples:
- only distinguishable by workflow phase, prior step, or session state

Fix:
- include workflow/stage/session constraints in the resolver input

Expected effect:
- separates cheap wins from deeper semantic collisions

## Phase C4 — Add `action_class` To The Resolver Surface

### C4.1 Add explicit `action_class` metadata

Do not rely only on FQN parsing.
Add an explicit normalized action class such as:
- `list`
- `read`
- `create`
- `update`
- `delete`
- `assign`
- `import`
- `compute`
- `review`
- `approve`
- `reject`

### C4.2 Score action-class alignment before lexical overlap

Current ranking is still too vulnerable to noun collisions.
Make the ordering roughly:
1. policy gate / harm filter
2. action-class compatibility
3. domain compatibility
4. state/precondition compatibility
5. parameter overlap
6. lexical tie-breaks

Expected effect:
- verbs with the wrong action stop surfacing just because they share nouns

## Phase C5 — Add State / Preconditions As Resolver Inputs

### C5.1 Surface state requirements in `VerbMetadataIndex`

Examples:
- required lifecycle state
- disallowed state
- workflow phase
- mandatory scope type

### C5.2 Filter candidates by current session/entity state

If the resolver knows:
- current workflow lane
- selected entity kind
- stage focus
- known object state

then verbs that are invalid in the current state should be removed before scoring.

Expected effect:
- eliminates state-differentiable clashes from the candidate set instead of hoping ranking solves them

## Phase C6 — Tighten `<none>` Handling

Current `<none>` failures indicate two different problems that should be separated:
- no valid candidate after policy/filtering
- valid candidate set exists but threshold rejects them

### C6.1 Split failure modes explicitly

Emit separate diagnostics for:
- `no_candidate_after_filters`
- `action_conflict`
- `domain_conflict`
- `state_conflict`
- `below_threshold`

### C6.2 Add fallback policy only for safe classes

If a fallback is used:
- only allow it for `ReadOnly`
- never use fallback to guess into a mutating verb

Expected effect:
- makes failure analysis actionable and safer

## Phase C7 — Add Coder Clash Regression Tests

Add focused tests covering:
- read-only path excludes mutating verbs by harm class
- action class breaks common collisions
- state filters remove invalid candidates
- ambiguous read queries prefer the lowest-harm valid match
- known clash pairs resolve deterministically

Also add a small fixture of real clash pairs from the exported matrix.

## Phase C8 — Only Then Consider Naming / Vocabulary Cleanup

After the matrix is classified, rename or merge only the clashes that remain genuinely ambiguous.

This is deliberately later because:
- many collisions will disappear once harm/action/state metadata is in play
- broad renaming before that is expensive and noisy

## Implementation Order

1. C1: `harm_class` metadata + index + pre-filter
2. C2: clash matrix export
3. C3: clash-pair bucket classification
4. C4: `action_class` metadata + scoring priority
5. C5: state/precondition-aware filtering
6. C6: explicit failure-mode diagnostics
7. C7: clash regression tests
8. C8: selective naming/alias cleanup only where still necessary

## Expected Files

Likely core files:
- `rust/src/sage/verb_index.rs`
- `rust/src/sage/verb_resolve.rs`
- `rust/src/sage/coder.rs`
- verb config / metadata YAML under `rust/config/verbs/`
- possibly Sem OS / domain metadata files if verb metadata is canonical there

Likely new artifacts/tools:
- clash export script or test harness under `rust/tests/` or `rust/examples/`
- generated CSV / Markdown under `rust/target/`

## Verification Gates

### Gate A
- `cargo check -p ob-poc` passes
- `harm_class` present for all verbs in the active registry/index

### Gate B
- clash matrix generated successfully
- clash count is measured and grouped by bucket

### Gate C
- coder regression tests pass
- `<none>` cases attributable to policy/threshold are separately reported

### Gate D
- rerun utterance coverage
- target: improve Sage+Coder accuracy materially from `9.70%`

## Success Criteria

Minimum success:
- Coder stops considering most unsafe verbs on read paths
- clash surface becomes explicit and measurable
- top collision families have deterministic fixes

Good success:
- Sage+Coder accuracy rises materially from `9.70%`
- `<none>` failures drop
- read-path false positives fall significantly

## Decision Boundary

If C1-C6 improves coverage materially, continue deterministic Coder tuning.
If coverage remains weak even after harm/action/state filtering, then the next issue is likely:
- bad argument extraction
- unresolved vocabulary aliasing
- or legacy fallback contamination
