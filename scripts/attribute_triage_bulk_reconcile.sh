#!/usr/bin/env bash
set -euo pipefail

INPUT="${1:-/tmp/attribute_triage_define_calls.dsl}"
DB_URL="${DATABASE_URL:-postgresql:///data_designer}"

if [ ! -f "$INPUT" ]; then
  echo "Input file not found: $INPUT" >&2
  exit 1
fi

echo "Bulk reconciling DSL from $INPUT"
cd "$(dirname "$0")/../rust"
DATABASE_URL="$DB_URL" cargo run --features database --bin bulk_attribute_reconcile -- "$INPUT"
