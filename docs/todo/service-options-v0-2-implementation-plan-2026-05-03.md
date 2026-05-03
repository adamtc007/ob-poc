# Service Options v0.2 Implementation Plan - 2026-05-03

Status: draft for Adam review. Updated with Adam feedback on scope and planning issues.

Source paper: `/Users/adamtc007/Downloads/service-options-architecture-vs-v0.2-2026-05-03.md`

Related evidence:

- `docs/review/product-service-resource-taxonomy-peer-review-2026-05-03.md`
- `rust/config/sem_os_seeds/dag_taxonomies/product_service_taxonomy_dag.yaml`
- `rust/config/sem_os_seeds/constellation_maps/product_service_taxonomy.yaml`
- `rust/config/packs/product-service-taxonomy.yaml`
- `rust/config/verbs/{product,service,service-resource,service-pipeline}.yaml`
- `rust/config/srdefs/{custody,connectivity,iam}.yaml`

## Sufficiency Assessment

The v0.2 paper is sufficient to prepare an implementation plan and to implement the core framework for the approved scope: Custody and Fund Accounting.

It is not sufficient to complete the wide rollout across all 30 services without additional SME input. That wide rollout is out of scope for this tranche. Missing data is handled by explicit validation gaps, not invented business content.

Implementation can proceed after approval with these assumptions:

- No new workspace. Extend `product_maintenance`, existing CBU workspace, and existing IM coupling.
- Preserve design-time/runtime separation. Product-maintenance can author and inspect definitions; CBU activation binds, validates, fans out, and provisions.
- Use existing plugin-op and YAML verb patterns.
- Use `service-resource.provision` for materialized instances; do not create special-case provisioning verbs where the generic verb can carry the request.
- Keep existing `service_intents.options` for compatibility during migration, but do not treat it as the new source of truth for option bindings.
- Implement pilot catalogue content only for Custody and Fund Accounting.
- Use gap-producing classifications for missing Fund Accounting/Custody content where SME data is not yet available.

In-scope services for this tranche:

- Custody: `SAFEKEEPING`, `SETTLEMENT`, `CASH_MGMT`, `CORP_ACTIONS` where existing SRDEF/catalogue data supports it.
- Fund Accounting: `NAV_CALC`, `FUND_REPORTING`, and `ASSET_PRICING` as either seeded content or explicit validation gaps.

## Decisions Needed Before Coding

1. `product_service_conditions` is required.
   - The paper says conditional predicates live in a separate table, referenced by `product_service_option_overrides.activation_condition_ref`.
   - That table does not exist and is not counted in the six-table headline.
   - Decision: add it as a named design-time table. The paper's "six-table extension" becomes a planning-layer correction to seven core option tables.

2. Product-version carrier is missing.
   - The paper recommends product-service overrides be implicitly scoped to product version.
   - The repo has `products.effective_from/effective_to` and `metadata`, but no `product_versions` table.
   - Decision for v1: do not add `product_versions`; add effective dating and supersession fields on `product_service_option_overrides`, then revisit explicit product versions later.
   - Phase 0 must check whether existing `products.effective_from/effective_to` columns are populated and used. If active, overrides should consume product effective dating where possible instead of creating a contradictory temporal model.

3. Lineage contribution types.
   - Paper recommends `eligibility`, `fanout`, `attribute_source` for v1, with possible later `dependency` and `precondition`.
   - Decision: implement only the three v1 values. Dependency lineage remains implicit through `resource_instance_dependencies` until a real gap appears.

4. Source-kind storage.
   - Existing `resource_attribute_requirements.source_policy` is JSONB and heavily used by SRDEF loader/discovery/API code.
   - Decision: add structured columns first, backfill from JSONB, update readers, then leave `source_policy` as deprecated compatibility data for one release rather than dropping it in the same tranche.
   - Add a named follow-up: Phase 11 source-policy retirement, blocked on no remaining readers.
   - Existing `entity` source kinds require a manual SME/architect pass; do not blanket-map `entity` to `legal_entity`.

