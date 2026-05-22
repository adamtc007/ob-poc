#!/usr/bin/env bash
# check-layering.sh — forbidden-edge guard for DSL / SemOS / app layering.
# Exit non-zero if any layer reaches into a layer it must not depend on.
# Comments (// //! ///) are excluded so doc-comments do not false-positive.
# grep -rn output format is "path:linenum:content" so we filter the content
# portion with ':[0-9]+:[[:space:]]*//' to exclude comment lines correctly.
#
# CURRENT PATHS (pre-extraction, phases 0-5):
#   dsl-lang = rust/crates/dsl-core/src
#   sem-os   = rust/crates/sem_os_core/src
#   bpmn     = bpmn-lite/bpmn-lite-engine/src
#
# AFTER PHASE 3 RENAME + PHASES 6-7 REPO SPLIT, update the three *_SRC
# variables to the new locations, e.g.:
#   DSL_LANG_SRC="dsl/crates/dsl-lang/src"
#   SEMOS_SRC="sem-os/crates/sem-os-core/src"
#   BPMN_SRC="bpmn-lite/bpmn-lite-engine/src"
set -uo pipefail

fail=0
note() { printf '  \033[31mFORBIDDEN EDGE\033[0m  %s\n' "$1"; fail=1; }

# Run from the repo root (ob-poc/).
DSL_LANG_SRC="rust/crates/dsl-core/src"
SEMOS_SRC="rust/crates/sem_os_core/src"
BPMN_SRC="bpmn-lite/bpmn-lite-engine/src"
BPMN_STORE_SRC="bpmn-lite/bpmn-lite-store/src"

# ob-poc domain/app crate names that library layers must never import.
OBPOC_DOMAIN='ob_poc_types|ob_poc_boundary|ob_poc_sage|ob_poc_journey|ob_poc_domain|ob_poc_authoring|entity_gateway|dsl_runtime|ob_workflow|ob_agentic|ob_poc_web|inspector_projection|playbook_core|playbook_lower'

echo "== Layering guard =="

# Rule 1: dsl-lang must NOT reference sem_os_core, sem_os_ontology,
#          sem_os_policy, or sem_os_types (any SemOS layer).
#
# KNOWN VIOLATIONS TODAY (Phase 0 baseline — targets for Phase 3):
#   frontier/hydrator.rs, resolver/composer.rs, resolver/shape_rule.rs,
#   resolver/mod.rs  →  sem_os_ontology::constellation_map_def
#
# After Phase 1 the constellation_map_def refs will point at dsl_types
# (which is the substrate, not SemOS). After Phase 3 the remaining
# sem_os_* refs disappear as those files move to sem-os.
if [ -d "$DSL_LANG_SRC" ]; then
  hits="$(grep -rnE 'sem_os_core|sem_os_ontology|sem_os_policy|sem_os_types' "$DSL_LANG_SRC" 2>/dev/null \
    | grep -vE ':[0-9]+:[[:space:]]*//' || true)"
  [ -n "$hits" ] && note "dsl-lang references SemOS (target for Phase 3):
$hits"
fi

# Rule 2: sem-os must NOT reference ob-poc domain/app crates.
if [ -d "$SEMOS_SRC" ]; then
  hits="$(grep -rnE "\b(${OBPOC_DOMAIN})\b" "$SEMOS_SRC" 2>/dev/null \
    | grep -vE ':[0-9]+:[[:space:]]*//' || true)"
  [ -n "$hits" ] && note "sem-os references ob-poc domain/app:
$hits"
fi

# Rule 3: bpmn-lite must NOT reach sideways into ob_poc_types.
if [ -d "$BPMN_SRC" ]; then
  hits="$(grep -rnE '\bob_poc_types\b' "$BPMN_SRC" 2>/dev/null \
    | grep -vE ':[0-9]+:[[:space:]]*//' || true)"
  [ -n "$hits" ] && note "bpmn-lite references ob_poc_types (target for Phase 4):
$hits"
fi

if [ -d "$BPMN_STORE_SRC" ]; then
  hits="$(grep -rnE '\bob_poc_types\b' "$BPMN_STORE_SRC" 2>/dev/null \
    | grep -vE ':[0-9]+:[[:space:]]*//' || true)"
  [ -n "$hits" ] && note "bpmn-lite-store references ob_poc_types (target for Phase 4):
$hits"
fi

if [ "$fail" -eq 0 ]; then
  echo "  OK — all layering rules hold."
else
  echo ""
  echo "== Layering guard FAILED =="
  echo "   (Failures above are EXPECTED at Phase 0 baseline."
  echo "    Each phase eliminates specific violations — re-run after each phase.)"
fi
exit "$fail"
