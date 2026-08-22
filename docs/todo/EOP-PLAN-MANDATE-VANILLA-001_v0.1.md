# Vanilla Investment Mandate — Vertical Slice Plan

### EOP-PLAN-MANDATE-VANILLA-001

| | |
|---|---|
| **Document** | EOP-PLAN-MANDATE-VANILLA-001 |
| **Type** | Tranche plan for implementation |
| **Version** | 0.1 — Draft |
| **Implements** | EOP-VS-MANDATE-001 v0.3 |
| **Target** | Zed / Sonnet implementation session |
| **Status** | Draft. T0 is blocking — no implementation tranche starts until it lands. |

---

## §0 What this slice is

The **narrowest end-to-end vanilla mandate** that exercises all four brick families and both attachment modes. Not a feature-complete matrix — a vertical slice proving the model works.

**In the slice:** universe coordinates, one ISDA/CSA prerequisite chain, two competing instruction routes (structured message and blotter), one pricing rule. Build via legal moves → save as blocks → reassemble → traverse → prove coverage and unambiguous resolution.

**Not in the slice:** the full instrument taxonomy, all settlement conventions, the complete pricing model, the view layer, the consumer API, retirement of the existing plpgsql generator (that is EOP-PLAN-MANDATE-001 T3).

**Why a slice first:** it validates the coordinate-vs-region distinction — the one genuinely novel piece — on a board small enough to enumerate exhaustively, before the full brick catalogue is committed to.

---

## §1 Candidate brick alphabet — **CORRECT THIS FIRST**

⚠ **This is the agent's domain inference, not the user's definitions.** No implementation tranche proceeds until it is red-penned. It is the grammar's alphabet: wrong here means an internally consistent grammar that cannot express real mandates — the failure that surfaces late, as an escape hatch added under delivery pressure.

**Universe coordinate dimensions**
- *Instrument type* — equity, government bond, corporate bond, ETF, money-market, FX, listed derivative, OTC derivative
- *Market / venue* — MIC
- *Currency* — ISO 4217
- *Settlement type* — DVP, FOP, (others?)

**Agreements**
- *ISDA* — counterparty-scoped; gates OTC derivatives
- *CSA* — child of ISDA; collateral terms

**Instruction routes**
- *Structured message* — SWIFT MT (MT540–543 settlement, MT535/536 statements) or ISO 20022 (sese.023 …)
- *Blotter / file* — CSV, batch, SFTP drop
- *Manual* — fallback

**Pricing**
- *Fee basis* — bps on AUM, per-transaction, tiered, minimum charge

**Known-uncertain (Open Rulings 3 & 4):**
- Route granularity — channel level or message-type level?
- Overlap — illegal, or resolved by specificity?
- Settlement-type enumeration — is DVP/FOP sufficient, or are there market-specific variants?

---

## §2 Tranches

### T0 — Alphabet and premise closure *(research only, no code)*

**Blocking.** Nothing downstream may proceed on inference.

**Deliverables**
1. The corrected brick catalogue: enumerated bricks per family, their attributes, their constitutive references.
2. The **type-compatibility matrix**: which brick types may attach to which.
3. Rulings on Open Rulings 3 & 4 (route granularity; overlap illegal vs specificity-resolved). *Ruling 4 changes Design Law 8 and the `resolution_unambiguous` gate — it cannot be deferred past T0.*
4. Closure of the ⚠ premises carried from EOP-PLAN-MANDATE-001: live generation path vs migration-only; table maturity for universe / booking-rule / ISDA / CSA; `sequence_order` as settlement fact or display ordinal; where instruction formats and pricing preferences live today; current AST consumers; whether a Rust-side taxonomy already exists.
5. Field-level classification of every mandate-related column: *constitutive fact* / *arrangement* / *presentation* / *negotiated term*.

**Gate:** every brick and column assigned exactly one category; disputed items flagged for ruling, not silently resolved.

---

### T1 — Brick types and content-derived identity

Define the sealed brick variants in Rust: discriminant, classifying attributes, constitutive references. Read path from existing entity tables. Identity derived from the entity's existing stable id (`uuidv7`), never minted.

**Not in scope:** assembly, rules, any write path.

**Gates**
- `no_arrangement_in_block` — *type-level*: no `parent`, `path`, `ordinal`, `depth`, `is_root`, or child collection on the block type. Non-gameable — a property of the type, not a value.
- `no_presentation_in_block` — same, for `status_color` / `is_loaded` / `leaf_count` / `label` / `sublabel`.
- `identity_is_stable` — same entity loaded twice yields the same block id; identity never calls a random or clock source.

---

### T2 — Coordinate space and universe placement

The coordinate type and the legality of a coordinate: which instrument/market/currency/settlement combinations are valid. Moves for placing universe bricks.

**Gates**
- `invalid_coordinate_rejected` — fuzz coordinate tuples; every invalid combination is rejected with a typed error.
- `duplicate_placement_rejected` — the same coordinate cannot be occupied twice (whole-board uniqueness, not cursor-local).
- `placement_is_total` — fuzz placement attempts; success or typed rejection, never a panic or a silently-wrong board.

