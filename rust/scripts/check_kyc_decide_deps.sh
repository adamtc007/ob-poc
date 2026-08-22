#!/usr/bin/env bash
# Dep-gate: ob-poc-kyc-decide must never gain a dependency on ANY crate
# that exposes a dsl.kyc fact-stream append:
#
#   ob-poc-kyc-store  -- PgKycEventStore::append  (the real append: stream
#                        lock, seq allocation, INSERT into kyc_intent_events)
#   ob-poc-kyc-seam   -- append_in_scope          (the governed chokepoint
#                        that wraps it)
#
# WIDENED 2026-08-22. The original gate named only ob-poc-kyc-seam, which is
# the sole GOVERNED chokepoint but NOT the only crate-level write path. The
# reconciliation proved this by execution: a probe inside ob-poc-kyc-decide
# appended a real event via the public PgKycEventStore::append with
# ob-poc-kyc-seam nowhere in its tree, while this script still reported PASS.
# The evaluation pack now depends on ob-poc-kyc-read, which has no append.
#
# This is the structural proof behind EOP-DD-KYCUBO-TS.6 §8's
# `evaluation_pack_cannot_write_facts`: the Evaluation pack (decide.approve,
# decide.reject) cannot write to the fact stream not because a test checks
# each op's body, but because the crate that hosts them has no dependency
# edge to the crate that owns the only write path into it.
#
# Usage: run from the rust/ workspace directory.
#   ./scripts/check_kyc_decide_deps.sh
#
# Exit 0 = clean. Exit 1 = forbidden dep found (CI fails).
set -euo pipefail

CRATE="ob-poc-kyc-decide"
FORBIDDEN=(ob-poc-kyc-seam ob_poc_kyc_seam ob-poc-kyc-store ob_poc_kyc_store)

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
