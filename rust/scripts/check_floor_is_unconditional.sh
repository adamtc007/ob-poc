#!/usr/bin/env bash
# T11.F.2 (EOP-PLAN-CONTROLPLANE-002): the definitional floor's rejection
# branches must never be wrapped by an env-var/feature toggle — that would
# turn a definitional control (no legitimate traffic can produce it) into a
# judgmental one gated by operator convenience, exactly what T11.F's model
# text forbids (see EOP-VS-CONTROLPLANE-001_Control-Plane_v0.4.2.md §7.1,
# and the T11.F design doc §0/§1).
#
# This is a source-scanning heuristic, not call-graph analysis (same caveat
# lint_write_paths.sh documents for its own allowlist): it checks the lines
# immediately preceding each floor call site for an `if` guarded by
# `std::env::var`/`cfg!(feature` and fails loudly if found. It does NOT
# prove the absence of a guard placed further up the call stack — this is
# a fast, cheap tripwire, not a substitute for reading the diff.
#
# Usage: run from the rust/ workspace directory.
#   ./scripts/check_floor_is_unconditional.sh
#
# Exit 0 = clean. Exit 1 = a floor call site appears env/flag-guarded (CI fails).
set -euo pipefail

# file:function-or-symbol pairs — the floor call sites landed in T11.F.2
# slices 2-4. Update this list if a floor call site moves or a new one is
# added; the check is only as good as this enumeration (same limitation
# lint_write_paths.sh's allowlist has).
declare -a SITES=(
    "src/agent/control_plane_envelope_store.rs:g1_verb_is_registered"
    "src/sem_os_runtime/verb_executor_adapter.rs:g1_verb_is_registered"
    "src/sequencer.rs:g3_input_is_floor_eligible"
    "src/sequencer.rs:g4_input_is_floor_eligible"
)

FOUND=0
for SITE in "${SITES[@]}"; do
    FILE="${SITE%%:*}"
    SYMBOL="${SITE##*:}"

    if [ ! -f "${FILE}" ]; then
        echo "FAIL: expected floor call site file not found: ${FILE}"
        FOUND=1
        continue
    fi

    LINE=$(grep -n "${SYMBOL}(" "${FILE}" | head -1 | cut -d: -f1)
    if [ -z "${LINE}" ]; then
        echo "FAIL: expected floor call site '${SYMBOL}' not found in ${FILE}"
        FOUND=1
        continue
    fi

    # Look at the 10 lines immediately before the call for an env/feature
    # guard that isn't this script's own commentary.
    CONTEXT=$(awk -v l="${LINE}" 'NR>=l-10 && NR<l' "${FILE}")
    if echo "${CONTEXT}" | grep -qE 'std::env::var|cfg!\(feature'; then
        echo "FAIL: possible env/feature guard near floor call site ${SYMBOL} in ${FILE}:${LINE}"
        echo "${CONTEXT}"
        FOUND=1
    fi
done

if [ "${FOUND}" -eq 0 ]; then
    echo "PASS: no env/feature guard found immediately preceding any floor call site"
    exit 0
else
    exit 1
fi