5. Service-version data state.
   - `rust/migrations/20260428_service_lifecycle.sql` defines `service_versions`, but the root `migrations/master-schema.sql` may be stale relative to `rust/migrations`.
   - Decision: Phase 0 refreshes schema export and confirms `service_versions` exists before adding `service_option_defs.service_version_id`.
   - Phase 0 must also seed a baseline published v1 service version for every active service that lacks one.
   - Phase 0 must confirm the actual service-version PK column name before writing the FK. The current migration appears to use `service_versions.id`; verify against exported schema.

6. IM source contract table/path.
   - The paper defines the twelve minimum IM field paths, but the implementation must map those paths to existing IM tables/attributes.
   - Decision: pilot only the fields required by SETTLEMENT and TRADE_CAPTURE, then expand after the IM path audit.
   - Pilot field paths are five total:
     - SETTLEMENT: `preferred_speed`, `counterparties`
     - TRADE_CAPTURE: `instruction_channel`, `bic`, `message_types`
   - Document these in `docs/architecture/im-source-paths.md` during Phase 6/7.

Additional ratified additions:

7. Add `activation_runs`.
   - `activation_run_id` should not remain an unanchored UUID.
   - Add a small runtime table for activation metadata, replay anchors, status, operator, timestamps, and summary gaps.

8. Pin `value_hash`.
   - Use SHA-256 over canonical JSON.
   - Canonicalization must be documented and stable across JSON key ordering, Postgres versions, and app restarts. Use RFC 8785 JCS if a suitable crate is acceptable; otherwise document the local canonicalization precisely and test it.

9. Add replay-from-history pilot test.
   - After IM mutation, replay the original activation run and verify identical bindings, resource fan-out, and attribute values to the original run.

10. Make closure-gap audit explicit.
    - Phase 4 must extend and rerun the DAG closure-gap audit for new Class A verbs and report zero new closure gaps.

## Current Repo Facts That Shape The Plan

- `products`, `services`, `product_services`, `service_resource_types`, `service_resource_capabilities`, `service_intents`, and `cbu_resource_instances` already exist.
- `service_resource_capabilities.supported_options` is the current loose eligibility precedent.
- `service_resource_types` already has legacy `per_market`, `per_currency`, and `per_counterparty` booleans, but the live data has all three false across active rows.
- `resource_attribute_requirements.source_policy` is JSONB with the current default `["derived", "entity", "cbu", "document", "manual"]`.
- `derived_attribute_values` and `derived_attribute_dependencies` already exist. The paper's derived-attribute persistence precondition appears to be satisfied, but needs a Phase 0 verification check.
- Historical `service_option_definitions` and `service_option_choices` tables were dropped by `migrations/071_drop_orphan_tables.sql`; do not resurrect that old model. Implement the v0.2 schema cleanly.
- `service-resource.provision` already supports `cbu-id`, `product-id`, `service-id`, `market-id`, `currency`, `counterparty-entity-id`, config, and dependency refs. This is the right provisioning primitive for fan-out output.

## Phase 0 - Baseline And Schema Truth

Goal: establish a clean baseline before adding the option model.

Deliverables:

- Run schema export and confirm current schema truth:
  - `cargo run -p xtask -- schema-export`
  - confirm `service_versions` appears in generated schema.
  - confirm `service_versions` PK column name.
- Confirm derived persistence exists:
  - `derived_attribute_values`
  - `derived_attribute_dependencies`
- Confirm `service_versions` row coverage:
  - every active service has at least one service version.
  - if missing, seed baseline `v1` rows with `lifecycle_status = published`.
- Confirm product effective dating usage:
  - check `products.effective_from/effective_to` population.
  - decide whether override effective dating should mirror product dating for v1.
- Snapshot current catalogue counts:
  - products/services/product_services
  - service_resource_types/capabilities
  - resource_attribute_requirements/dependencies
  - service_intents/service_delivery_map/cbu_resource_instances
- Confirm existing test baseline before edits:
  - `cargo check`
  - `cargo test --workspace`
  - `cargo run -p xtask -- verbs lint`
  - `cargo run -p xtask -- reconcile validate`

