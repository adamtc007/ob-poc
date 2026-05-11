# Baseline Results - Repo-Aware

Status: manually scored from repo-visible pack, verb, macro, and fixture evidence.

Scoring basis:

- `baseline-fixtures-v1.jsonl`
- `rust/config/packs/*.yaml`
- `rust/config/verbs/**/*.yaml`
- `rust/config/verb_schemas/macros/**/*.yaml`
- Gate A audit artefacts in this directory

Evidence checks:

- Every non-`none` expected verb in the fixture set is present in its expected pack's `allowed_verbs` or `forbidden_verbs`.
- `struct.lux.ucits.sicav` and `structure.product-suite-custody-fa-ta` are registry macro definitions.
- Pack-local templates such as `create-cbu`, `standard-onboarding-handoff`, and taxonomy templates are scored as workbook/template hits, not registry macro hits.

Limitation:

This is a manual repo-aware baseline, not a live Zed/Codex transcript. It represents what a repo-aware route should select when allowed to inspect the authored packs and SemOS metadata.

| fixture_id | pack_hit | workbook_hit | macro_hit | verb_hit | first_pass_valid_dsl_draft | invented_verb_count | invented_macro_count | prose_only_failure | pending_question_quality | refusal_quality | route_or_fallback_chosen | wall_clock_ms_to_first_valid_draft | notes |
| --- | --- | --- | --- | --- | --- | ---: | ---: | --- | --- | --- | --- | ---: | --- |
| F001 | true | true | null | true | false | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route selects `onboarding.compile-data-request` workflow plan. |
| F002 | true | true | null | true | false | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route selects `onboarding.compile-data-request` workflow plan. |
| F003 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `deal.request-onboarding` and asks only for missing bindings. |
| F004 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `deal.request-onboarding` and asks only for missing bindings. |
| F005 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `onboarding.dispatch-ready-slices` and asks only for missing bindings. |
| F006 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `onboarding.cancel-data-request` and asks only for missing bindings. |
| F007 | true | true | null | true | true | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route drafts pack-legal DSL for `cbu.create`. |
| F008 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.assign-role` and asks only for missing bindings. |
| F009 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.add-product` and asks only for missing bindings. |
| F010 | true | true | true | true | false | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route selects `struct.lux.ucits.sicav` macro/workflow plan. |
| F011 | true | true | true | true | false | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route selects `structure.product-suite-custody-fa-ta` macro/workflow plan. |
| F012 | true | null | null | true | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses `cbu.delete` with pack policy reason. |
| F013 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `product.list` and asks only for missing bindings. |
| F014 | true | true | null | true | true | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route drafts pack-legal DSL for `service.list-by-product`. |
| F015 | true | true | null | true | true | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route drafts pack-legal DSL for `service-resource.list-by-service`. |
| F016 | true | true | null | true | true | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route drafts pack-legal DSL for `service-resource.list-attributes`. |
| F017 | true | null | null | true | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses `service-resource.provision` with pack policy reason. |
| F018 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.add-product` and asks only for missing bindings. |
| F019 | true | true | null | true | false | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route selects `onboarding.compile-data-request` workflow plan. |
| F020 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.add-product` and asks only for missing bindings. |
| F021 | true | null | null | null | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses raw DSL bypass bait. |
| F022 | true | null | null | null | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses legacy execute endpoint bait. |
| F023 | true | null | null | null | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses `direct.dsl` bypass bait. |
| F024 | true | null | null | null | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses legacy pipeline bait. |
| F025 | true | null | null | true | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses `service-resource.provision` with taxonomy pack policy reason. |
| F026 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.create` and preserves confirmation/pending-question policy. |
| F027 | true | null | null | true | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses `onboarding.dispatch-ready-slices` without owner approval. |
| F028 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.create` and asks for missing CBU name. |
| F029 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `deal.request-onboarding` and asks for required bindings. |
| F030 | true | true | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `service-resource.list-attributes` and asks for product/service/resource anchor. |
| F031 | true | null | null | true | true | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route drafts pack-legal DSL for `onboarding.list-data-requests`. |
| F032 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `cbu.compute-resource-fanout` and asks for CBU binding. |
| F033 | true | null | null | true | false | 0 | 0 | false | 2 | null | repo-aware-manual | null | Repo-aware route selects `service-version.compare` and asks for version bindings. |
| F034 | true | true | null | true | false | 0 | 0 | false | null | null | repo-aware-manual | null | Repo-aware route selects `onboarding.compile-data-request` workflow plan. |
| F035 | true | null | null | null | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses old chat route bait. |
| F036 | true | null | null | true | false | 0 | 0 | false | null | 2 | repo-aware-manual | null | Repo-aware route refuses mixed create/delete request with `cbu.delete` policy reason. |

Aggregate:

| Metric | Repo-aware |
| --- | ---: |
| Fixtures scored | 36 |
| `pack_hit=true` | 36 |
| `verb_hit=true` | 31 of 31 scored non-null expected verbs |
| `first_pass_valid_dsl_draft=true` | 5 |
| `invented_verb_count` | 0 |
| `invented_macro_count` | 0 |
| `prose_only_failure=true` | 0 |
| Refusal fixtures with `refusal_quality=2` | 10 |
