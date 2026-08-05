# Database bootstrap and schema release contract

## Supported database

`ob-poc` clean installations require PostgreSQL 18 or newer with the
extensions declared by the canonical schema, including pgvector. The supported
container image for qualification is `pgvector/pgvector:pg18`.

## Clean installation

The clean-install authority is `migrations/master-schema.sql`.
`schema_export.sql` is a byte-identical convenience copy. Create an empty
database, then run:

```bash
DATABASE_URL=postgresql://... ./scripts/bootstrap-database.sh
```

The bootstrap command fails closed when the database is non-empty, when the
server is older than PostgreSQL 18, when the two checked-in schema artifacts
have drifted, or when required control-plane columns are absent after import.
It does not load seed or production data.

Do not use `cargo sqlx migrate run --source rust/migrations` to create a new
database. That directory is the historical incremental-change ledger and
assumes foundational tables from the canonical snapshot. In particular,
migration `073_entity_linking_support.sql` references the existing entity
model. Treating that ledger as a baseline is unsupported.

## Regenerating the schema artifacts

Bring a database to the intended release schema, then run from `rust/`:

```bash
DATABASE_URL=postgresql://... cargo x schema-export
```

The exporter performs one `pg_dump` into the canonical path and copies those
exact bytes to `schema_export.sql`. This avoids the random `psql` restrict token
drift produced by independent dumps and pins a deterministic restrict key so
equivalent database schemas produce identical release hashes. SQLx's
`_sqlx_test` schema and
`public._sqlx_migrations` bookkeeping table are excluded because neither is an
application schema contract. Verify the result with:

Because the restrict key is deterministic, run the exporter only against a
trusted release database. Do not use it with an arbitrary or untrusted
PostgreSQL server: a server that knows the key can craft dump output around
the client-side `psql` safety marker.

```bash
./scripts/verify-schema-artifacts.sh
```

## Upgrade discipline

Incremental migrations remain reviewable upgrade evidence for databases that
already contain their prerequisites. A release owner must apply the migrations
introduced since the deployed schema snapshot in filename order, verify each
precondition against a restored production backup, and regenerate the
canonical snapshot after a successful upgrade rehearsal.

The historical directory contains repeated date prefixes and predates a
single automated deployment runner. It is not represented as a fully ordered
SQLx clean-install chain. A future migration must use a unique timestamp prefix
and must not silently rewrite a migration already recorded against a deployed
database.

## Release binding

`scripts/build-release-candidate.sh` verifies the schema artifacts and records
their SHA-256 digest in both the image labels and the release receipt. The
application image and schema snapshot can therefore be promoted and audited as
one release input without packaging database credentials into the image.