Acceptance:

- Baseline is known.
- Any pre-existing failures are recorded before Phase 1.

## Phase 1 - Schema Foundation

Goal: add the v0.2 schema with compatibility-safe migrations.

This phase adds seven core option-model tables plus the supporting `activation_runs` table.

### 1.A Core Design-Time Tables

Add `service_option_defs`.

Required shape:

- `service_option_def_id uuid primary key`
- `service_id uuid not null references services(service_id)`
- `service_version_id uuid not null references service_versions(id)`
- `option_key text not null`
- `option_kind text not null`
- `allowed_values jsonb`
- `default_value jsonb`
- `is_required boolean not null default false`
- `is_fanout_driver boolean not null default false`
- `fanout_axis text not null default 'none'`
- `default_source_kind text not null`
- `source_path text`
- `fallback_policy jsonb not null default '[]'`
- `override_policy text not null default 'allowed_with_reason'`
- `lifecycle_status text not null default 'drafted'`
- `description text`
- `created_at/updated_at`

Constraints:

- unique `(service_version_id, option_key)`
- check `option_kind in ('single_choice','multi_choice','range','boolean','structured','string')`
- check `fanout_axis in ('none','market','currency','counterparty','account','fund','share_class','legal_entity','instruction_channel','jurisdiction','booking_principal')`
- check `default_source_kind in ('derived','cbu_profile','instrument_matrix','legal_entity','document','product_option','manual','option_binding')`
- check lifecycle states `drafted`, `active`, `deprecated`, `retired`
- check `is_fanout_driver = false` implies `fanout_axis = 'none'`

Add `product_service_conditions`.

Required shape:

- `condition_id uuid primary key`
- `condition_key text unique not null`
- `description text`
- `predicate jsonb not null`
- `predicate_dsl text`
- `lifecycle_status text not null default 'active'`
- `created_at/updated_at`

Purpose:

- structured, queryable predicates for conditional product-service and product-service-option applicability.
- `predicate_dsl` is governance/readability only; execution uses structured `predicate` JSONB.

Add `product_service_option_overrides`.

Required shape:

- `override_id uuid primary key`
- `product_id uuid not null references products(product_id)`
- `service_id uuid not null references services(service_id)`
- `service_option_def_id uuid not null references service_option_defs(service_option_def_id)`
- `default_value_override jsonb`
- `allowed_values_override jsonb`
- `is_required_override boolean`
- `source_precedence_override jsonb`
- `activation_condition_ref uuid references product_service_conditions(condition_id)`
- `effective_from timestamptz not null default now()`
- `effective_to timestamptz`
- `supersedes_override_id uuid references product_service_option_overrides(override_id)`
- `created_at/updated_at`

Constraints:

- one current row per `(product_id, service_id, service_option_def_id)` where `effective_to is null`
- allowed-values override narrows generic allowed values in application validation.

Add `service_resource_option_constraints`.

Required shape:

- `constraint_id uuid primary key`
- `service_id uuid not null references services(service_id)`
- `resource_id uuid not null references service_resource_types(resource_id)`
- `service_option_def_id uuid not null references service_option_defs(service_option_def_id)`
- `supported_values jsonb not null default '{}'`
- `match_operator text not null default 'intersect'`
- `priority integer not null default 100`
- `is_required_for_coverage boolean not null default false`
- `is_active boolean not null default true`
- `created_at/updated_at`

Constraints:

- check `match_operator in ('exact','subset','superset','intersect')`
- unique active logical row where appropriate.

Add `service_resource_fanout_rules`.

Required shape:

- `fanout_rule_id uuid primary key`
- `service_id uuid not null references services(service_id)`
- `resource_id uuid not null references service_resource_types(resource_id)`
- `service_option_def_id uuid references service_option_defs(service_option_def_id)`
- `fanout_axis text not null`
- `fanout_mode text not null`
- `group_by_policy jsonb not null default '{}'`
- `shared_when_null boolean not null default true`
- `priority integer not null default 100`
- `is_active boolean not null default true`
- `created_at/updated_at`

