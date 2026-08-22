# The Instruction Matrix as a Generated Board

### Vision & Scope — EOP-VS-MANDATE-001

| | |
|---|---|
| **Document** | EOP-VS-MANDATE-001 |
| **Type** | Vision & Scope — conceptual, for sign-off before implementation |
| **Version** | 0.3 — supersedes v0.2 |
| **Status** | Draft. No code or schema changes in this document. |
| **Change from v0.2** | Extends scope to the **full instruction matrix**: adds the four brick families, the coordinate-vs-region distinction, agreement prerequisites, and the coverage / unambiguous-resolution properties that this distinction makes load-bearing. |

---

## §1 Vision

An investment mandate is the agreement defining what may be traded, for whom, under which agreements — extended here with the operational overlay of **how instructions route** and **how it is priced**.

Modelled as a **board**: a set of self-locating blocks assembled into a traversable map. Every part is a first-class entity carrying the references that constitute it. The **structure is never stored** — it is derived by code from the block set, under rules that change without migration.

The board is built through a **legal-move grammar**: a governed session with an active cursor, where permitted moves are a function of current board state. State changes execute *only* through that grammar, so malformed structure is unconstructible rather than merely invalid.

The assembled board is **traversable, not executable** — no token, no current node, no runtime state.

---

## §2 The four brick families

The matrix is not one homogeneous graph. Four families attach in structurally different ways, and this is what drives the grammar.

### 2.1 Universe bricks — *coordinates*

Instrument type × market/venue × currency × settlement type. These define the **eligible space**: what may be traded. A universe entry is a *point* in that space.

Legality is combinatorial: is this instrument tradeable in this market, is this settlement type valid for this instrument, is this currency valid for this market.

### 2.2 Agreement bricks — *scoped prerequisites*

ISDA and CSA. Not coordinates — counterparty-scoped **enablers** that gate whether a class of activity is permitted at all (an ISDA gates OTC derivatives with that counterparty). CSA is a child of ISDA: no CSA without its parent agreement.

Legality is prerequisite-shaped: a coordinate requiring an agreement cannot be placed until the agreement is on the board.

### 2.3 Instruction-route bricks — *region claims*

How instructions are delivered: structured message (SWIFT MT / ISO 20022) or blotter/file, plus manual fallback.

**A route does not sit at a coordinate — it claims a region of the coordinate space** ("equities in this market settling DVP route this way"). This is why the existing schema carries `match_criteria` and `specificity_score`.

### 2.4 Pricing bricks — *region claims*

Fee basis over a region of the same space. Structurally identical to routes: matchers with specificity, requiring deterministic precedence where regions overlap.

### 2.5 Why the distinction matters

Coordinate bricks are *placed at a point*; route and pricing bricks *claim an area*. The interesting legality rules for the latter are therefore not "may this attach here" but **coverage** and **overlap** — which is a class of rule that point-placement thinking misses, and the one most likely to fail silently.

---

## §3 Approach

### 3.1 Self-locating blocks

A block carries everything constitutively true about it and **nothing about the arrangement it currently sits in**: content-derived stable identity, type discriminant, classifying attributes (its coordinates), and constitutive references.

It holds no `parent_id`, path, ordinal, depth, `is_root`, child list, or presentation field.

> **Test for any candidate field:** would this still be true in a differently-arranged board? If yes it belongs on the block; if no it is derived.

### 3.2 Rules derive arrangement

References are the raw material; arrangement is computed. Because a universe entry carries refs to both market and instrument type, rules can nest either way from the *same* blocks. Rules are a **versioned input to board identity** — a board is reproducible only as (block set × rule version).

### 3.3 Legal-move grammar as integrity boundary

The grammar is the sole mutation path. Prevention, not detection: if no legal move produces a dangling reference or an ambiguous overlap, those states are unconstructible. Hashing then only catches what bypassed the grammar.

**Legality rule taxonomy** — expressed uniformly, evaluated against whole-board state (not merely the cursor neighbourhood, or global rules such as uniqueness cannot be enforced):

| Kind | Example |
|---|---|
| Type compatibility | may this brick type attach to that one |
| Cardinality | how many of this brick at this coordinate |
| Uniqueness | is this coordinate already occupied |
| Prerequisite | is the ISDA present before OTC is placed |
| Completeness | is the block itself fully specified enough to place |
| Exclusion | do two placed bricks contradict |
| **Coverage** | is every eligible coordinate served by a route |
| **Unambiguous overlap** | do two region claims tie with equal specificity |

The last two are specific to region-claiming families and have no analogue in point placement.

The cursor is **session-scoped and transient** — never written to a block. A saved board has no active node.

### 3.4 Assembly derives the map

