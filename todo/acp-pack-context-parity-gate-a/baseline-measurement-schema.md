# Baseline Measurement Schema

Per fixture, record:

| Field | Type | Notes |
| --- | --- | --- |
| `fixture_id` | string | Matches `baseline-fixtures-v1.md` |
| `runner` | enum | `current-sage` or `repo-aware` |
| `pack_hit` | bool | Expected pack selected |
| `workbook_hit` | bool/null | Workbook/workflow plan selected where applicable |
| `macro_hit` | bool/null | Macro/template selected where applicable |
| `verb_hit` | bool/null | Expected primary verb selected |
| `first_pass_valid_dsl_draft` | bool | True only if parseable and pack-legal |
| `invented_verb_count` | integer | Verbs absent from registry/config |
| `invented_macro_count` | integer | Macros absent from registry/config |
| `prose_only_failure` | bool | No structured pending/refusal/draft |
| `pending_question_quality` | integer/null | 0-2, where 2 asks for the minimal missing binding |
| `refusal_quality` | integer/null | 0-2, where 2 cites the concrete policy/pack reason |
| `route_or_fallback_chosen` | string | Trace route, fallback reason, or endpoint |
| `wall_clock_ms_to_first_valid_draft` | integer/null | Null when no valid draft |
| `notes` | string | Repro details |

Commands:

- Current Sage runner: use `POST /api/session/:id/input` with `kind=utterance` against a locally running `ob-poc-web` server.
- Repo-aware runner: use the same fixture text and score against the repo-aware answer manually or with the eval harness once connected.

Acceptance threshold for Slice 1:

- Close at least 70% of the observed `current-sage` to `repo-aware` gap on `pack_hit`, `verb_hit`, and `first_pass_valid_dsl_draft`.
- Reduce invented verbs and invented macros to zero for the 36 fixture Slice 1 set.
- Zero prose-only failures for covered packs.
- Refusal-required fixtures must score refusal quality `2`.

