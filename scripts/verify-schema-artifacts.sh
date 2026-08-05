#!/usr/bin/env bash

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/.." && pwd)
canonical="$repo_root/migrations/master-schema.sql"
convenience="$repo_root/schema_export.sql"

if [ ! -s "$canonical" ] || [ ! -s "$convenience" ]; then
  echo "schema snapshots are missing or empty" >&2
  exit 1
fi

if ! cmp -s "$canonical" "$convenience"; then
  echo "schema_export.sql has drifted from migrations/master-schema.sql" >&2
  echo "regenerate both with: DATABASE_URL=... cargo x schema-export" >&2
  exit 1
fi

require_pattern() {
  local pattern=$1
  local description=$2
  if ! grep -Eq "$pattern" "$canonical"; then
    echo "canonical schema is missing $description" >&2
    exit 1
  fi
}

require_pattern 'CREATE TABLE "ob-poc"\.control_plane_audit' 'control_plane_audit'
require_pattern 'CREATE TABLE "ob-poc"\.control_plane_envelopes' 'control_plane_envelopes'
require_pattern 'CREATE TABLE "ob-poc"\.control_plane_shadow_decisions' 'control_plane_shadow_decisions'
require_pattern 'entry_id uuid' 'control-plane entry identity'
require_pattern 'execution_path text' 'control-plane execution path'
require_pattern 'decision_id uuid' 'control-plane decision identity'
require_pattern 'floor_rejected boolean' 'control-plane floor decision fields'

if grep -Eq 'CREATE SCHEMA _sqlx_test|CREATE TABLE public\._sqlx_migrations' "$canonical"; then
  echo "canonical deployment schema contains SQLx test or migration-bookkeeping objects" >&2
  exit 1
fi

schema_sha256=$(shasum -a 256 "$canonical" | awk '{print $1}')
echo "schema_artifacts=ok"
echo "schema_sha256=$schema_sha256"
