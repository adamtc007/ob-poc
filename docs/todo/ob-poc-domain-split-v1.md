# ob-poc-domain split v1 — per-business-capability crates

> **Status:** PROPOSAL — awaiting review.
> **Author:** prepared 2026-05-14.
> **Drives:** Tier 3 #7 of the post-`pub`-surface audit; the open question in
> `ob-poc-domain/src/lib.rs:53-55` ("if this crate grows past ~2k LOC across
> unrelated domains, split into per-domain crates").

## Why now

Capability-crate restructure v1 created `ob-poc-domain` as a single flat-DTO
crate (Phase 4, 2026-05-13). The crate has since grown to **17,552 LOC across
10 unrelated business domains** — 8.8× the threshold the v1 plan called out
as the trigger for per-domain splits. The author's own note in `lib.rs`
("re-evaluate at bed-in review per plan §9 … split into per-domain crates
(`ob-poc-deal`, `ob-poc-booking-principal`, …)") IS this slice.

The audit recommended waiting 30 days for cross-domain co-edit data before
splitting. The user has chosen to skip that wait on the basis that the
business capabilities here are genuinely independent (deal vs trading profile
vs BODS/LEI vs entity linking are different things, owned by different
parts of the bank). The cost of getting seams wrong is small because the
internal dep graph (below) is almost fully orthogonal.

## Module inventory (post Phase 4, current state)

| Module | LOC | Files | Feature | Internal deps | External deps |
|---|---|---|---|---|---|
| `trading_profile` | 5,632 | 6 | `database` | (self) | `ob_poc_types` |
| `taxonomy` | 5,438 | 11 | `database` | `view_config_service` | `ob_poc_types` |
| `view_config_service` | 1,032 | 1 | `database` | — | `ob_poc_types` |
| `entity_linking` | 1,664 | 7 | `database` | (self) | `ob_poc_types` |
| `ontology` | 1,242 | 6 | (no DB) | (self) | `ob_poc_types::semantic_stage` |
| `derived_attributes` | 739 | 2 | `database` | `advisory_lock` | `ob_poc_types` |
| `semtaxonomy` | 514 | 1 | `database` | — | `ob_poc_types` |
| `booking_principal_types` | 485 | 1 | (no DB) | — | `chrono`, `serde`, `uuid` |
| `deal_types` | 287 | 1 | `database` | — | `chrono`, `rust_decimal` |
| `bods_types` | 218 | 1 | `database` | — | `chrono`, `rust_decimal` |
| `advisory_lock` | ~90 | 1 | `database` | — | `sqlx` |
| **Total** | **17,341** | 36 | — | — | — |

(LOC count differs slightly from `wc -l` totals because of `mod.rs` lines and
duplicate counting; the per-file split is what matters for cluster sizing.)

## Internal dependency graph

```
trading_profile        (self-contained)
booking_principal_types (no deps)
deal_types             (no deps)
bods_types             (no deps)
semtaxonomy            (no deps)
ontology               (self only)
entity_linking         (self only)

derived_attributes ──► advisory_lock      [forced pairing #1]
taxonomy           ──► view_config_service [forced pairing #2]
```

Two forced pairings — anything else can move independently.

## External consumer map

Every module is consumed by `ob-poc` (the application crate) and only there.
No other capability crate reaches into `ob-poc-domain::*` directly:

| Module | Consumer files in `rust/src/` |
|---|---|
| `booking_principal_types` | 5 (api, database, domain_ops × 3) |
| `deal_types` | 4 (api × 2, database, graph) |
| `bods_types` | 1 (database/mod.rs re-export) |
| `view_config_service` | 1 (database/mod.rs re-export) |
| `advisory_lock` | 1 (database/locks.rs) |
| all other 6 modules | 1 each (top-level `lib.rs` re-export) |

This means **every split is just an internal rename inside `ob-poc`** — no
cross-crate API surface changes. The capability-crate restructure
"opaque/break/break/keep" edge rules don't apply because no capability crate
reaches `ob-poc-domain` directly today.

## Proposal — 9-crate split

| New crate | LOC | Sources | Why one crate |
|---|---|---|---|
| `ob-poc-booking-principal` | 485 | `booking_principal_types` | Onboarding/commercial reference data — distinct ownership |
| `ob-poc-bods` | 218 | `bods_types` | BODS 0.4 / LEI spine — distinct reference standard |
| `ob-poc-deal` | 287 | `deal_types` | Deal taxonomy / fee billing — distinct lifecycle |
| `ob-poc-trading-profile` | 5,632 | `trading_profile/` | Largest single business capability; mostly self-contained |
| `ob-poc-taxonomy` | 6,470 | `taxonomy/` + `view_config_service.rs` | Forced pairing; both are vocabulary/layout infrastructure |
| `ob-poc-ontology` | 1,242 | `ontology/` | Lifecycle stage definitions — distinct from taxonomy combinators |
| `ob-poc-semtaxonomy` | 514 | `semtaxonomy.rs` | Entity-extraction layer — distinct concern |
| `ob-poc-derived-attributes` | 829 | `derived_attributes/` + `advisory_lock.rs` | Forced pairing; canonical derived-value plane |
| `ob-poc-entity-linking` | 1,664 | `entity_linking/` | Mention extraction / resolution — distinct concern |
| **Total** | **17,341** | — | — |

**Workspace net:** 33 crates today → 33 − 1 (delete `ob-poc-domain`) + 9 = **41 crates**.

### Alternative: 5-cluster grouping

If 9 crates feels noisy, the cluster grouping is:

- `ob-poc-commercial` (booking_principal + bods + deal) — 990 LOC
- `ob-poc-trading-profile` (alone) — 5,632 LOC
- `ob-poc-taxonomy` (taxonomy + view_config_service + ontology + semtaxonomy) — 8,226 LOC
- `ob-poc-derived-attributes` (derived + advisory_lock) — 829 LOC
- `ob-poc-entity-linking` (alone) — 1,664 LOC

The 9-crate split better matches the user's intent ("they are separate
capabilities") and gives finer compile-time granularity. The 5-cluster
version is the conservative fallback if you decide 9 is overshooting.

**Recommended:** go with 9. Reverting a too-fine split (merge two crates
back into one) is mechanical; un-merging a too-coarse split requires
re-detangling.

## Open decisions for review

1. **9 crates vs 5 clusters?** (recommendation: 9)
2. **Where does `advisory_lock` settle long-term?** It only has one current
   consumer (`derived_attributes`), but if a second consumer appears it
   should hoist to `ob-poc-types`. Park it inside `ob-poc-derived-attributes`
   for now (consistent with v1 plan §6 decision 3 "helpers go with their
   primary consumer").
3. **Naming pattern.** Today the convention is `ob-poc-<capability>`
   (`ob-poc-sage`, `ob-poc-domain`, `ob-poc-authoring`). Following that
   pattern is straightforward except `ob-poc-derived-attributes` is
   uncomfortably long — `ob-poc-derived` is a fine alias if you prefer.
4. **Where does the `ob-poc-domain` crate itself go?** It vanishes
   completely after the split. The directory `crates/ob-poc-domain/` is
   deleted; the workspace member entry is removed; no compat re-export
   stub is preserved (nothing depends on it externally — see consumer
   map above).
5. **Do we update `ob-poc-types` at the same time?** No. The plan §6.5
   rule still says "cross-capability DTOs (≥2 consumers) live in
   `ob-poc-types`." Nothing in the proposed splits has ≥2 capability-crate
   consumers today. If a type promotes later, that's a separate slice.

## Slice plan

The split is one logical change but lands across multiple commits to keep
each step independently verifiable. The pattern mirrors capability-crate
restructure v1 Phases 2–5: skeleton → relocate → wire → tighten.

### Pre-flight (single commit)

- Create 9 empty crate skeletons (`Cargo.toml` + `src/lib.rs` with `//!`
  charter + `unreachable_pub = "deny"`) under `crates/`.
- Add all 9 to `rust/Cargo.toml` workspace members.
- Add `ob-poc-*` deps to `ob-poc` root `Cargo.toml` (gated `database`
  where appropriate).
- `cargo check --workspace` clean. No code moved yet.

### Slice A — small leaf DTOs (3 commits, low risk)

Order chosen by zero internal deps first; each is a single-file `git mv`
plus consumer rewires inside `ob-poc`:

1. `ob-poc-bods` ← `bods_types.rs` (218 LOC, 1 consumer: `database/mod.rs`)
2. `ob-poc-deal` ← `deal_types.rs` (287 LOC, 4 consumers in `api/database/graph`)
3. `ob-poc-booking-principal` ← `booking_principal_types.rs` (485 LOC, 5 consumers)

Each commit: `git mv` the file, update the 1–5 consumer imports in
`rust/src/`, delete the `pub mod` line from `ob-poc-domain/src/lib.rs`,
run `cargo check --features database` + `cargo test --no-run`.

### Slice B — self-contained domains (4 commits, medium risk)

4. `ob-poc-semtaxonomy` ← `semtaxonomy.rs` (514 LOC)
5. `ob-poc-ontology` ← `ontology/` (6 files, 1,242 LOC)
6. `ob-poc-entity-linking` ← `entity_linking/` (7 files, 1,664 LOC)
7. `ob-poc-trading-profile` ← `trading_profile/` (6 files, 5,632 LOC) — the
   largest single relocation; do it in isolation to bound the diff blast
   radius.

Each commit follows the same pattern. Trading profile is the only one large
enough that a separate `pub(crate)`-tightening follow-up may be worth doing.

### Slice C — forced pairings (2 commits, medium risk)

8. `ob-poc-derived-attributes` ← `derived_attributes/` + `advisory_lock.rs`
   (combined 829 LOC). One consumer: `src/database/locks.rs` uses
   `advisory_lock`; `src/lib.rs` re-exports `derived_attributes`.
9. `ob-poc-taxonomy` ← `taxonomy/` + `view_config_service.rs` (combined
   6,470 LOC). The taxonomy submodule already reaches view_config_service
   via `use crate::view_config_service::*` — that stays an intra-crate
   import after the move.

### Slice D — bury `ob-poc-domain` (1 commit)

After all 9 modules have moved, `ob-poc-domain` is empty (just the
`//!` charter doc). Delete the crate directory, remove the workspace
member entry, remove the `ob-poc-domain` dep from `rust/Cargo.toml` root
package. Update `CLAUDE.md` crate count (33 → 41) and the breakdown line.

### Slice E — `pub(crate)` tightening pass (1 commit, optional follow-up)

Each new crate's `lib.rs` will have lossy `pub use module::*` glob
re-exports inherited from the old `ob-poc-domain`. Walk each new crate
and convert glob re-exports to explicit allowlists per the v1 CLAUDE.md
Rule 5 ("Re-export Types at Module Boundary"). This is the only step that
might shake out true-unused-pub items — `unreachable_pub = "deny"` will
catch the rest.

Estimated total: **11 commits**, each individually buildable + testable.

## Risk register

- **Compat re-exports inside `ob-poc`.** Today `src/api/mod.rs` does
  `pub use ob_poc_domain::deal_types;` etc. Each slice rewrites those
  to `pub use ob_poc_deal::*;`. No call site changes because callers
  use `crate::api::deal_types::*` paths via the re-export. Verify the
  full set in Slice A.

- **`feature = "database"` gating.** Most relocated modules are gated
  `#[cfg(feature = "database")]` in `ob-poc-domain`. The new crates
  carry their own `[features] database = ["sqlx/...", ...]` block;
  `ob-poc`'s `database` feature must activate the corresponding feature
  on each new dep. Mechanical but easy to miss for one crate.

- **`ob-poc-types` cross-dep growth.** Several modules use
  `ob_poc_types::{semantic_stage, ...}`. Each new crate will declare
  `ob-poc-types` as a dep. No risk, just hygiene — but verify
  `ob-poc-types` doesn't grow accidentally during the cutover.

- **Cargo cycle risk.** None expected — none of the 9 new crates
  depend on each other, and none depend on any capability crate
  (`ob-poc-boundary`, `ob-poc-sage`, `ob-poc-journey`, `ob-poc-authoring`).
  They all sit at the same layer as `ob-poc-types`. Confirm with
  `cargo tree -p ob-poc-trading-profile --depth 2` after Slice B step 7.

- **Test fixtures.** A few of these modules carry inline `#[cfg(test)]`
  tests. They move with the file via `git mv`, preserving git blame.
  No external test files touch `ob-poc-domain::*` paths directly.

## Verification gate

For each slice commit:

1. `cargo check --workspace --all-features` clean.
2. `cargo test --workspace --all-features --no-run` clean.
3. `cargo tree -p ob-poc-<new-crate>` shows no cycles, correct dep set.
4. `cargo build -p ob-poc --features database` clean (smoke for the
   integrated app).

At the end of Slice E, also run:

5. `cd rust && cargo test --workspace --all-features` (full test execution).
6. Visual diff of `rust/Cargo.toml` workspace members.
7. Update memory file `project_capability_crate_restructure.md` with the
   post-split state.

## Things this plan does NOT do

- Does **not** move any logic out of `ob-poc::database::*` /
  `ob-poc::services::*`. Those repositories stay where they are — only
  the DTOs/typed reference data move.
- Does **not** introduce a new capability tier above these crates. The
  9 new crates sit at the same layer as today's `ob-poc-domain`.
- Does **not** touch `ob-poc-types`. Promotion of any DTO to `ob-poc-types`
  is a separate decision per the §6.5 rule.
- Does **not** consider per-domain databases or schema splits. Domain
  data still lives in the single `data_designer` PostgreSQL DB.

## Decision needed

Approve / redirect / modify:

- [ ] **Option A**: 9 per-capability crates (recommended)
- [ ] **Option B**: 5 cluster crates (conservative)
- [ ] **Option C**: redirect — split only the largest (trading_profile +
       taxonomy) and leave the rest in a slimmer `ob-poc-domain`
- [ ] Other naming / clustering preference

Once approved, execution proceeds in the slice order above. Each slice
is independently revertable.