Constraints:

- check same `fanout_axis` enum as `service_option_defs`
- check `fanout_mode in ('per_value','shared','grouped','conditional')`

### 1.B Runtime Tables

Add `activation_runs`.

Required shape:

- `activation_run_id uuid primary key`
- `cbu_id uuid not null references cbus(cbu_id)`
- `product_id uuid references products(product_id)`
- `run_kind text not null`
- `status text not null default 'started'`
- `triggered_by text`
- `started_at timestamptz not null default now()`
- `completed_at timestamptz`
- `failed_at timestamptz`
- `failure_reason text`
- `input_snapshot jsonb not null default '{}'`
- `result_summary jsonb not null default '{}'`
- `created_at/updated_at`

Constraints:

- check `run_kind in ('bind_options','validate_coverage','compute_fanout','activate','replay')`
- check `status in ('started','succeeded','failed','cancelled')`

Add `cbu_service_option_bindings`.

Required shape:

- `binding_id uuid primary key`
- `cbu_id uuid not null references cbus(cbu_id)`
- `product_id uuid references products(product_id)`
- `service_id uuid not null references services(service_id)`
- `service_version_id uuid not null references service_versions(id)`
- `service_option_def_id uuid not null references service_option_defs(service_option_def_id)`
- `option_key text not null`
- `value jsonb not null`
- `source_kind text not null`
- `source_ref jsonb`
- `source_version text`
- `value_hash text not null`
- `coherence_status text not null default 'clean'`
- `is_locked boolean not null default false`
- `valid_from timestamptz not null default now()`
- `valid_to timestamptz`
- `supersedes_binding_id uuid references cbu_service_option_bindings(binding_id)`
- `activation_run_id uuid references activation_runs(activation_run_id)`
- `created_at/updated_at`

Constraints/indexes:

- check `source_kind` enum from the paper.
- check `coherence_status in ('clean','dirty','stale')`.
- one current row per `(cbu_id, service_id, service_option_def_id)` where `valid_to is null`.
- index by `source_kind`, `source_ref`, `coherence_status` for dirty-flag propagation.

Add `cbu_resource_instance_option_lineage`.

Required shape:

- `lineage_id uuid primary key`
- `resource_instance_id uuid not null references cbu_resource_instances(instance_id) on delete cascade`
- `binding_id uuid not null references cbu_service_option_bindings(binding_id)`
- `contribution_type text not null`
- `fanout_axis text`
- `fanout_value jsonb`
- `created_at`

Constraints:

- check `contribution_type in ('eligibility','fanout','attribute_source')`
- unique `(resource_instance_id, binding_id, contribution_type, fanout_axis, fanout_value)`.

### 1.C Attribute Source Policy Compatibility

Add columns to `resource_attribute_requirements`:

- `source_kind text`
- `source_fallback text[]`
- `derivation_input_type text`
- `derivation_input_ref jsonb`

Backfill:

- `source_kind` from first element of `source_policy`.
- `source_fallback` from remaining elements of `source_policy`.
- Map old `entity` to `legal_entity` only where the attribute semantics are clearly legal-entity authoritative; otherwise preserve as compatibility pending SME review.

Compatibility rule:

- Do not drop `source_policy` in this tranche.
- Update readers to prefer structured columns and fall back to JSONB.
- Add schema comment marking `source_policy` deprecated once structured columns are present.
- Schedule Phase 11 retirement after no-readers verification.

Content rule:

- Do not blanket-map old `entity` source policy to `legal_entity`.
- Produce a 34-row review table for `resource_attribute_requirements` showing proposed `source_kind` and `source_fallback`.
- SME/architect walkthrough required before final source-kind classification.

Acceptance:

- Migration applies against live database.
- Schema export updated.
- Existing service-resource discovery/API behavior still works.

## Phase 2 - Rust Data Access And Core Services

Goal: add implementation support without wiring user-facing workflows yet.

Phase boundary:

- No verb registration.
- No DAG slot wiring.
- No constellation slot definitions.
- No UI/API exposure.
- This phase is repository and service logic only.

Deliverables:

- Add typed structs/repositories for:
  - option definitions
  - product-service overrides
  - option bindings
  - eligibility constraints
  - fan-out rules
  - resource instance lineage
- Add an `OptionResolver` service:
  - loads active service options for current service version
  - applies product-service override
  - resolves source value from `cbu_profile`, `instrument_matrix`, `legal_entity`, `document`, `product_option`, `manual`, or `derived`
  - computes stable `value_hash`
  - writes superseding binding rows rather than overwriting current history
- Add canonical JSON hashing utility:
  - SHA-256 over canonical JSON.
  - tests prove stable hash for reordered JSON object keys.
- Add a `CoverageValidator` service:
  - implements the seven validation levels in the paper
  - returns named gaps, not string-only failures
- Add a `ResourceFanoutPlanner` service:
  - intersects option bindings with eligibility constraints
  - applies fan-out rules
  - emits a deterministic resource instance plan
- Add a `LineageRecorder` helper:
  - writes `cbu_resource_instance_option_lineage`
  - records eligibility, fanout, and attribute-source contributions

Existing integration points:

- Use `service-resource.provision` to materialize planned instances.
- Reuse existing `resource_instance_dependencies` for dependencies.
- Reuse `derived_attribute_values`/`derived_attribute_dependencies` for derived value persistence.

Acceptance:

- Unit tests cover resolver precedence, supersession, validation gaps, fan-out modes, and lineage rows.
- No UI/API behavior changes yet.

## Phase 3 - Verb Surface And YAML

Goal: expose the model through SemOS verbs while preserving workspace boundaries.

Design-time verbs in product maintenance:

- `service-option.list-by-service`
- `service-option.read`
- `service.declare-option`
- `service.constrain-option-values`
- `service.bind-option-source`
- `service.deprecate-option`
- `product-service.override-option`
- `service-resource.declare-eligibility`
- `service-resource.declare-fanout-rule`

Runtime verbs in CBU activation:

- `cbu.bind-service-options`
- `cbu.override-option-binding`
- `cbu.validate-option-coverage`
- `cbu.dirty-flag-bindings`
- `cbu.recompute-bindings`
- `cbu.compute-resource-fanout`

Files likely affected after approval:

- `rust/config/verbs/service.yaml`
- `rust/config/verbs/service-resource.yaml`
- new `rust/config/verbs/service-option.yaml` if a separate domain is cleaner
- new or existing `rust/config/verbs/product-service.yaml`
- `rust/config/packs/product-service-taxonomy.yaml`
- `rust/config/packs/cbu-maintenance.yaml`
- `rust/crates/sem_os_postgres/src/ops/mod.rs`
- new `rust/crates/sem_os_postgres/src/ops/service_options.rs`

Pack boundary:

- `product-service-taxonomy` pack may read and govern design-time definitions.
- It must continue to forbid provisioning, activation, option binding, and fan-out execution.
- CBU maintenance/activation pack owns runtime binding, validation, fan-out, and provisioning.

Acceptance:

- `cargo run -p xtask -- verbs lint` passes.
- New verbs have YAML and Rust implementation parity.
- Existing utterance tests continue to route product taxonomy browsing without runtime side effects.

## Phase 4 - DAG, Constellation, And SemReg Visibility

Goal: make the option model navigable in SemOS and visible to validators.

Product-service taxonomy DAG additions:

- Add stateless/design-time slots:
  - `service_option`
  - `product_service_option_override`
  - `product_service_condition`
  - `service_resource_option_constraint`
  - `service_resource_fanout_rule`
- Add lifecycle state where needed:
  - `service_option`: `drafted`, `active`, `deprecated`, `retired`
  - overrides/constraints/fanout rules can start as active/inactive design-time records unless a stronger lifecycle is required.

CBU DAG additions:

- Add runtime slots:
  - `cbu_service_option_binding`
  - `cbu_resource_instance_option_lineage`