---

### T3 — Agreements and prerequisites

ISDA and CSA bricks; the prerequisite rule (OTC coordinates require a matching counterparty ISDA) and the parent rule (no CSA without its ISDA).

**Gates**
- `otc_requires_isda` — placing an OTC coordinate without a matching ISDA is rejected.
- `csa_requires_isda` — an orphan CSA is unconstructible.
- `prerequisite_order_independent` — placing ISDA-then-OTC and OTC-then-ISDA (where the grammar permits deferral) converge, or the deferral is explicitly illegal. *Forces the ruling on whether prerequisites are strictly ordered.*

---

### T4 — Region claims: routes and pricing

The region-claim mechanism shared by both families: a matcher over the coordinate space with specificity. Moves for claiming a region.

**This is the novel tranche.** Everything before it is point placement; this is where the model is genuinely new and where silent failure is most likely.

**Gates**
- `region_claim_matches_expected_coordinates` — a claim covers exactly the coordinates its criteria describe; fuzz criteria and assert the covered set.
- `resolution_unambiguous` — fuzz coordinates; each resolves to exactly one route and one price. *Behaviour depends on Ruling 4: if overlap is illegal the grammar rejects the second claim; if specificity-resolved, resolution is deterministic and total.*
- `coverage_complete` — no eligible coordinate lacks a route or a price. Fails loudly on a gap — a tradeable instrument that cannot settle is a real operational failure, not a modelling nicety.

---

### T5 — Assembly

`assemble(blocks) -> Board`: resolve references into adjacency, derive the reverse index, derive root, verify, hash.

**Gates**
- `order_independence` — `assemble(shuffle(blocks)) == assemble(blocks)`, fuzzed, on a fixture with a join. *Keystone: proves no arrangement leaked into storage or row order.*
- `assembly_is_total` — fuzz **arbitrary** block sets, including sets no move sequence could produce (imports, repairs, migrations bypass the grammar). Well-formed board or typed rejection. Needs structured generation — this target does not get the grammar's free lunch.
- `dangling_ref_is_loud` — an unresolvable reference is a typed error, never a silent orphan or an "unclassified" bucket.
- `reverse_index_derived` — source assertion that no back-reference is read from storage.

---

### T6 — Legal-move grammar and the round trip

Make the grammar the sole mutation path, with a transient session cursor. Enabled-move **enumeration** (not merely checking) — required for cheap fuzzing and precedented by the KYC substrate's placement-set machinery.

⚠ *Reuse fit against the actual KYC substrate types is a premise, not a fact — verify before detailed planning.*

**Gates**
- `round_trip` — build via legal moves → break into blocks → save → reassemble from the table alone → assert equality. Fuzzed over random legal sequences. **The plan's keystone.**
- `grammar_preserves_invariants` — fuzz legal sequences; every reachable board is well-formed.
- `illegal_moves_rejected` — fuzz *illegal* moves against reachable states; all rejected. *The direction usually forgotten, and the only one that finds a hole in the legality check.*
- `cursor_never_persisted` — type/source assertion that no block field carries the cursor.
- `known_mandates_reachable` — each fixture mandate is constructible by some legal sequence. **This is the coverage proof for "any mandate from the defined brick set"** — it fails the day a real mandate cannot be built, instead of that surfacing as a production escape hatch.

---

### T7 — Vanilla mandate end-to-end

Assemble the full slice as a worked fixture and traverse it: get to a node, inspect neighbours and metadata.

**Gates**
- `vanilla_mandate_builds` — the complete slice constructs via legal moves only.
- `vanilla_mandate_round_trips` — saves, reassembles, and is identical.
- `traversal_neighbourhood` — from any node, predecessors and successors are reachable in O(neighbours) via the derived reverse index.

---

## §3 Sequencing

```
T0 (alphabet — BLOCKING)
   ↓
T1 (bricks) → T2 (coordinates) → T3 (agreements)
                                      ↓
                                 T4 (regions — novel)
                                      ↓
                          T5 (assembly) → T6 (grammar) → T7 (slice)
```

- **T0 blocks everything** — the brick shape determines every later type.
- **T2 before T4** — regions claim over the coordinate space, so the space must exist first.
- **T5 before T6** — assembly is testable without the grammar; the grammar's `round_trip` gate depends on assembly.
- **T4 is the risk tranche** — schedule review attention there.

## §4 Risks

1. **Alphabet is inferred (§1).** Highest risk in the plan; T0 exists solely to retire it.
2. **Ruling 4 (overlap) changes T4's gate semantics** — cannot be deferred past T0.
3. **KYC grammar reuse is a premise** (T6), not a verified fact.
4. **Coverage may be intentionally partial** — some mandates may legitimately leave coordinates unrouted (manual fallback). If so, `coverage_complete` must assert *coverage-or-explicit-fallback*, not blanket coverage. Confirm in T0.

## §5 What this plan does not specify

Gates pin the **contract**, not the mechanism. Internal data structures, module layout, and algorithms are the implementer's choice — except "adopt a published RPST rather than hand-rolling" — provided the gates hold.
