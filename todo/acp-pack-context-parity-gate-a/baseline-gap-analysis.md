# Baseline Gap Analysis

Status: refreshed after Gate E envelope-driven DAG semantic routing and HTTP REPL trace emission, using current-Sage capture plus manual repo-aware scoring.

Sources:

- Current Sage: `baseline-results-current-sage.md`
- Repo-aware: `baseline-results-repo-aware.md`
- Current Sage raw capture: `baseline-runs/current-sage-20260510T200520Z`
- Pre-envelope current-Sage raw capture: `baseline-runs/current-sage-20260510T123212Z`
- Previous current-Sage raw capture: `baseline-runs/current-sage-20260510T114126Z`
- Pre-draft current-Sage raw capture: `baseline-runs/current-sage-20260510T112045Z`
- Original current-Sage raw capture: `baseline-runs/current-sage-20260510T101839Z`

Gap formula:

- For hit-rate metrics: `repo_aware_rate - current_sage_rate`.
- For invented counts and prose-only failures: `current_sage_count - repo_aware_count`.
- For latency: compare median `wall_clock_ms_to_first_valid_draft` only for fixtures where both runners produced valid drafts.

## Aggregate Gap

| Metric | Current Sage | Repo-aware | Gap |
| --- | ---: | ---: | ---: |
| `pack_hit` | 31/36 = 86.1% | 36/36 = 100.0% | +13.9 pp |
| `verb_hit` | 31/31 = 100.0% | 31/31 = 100.0% | 0 pp |
| `first_pass_valid_dsl_draft` across all fixtures | 8/36 = 22.2% | 5/36 = 13.9% | -8.3 pp |
| `first_pass_valid_dsl_draft` on draft-expected fixtures | 5/5 = 100.0% | 5/5 = 100.0% | 0 pp |
| `invented_verb_count` | 0 | 0 | 0 |
| `invented_macro_count` | 0 | 0 | 0 |
| `prose_only_failure` | 0/36 = 0.0% | 0/36 = 0.0% | 0 pp |
| Refusal quality `2` on refusal fixtures | 10/10 = 100.0% | 10/10 = 100.0% | 0 pp |
| `registry_verified` trace coverage | 36/36 = 100.0% | n/a | n/a |
| `envelope_verified` trace coverage | 31/36 = 86.1% | n/a | n/a |

## Interpretation

Current Sage now produces structured, non-prose-only responses, does not invent verbs or macros, emits quality-2 structured refusals for all 10 refusal fixtures, and exceeds the original Slice 1 `pack_hit`, `verb_hit`, and draft-expected `first_pass_valid_dsl_draft` targets.

Gate E trace coverage is present on the envelope-driven run: all 36 fixtures carry verified registry trace data, and all 31 pack-bound fixtures carry verified envelope trace data. The five no-pack ghost-route refusal fixtures correctly remain unbound to a pack envelope.

The remaining scoring gap is concentrated in one intentional area:

1. **No-pack refusal fixtures:** five ghost-route refusal fixtures intentionally have no pack hit because their expected pack is `none`; they should remain structured refusals rather than being forced into a pack.

## Slice 1 Acceptance Threshold

Original Slice 1 threshold, frozen before implementation:

- Close at least 70% of the observed gap on `pack_hit`, `verb_hit`, and `first_pass_valid_dsl_draft`.
- Keep invented verbs and invented macros at zero.
- Keep prose-only failures at zero for covered packs.
- All refusal-required fixtures must score refusal quality `2`.
- Pending-question fixtures must ask only for missing required bindings or disambiguation needed to proceed.

Concrete post-Slice 1 targets from this baseline:

| Metric | Required target |
| --- | ---: |
| `pack_hit` | At least 83.3%: `44.4 + 0.70 * 55.6` |
| `verb_hit` | At least 88.4%: `61.3 + 0.70 * 38.7` |
| `first_pass_valid_dsl_draft` on draft-expected fixtures | At least 70.0% |
| Invented verbs/macros | 0 |
| Prose-only failures | 0 |
| Refusal quality `2` on refusal fixtures | 10/10 |

Current movement against the original threshold:

| Metric | Current result | Original target | Status |
| --- | ---: | ---: | --- |
| `pack_hit` | 86.1% | >= 83.3% | Met |
| `verb_hit` | 100.0% | >= 88.4% | Met |
| `first_pass_valid_dsl_draft` on draft-expected fixtures | 100.0% | >= 70.0% | Met |
| Invented verbs/macros | 0 | 0 | Met |
| Prose-only failures | 0 | 0 | Met |
| Refusal quality `2` on refusal fixtures | 10/10 | 10/10 | Met |

## Gate B Implications

- Route hygiene and targeted Slice 1 routing now meet the original `pack_hit`, `verb_hit`, draft-expected first-pass DSL, refusal, invented-count, and prose-only objectives.
- Pack-local templates now have traceable selected template IDs for the Slice 1 template fixtures exercised by the current baseline.
- F018 now resolves deterministically to `cbu.add-product` and asks only for missing required bindings.
- Registry-grade macros must remain separate from pack templates and research macros.