- Add cross-workspace constraint from CBU binding to IM source where `source_kind = instrument_matrix`.
- Add dirty/stale/clean lifecycle semantics to bindings.

Constellation map additions:

- Extend product-service map with service option and rule slots.
- Extend CBU map with option binding and lineage slots.
- Fix current naming mismatch while in the area:
  - `service_resource_types` uses `resource_id`; the map currently says `resource_type_id`.

SemReg visibility:

- Register flat v1 taxonomies:
  - `taxonomy.product`
  - `taxonomy.service`
  - `taxonomy.service-resource`
  - `taxonomy.service-option`

Validator additions:

- Extend the DAG closure-gap audit for new Class A verbs.
- Extend reconcile/closure checks so new states and verbs participate in DAG hygiene.
- Add checks that every active service option has either a default, a source policy, or an explicit required manual binding path.
- Add checks that fan-out drivers have at least one active fan-out rule where the service has active resource capabilities.

Acceptance:

- Product/service/resource option taxonomy is navigable in SemOS.
- `cargo run -p xtask -- reconcile validate` passes after expected new checks.
- Closure-gap audit rerun reports zero closure gaps for the new option verbs.

## Phase 5 - SRDEF And Catalogue Reconciliation

Goal: align SRDEF YAML, DB resource catalog, eligibility constraints, and fan-out rules.

Deliverables:

- Extend SRDEF loader/sync to write:
  - `resource_type`
  - `resource_purpose`
  - `per_market`
  - `per_currency`
  - `per_counterparty`
  - service triggers
  - dependencies
  - attribute source structured columns
- Backfill `service_resource_fanout_rules` from SRDEF dimensionality:
  - `per_market = true` -> `fanout_axis = market`, `fanout_mode = per_value`
  - `per_currency = true` -> `fanout_axis = currency`, `fanout_mode = per_value`
  - `per_counterparty = true` -> `fanout_axis = counterparty`, `fanout_mode = per_value`
  - false for all -> `fanout_mode = shared` where resource is service-required.
- Migrate existing SETTLEMENT `supported_options` JSONB into `service_resource_option_constraints`.
- Classify the ten placeholder `Operations::Resource::*` rows before deletion:
  - promote to typed rows if backed by SRDEF intent
  - archive/delete only if still unclassifiable after review
- Remove or archive duplicate inactive product rows only after confirming no FK dependencies.

Acceptance:

- SETTLEMENT resource eligibility exists in the new table.
- SRDEF dimensionality no longer conflicts with DB flags.
- No placeholder cleanup removes active runtime history.

## Phase 6 - Pilot SETTLEMENT

Goal: prove the complete model end-to-end on the service with the existing supported-options precedent.

Prerequisite:

- IM-to-CBU attachment exposes these pilot paths via a queryable interface:
  - `preferred_speed`
  - `counterparties`
- Paths documented in `docs/architecture/im-source-paths.md`.

Seed catalogue:

- SETTLEMENT service options:
  - `markets`: `multi_choice`, fan-out driver, axis `market`, default source `cbu_profile`
  - `settlement_speed`: `single_choice`, no fan-out, default source `instrument_matrix`
  - `default_counterparties`: `multi_choice`, fan-out driver, axis `counterparty`, default source `instrument_matrix`
- CUSTODY -> SETTLEMENT override:
  - mark SETTLEMENT mandatory/default for CUSTODY after metadata sign-off
  - narrow allowed markets as in the paper
  - default markets from contracted custody mandate / CBU profile
- Eligibility constraints:
  - DTCC for US equity
  - EUROCLEAR for EU equity
  - APAC_CLEAR for APAC equity
  - SWIFT_CONN for counterparty BIC set
  - SETTLE_ACCT shared
- Fan-out:
  - DTCC/EUROCLEAR/APAC_CLEAR per market
  - custody securities per market
  - custody cash per currency
  - SWIFT per counterparty
  - settlement instruction engine shared

Runtime flow:

