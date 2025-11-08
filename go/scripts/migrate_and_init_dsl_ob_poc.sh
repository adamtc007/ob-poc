#!/usr/bin/env bash
#
# Rename schema "kyc-dsl" -> "dsl-ob-poc", set owner to adamtc007,
# run init.sql to ensure tables exist, and verify objects.
#
# Usage:
#   bash scripts/migrate_and_init_dsl_ob_poc.sh [DBNAME]
#
# If DBNAME is omitted, defaults to "$DB" env var, else "postgres".

set -euo pipefail

DB_INPUT="${1:-}"            # optional positional DB name
DB_ENV="${DB:-}"             # optional env var DB
DB_NAME="${DB_INPUT:-${DB_ENV:-postgres}}"

echo "Using database: ${DB_NAME}"

if ! command -v psql >/dev/null 2>&1; then
  echo "Error: psql not found in PATH" >&2
  exit 1
fi

# 1) Verify current schemas (optional)
echo "\n== Schemas before =="
psql -d "$DB_NAME" -c "\\dn+" || true

# 2) Migrate: rename schema and set owner to adamtc007
echo "\n== Running migration: rename schema and set owner =="
# If you are migrating from older schemas, adjust this migration as needed
# psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -f sql/migrate_kyc-dsl_to_ob-poc.sql

# 3) Initialize/ensure tables in new schema (idempotent)
echo "\n== Running init.sql to ensure tables =="
psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -f sql/init.sql

# 4) Ensure schema owner (redundant if migration ran, but safe)
echo "\n== Ensuring schema owner is adamtc007 =="
psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -c 'ALTER SCHEMA "ob-poc" OWNER TO "adamtc007";'

# 5) Ensure all objects in schema are owned by adamtc007 (tables + sequences)
echo "\n== Ensuring all objects in schema are owned by adamtc007 =="
psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "DO $$ DECLARE r RECORD; BEGIN \
  FOR r IN SELECT '"' || schemaname || '"."' || tablename || '"' AS q FROM pg_tables WHERE schemaname='ob-poc' LOOP \
    EXECUTE 'ALTER TABLE ' || r.q || ' OWNER TO \"adamtc007\"'; \
  END LOOP; \
  FOR r IN SELECT '"' || sequence_schema || '"."' || sequence_name || '"' AS q FROM information_schema.sequences WHERE sequence_schema='ob-poc' LOOP \
    EXECUTE 'ALTER SEQUENCE ' || r.q || ' OWNER TO \"adamtc007\"'; \
  END LOOP; \
END $$;"

# 6) Verify
echo "\n== Schemas after =="
psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "\\dn+"

echo "\n== Tables in schema ob-poc =="
psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -c '"\\dt+ \"ob-poc\".*"'

echo "\n== Sample latest DSL rows (if any) =="
psql -d "$DB_NAME" -v ON_ERROR_STOP=1 -c 'SELECT version_id, cbu_id, created_at FROM "ob-poc".dsl_ob ORDER BY created_at DESC LIMIT 5;'

echo "\nAll done."
