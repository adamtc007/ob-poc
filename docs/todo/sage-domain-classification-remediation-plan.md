# Sage Domain Classification Remediation Plan

## Goal

Raise Sage domain classification materially without changing the core architecture.

This plan does **not** change the current approach:
- utterance -> Sage intent/outcome
- outcome -> Coder verb resolution
- verb -> DSL proposal / runbook
- execution stays on the existing deterministic REPL/runbook side

This is a tuning and calibration pass on the deterministic Sage domain layer so the existing architecture produces better outcomes.

## Non-Goals

- No re-architecture of Sage/Coder boundaries
- No execution-path redesign
- No new LLM dependency
- No replacement of Sem OS context resolution
- No major UI redesign

## Current Baseline

Latest observed metrics:
- Sage plane accuracy: `96.0%`
- Sage polarity accuracy: `84.1%`
- Sage domain accuracy: `57.4%`
- End-to-end Sage+Coder verb accuracy: `13/134` = `9.70%`

Observed domain failure clusters:
- expected `cbu`: `15`
- expected `ubo`: `12`
- expected `struct`: `11`
- expected `screening`: `8`
- expected `case`: `5`
- expected `client-group`: `5`
- expected `fund`: `5`
- expected `kyc`: `5`
- expected `ownership`: `5`

Observed wrong actual domains:
- empty domain: `22`
- `fund`: `16`
- `entity`: `9`
- `document`: `8`
- `ownership`: `8`

## Root Cause

The current deterministic domain layer is too shallow:
- `extract_domain_hints()` is mainly a flat keyword scan
- `select_domain_concept()` largely picks the first non-generic hint
- confidence is too easily promoted to `medium`
- nearby nouns hijack the true task domain
- workflow/scenario phrasing is not treated as a first-class signal
- session context is underused for domain tie-breaking

## Remediation Strategy

### D1. Replace Flat Hint Picking with Weighted Domain Scoring

Introduce a deterministic scored domain resolver instead of relying on first-hit hints.

New scoring inputs:
- exact phrase match weight
- multi-token phrase match weight
- single noun/token weight
- action-domain coupling weight
- stage/workflow bias weight
- carry-forward context weight
- negative/suppression weight
- tie-break margin

Expected outcome:
- fewer empty domains
- fewer wrong `fund` / `entity` / `document` grabs
- clearer confidence boundaries

### D2. Add Domain Precedence Rules for Scenario Phrases

Add explicit precedence for multi-domain workflow utterances.

Priority families to implement first:
- `kyc` vs `case` vs `document` vs `screening`
- `ubo` vs `ownership`
- `struct` vs `fund`
- `cbu` vs `fund`
- `client-group` vs `entity`

Examples to encode:
- `open a case` -> `case`
- `collect documents for kyc` -> `kyc`
- `full kyc onboarding` -> `case` or `kyc` by explicit phrase precedence
- `ownership structure` -> `ubo` or `ownership` by phrase family, not generic noun hit
- `irish icav`, `lux sicav`, `oeic`, `40-act` -> `struct`

Expected outcome:
- scenario utterances stop collapsing into nearby noun domains

### D3. Add Action-Domain Coupling

Use the inferred action to bias domain selection.

Examples:
- `open/create/request/start` + `case` language -> `case`
- `screen/check pep/sanctions/adverse media` -> `screening`
- `show/list/read` + `ownership/ubo/control` -> `ubo` or `ownership`
- `set up/onboard/create structure` + `sicav/icav/oeic/fund structure` -> `struct`
- `assign/add role/custodian/transfer agent` -> `cbu.role` / `cbu-role` / `cbu`

Expected outcome:
- fewer cases where noun-only domains dominate verbs that clearly imply another domain

### D4. Use Session Context in Domain Tie-Breaking

Exploit `SageContext` more explicitly during domain scoring.

Tie-break inputs:
- `stage_focus`
- `entity_kind`
- `dominant_entity_name`
- `last_intents`

Rules:
- if stage focus is KYC-related, bias `case`, `kyc`, `screening`, `document`, `ubo`
- if stage focus is data management, bias `struct` and schema-oriented reads
- if last intent was a strong domain hit and current utterance is elliptical, allow carry-forward bias
- if current dominant entity scope is CBU, prefer `cbu` over `entity` for ambiguous operational edits

Expected outcome:
- better multi-turn continuity
- fewer empty domain cases on follow-up utterances

### D5. Recalibrate Domain Confidence

Current confidence labels are not trustworthy.

New confidence rules:
- `high`: strong phrase match + clear domain margin + no serious competitor
- `medium`: decent score with a meaningful lead over runner-up
- `low`: weak or tied signal, generic language, or no domain winner

Confidence must depend on:
- top score
- runner-up gap
- whether the winner came from exact phrase vs generic token hit
- whether context carry-forward was required

Expected outcome:
- `medium` stops being the default bucket
- low-confidence cases become visible instead of being overclaimed

### D6. Add Domain Confusion Regression Suite

Add a dedicated deterministic test suite for domain disambiguation.

Initial confusion packs:
- `cbu` vs `fund`
- `struct` vs `fund`
- `ubo` vs `ownership`
- `kyc` vs `case` vs `document` vs `screening`
- `entity` vs `client-group`
- `session` vs `view`

This suite should assert both:
- expected domain
- expected confidence bucket for key examples

Expected outcome:
- domain improvements become durable instead of anecdotal

### D7. Extend Coverage Harness Reporting

Keep the existing harnesses, but add domain analytics so failures are easier to attack.

Add reporting for:
- domain confusion matrix
- domain accuracy by expected domain
- confidence bucket distribution by expected domain
- top phrase families for empty-domain results

Expected outcome:
- faster iteration on the next tuning passes

## Implementation Order

1. D1: weighted domain scorer
2. D2: scenario/domain precedence rules
3. D3: action-domain coupling
4. D4: context-aware tie-breaks
5. D5: confidence recalibration
6. D6: domain confusion regression suite
7. D7: coverage harness reporting improvements

## Files Expected to Change

Core logic:
- `rust/src/sage/pre_classify.rs`
- `rust/src/sage/deterministic.rs`
- possibly `rust/src/sage/outcome.rs` if confidence evidence needs to be surfaced

Tests/harnesses:
- `rust/tests/sage_coverage.rs`
- `rust/tests/fixtures/intent_test_utterances.toml`
- new focused domain confusion tests under `rust/src/sage/` or `rust/tests/`

Optional reporting support:
- `rust/tests/utterance_api_coverage.rs`

## Verification Gates

### Gate 1
- `cargo check -p ob-poc` passes
- new domain confusion tests pass

### Gate 2
- `sage_coverage` rerun shows domain accuracy materially above current `57.4%`
- target for first pass: `>= 70%`

### Gate 3
- re-run end-to-end utterance coverage
- Sage+Coder top-verb accuracy should improve from current `9.70%`
- if domain improves but end-to-end does not, the next bottleneck is Coder resolution rather than Sage classification

## Success Criteria

Minimum success:
- domain accuracy improves materially from `57.4%`
- confidence buckets become meaningfully separated
- empty-domain cases reduce substantially

Good success:
- domain accuracy reaches `70-80%`
- top confusion families (`cbu`, `ubo`, `struct`, `screening`) show clear improvement
- end-to-end Sage+Coder accuracy rises as a result

## Decision Boundary

If domain accuracy improves but end-to-end verb accuracy barely moves, stop tuning Sage and shift effort to:
- Coder scoring
- arg assembly
- legacy fallback leakage

If both improve, continue deterministic tuning before spending more on LLM Sage.
