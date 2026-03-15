# Cross-Border Constellation Runtime Remediation Plan

## Objective

Close the runtime gap for the two cross-border constellation maps:

- `struct.hedge.cross-border`
- `struct.pe.cross-border`

without changing the existing reducer/constellation architecture.

The current state is:

- the YAML maps exist and validate
- the map schema can express child `cbu` slots
- the runtime does not have a persisted CBU-to-CBU structure-link model that child `cbu` hydration can query

This plan preserves:

- the current constellation map schema
- the current reducer architecture
- the current action-surface model
- the current REST/UI contract shape

It adds only the missing persistence and hydration semantics for linked CBUs.

## Constraints

- Do not redesign constellation maps.
- Do not redesign slot normalization.
- Do not redesign reducer evaluation.
- Do not replace the current `cbus` / `cases` / `cbu_entity_roles` based hydration model.
- Do not introduce a separate graph engine for CBU structure.

## Design Principle

Treat cross-border CBU relationships as first-class persisted facts, analogous to how:

- role slots use `cbu_entity_roles`
- ownership chains use `entity_relationships`

Cross-border child CBU slots should hydrate from a dedicated CBU structure-link table, not from inferred naming or ad hoc joins.

## Phase 1: Persisted Link Model

Add a new table for CBU-to-CBU structure relationships.

Proposed shape:

- `link_id UUID PK`
- `parent_cbu_id UUID NOT NULL`
- `child_cbu_id UUID NOT NULL`
- `relationship_type VARCHAR(...) NOT NULL`
- `relationship_selector VARCHAR(...) NOT NULL`
- `status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE'`
- `capital_flow VARCHAR(...) NULL`
- `effective_from DATE NULL`
- `effective_to DATE NULL`
- `created_at TIMESTAMPTZ NOT NULL`
- `updated_at TIMESTAMPTZ NOT NULL`

Required relationship types:

- `FEEDER`
- `PARALLEL`
- `AGGREGATOR`

Optional but useful:

- `MASTER`
- `CO_INVEST_VEHICLE`

Required statuses:

- `ACTIVE`
- `TERMINATED`
- `SUSPENDED`

Rules:

- uniqueness on active `(parent_cbu_id, child_cbu_id, relationship_type)`
- prevent self-links
- FK to `"ob-poc".cbus(cbu_id)` on both sides

Recommended implementation:

- `CHECK (status IN ('ACTIVE', 'TERMINATED', 'SUSPENDED'))`
- partial unique index scoped to active links
- hydrator filters to `status = 'ACTIVE'`
- keep both:
  - `relationship_type` for coarse business meaning such as `FEEDER`
  - `relationship_selector` for slot-level disambiguation such as `feeder:us`

Rationale:

- this keeps cross-border structure as a CBU-level fact
- it avoids overloading `entity_relationships`, which is entity-level
- it matches the already parked `cbu.link-structure` macro intent

## Phase 2: Verb Surface

Implement the missing primitive verb:

- `cbu.link-structure`

Minimal responsibilities:

- validate both CBUs exist
- validate relationship type
- validate relationship selector when provided
- insert or upsert active link
- return the `child_cbu_id` as the operational result, not the link row id

Add read helpers as needed:

- `cbu.list-structure-links`
- optional `cbu.unlink-structure`

Macro alignment:

- update the cross-border macros so the currently parked `cbu.link-structure` steps can be enabled

Non-goals:

- no macro redesign
- no new operator-domain abstraction

## Phase 3: Constellation Hydration Support

Extend child `cbu` slot hydration in `rust/src/sem_reg/constellation/hydration.rs`.

Current behavior:

- root `cbu` slot hydrates from the requested `cbu_id`
- non-root `cbu` slots return empty

Target behavior:

- if a non-root `cbu` slot has a declared relationship meaning, hydrate matching child CBU rows from the persisted link table

Hydration direction convention for this remediation:

- constellation root is always the parent CBU
- child `cbu` slots hydrate downward from `parent_cbu_id -> child_cbu_id`
- loading a feeder CBU directly does not reverse-resolve the parent constellation
- reverse navigation is explicitly out of scope for this pass

Implementation approach:

1. Add a small convention for child `cbu` slots in the two cross-border maps.
2. Use `join.filter_value` on the child `cbu` slot to carry the relationship selector, while keeping the existing slot schema.
3. In `query_slot_rows` / `query_slot_rows_tx`:
   - for root `cbu`, preserve current behavior
   - for child `cbu`, query the structure-link table by:
     - `parent_cbu_id = root cbu id`
     - `relationship_selector = join.filter_value`
     - `status = 'ACTIVE'`

Why this preserves architecture:

