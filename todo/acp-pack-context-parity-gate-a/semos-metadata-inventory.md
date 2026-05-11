# SemOS Metadata Inventory

Status: refreshed during Gate C. Slice 1 static metadata projection is now complete for the current acceptance scope. Remaining work belongs to Gate D/envelope hardening: returns/producers normalization into envelope v2, signing/build packaging, and runtime use.

Evidence commands:

- `todo/acp-pack-context-parity-gate-a/run_audit_inventory.sh`
- `ruby -ryaml -e 'count=0; files=Dir["../../rust/config/verbs/**/*.y{a,}ml"]; files.each{|f| y=YAML.load_file(f) rescue next; (y["domains"]||{}).each{|_,d| count += (d["verbs"]||{}).size}}; puts count'`
- `ruby -ryaml -e 'files=Dir["../../rust/config/packs/*.y{a,}ml"]; files.sort.each{|f| y=YAML.load_file(f); puts "#{File.basename(f)} #{y["id"]} verbs=#{(y["allowed_verbs"]||[]).size} forbidden=#{(y["forbidden_verbs"]||[]).size} templates=#{(y["templates"]||[]).size}"}'`

Inventory snapshot:

| Surface | Count | Source |
| --- | ---: | --- |
| Verb YAML files | 154 | `rust/config/verbs` |
| Verb definitions under `domains.*.verbs` | 1324 | YAML parse |
| Journey packs | 12 | `rust/config/packs` |
| SemOS seed YAML files | 142 | `rust/config/sem_os_seeds` |
| Stategraph YAML files | 9 | `rust/config/stategraphs` |
| State-machine YAML files | 30 | `rust/config/sem_os_seeds/state_machines` |
| Macro schema/registry YAML files | 29 | `rust/config/verb_schemas/macros`, `rust/config/macros` |
| Registry-style macro definitions | 140 | YAML parse of `kind: macro` |
| Workflow YAML files | 7 | `rust/config/workflows` |

Pack coverage:

| Pack | Allowed verbs/macros | Forbidden verbs | Templates |
| --- | ---: | ---: | ---: |
| `book-setup` | 49 | 2 | 2 |
| `booking-principal` | 9 | 0 | 0 |
| `catalogue` | 4 | 0 | 0 |
| `cbu-maintenance` | 43 | 1 | 2 |
| `deal-lifecycle` | 5 | 0 | 1 |
| `instrument-matrix` | 172 | 0 | 0 |
| `kyc-case` | 68 | 1 | 2 |
| `lifecycle-resources` | 16 | 0 | 0 |
| `onboarding-request` | 17 | 2 | 1 |
| `product-service-taxonomy` | 32 | 4 | 3 |
| `semos-maintenance` | 73 | 0 | 0 |
| `session-bootstrap` | 2 | 0 | 1 |

Verb metadata completeness:

| Field | Present | Total | Finding |
| --- | ---: | ---: | --- |
| `args` | 1324 | 1324 | Complete. Five no-input verbs were normalized to explicit `args: []`. |
| `metadata.side_effects` | 1324 | 1324 | Present at this broad level. Slice 1 projection now derives authored verb read/write entity grains for allowed and forbidden verbs. |
| `returns` | 1219 | 1324 | Incomplete for envelope projection. |
| `produces` | 30 | 1324 | Sparse. Cannot infer output entity grain from current config alone. |
| `handler` | 362 | 1324 | Many verbs are metadata-only or not directly bound to a handler in YAML. |

Gate A finding:

SemOS has enough source material to build a Slice 1 projection inventory, and the first read-only projection boundary now exists in `rust/src/acp_registry_projection.rs`. That projection now carries 71 pack-scoped verb binding records, 78 pack-scoped verb effect records, and 21 macro-tier records for Slice 1. Macro tiering currently classifies 18 direct registry macros as `project`, 3 nested-composite macros as `lift`, and 0 as `quarantine`. Verb effects, macro tiers, and workbook plans also carry policy grade, confirmation/HITL flags, dry-run required/supported flags, refusal conditions, and policy sources. The top-level diagnostic taxonomy covers ambiguous pack, unsupported macro tier, forbidden verb, missing binding, and legacy route bait. `cargo run --bin acp_registry_projection -- config` emits deterministic pretty JSON; the current sample is 287,219 bytes with SHA-256 `1c915fc54d3d2ee0632d4c3da6506e5e87c5447dbfa8d69b765257503aeb8dcc`. The remaining gaps are Gate D/envelope concerns rather than Gate C static projection blockers.
