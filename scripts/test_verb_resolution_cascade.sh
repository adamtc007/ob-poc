#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
output_path="${1:-$repo_root/artifacts/footprints/phase_s8_cascade_results.json}"

cd "$repo_root"

cargo run --manifest-path rust/Cargo.toml --bin sem_os_footprint_audit -- \
  cascade-test "$output_path"
