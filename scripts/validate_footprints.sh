#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_PATH="${1:-$ROOT_DIR/artifacts/footprints/phase_s1_validation.json}"

cd "$ROOT_DIR"
env RUSTC_WRAPPER= cargo run --manifest-path rust/Cargo.toml --bin sem_os_footprint_audit -- validate "$OUTPUT_PATH"
