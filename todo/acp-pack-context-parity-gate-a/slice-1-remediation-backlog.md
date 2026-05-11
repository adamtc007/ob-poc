# Slice 1 Remediation Backlog

Status: route hygiene, targeted Slice 1 routing, template trace surfacing, draft-expected DSL draft surfacing, and F018 ambiguity remediation implemented and measured. Remaining remediation is limited to guardrails.

## Batch 1 - Refusal And Trace Hygiene

Status: implemented and recaptured through `baseline-runs/current-sage-20260510T123212Z`.

Goal: close the highest-risk gap without changing broad routing architecture.

Tasks:

1. [x] Add/normalize structured refusal outcomes for:
   - raw DSL bait,
   - legacy execute endpoint bait,
   - `direct.dsl` bypass bait,
   - legacy pipeline bait,
   - old chat-route bait,
   - forbidden `cbu.delete`,
   - forbidden taxonomy mutation,
   - owner-approval-gated onboarding dispatch.
2. [x] Populate `acp_trace.pack_id` for selected Slice 1 routes that already have pack evidence.
3. [x] Add regression tests for refusal quality and pack trace.
4. [x] Re-run current-Sage baseline and compare against `baseline-gap-analysis.md`.

Measured movement:

- Refusal quality improved from `0/10` to `10/10`.
- Pack hit improved from `16/36` to `31/36`.
- Verb hit improved from `19/31` to `31/31`.
- Remaining pack misses are the five no-pack ghost-route refusal fixtures, where expected pack is `none`.
- No non-null expected verb misses remain.

## Batch 2 - Workbook And Template Traceability

Goal: make pack-local templates and workflow plans first-class enough to score.

Status: implemented and recaptured through `baseline-runs/current-sage-20260510T123212Z`. Registry-grade CBU macros route separately from pack-local template IDs.

Tasks:

1. [x] Trace `create-cbu`, `add-entity-and-role`, `standard-onboarding-handoff`, and taxonomy templates when selected.
2. [x] Trace workflow-plan IDs for onboarding compile-data-request routes.
3. [x] Keep registry macro trace separate from pack-template trace.
4. [x] Re-score macro/workbook fixtures.

Measured movement:

- Workbook/template hit improves for F003, F004, F007, F008, F013-F016, F026, F028-F030.
- Registry macro hit for F010 and F011 is already resolved through targeted macro rows.

## Batch 3 - Draft-Expected DSL

Goal: emit first-pass valid DSL for the five draft-expected fixtures.

Fixtures:

- F007 `cbu.create`
- F014 `service.list-by-product`
- F015 `service-resource.list-by-service`
- F016 `service-resource.list-attributes`
- F031 `onboarding.list-data-requests`

Tasks:

1. [x] Confirm required bindings are inferable from utterance or fixture text.
2. [x] Emit non-executing DSL drafts only when pack-legal.
3. [x] Add parse/pack legality checks to tests.

Measured movement:

- Draft-expected `first_pass_valid_dsl_draft` moved from `0/5` to `5/5`; threshold was at least `4/5` if using integer fixtures for the 70% target.

## Batch 3.5 - Residual Ambiguity Replan

Goal: decide whether F018 should stay an explicit disambiguation or gain a deterministic `cbu.add-product` tie-breaker.

Status: implemented and recaptured through `baseline-runs/current-sage-20260510T123212Z`.

Tasks:

1. [x] Review whether the utterance "add product" should prefer product onboarding or CBU product attachment when no CBU/product IDs are present.
2. [x] If deterministic, add a narrow rule and test for `cbu.add-product`.
3. [x] Re-score F018 and update acceptance notes so it no longer remains a false failure.

## Batch 4 - Public Surface Guardrail

Goal: prevent route hygiene from regressing through accidental public APIs.

Status: not started; safe to defer until the next gate because no current baseline metric depends on it.

Tasks:

1. Inventory root `ob-poc` public exports used by `ob-poc-web`, `dsl-lsp`, and tests.
2. Add a narrow `pub` lint/report for migrated route modules.
3. Convert internal route helpers to narrower visibility after tests pass.

Expected movement:

- No baseline metric change required; this protects later envelope work.
