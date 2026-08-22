# Instruction Matrix Refactor — Research Findings & Phased Plan

### EOP-PLAN-MANDATE-001

| | |
|---|---|
| **Document** | EOP-PLAN-MANDATE-001 |
| **Type** | Research findings + phased implementation plan |
| **Version** | 0.1 — Draft for review |
| **Implements** | EOP-VS-MANDATE-001 (see v0.3 for the extended matrix scope) |
| **Status** | Draft. Premises marked ⚠ are unverified and are the first task of T0. |

---

# Part A — Research Findings

## A1. What was verified

Read directly from `schema_export.sql` and `bpmn-lite-store-postgres/migrations`.

**A1.1 — The parts already have real relational tables.** This is the most important finding, and it inverts an earlier assumption. `cbu_ssi` is a proper table with typed columns (`ssi_id`, `cbu_id`, `ssi_name`, `ssi_type`, account/BIC fields, `effective_date`, `expiry_date`) and — critically — a `market_id uuid` **foreign-key reference**. Its own comment states the intent explicitly:

> *"Layer 2: Pure SSI account data. No routing logic — just the accounts themselves."*

That is already the design law "a block stores what it is, not where it sits." **The entity layer is largely correct today.** The refactor is therefore much smaller than first implied: it is not "promote JSONB to entities," it is "stop generating structure in SQL and stop storing arrangement."

**A1.2 — Identity is already content-stable in the entity tables.** `cbu_ssi.ssi_id` uses `uuidv7()`, persistent per row. The unstable identity problem is **confined to the AST generator**, which mints `gen_random_uuid()` per node at build time. So identity is stable in storage and unstable only in the generated document — exactly the wrong way round, and cheap to fix by deriving node identity from the entity's existing id.

**A1.3 — There is an existing arrangement-storing pattern to avoid.** `cbu_ssi_agent_override` carries `sequence_order integer NOT NULL`. That is a stored ordinal. It may be legitimate (SSI agent chains are genuinely ordered by settlement convention — the order is a *fact*, not a display choice), but it is the exact shape the design laws forbid, so it must be explicitly adjudicated rather than copied. ⚠

**A1.4 — `cbu_structure_links` is already a well-formed edge table.** `parent_cbu_id`, `child_cbu_id`, `relationship_type`, `relationship_selector`, effective dating, a `no_self_link` CHECK constraint, and TERMINATED/SUSPENDED status. This is a property-graph edge table with temporal validity and integrity constraints already in place — a **precedent to follow** for any edges the matrix needs, not something to reinvent.

**A1.5 — The AST generator is the defect, and it is contained.** `migrate_trading_profile_to_ast(...)` builds the typed node tree in plpgsql: a discriminated `node_type` union (`category`, `instrument_class`, `market`, `universe_entry`, `ssi`, `booking_rule`, `isda_agreement`), mints node ids, computes `specificity_score`, and bakes presentation (`status_color`, `is_loaded`, `leaf_count`, `label`, `sublabel`) into the same nodes as the facts. CSA exists only as a comment: *"CSAs would be children here."*

**A1.6 — Materialization runs document → tables.** `020_trading_profile_materialization.sql` records `trading-profile:materialize` events, described as *"when trading profile documents are projected to operational tables (universe, SSIs, booking rules, ISDA/CSA)"*, tracking `sections_materialized` and per-table created/updated/deleted counts. **The current arrow is document → entities.** The target arrow is entities → assembled board. This is the single largest directional change in the refactor.

**A1.7 — BPMN confirms the target pattern.** `bpmn_process_instance` stores only runtime marking (`current_node`, `status`, `variables`, waiting-on pointers) — **no node or edge tables**. The definition is a content-hashed compiled artifact (`compiled_programs`, unique on `artifact_hash`). The graph is *not* stored relationally; identity *is* the content hash. The matrix should follow the definition half and omit the runtime half entirely.

## A2. Unverified premises (⚠ — resolve in T0)

