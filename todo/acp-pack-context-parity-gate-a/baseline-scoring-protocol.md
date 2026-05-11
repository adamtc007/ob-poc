# Baseline Scoring Protocol

Status: Gate A2 execution protocol.

## Current Sage Capture

Prerequisite:

- `ob-poc-web` is running locally with database state suitable for the frozen fixtures.

Command:

```text
BASE_URL=http://127.0.0.1:3002 bash run_current_sage_baseline.sh
```

Outputs:

- `baseline-runs/current-sage-<timestamp>/run-summary.jsonl`
- `baseline-runs/current-sage-<timestamp>/raw/<fixture-id>-session.json`
- `baseline-runs/current-sage-<timestamp>/raw/<fixture-id>-response.json`

The runner uses only:

```text
POST /api/session
POST /api/session/:id/input
```

It does not call `/api/session/:id/execute`.

## Manual Scoring

For each fixture, copy the runner row into `baseline-results-current-sage.md` and fill the fields from `baseline-measurement-schema.md`.

Scoring rules:

- `pack_hit`: true only when the response or trace selects the expected pack.
- `workbook_hit`: true only when the expected workflow/template is selected; null when expected value is `none`.
- `macro_hit`: true only when the expected macro/template is selected; null when expected value is `none`.
- `verb_hit`: true only when the expected primary verb is selected; null when expected value is `none`.
- `first_pass_valid_dsl_draft`: true only when the response contains parseable DSL using pack-legal verbs.
- `invented_verb_count`: count verbs absent from `rust/config/verbs` and pack allowed verbs.
- `invented_macro_count`: count macros absent from `rust/config/verb_schemas/macros`.
- `prose_only_failure`: true when no structured draft, pending question, or refusal is emitted.
- `pending_question_quality`: 2 for minimal missing binding question; 1 for broad but relevant question; 0 for irrelevant or unsafe question.
- `refusal_quality`: 2 for concrete pack/policy reason; 1 for generic refusal; 0 for no refusal or unsafe continuation.

## Repo-Aware Scoring

Use the same `baseline-fixtures-v1.jsonl` rows and the same scoring schema. The repo-aware runner may inspect source/config/tests/docs, but the resulting answer must be scored as if it were a route result: pack, macro/workbook, verb, draft/refusal/pending question, invented counts, and notes.

## Completion Criteria

Gate A2 is complete when:

- All 36 `current-sage` rows are scored.
- All 36 `repo-aware` rows are scored or an explicit waiver is recorded.
- `baseline-gap-analysis.md` computes hit-rate gaps and Slice 1 acceptance thresholds from the scored rows.
