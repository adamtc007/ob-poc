#!/usr/bin/env bash
set -euo pipefail

# Migrate tables from schema "dsl-ob-poc" in existing DBs
# into a new database named "ob-poc" with schema "ob-poc".
#
# Defaults (override via env):
#   DEST_DB        - target database name (default: ob-poc)
#   SRC_CBUS_DB    - source DB for cbus table (default: postgres)
#   SRC_DICT_DB    - source DB for dictionary table (default: dsl_poc)
#   PSQL           - psql command (default: psql)

DEST_DB=${DEST_DB:-ob-poc}
SRC_CBUS_DB=${SRC_CBUS_DB:-postgres}
SRC_DICT_DB=${SRC_DICT_DB:-dsl_poc}
PSQL=${PSQL:-psql}

here() { echo "[migrate] $*"; }

run_sql_file() {
  local db=$1; shift
  local file=$1; shift
  if [[ ! -f "$file" ]]; then
    echo "File not found: $file" >&2
    return 1
  fi
  here "Applying $(basename "$file") to $db"
  # Normalize any lingering dsl-ob-poc schema references to ob-poc on the fly
  sed 's/"dsl-ob-poc"/"ob-poc"/g' "$file" | $PSQL -v ON_ERROR_STOP=1 -q -d "$db" -f -
}

ensure_database() {
  local db=$1
  here "Ensuring database $db exists"
  if ! $PSQL -q -d postgres -Atc "SELECT 1 FROM pg_database WHERE datname = '$db'" | grep -q 1; then
    $PSQL -q -d postgres -c "CREATE DATABASE \"$db\""
  fi
}

ensure_schema() {
  local db=$1
  here "Ensuring schema \"ob-poc\" exists in $db"
  $PSQL -q -d "$db" -c 'CREATE SCHEMA IF NOT EXISTS "ob-poc";'
}

copy_table_csv() {
  local src_db=$1
  local src_schema=$2
  local table=$3
  local dest_db=$4
  local dest_schema=$5
  local tmp_csv
  tmp_csv=$(mktemp "/tmp/${table}.XXXX.csv")
  here "Exporting $src_db.$src_schema.$table -> $tmp_csv"
  $PSQL -v ON_ERROR_STOP=1 -q -d "$src_db" -c "\\copy \"$src_schema\".$table to '$tmp_csv' csv header" || {
    rm -f "$tmp_csv"; return 1;
  }
  here "Importing $tmp_csv -> $dest_db.$dest_schema.$table"
  $PSQL -v ON_ERROR_STOP=1 -q -d "$dest_db" -c "\\copy \"$dest_schema\".$table from '$tmp_csv' csv header"
  rm -f "$tmp_csv"
}

verify_count() {
  local db=$1
  local schema=$2
  local table=$3
  local cnt
  cnt=$($PSQL -q -d "$db" -Atc "SELECT count(*) FROM \"$schema\".$table" 2>/dev/null || echo 0)
  echo "$db.$schema.$table|$cnt"
}

main() {
  here "Starting migration to database=$DEST_DB schema=\"ob-poc\""

  ensure_database "$DEST_DB"
  ensure_schema "$DEST_DB"

  # Apply base schema and migrations (normalized to ob-poc schema)
  run_sql_file "$DEST_DB" "sql/00_init_schema.sql"
  run_sql_file "$DEST_DB" "sql/migrations/001_dsl_domain_architecture.sql"
  run_sql_file "$DEST_DB" "sql/migrations/001_standardize_cbu_id_uuid.sql"
  run_sql_file "$DEST_DB" "sql/migrations/002_fix_foreign_key_constraints.sql"
  run_sql_file "$DEST_DB" "sql/migrations/003_runtime_api_endpoints.sql"
  run_sql_file "$DEST_DB" "sql/migrations/004_rename_individuals_to_proper_persons.sql"
  run_sql_file "$DEST_DB" "sql/migrations/005_dsl_business_request_lifecycle.sql"

  # Copy data from existing schemas where present
  # cbus from SRC_CBUS_DB.dsl-ob-poc
  if $PSQL -q -d "$SRC_CBUS_DB" -Atc "SELECT 1 FROM information_schema.tables WHERE table_schema='dsl-ob-poc' AND table_name='cbus'" | grep -q 1; then
    copy_table_csv "$SRC_CBUS_DB" "dsl-ob-poc" "cbus" "$DEST_DB" "ob-poc" || here "cbus copy skipped (no data)"
  else
    here "Source cbus table not found in $SRC_CBUS_DB.dsl-ob-poc"
  fi

  # dictionary from SRC_DICT_DB.dsl-ob-poc
  if $PSQL -q -d "$SRC_DICT_DB" -Atc "SELECT 1 FROM information_schema.tables WHERE table_schema='dsl-ob-poc' AND table_name='dictionary'" | grep -q 1; then
    copy_table_csv "$SRC_DICT_DB" "dsl-ob-poc" "dictionary" "$DEST_DB" "ob-poc" || here "dictionary copy skipped (no data)"
  else
    here "Source dictionary table not found in $SRC_DICT_DB.dsl-ob-poc"
  fi

  here "Verification counts in destination:"
  verify_count "$DEST_DB" "ob-poc" "cbus"
  verify_count "$DEST_DB" "ob-poc" "dictionary"
}

main "$@"

