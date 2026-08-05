#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/.." && pwd)
canonical="$repo_root/migrations/master-schema.sql"
database_url=${DATABASE_URL:?DATABASE_URL must identify a newly-created PostgreSQL database}

"$script_dir/verify-schema-artifacts.sh" >/dev/null

server_version_num=$(psql -X -At "$database_url" -c 'SHOW server_version_num')
if [ "$server_version_num" -lt 180000 ]; then
  echo "the canonical snapshot requires PostgreSQL 18 or newer; server reports $server_version_num" >&2
  exit 1
fi

user_relation_count=$(psql -X -At "$database_url" <<'SQL'
SELECT count(*)
FROM pg_class AS relation
JOIN pg_namespace AS namespace ON namespace.oid = relation.relnamespace
WHERE namespace.nspname NOT IN ('pg_catalog', 'information_schema')
  AND namespace.nspname !~ '^pg_toast'
  AND relation.relkind IN ('r', 'p', 'v', 'm', 'S', 'f')
  AND NOT EXISTS (
    SELECT 1
    FROM pg_depend AS dependency
    WHERE dependency.classid = 'pg_class'::regclass
      AND dependency.objid = relation.oid
      AND dependency.deptype = 'e'
  );
SQL
)

if [ "$user_relation_count" -ne 0 ]; then
  echo "refusing to bootstrap a non-empty database ($user_relation_count user relations found)" >&2
  exit 1
fi

psql -X -v ON_ERROR_STOP=1 --single-transaction "$database_url" -f "$canonical" >/dev/null

missing_contracts=$(psql -X -At "$database_url" <<'SQL'
WITH required(schema_name, table_name, column_name) AS (
  VALUES
    ('ob-poc', 'control_plane_audit', 'decision_id'),
    ('ob-poc', 'control_plane_envelopes', 'entry_id'),
    ('ob-poc', 'control_plane_shadow_decisions', 'execution_path'),
    ('ob-poc', 'control_plane_shadow_decisions', 'decision_id'),
    ('ob-poc', 'control_plane_shadow_decisions', 'floor_rejected')
)
SELECT string_agg(format('%I.%I.%I', schema_name, table_name, column_name), ', ')
FROM required
WHERE NOT EXISTS (
  SELECT 1
  FROM information_schema.columns AS actual
  WHERE actual.table_schema = required.schema_name
    AND actual.table_name = required.table_name
    AND actual.column_name = required.column_name
);
SQL
)

if [ -n "$missing_contracts" ]; then
  echo "bootstrap completed without required contracts: $missing_contracts" >&2
  exit 1
fi

schema_sha256=$(shasum -a 256 "$canonical" | awk '{print $1}')
echo "database_bootstrap=ok"
echo "postgres_server_version_num=$server_version_num"
echo "schema_sha256=$schema_sha256"