1. **Is there a live AST generation path, or only the `migrate_*` one-off?** Decides whether T2 replaces a live path or only a migration artifact.
2. **Do `universe`, `booking_rule`, `isda`/`csa` have tables of the same maturity as `cbu_ssi`?** Verified for SSI; assumed for the rest.
3. **Is `sequence_order` in `cbu_ssi_agent_override` a settlement fact or a display ordinal?** (A1.3.) Determines whether it stays or is derived.
4. **Where do instruction formats and pricing preferences live?** Named in scope; not found in this pass.
5. **Which consumers read the AST JSONB today?** Determines the compatibility surface for T3.
6. **Is a Rust-side taxonomy for these domains already present** (which assembly must be consistent with, not a second author of)?

## A3. Revised scope assessment

The entity layer is in better shape than assumed; the defect is concentrated in **generation** (plpgsql), **arrangement storage** (the AST document), and **direction of flow** (document → entities instead of entities → board). This is a *generation* refactor, not a data-model rebuild.

---

# Part B — Phased Plan

Each tranche is independently landable, RED-first, and closes with gates that an intent-driven implementer cannot satisfy the wrong way.

---

## T0 — Premise closure (research only, no code)

**Purpose:** close the six ⚠ premises before any code is written. Nothing downstream should inherit an assumption.

**Deliverable:** a findings note answering each ⚠, plus a field-level classification of every column in the mandate-related tables into: *constitutive fact* / *arrangement* / *presentation* / *negotiated term*.

**Gate:** the classification table is complete and every mandate-related column is assigned exactly one category, with disputed ones flagged for ruling. **No implementation tranche starts until T0 lands.**

**Why first:** T1's block shape is derived from this classification. Getting it wrong means the block type is wrong and everything after it inherits the error.

---

## T1 — Block types and content-derived identity

**Purpose:** define the self-locating block in Rust — the sealed variant set, its classifying attributes, and its constitutive references. No arrangement fields, no presentation fields.

