#!/usr/bin/env bash
# check-layering.sh — ob-poc-side layering guard.
# Run from the repo root (ob-poc/).
#
# Library repo guards (each enforced in the repo that owns the code):
#   ~/dev/dsl/scripts/check-layering.sh      — dsl-core has no SemOS refs
#   ~/dev/sem-os/scripts/check-layering.sh   — sem-os has no ob-poc domain refs
#   bpmn-lite/scripts/check-layering.sh      — bpmn-lite has no ob_poc_types refs
#
# This guard covers what only ob-poc can see:
#   Rule 1: no ob-poc Cargo.toml uses a hard-coded path dep back to any of
#            the six extracted crate directories (must use workspace = true)
set -uo pipefail

fail=0
note() { printf '  \033[31mFORBIDDEN EDGE\033[0m  %s\n' "$1"; fail=1; }

echo "== ob-poc layering guard =="

# Rule 1: no Cargo.toml in rust/ should path-dep back to the extracted crates.
EXTRACTED='dsl-core|dsl_types|sem_os_core|sem_os_types|sem_os_ontology|sem_os_policy'
hits="$(grep -rnE "($EXTRACTED).*path\s*=" \
  rust/crates/*/Cargo.toml rust/xtask/Cargo.toml rust/Cargo.toml 2>/dev/null \
  | grep -vE ':[0-9]+:[[:space:]]*//' \
  | grep -vE 'workspace\.dependencies' || true)"
[ -n "$hits" ] && note "path dep to extracted crate (should be workspace = true):
$hits"

if [ "$fail" -eq 0 ]; then
  echo "  OK — ob-poc layering rules hold."
  echo "  (bpmn-lite rule enforced by bpmn-lite/scripts/check-layering.sh)"
else
  echo ""
  echo "== Layering guard FAILED =="
fi
exit "$fail"
