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
#   Rule 2 (T11.2 Part A, 2026-07-13): ob-poc-agent has no dependency edge
#            back to ob-poc (L1, EOP-VS-CONTROLPLANE-001 dependency-direction
#            lock). Verified by cargo tree, not by convention — a future
#            agent-tier function reaching for a capability handle directly
#            would reintroduce this edge, and this rule fails the build
#            instead of waiting for someone to notice.
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

# Rule 2: ob-poc-agent must not resolve an edge to ob-poc (L1).
agent_tree="$(cd rust && cargo tree -p ob-poc-agent --edges normal 2>&1)"
if echo "$agent_tree" | grep -qE '(^| )ob-poc v[0-9]'; then
  note "ob-poc-agent has a dependency edge to ob-poc (L1 violation):
$(echo "$agent_tree" | grep -E '(^| )ob-poc v[0-9]')"
fi

if [ "$fail" -eq 0 ]; then
  echo "  OK — ob-poc layering rules hold."
  echo "  (bpmn-lite rule enforced by bpmn-lite/scripts/check-layering.sh)"
else
  echo ""
  echo "== Layering guard FAILED =="
fi
exit "$fail"
