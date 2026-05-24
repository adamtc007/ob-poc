#!/usr/bin/env bash
# check-layering.sh — forbidden-edge guard for ob-poc.
# Run from the repo root (ob-poc/).
#
# After the library extraction (2026-05-22), the dsl and sem-os repos each
# carry their own guard (scripts/check-layering.sh in ~/dev/dsl/ and
# ~/dev/sem-os/). This guard is now scoped to what lives IN ob-poc:
#
#   Rule 1: bpmn-lite must NOT reach sideways into ob_poc_types
#           (bpmn-lite-engine + bpmn-lite-store — the two files that
#            previously imported SessionStackState etc.)
#
#   Rule 2: no ob-poc crate Cargo.toml may use a hard-coded path dep back
#           to the six extracted crate directories. They must all use
#           workspace = true (pointing at the git-tag workspace.dependencies).
#
# The dsl-lang and sem-os layering rules are enforced inside those repos:
#   ~/dev/dsl/scripts/check-layering.sh
#   ~/dev/sem-os/scripts/check-layering.sh
set -uo pipefail

fail=0
note() { printf '  \033[31mFORBIDDEN EDGE\033[0m  %s\n' "$1"; fail=1; }

echo "== ob-poc layering guard =="

# Rule 1: bpmn-lite must NOT reach into ob_poc_types.
for bpmn_src in bpmn-lite/bpmn-lite-engine/src bpmn-lite/bpmn-lite-store/src; do
  [ -d "$bpmn_src" ] || continue
  hits="$(grep -rnE '\bob_poc_types\b' "$bpmn_src" 2>/dev/null \
    | grep -vE ':[0-9]+:[[:space:]]*//' || true)"
  [ -n "$hits" ] && note "$(basename $(dirname $bpmn_src)) references ob_poc_types:
$hits"
done

# Rule 2: no Cargo.toml in rust/crates/ or rust/xtask/ may use a
# hard-coded path dep to any of the six extracted crate directories.
EXTRACTED='dsl-core|dsl_types|sem_os_core|sem_os_types|sem_os_ontology|sem_os_policy'
hits="$(grep -rnE "($EXTRACTED).*path\s*=" \
  rust/crates/*/Cargo.toml rust/xtask/Cargo.toml rust/Cargo.toml 2>/dev/null \
  | grep -vE ':[0-9]+:[[:space:]]*//' \
  | grep -vE 'workspace\.dependencies' || true)"
[ -n "$hits" ] && note "path dep to extracted crate (should be workspace = true):
$hits"

if [ "$fail" -eq 0 ]; then
  echo "  OK — all ob-poc layering rules hold."
else
  echo ""
  echo "== Layering guard FAILED =="
fi
exit "$fail"