`assemble(blocks) -> Board` is the single hinge: resolve references into adjacency (dangling refs fail loudly); derive the reverse index; derive root and nesting; verify well-formedness; hash the result. **All validation happens at build and assembly — traversal never has to defend itself.**

### 3.5 Traversable, not executable

| | Instruction matrix | BPMN process |
|---|---|---|
| Nature | Traversable | Executable |
| Runtime state | None | Active token |
| "Active node" | Transient build cursor only | Live, persisted per instance |
| History means | Construction provenance | The token's path |

Confirmed by `bpmn_process_instance`, which persists only marking + status + variables, while the definition is a content-hashed compiled artifact — not a graph in tables.

### 3.6 Reuse, don't reinvent

Adopt: property-graph modelling; projection/read-model discipline; SESE/RPST decomposition for nested regions; a graph library for adjacency and traversal.

Ours: the **governed legal-move grammar** — no graph database or workflow engine provides placement legality as a function of board state, and it already exists for KYC/UBO.

> A graph database is explicitly **not** the storage answer: it would become a second authority for structure, reintroducing the reconciliation break this design prevents.

---

## §4 Capabilities delivered

- Reshapeable taxonomy without migration.
- Deterministic assembly — same blocks, same board, any retrieval order.
- **No reconciliation surface** — one authority, one derivation path.
- Corruption resistance by construction.
- Incremental construction across sessions (content-derived identity is idempotent).
- Point-in-time reconstruction as (blocks as of T × rule version).
- **Provable coverage** — every tradeable coordinate has a route and a price.
- **Provable resolution** — every coordinate resolves to exactly one route and one price.

---

## §5 Design laws

1. **No arrangement in storage.** A block stores what it is, never where it sits.
2. **No presentation in the board.**
3. **One mutation path** — the legal-move grammar.
4. **Assembly is total and deterministic** — well-formed board or typed rejection; never a wrong board, never a panic.
5. **Identity is content-derived** — never minted at build.
6. **Rules are versioned and pinned.**
7. **One authority** — nothing derivable is stored as a second source.
8. **Resolution is single-valued** — every coordinate yields exactly one route and one price, or the board is illegal.

---

## §6 Acceptance gates (RED-first, fuzzable, non-gameable)

- **`round_trip`** — build via legal moves → break into blocks → save → reassemble from the table alone → assert equality. *Keystone: any relation living only in the build cursor fails it.*
- **`order_independence`** — `assemble(shuffle(blocks)) == assemble(blocks)`, fuzzed.
- **`grammar_preserves_invariants`** — fuzz legal move sequences; every reachable board is well-formed.
- **`illegal_moves_rejected`** — fuzz *illegal* moves against reachable states; all rejected. *Legal-only fuzzing cannot find a hole in the legality check.*
- **`assembly_is_total`** — fuzz arbitrary block sets (including unreachable ones — imports and repairs bypass the grammar).
- **`coverage_complete`** — no eligible coordinate lacks a route or price.
- **`resolution_unambiguous`** — fuzz coordinates; each resolves to exactly one route and one price, deterministically.
- **`no_presentation_in_board`** / **`no_arrangement_in_block`** — type-level assertions, not value checks.
- **`identity_is_stable`** — same brick across sessions yields the same id.

### 6.1 Why this fuzzes without combinatorial explosion

Legality is checked **at generation time, not assertion time**. The fuzzer walks `enumerate_legal_moves(board)` — a small set per state — so every iteration is a reachable, meaningful board rather than a rejected candidate. Purity makes failures reproducible and shrinkable. For small boards, exhaustive enumeration to depth N is feasible and strictly stronger than sampling.

**Caveat:** this free lunch applies to grammar-walking targets only. `assembly_is_total` deliberately generates arbitrary block sets and needs structured generation to stay useful.

---

## §7 Out of scope

- The Java 25 POJO consumer API — downstream; depends on the board's shape.
- Changing what a mandate means commercially.
- The view layer.
- BPMN runtime changes — the matrix needs no token.

## §8 Open rulings

1. **Booking rules** — per-CBU stored fact with code-interpreted matching, or generated taxonomy? Evidence favours the former (negotiated `priority`); this is the call most at risk of flattening negotiated terms into taxonomy.
2. **Brick alphabet** — the enumerated brick set and compatibility matrix is a **T0 deliverable**, not settled here.
3. **Route granularity** — are instruction formats modelled at channel level (SWIFT / blotter / manual) or message-type level (MT540, sese.023)? Determines compatibility-rule granularity.
4. **Overlap precedence** — is ambiguous overlap *illegal* (grammar rejects) or *resolved* by a specificity rule? Law 8 assumes the former; the existing `specificity_score` suggests the latter is current practice.
