# SemOS Gap Matrix

Status: Gate C Slice 1 static metadata projection complete.

| Metadata area | Current evidence | Slice 1 impact | Decision |
| --- | --- | --- | --- |
| Verb argument contracts | `args` present on 1324 of 1324 parsed verb definitions. Five no-input verbs now use explicit `args: []`. | Usable for draft argument shape, but not enough for binding confidence. | Complete for presence; still normalize required/optional/default semantics into binding metadata. |
| Per-argument binding rules | Slice 1 projection now normalizes arg type, required/default status, lookup metadata, and pack-question joins for 71 authored verb bindings. | Unblocks deterministic pending-question quality for Slice 1. | Complete for projection presence; next hardening is policy-grade confidence/scoping semantics before envelope projection. |
| Lookup metadata | Present inside selected args and constellation maps, but distributed. | Blocks consistent entity resolution hints. | Normalize lookup surface into SemOS registry projection. |
| Entity-grain effects | Slice 1 projection now emits 78 authored allowed/forbidden verb effect records with read/write entity grains, source tables, side-effect class, CRUD/return shape, produced grain, lifecycle arg, and transition arg. | Partially unblocks safe refusal and impact narration for Slice 1 authored verbs. | Complete for authored Slice 1 verbs; extend to tiered macros/workbook plans before envelope projection. |
| FSM transition references | State machines exist separately from verbs; some DAG entries reference `via` verbs. | Blocks state-aware macro projection. | Link Slice 1 verbs/macros to state definitions explicitly. |
| HITL flags | Slice 1 verb effects, macro tiers, and workbook plans now project confirmation/HITL requirements from exposure, mutation evidence, and pack risk policy. | Partially unblocks owner/approval gates for Slice 1. | Complete for projection presence; later runtime gates still need owner identity and authorization policy. |
| Dry-run flags | Slice 1 policy projection now marks mutating paths as dry-run-required and records whether explicit `dry-run` args are present. Missing dry-run support becomes a `policy_gap`/refusal condition. | Partially unblocks dry-run plan guarantees by making gaps explicit. | Complete for static projection; later remediation should add true dry-run support where required. |
| Diagnostic codes | Projection now includes taxonomy entries for ambiguous pack, unsupported macro tier, forbidden verb, missing binding, and legacy route bait. | Unblocks structured refusal code selection for Slice 1. | Complete for static projection; runtime emission remains later envelope/gate work. |
| Macro slots and steps | Slice 1 projection now tiers 21 pack macro refs: 18 direct registry macros as `project`, 3 nested-composite registry macros as `lift`, and 0 as `quarantine`. Research macros remain out of Slice 1. | Unblocks macro-grade separation for Slice 1. | Complete for Slice 1 tiering; macro expansion belongs to Gate D/envelope work. |
| Workbook execution plans | 7 workflow YAML files and pack templates exist. Slice 1 pack templates are now lifted into six read-only workbook-plan projection records. | Partially closes context parity for workflow-plan fixtures. | Continue with policy, state-effect, and refusal metadata before envelope projection. |

Slice 1 blockers:

- Gate D envelope v2 construction and signing.
- Returns/producers normalization into production envelope fields.
- Runtime context planning for Slice 2.
