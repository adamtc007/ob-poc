# SemOS Enrichment Work Plan

Status: Gate C implementation complete for Slice 1 static metadata. Projection shape, explicit no-arg verb contracts, per-argument binding metadata, entity-grain read/write effects, macro tiering, policy/refusal metadata, diagnostic taxonomy, and deterministic JSON output are implemented and test-covered.

Dependency order:

1. [x] Normalize Slice 1 pack IDs, allowed verbs, forbidden verbs, and templates into a single registry projection.
2. [x] Complete missing `args` on the 5 parsed verb definitions without argument contracts.
3. [x] Add per-argument binding metadata for Slice 1 required/pending-question fixtures.
4. [x] Add entity-grain read/write effects for Slice 1 allowed and forbidden verbs.
5. [x] Tier all macros used by Slice 1 packs as project/lift/retire/quarantine.
6. [x] Lift selected pack templates into workbook-plan records.
7. [x] Add HITL, dry-run, and refusal metadata to mutating verbs/macros/workbook plans.
8. [x] Add diagnostic taxonomy entries for ambiguous pack, unsupported macro tier, forbidden verb, missing binding, and legacy route bait.
9. [x] Add deterministic projection command and byte-equality check before production envelope work.

Current implementation:

- `rust/src/acp_registry_projection.rs` builds `acp_registry_projection_v1` for `onboarding-request`, `cbu-maintenance`, and `product-service-taxonomy`.
- The projection includes manifest hashes, deterministic projection hash, allowed/forbidden verbs, 71 pack-scoped verb binding records, 78 pack-scoped verb effect records, 21 macro-tier records, required/optional question metadata, risk policy, and six lifted workbook-plan records from Slice 1 pack templates.
- Verb binding records include arg type, required/default status, maps-to field, lookup table/entity/search metadata, binding source, and pack-question joins using hyphen/underscore-normalized field names.
- Verb effect records cover authored allowed and forbidden Slice 1 verbs, including exposure, behavior, side-effects class, CRUD/return shape, produced entity grain, subject grains, read/write entity grains, source tables, lifecycle entity arg, transition entity arg, and a deterministic effect hash.
- Macro tier records cover all Slice 1 macro references. The current shape is 18 direct `project` macros, 3 nested-composite `lift` macros, and 0 `quarantine` macros.
- Policy records now hang off authored verb effects, macro tiers, and workbook plans. They carry policy grade, confirmation/HITL requirements, dry-run required/supported flags, refusal conditions, and policy sources.
- The top-level diagnostic taxonomy includes `acp_ambiguous_pack`, `acp_unsupported_macro_tier`, `acp_forbidden_verb`, `acp_missing_binding`, and `acp_legacy_route_bait`.
- `cargo run --bin acp_registry_projection -- config` emits deterministic pretty JSON. Current sample output: 287,219 bytes, SHA-256 `1c915fc54d3d2ee0632d4c3da6506e5e87c5447dbfa8d69b765257503aeb8dcc`.
- `booking-location.list`, `booking-principal.coverage-matrix`, `booking-principal.gap-report`, `legal-entity.list`, and `research.sources.list` now carry explicit `args: []`.
- Regression coverage asserts the Slice 1 projection shape, stable hashes, byte-equal JSON serialization, workbook binding sources, `cbu.add-product` lookup bindings, `deal.request-onboarding` pack-question joins, entity-grain effects for allowed/forbidden verbs, macro tiering counts, policy/refusal metadata, diagnostic taxonomy, and `1324/1324` verb definitions with explicit `args`.

Initial Slice 1 focus:

- `onboarding-request`
- `cbu-maintenance`
- `product-service-taxonomy`

Deferred unless reviewer expands scope:

- KYC workflow plans.
- Instrument matrix pack.
- Research macros under `rust/config/macros/research`.
