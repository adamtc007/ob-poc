#!/usr/bin/env bash
# Dep-gate: ob-poc-kyc-substrate must never gain a transitive path to
# sqlx, tokio-postgres, sem_os_postgres, or dsl-runtime.
#
# These dependencies would mean the crate is no longer a pure in-memory
# semantic model (violates the W1-substrate contract in EOP-DD-KYCUBO-001).
#
# Usage: run from the rust/ workspace directory.
#   ./scripts/check_kyc_substrate_deps.sh
#
# Exit 0 = clean. Exit 1 = forbidden dep found (CI fails).
set -euo pipefail

CRATE="ob-poc-kyc-substrate"
FORBIDDEN=(sqlx tokio-postgres sem_os_postgres dsl-runtime dsl_runtime tokio_postgres)

echo "Checking transitive deps of ${CRATE} for forbidden crates..."
TREE=$(cargo tree -p "${CRATE}" 2>&1)

FOUND=0
for DEP in "${FORBIDDEN[@]}"; do
    if echo "${TREE}" | grep -q "${DEP}"; then
        echo "FAIL: forbidden dep '${DEP}' found in ${CRATE} dep tree"
        FOUND=1
    fi
done

if [ "${FOUND}" -eq 0 ]; then
    echo "PASS: no forbidden deps in ${CRATE}"
    exit 0
else
    echo ""
    echo "Dep tree for ${CRATE}:"
    echo "${TREE}"
    exit 1
fi