1. Attach IM to synthetic CBU.
2. Create or select CUSTODY service intent.
3. Run `cbu.bind-service-options`.
4. Run `cbu.validate-option-coverage`.
5. Run `cbu.compute-resource-fanout`.
6. Materialize via `service-resource.provision`.
7. Record lineage.
8. Replace IM and prove dirty/recompute/supersession behavior.
9. Replay the original activation run after IM mutation and prove deterministic historical replay.

Acceptance:

- Worked example produces deterministic resource count and lineage.
- Resource instance lookup can answer "why does this exist?"
- IM version change creates superseding bindings rather than overwrites.
- T0/EUROCLEAR failure path produces a level-4 eligibility gap.
- Replay of original `activation_run_id` after IM mutation produces the original bindings, fan-out, lineage, and attribute values.

## Phase 7 - Pilot Fund Accounting And TRADE_CAPTURE Boundary

Goal: prove Fund Accounting service-option coverage in scope, and keep TRADE_CAPTURE to the minimum IM-coupling proof needed by the architecture if required by the Custody/Fund Accounting flow.

Fund Accounting seed catalogue:

- NAV_CALC options:
  - `frequency`
  - `pricing_source_priority`
  - `valuation_cutoff`
  - `share_classes` if current catalogue data supports it; otherwise explicit validation gap.
- FUND_REPORTING options:
  - `reporting_frequency`
  - `report_types`
  - `delivery_format`
  - `recipient_groups` if current catalogue data supports it; otherwise explicit validation gap.
- ASSET_PRICING:
  - classify as seeded or gap-producing based on available pricing resource data.

Fund Accounting resources:

- NAV_ENGINE
- REPORTING_HUB
- pricing resources where available (`BLOOMBERG_BVAL`, `ICE_PRICING`, `MARKIT_PRICING`, `REFINITIV_FEED`) only if SME/catalogue mapping is defensible from current data.

Fund Accounting acceptance:

- NAV_CALC binds and validates options where source data exists.
- Missing pricing/reporting semantics produce named validation gaps.
- No invented NAV/pricing business semantics.

TRADE_CAPTURE note:

- TRADE_CAPTURE is not a wide-scope rollout item for this tranche unless needed to prove the IM option-source contract. If used, keep it to the three pilot paths below.

TRADE_CAPTURE prerequisite:

- IM-to-CBU attachment exposes:
  - `instruction_channel`
  - `bic`
  - `message_types`
- Paths documented in `docs/architecture/im-source-paths.md`.

Seed catalogue:

- TRADE_CAPTURE options:
  - `instruction_channel`
  - `im_bic`
  - `message_types`
  - any required counterparty/routing fields from IM contract
- Product-service override:
  - MIDDLE_OFFICE -> TRADE_CAPTURE defaults from IM
  - confirm whether CUSTODY also includes TRADE_CAPTURE, or whether it remains MIDDLE_OFFICE only in v1
- Resources:
  - IBOR_SYSTEM
  - CTM_CONNECTION
  - FIX_SESSION
  - SWIFT or API resources if instruction channel requires them

Acceptance:

- IM fields populate TRADE_CAPTURE option bindings.
- Instruction channel drives resource eligibility and, where needed, fan-out.
- Coverage gaps distinguish missing IM data from missing resource capability.

## Phase 8 - UI And Workspace Visibility

Goal: make the option model visible and testable through the app.

Product maintenance UI:

- Browse product -> services -> service options -> resource eligibility/fan-out rules.
- Inspect option source policy and override policy.
- Show product-service overrides separately from generic service definitions.

CBU maintenance/activation UI:

- Show bound option values with source kind/source ref/source version.
- Show coherence status: clean, dirty, stale.
- Show coverage validation gaps by level.
- Show planned resource fan-out before provisioning.
- Show lineage from resource instance back to option binding.

Utterance tests:

- "show me CUSTODY settlement options"
- "validate service option coverage for this CBU"
- "why was this SWIFT resource created?"
- "recompute bindings after instrument matrix change"

Acceptance:

- Chrome MCP utterance tests pass for product-maintenance and CBU activation happy paths.
- The UI does not allow runtime binding/provisioning from the product-maintenance pack.

