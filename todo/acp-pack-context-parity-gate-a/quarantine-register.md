# Quarantine Register

Status: proposed Gate A register.

| Item | Owner | Retirement date | Exclusion mechanism | Final disposition |
| --- | --- | --- | --- | --- |
| `rust/config/macros/research/*.yaml` | SemOS metadata owner | Before Gate C | Do not include in envelope projection source glob. | Lift into registry-grade macros or keep out of production envelopes. |
| `/api/session/:id/execute` raw DSL route | Routing owner | Before Gate E | Exclude from utterance baseline and UI; require explicit raw DSL request if retained. | Remove from normal server or keep as admin-only maintenance route. |
| `try_route_through_repl` fallback | Routing owner | Before Gate E | Feature/route guard plus tests proving no envelope bypass. | Replace with envelope-gated route or delete. |
| ACP DAG semantic resolver direct calls | ACP routing owner | Before Gate D | Do not wire directly into production envelope path until classified. | Integrate as route candidate scorer or retire. |
| Pack templates not modeled as workbook plans | SemOS metadata owner | Before Gate C | Exclude from macro projection. | Lift into workbook-plan entities. |

Note:

Quarantine is temporary. Any item still quarantined at Gate E should block Slice 1 acceptance unless peer review explicitly narrows the slice.
