# Sem OS Domain Pack Taxonomy Reload Architecture

> **Date:** 2026-05-14
> **Status:** Implemented architecture note
> **Applies to:** `rust/config/sem_os_seeds/domain_packs`, Sem OS seed bundles, DAG taxonomies, DSL packs, reload index

## Decision

Sem OS domain-specific shapes are configuration, not code.

The primary source of truth for a domain's capability surface is the YAML corpus under:

- `rust/config/sem_os_seeds/domain_packs/*.yaml`
- `rust/config/sem_os_seeds/dag_taxonomies/*.yaml`
- `rust/config/sem_os_seeds/state_machines/*.yaml`
- `rust/config/sem_os_seeds/constellation_maps/*.yaml`
- `rust/config/sem_os_seeds/constellation_families/*.yaml`
- `rust/config/sem_os_seeds/universes/*.yaml`
- `rust/config/packs/*.yaml`
- `rust/config/ontology/entity_taxonomy.yaml`

Domain crates such as `ob-poc-deal`, `ob-poc-booking-principal`, and similar business crates are clients and execution/mechanics homes. They do not own Sem OS taxonomy shape. Domain-specific Sem OS shape belongs in Sem OS domain packs and their owned YAML surfaces.

## Ownership Model

A Domain Pack manifest declares the YAML surfaces it owns:

- `owned_dags`
- `owned_packs`
- `owned_state_machines`
- `owned_constellation_maps`
- `owned_constellation_families`
- `owned_universes`
- `owned_verb_prefixes`
- `owned_entity_kinds`
- `business_crates`

This lets Sem OS answer: "which domain pack is authoritative for this DAG, DSL pack, state machine, constellation, universe, verb prefix, or entity kind?"

The `business_crates` field is an informational link to code that may consume or implement behavior for the domain. It is not ownership of taxonomy shape.

## Reload Model

Reload uses a build-engine pattern:

1. Keep a persisted reload index per `pack_id`.
2. Store source file fingerprints: relative path, byte length, and modified timestamp.
3. On check, use those fingerprints as the cheap "maybe dirty" test.
4. If fingerprints match, skip parsing and hashing.
5. If any fingerprint changed, reload YAML, canonicalize surfaces, and compute a deterministic content hash.
6. If the content hash is unchanged, update the reload index only.
7. If the content hash changed, mark the pack as `publish_required`.

Timestamps and file sizes are an optimization only. They are never the correctness gate. The canonical surface hash decides whether Sem OS content actually changed.

## Publication Model

The reload checker does not publish Sem OS snapshots directly.

Publication stays behind the existing Sem OS seed bootstrap path:

- `sem_os_obpoc_adapter` scans YAML into a `SeedBundle`.
- `SeedBundle` includes domain packs as `DomainPack` objects.
- `CoreService::bootstrap_seed_bundle()` compares incoming seed payloads to active snapshots.
- Identical payloads are skipped.
- Changed payloads publish non-breaking successor snapshots with predecessor links.
- Type conflicts fail closed.

This keeps reload detection separate from registry mutation and preserves Sem OS immutability.

## Persistent Index

The reload index is stored in:

```sql
sem_reg.domain_pack_reload_index
```

Key fields:

- `pack_id`
- `source_fingerprints`
- `surface_hash`
- `snapshot_set_id`
- `last_checked_at`
- `last_loaded_at`
- `status`
- `diagnostics`

Statuses:

- `clean` — source fingerprints match the previous index.
- `index_only` — source fingerprints changed but canonical content hash did not.
- `publish_required` — canonical content hash changed and Sem OS seed bootstrap should publish.
- `loaded` — index row represents content known to have been loaded/published.

## Manual Trigger

The initial operational trigger is manual:

```bash
cd rust
cargo run --manifest-path xtask/Cargo.toml -- sem-reg domain-pack-check
cargo run --manifest-path xtask/Cargo.toml -- sem-reg domain-pack-check --pack-id ob-poc.cbu
cargo run --manifest-path xtask/Cargo.toml -- sem-reg domain-pack-check --force-check --update-index --json
```

The command checks all domain packs by default. It can persist refreshed index rows with `--update-index`. It intentionally reports `publish_required` rather than mutating Sem OS snapshots itself.

Startup should not perform full YAML reconciliation unconditionally. A future startup hook may run the cheap fingerprint check, but full reload and publication should remain explicit or environment-gated.

## CBU Reconciliation Rule

CBU is the reference case for this architecture.

The CBU domain pack owns the CBU DAG taxonomy, the CBU DSL pack, related CBU state machines/constellations/universes, CBU verb prefixes, and CBU entity kinds. Tests cross-reference CBU macros, DSL verbs, and DAG taxonomy entries to ensure macro expansion does not refer to verbs or states that are absent from the Sem OS-owned taxonomy.

This is the pattern for future domains: add or update YAML, declare ownership in a Domain Pack, run the reload check, then publish through the Sem OS seed bootstrap path when the canonical content hash changes.

## Implementation Map

- Policy/reload planner: `rust/crates/sem_os_policy/src/domain_pack.rs`
- Reload index store: `rust/crates/sem_os_postgres/src/store.rs`
- Seed bundle type: `rust/crates/sem_os_core/src/seeds.rs`
- YAML scanner: `rust/crates/sem_os_obpoc_adapter/src/pipeline_seeds.rs`
- Manual trigger: `rust/xtask/src/sem_reg.rs`
- Domain pack object migration: `rust/migrations/20260514_sem_os_domain_pack_object.sql`
- Reload index migration: `rust/migrations/20260514_domain_pack_reload_index.sql`
- CBU reconciliation harness: `rust/crates/dsl-core/tests/cbu_dsl_reconciliation.rs`
- DB reload-index harness: `rust/crates/sem_os_harness/src/lib.rs`