## Phase 9 - Validation, Reconcile, And Regression Tests

Required tests:

- Migration applies cleanly to current DB.
- Schema export includes all new tables/columns.
- Unit tests for:
  - source precedence
  - override narrowing
  - binding supersession
  - value hashing
  - fan-out modes
  - seven-level validation failures
- Integration tests for:
  - SETTLEMENT happy path
  - SETTLEMENT IM replacement failure path
  - TRADE_CAPTURE IM-coupled path
  - service-resource provisioning via generic verb
  - lineage reverse lookup
- SemOS tests for:
  - DAG slot parse
  - constellation navigation
  - pack allowed/forbidden verbs
  - reconcile validate
- Existing suite:
  - `cargo check`
  - `cargo fmt`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo run -p xtask -- verbs lint`
  - `cargo run -p xtask -- reconcile validate`

Acceptance:

- Existing CBU/product/service/resource lifecycle tests remain green.
- New pilot tests prove replay, dirty-flagging, coverage gaps, fan-out, and lineage.

## Phase 10 - Scope Closeout And Future Wide Rollout Gate

Do not start wide rollout until Custody and Fund Accounting are accepted.

Inputs required from peer review/SMEs:

- corrected product-service mandatory/default/optional matrix
- option definitions for each service
- product-service overrides
- service availability by booking principal/jurisdiction/fund type
- resource eligibility by option value
- resource fan-out dimensionality
- attribute source policy per resource attribute

Wide rollout deliverables:

- options and overrides for remaining 28 services
- eligibility/fan-out rules for all active resources
- cleanup of stale resource rows
- SemReg flat taxonomy content

Acceptance:

- Every active product-service link has explicit option semantics or explicit "no options" classification.
- Every active service with runtime activation has resource eligibility coverage or an explicit "manual/non-system" classification.

## Phase 11 - Source Policy Retirement

Goal: remove compatibility debt after the structured source policy model is proven.

Preconditions:

- Phase 10 accepted.
- No code path reads `resource_attribute_requirements.source_policy` except fallback diagnostics.
- SRDEF loader writes structured source columns.
- Discovery/API/UI all consume structured source columns.

Deliverables:

- Remove `source_policy` readers.
- Migrate any remaining content to `source_kind/source_fallback/derivation_input_*`.
- Drop or hard-deprecate `source_policy` in a dedicated migration.

Acceptance:

- No `source_policy` references remain outside migration/history docs.
- Tests prove attribute source resolution still works for Custody and Fund Accounting.

## Implementation Order Summary

1. Phase 0 baseline and schema truth.
2. Phase 1 schema migration with compatibility columns.
3. Phase 2 repository/services.
4. Phase 3 verbs/YAML.
5. Phase 4 DAG/constellation/SemReg.
6. Phase 5 SRDEF/catalogue reconciliation.
7. Phase 6 SETTLEMENT pilot.
8. Phase 7 Fund Accounting pilot and TRADE_CAPTURE boundary proof if needed.
9. Phase 8 UI/workspace visibility.
10. Phase 9 full validation.
11. Phase 10 closeout, with wide rollout deferred until SME sign-off.
12. Phase 11 source-policy retirement after compatibility window.

## Residual Risks

- Product versioning remains under-specified in the current repo. The plan avoids adding a product-version architecture in v1, but this should be revisited.
- Product-level versioning is deferred. When introduced, override temporality migrates from row-level effective dating to product-version-scoped uniqueness; data migration should be mechanical if override rows keep clean effective dating and supersession.
- Source-kind migration touches SRDEF loader, discovery, API, and UI paths. Keeping `source_policy` for compatibility reduces blast radius.
- Fan-out correctness is catalogue-content sensitive; wrong rules produce wrong instance counts.
- The IM path contract needs a concrete mapping to current IM carriers before runtime binding can be complete.
- Historical runtime rows are all mostly `PENDING`; validation against real activated data may be limited.
- `activation_runs` adds a replay anchor but depends on canonical JSON hashing and stable source-version references to deliver deterministic replay.