**Scope:** the block type; content-derived identity (from the entity's existing stable id, per A1.2); a read path that loads blocks from the existing entity tables.

**Explicitly not:** assembly, rules, or any write path.

**Gates:**
- `no_arrangement_in_block` — a *type-level* assertion: the block type has no `parent`, `path`, `ordinal`, `depth`, `is_root`, or child-collection field. Non-gameable, because it is a property of the type, not a value.
- `no_presentation_in_block` — same, for `status_color`/`is_loaded`/`leaf_count`/`label`/`sublabel`.
- `identity_is_stable` — loading the same entity twice yields the same block id; identity never calls a random or clock source.

---

## T2 — Assembly (the hinge)

**Purpose:** `assemble(blocks) -> Board`, replacing the plpgsql generator. Resolve references into adjacency, derive the reverse index, derive root, verify well-formedness, hash the result.

**Scope:** reference resolution with loud failure on dangling refs; bidirectional adjacency derived (never stored); canonical content-tie-broken ordering; board content hash.

**Explicitly not:** nesting/region decomposition (T4), the construction grammar (T5), or removal of the plpgsql function (T3).

**Gates:**
- `order_independence` — `assemble(shuffle(blocks)) == assemble(blocks)`, fuzzed, on a fixture containing a join. *The keystone: proves no arrangement leaked into storage or row order.*
- `assembly_is_total` — fuzz over arbitrary block sets; assert every outcome is a well-formed board or a typed rejection. Never a wrong board, never a panic.
- `dangling_ref_is_loud` — an unresolvable reference produces a typed error, never a silent orphan or an "unclassified" bucket.
- `reverse_index_derived` — a source assertion that no predecessor/back-reference is read from storage.

---

## T3 — Behavioural equivalence and cutover

**Purpose:** prove the Rust assembly reproduces the plpgsql AST's **facts and structure**, then retire the SQL generator.

**Scope:** differential test over real CBU profiles; presentation-stripping comparator; deprecation of `migrate_trading_profile_to_ast`.

**Gates:**
- `reconstruction_matches_reference` — for known CBUs, `assemble(...)` equals the reference AST **with presentation stripped and node ids normalised**. (Ids will differ by design — T1 makes them content-derived; the comparator must normalise, and that normalisation must be explicit, not a loophole.)
- `no_taxonomy_in_sql` — a source scan asserting no remaining plpgsql function computes node structure or classification. Denylist finalised against the real schema during this tranche, **not** inherited as a placeholder.

---

## T4 — Nested regions (derived, not stored)

**Purpose:** derive the nested mini-taxonomies (branch/merge regions) from the flat block set.

**Scope:** adopt a published SESE / RPST decomposition rather than hand-rolling it (VS §3.6); a graph library for traversal and cycle detection.

**Gates:**
- `nesting_is_derived` — the nesting tree is identical under input shuffling.
- `decomposition_is_total` — every board the grammar can produce decomposes deterministically; no arbitrary tie-breaks.

**Dependency note:** if T5's grammar guarantees well-nested construction, T4's derivation is always total. If arbitrary structures are permitted, decomposition is best-effort and this tranche's gate weakens — so **T4's strength depends on T5's constraint**, and the two should be planned together even though they land separately.

---

## T5 — Legal-move grammar (the integrity boundary)

**Purpose:** make the construction grammar the **sole mutation path**, with a transient session cursor.

**Scope:** enabled-move computation from board state; move execution; session cursor held in the session, never written to a block. Reuse the KYC/UBO placement-set and preview machinery (VS §3.6) rather than authoring a second grammar. ⚠ *Reuse fit must be verified against the actual KYC substrate types before this tranche is planned in detail.*

**Gates:**
- `grammar_preserves_invariants` — fuzz random legal-move sequences; every reachable board is well-formed. *Proves the grammar is the integrity guarantee rather than a convention.*
- `round_trip` — build via legal moves → break into blocks → save → reassemble from the table alone → assert equality. Fuzzed over random legal sequences. **This is the plan's keystone gate: any relation that lived only in the build cursor fails it.**
- `cursor_never_persisted` — a source/type assertion that no block field carries the active-node cursor.

---

## T6 — Rule versioning and pinning

**Purpose:** make board identity `(block set × rule version)` reproducible.

**Scope:** rule-set versioning; a pin recording which rule version produced a given board, following the KYC substrate's determination-pin pattern.

**Gates:**
- `board_is_reproducible` — the same blocks + same pinned rule version reproduce a bit-identical board.
- `rule_change_is_visible` — changing the rule version changes the board hash, and the pin records which version applied.

---

## B1. Sequencing and rationale

```
T0 (premises) → T1 (blocks) → T2 (assembly) → T3 (equivalence + cutover)
                                    ↓
                              T4 (nesting) ⟷ T5 (grammar) → T6 (pinning)
```

- **T0 first, always** — every later tranche's block shape depends on the classification.
- **T2 before T3** — build the replacement before retiring the incumbent; T3 is where the risk is retired, not where it is taken.
- **T4 and T5 are coupled** (see T4's dependency note) — plan together, land separately.
- **T6 last** — pinning only has meaning once rules exist and assembly is stable.

## B2. Risks

1. **Materialization direction reversal (A1.6)** is the largest change and touches live behaviour. T3 must establish behavioural equivalence *before* anything is retired.
2. **Consumers of the AST JSONB (⚠5)** are an unknown compatibility surface. If consumers bind to the current node shape, T3 needs an adapter or a coordinated consumer change.
3. **`sequence_order` (A1.3)** — if it turns out to be a genuine settlement fact, the design law needs an explicit carve-out for *ordered facts*, distinguishing them from display ordinals. Better to name the carve-out than to let the law be quietly violated.
4. **Grammar reuse fit (T5 ⚠)** — "reuse the KYC placement machinery" is a premise, not a verified fact. Verify before T5 is planned in detail.

## B3. What this plan deliberately does not specify

Per the V&S, gates pin the **contract**, not the mechanism. The plan does not specify internal data structures, module layout, or algorithms beyond "adopt a published RPST rather than hand-roll." Those are the implementer's to choose, provided the gates hold.
