# Macro Tier Classification

Status: audit draft for Gate A replan.

Evidence commands:

- `ruby -ryaml -e 'files=Dir["../../rust/config/verb_schemas/macros/**/*.y{a,}ml"]; ids=[]; files.each{|f| y=YAML.load_file(f) rescue next; ids.concat(y.keys.select{|k| y[k].is_a?(Hash) && y[k]["kind"]=="macro"}.map{|k| [k,f]})}; puts ids.size'`
- `find ../../rust/config/macros ../../rust/config/verb_schemas/macros -type f \( -name '*.yaml' -o -name '*.yml' \) | sort`

Classification:

| Source | Count | Tier | Recommendation |
| --- | ---: | --- | --- |
| `rust/config/verb_schemas/macros/*.yaml` definitions with `kind: macro` | 140 | Registry-grade candidate | Project only after slot, precondition, refusal, dry-run, and HITL checks pass. |
| `rust/config/macros/research/*.yaml` | 3 | YAML-grade/research | Quarantine from production envelopes. These use prompt/tool schemas and web research, not deterministic SemOS steps. |
| Pack `templates` | 12 across all packs | Workbook/template-grade | Do not project as macros until modeled as first-class workbook plans or registry macros. |

Slice 1 macro recommendation:

| Macro family | Examples | Decision |
| --- | --- | --- |
| Fund structure macros | `struct.lux.ucits.sicav`, `struct.ie.ucits.icav`, `struct.uk.authorised.oeic` | Lift/project only for `cbu-maintenance` after binding/HITL metadata is completed. |
| Product suite macros | `structure.product-suite-custody-fa-ta`, `structure.product-suite-full` | Lift/project after product/service option effects are explicit. |
| Research macros | `client-discovery`, `regulatory-check`, `ubo-investigation` | Quarantine for Slice 1; not deterministic and not registry-grade. |
| Pack templates | `create-cbu`, `standard-onboarding-handoff`, taxonomy templates | Treat as workbook-plan candidates, not macros. |

Open audit point:

The v0.5 plan calls out M1-M18. The current registry contains more than that family, so Gate B needs a narrowed projection list rather than a blanket "all macros" decision.
