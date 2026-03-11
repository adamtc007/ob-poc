# StateGraph Phase 1: Remaining Inputs Needed

## Purpose

This note records what can be fixed safely from the current repository alone and what still requires the external reconciliation artifacts before further Phase 1 cleanup is safe.

The goal is to avoid guessing on graph verb rewrites, phase enums, or corpus remaps that were explicitly called out as corrected outside the checked-in codebase.

## Safe Work Already Completed

- Added [stategraph_phase1_reconciliation_checklist.md](/Users/adamtc007/Developer/ob-poc/docs/todo/stategraph_phase1_reconciliation_checklist.md)
- Normalized clearly stale fixture aliases in [intent_test_utterances.toml](/Users/adamtc007/Developer/ob-poc/rust/tests/fixtures/intent_test_utterances.toml):
  - `case.open` -> `kyc.open-case`
  - `screening.pep-check` -> `screening.pep`
  - `screening.sanctions-check` -> `screening.sanctions`
  - `screening.media-check` -> `screening.adverse-media`
- Verified `cargo check -p ob-poc` after those changes

## What Can Still Be Done Safely From The Repo Alone

These items do not require the external correction table, because the current registry and codebase already provide enough evidence:

1. Entity-context signal enrichment in [discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs)
   - split screening counts
   - add granular document counts
   - enrich KYC/UBO/fund-linked signals where the current schema supports them

2. Invocation phrase enrichment in the current canonical verb YAMLs:
   - [cbu.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/cbu.yaml)
   - [screening.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/screening.yaml)
   - [ubo.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/ubo.yaml)
   - [document.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/document.yaml)
   - [deal.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/deal.yaml)
   - [fund.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/fund.yaml)
   - [entity.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/entity.yaml)

3. Additional fixture normalization only where the current registry proves the canonical replacement directly.

## What Is Blocked Pending External Inputs

These changes should not be applied by inference from the checked-in graph files:

1. Graph edge verb rewrites
   - The user indicated the corrected graph verb IDs exist in generated graph YAMLs outside the repo.
   - The checked-in files under `rust/config/stategraphs/` are not the source of truth for those corrections.

2. `struct.*` corpus reconciliation
   - The current fixture still contains `struct.*` expectations.
   - The current registry does not expose those exact canonical verbs.
   - Safe resolution requires the external mapping:
     - restore missing verbs
     - or replace with canonical current verbs
     - or explicitly remove them from the corpus

3. `screening.full` corpus reconciliation
   - The current registry does not define `screening.full`.
   - Safe resolution requires an explicit decision:
     - replace with `screening.run`
     - expand into subtype screening verbs
     - or keep as macro-level expectation in a different harness

4. Phase enum corrections
   - The audit indicates phase/status names in the graph layer may not match live DB enum values.
   - These should not be patched by guesswork.
   - The exact canonical replacements need the correction table.

5. Signal SQL corrections tied to the reconciliation report
   - The audit says some signal enrichments were already corrected externally.
   - Those exact SQL/field changes should be taken from the external artifact, not recreated ad hoc.

## External Inputs Required

To complete the remaining Phase 1 cleanup safely, the following must be available inside the workspace:

1. Detailed reconciliation report
   - expected contents:
     - wrong graph verb ID -> canonical verb ID table
     - stale phase/status name -> canonical DB enum mapping
     - exact signal enrichment additions
     - explicit decision for `screening.full`
     - explicit decision for the `struct.*` family

2. Corrected generated graph YAMLs
   - these should be diffed against `rust/config/stategraphs/*.yaml`
   - they should be treated as the source of truth for graph edge corrections

## Immediate Next Safe Batch

If the external artifacts are still unavailable, the next safe implementation batch is:

1. Enrich `entity-context` signals in [discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs)
2. Enrich invocation phrases in the current canonical verb YAMLs
3. Re-run `cargo check -p ob-poc`
4. Stop before graph rewrites or `struct.*` remaps

## Examples

Example of safe normalization:

- `screening.pep-check` -> `screening.pep`

Example of blocked normalization:

- `struct.lux.ucits.sicav` -> unknown canonical replacement without the external mapping