- no schema redesign
- no special-case map family branching
- child `cbu` hydration remains slot-driven

## Phase 4: Cross-Border Map Remediation

Update only M17 and M18 map YAMLs to use the persisted relationship semantics.

For example:

- `us_feeder` child slot uses `type: cbu`
- `join.via: cbu_structure_links`
- `join.parent_fk: parent_cbu_id`
- `join.child_fk: child_cbu_id`
- `join.filter_column: relationship_selector`
- `join.filter_value: feeder:us`

Equivalent for:

- `ie_feeder` -> `feeder:ie`
- `us_parallel` -> `parallel:us`
- `aggregator` -> `aggregator`

This keeps the maps declarative and avoids runtime hardcoding of slot names.

Why selector values instead of bare relationship types:

- M17 has multiple feeder children under one parent
- both are semantically feeders but must hydrate into different slots
- encoding the distinction in the data avoids brittle runtime branching on slot names or child CBU jurisdiction

The persisted link row therefore needs both:

- a coarse `relationship_type` for business semantics, such as `FEEDER`
- a slot-facing `relationship_selector` string, such as `feeder:us`

## Validator Compatibility

The remediation assumes the existing map schema and validator continue to allow:

- `type: cbu` on non-root child slots
- `join` blocks on child `cbu` slots

If any validator path still treats `cbu` as root-only, the fix remains intentionally narrow:

- extend the existing `SlotType::Cbu` acceptance path for child positions
- do not introduce a new slot type

This keeps M17/M18 expressible without changing map architecture.

## Phase 5: Action Surface and Summary Semantics

Preserve current per-slot action-surface computation.

Required clarification to encode in implementation:

- child CBUs are structure slots, not entity slots
- they should surface:
  - `filled` when linked CBU exists
  - `empty` when link absent
- they should not run `entity_kyc_lifecycle` unless a dedicated child-CBU reducer contract is later introduced

For this remediation:

- keep child `cbu` slots structural only
- no reducer state machine on child `cbu` slots
- no roll-up semantics across parent/child CBUs yet

This avoids architectural expansion and keeps the fix narrow.

## Phase 6: API / UI Impact

No REST contract redesign is required.

Existing constellation hydrate/summary endpoints remain unchanged.

Expected improvement:

- M17/M18 child CBU slots stop appearing as permanently empty placeholders
- the UI can render linked feeders/parallels/aggregators as hydrated child slots automatically

No UI protocol changes are required unless we later choose to expose:

- relationship type badges
- capital flow annotations

## Phase 7: Tests

Add:

- migration-level smoke test if available
- verb tests for `cbu.link-structure`
- constellation hydration tests covering:
  - parent with one feeder
  - parent with two feeders
  - parent with no feeders
  - PE main fund with US parallel and aggregator

Add integration coverage for:

- linked child CBUs appear under root in hydrated constellation
- child slot names map to the expected relationship types
- explicit case selection still works independently per CBU

Preserve current tests for non-cross-border maps unchanged.

## Phase 8: Documentation

Update:

- constellation map docs
- macro docs for M17/M18
- SemOS architecture notes where cross-border structure is described

Document explicitly:

- child `cbu` slots are backed by persisted CBU structure links
- `entity_relationships` remains entity-level only
- cross-border constellation hydration is downward only from the parent/root CBU

## Implementation Order

1. Add `cbu_structure_links` migration.
2. Implement `cbu.link-structure`.
3. Extend hydrator for non-root `cbu` slots.
4. Update M17/M18 YAMLs to use real child-CBU joins.
5. Add tests.
6. Run `cargo check` and constellation test suite.

## Risks

### Risk 1: Relationship semantics leak into reducer logic

Mitigation:

- keep child `cbu` slots structural only in this pass

### Risk 2: Multiple child CBUs of same relationship type

Mitigation:

- use explicit `relationship_selector` values such as `feeder:us` and `feeder:ie`
- keep `relationship_type` as the coarse business category
- avoid ambiguous “pick one of many FEEDER children” logic in the hydrator

### Risk 3: Child `cbu` slots rejected by validator

Mitigation:

- preserve `type: cbu` as a valid slot type for child positions
- keep the existing validator relaxation for non-root `cbu` slots that hydrate structurally
- do not introduce a new slot class

### Risk 4: Existing macros create child CBUs but do not persist links

Mitigation:

- enable the parked `cbu.link-structure` steps as part of the same remediation

## Completion Criteria

The remediation is complete when:

- `cbu.link-structure` exists and persists links
- M17/M18 maps validate and hydrate child `cbu` slots from real data
- linked feeders/parallels/aggregators appear in hydrated constellation payloads
- `cargo check -p ob-poc` passes
- constellation test coverage passes with cross-border fixtures
