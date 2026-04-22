#!/usr/bin/env bash
# CI lint L4 (Phase F guardrail, Pattern B A1 remediation ledger G3/G5):
# verify no NEW external I/O (HTTP / gRPC / subprocess) appears inside
# `execute` / `execute_json` bodies in plugin op files.
#
# Spec: `docs/todo/pattern-b-a1-remediation-ledger.md` §1 G3.
# A1 rule: no SemOS state advance requires external effects inside the
# inner transaction. Plugin ops that call out to HTTP / gRPC / spawn
# subprocesses violate this rule.
#
# Scope limitation:
#   This lint catches DIRECT external-I/O tokens in plugin op files. It does
#   NOT chase helpers — `bpmn_lite_ops.rs` routes gRPC via
#   `crate::bpmn_integration::client` (a helper module), so the regex sees
#   nothing in the op body itself even though the transitive effect is
#   external. Closing that scope gap needs dependency-graph analysis
#   (cargo-deny or a clippy lint with call-graph traversal) — Phase F.4
#   territory.
#
# What this lint DOES catch:
#   - New direct `reqwest::` / `tonic::` / `Command::new` usage in any
#     plugin op file — the pattern most likely to be introduced by
#     accident.
#   - Regression of the Phase 0g fix (subprocess spawn inside a plugin
#     body without outbox deferral).
#
# Four files are currently grandfathered in the remediation ledger:
#   - rust/src/domain_ops/bpmn_lite_ops.rs     (5 ops, gRPC via helper)
#   - rust/src/domain_ops/source_loader_ops.rs (16 ops, HTTP via helper)
#   - rust/src/domain_ops/gleif_ops.rs         (17 ops, HTTP via helper)
#   - rust/src/domain_ops/request_ops.rs       (Pattern B staging)
#
# plus the Slice-#80 regression site:
#   - rust/crates/sem_os_postgres/src/ops/agent.rs (ActivateTeaching
#     op — direct `tokio::process::Command::new` at line 796)
#
# This script tolerates those five files (they are in the ledger, scheduled
# for Phase F.1-F.4 remediation). ANY OTHER file containing an A1
# violation fails the lint — the compromise cannot be quietly widened.
#
# Usage:
#   ./scripts/lint_external_effects_in_verbs.sh         # Check for new violations
#   ./scripts/lint_external_effects_in_verbs.sh --list  # Show current grandfathered hits
#
# Exit codes:
#   0 — no new A1 violations
#   1 — one or more NEW violations detected (fails CI)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$SCRIPT_DIR/.."

# ── Grandfathered paths ────────────────────────────────────────────────
# These files are known A1 violators. They must remediate per the ledger
# (Phase F.1-F.3). Until remediation, they are the ONLY files allowed to
# contain external-I/O inside verb bodies.
GRANDFATHERED=(
    "src/domain_ops/bpmn_lite_ops.rs"
    "src/domain_ops/source_loader_ops.rs"
    "src/domain_ops/gleif_ops.rs"
    "src/domain_ops/request_ops.rs"
    # Slice #80 relocation regression: `ActivateTeaching` op spawns
    # `populate_embeddings` via `tokio::process::Command::new("cargo")`
    # at line 796. Same pattern as the original Phase 0g-remediated
    # `MaintenanceReindexEmbeddingsOp` but in a different file. Added
    # to the ledger §2 on 2026-04-22 for Phase F follow-on remediation.
    "crates/sem_os_postgres/src/ops/agent.rs"
)

# ── External-I/O patterns ──────────────────────────────────────────────
# Tokens that indicate an external effect inside a verb body. Catches:
#   HTTP:   reqwest::, .get(, .post(, .send(, http::, hyper::
#   gRPC:   tonic::, .call(
#   subprocess: tokio::process::, Command::new, std::process::Command
#
# Note: `.get(` / `.post(` / `.send(` match many innocuous things (map.get,
# Vec::post doesn't exist but a mock might, Option::send doesn't exist).
# The lint is pessimistic — use `#[allow(external_effects_in_verb)]` with
# a comment citing this ledger for false positives.
EXTERNAL_IO_REGEX='reqwest::|tonic::|tokio::process::|Command::new|hyper::|\.\s*http\(\)|hyper_util::|std::process::Command'

# ── Plugin op files ────────────────────────────────────────────────────
# Every file containing `impl SemOsVerbOp` — these are the A1-relevant
# bodies. Pre-slice-#80 this was `impl CustomOperation`; post-slice-#80
# it's `impl SemOsVerbOp`.
PLUGIN_FILES="$(cd "$ROOT" && grep -rl "impl SemOsVerbOp" src/ crates/ 2>/dev/null | sort -u)"

MODE="check"
if [[ "${1:-}" == "--list" ]]; then
    MODE="list"
fi

VIOLATIONS=()
GRANDFATHERED_HITS=()

for file in $PLUGIN_FILES; do
    hits=$(cd "$ROOT" && grep -En "$EXTERNAL_IO_REGEX" "$file" 2>/dev/null || true)
    if [[ -z "$hits" ]]; then
        continue
    fi
    # Is this file grandfathered?
    is_grandfathered="false"
    for g in "${GRANDFATHERED[@]}"; do
        if [[ "$file" == "$g" ]]; then
            is_grandfathered="true"
            break
        fi
    done
    if [[ "$is_grandfathered" == "true" ]]; then
        while IFS= read -r line; do
            GRANDFATHERED_HITS+=("$file:$line")
        done <<< "$hits"
    else
        while IFS= read -r line; do
            VIOLATIONS+=("$file:$line")
        done <<< "$hits"
    fi
done

if [[ "$MODE" == "list" ]]; then
    echo "═══════════════════════════════════════════════════════════════════"
    echo "  Grandfathered A1 violations (per ledger §2 + §3)"
    echo "═══════════════════════════════════════════════════════════════════"
    if [[ ${#GRANDFATHERED_HITS[@]} -eq 0 ]]; then
        echo "  (none — ledger closed)"
    else
        for h in "${GRANDFATHERED_HITS[@]}"; do
            echo "  $h"
        done
    fi
    echo ""
    echo "═══════════════════════════════════════════════════════════════════"
    echo "  Non-grandfathered violations (MUST BE ZERO)"
    echo "═══════════════════════════════════════════════════════════════════"
    if [[ ${#VIOLATIONS[@]} -eq 0 ]]; then
        echo "  (none — invariant holds)"
    else
        for v in "${VIOLATIONS[@]}"; do
            echo "  $v"
        done
    fi
    exit 0
fi

# Check mode: fail on any non-grandfathered violation.
if [[ ${#VIOLATIONS[@]} -gt 0 ]]; then
    echo "✗ LINT L4 FAIL: ${#VIOLATIONS[@]} new A1 violation(s) detected."
    echo ""
    echo "External I/O (HTTP / gRPC / subprocess) inside a plugin op body"
    echo "violates the A1 invariant per three-plane v0.3 §11.2:"
    echo "  'no SemOS state advance requires external effects in the inner txn'"
    echo ""
    echo "Violations:"
    for v in "${VIOLATIONS[@]}"; do
        echo "  $v"
    done
    echo ""
    echo "Options:"
    echo "  1. Refactor to two-phase fetch-then-persist"
    echo "  2. Defer the external call via public.outbox + a drainer consumer"
    echo "     (pattern: see sem_os_maintenance_ops.rs MaintenanceReindexEmbeddingsOp)"
    echo "  3. Add an #[allow(external_effects_in_verb)] with a TODO citing the"
    echo "     remediation ledger — ONLY if the file is being added to §2 or §3"
    echo "     of docs/todo/pattern-b-a1-remediation-ledger.md in the same PR."
    echo ""
    exit 1
fi

echo "✓ LINT L4 PASS: no new A1 violations."
echo "  Grandfathered hits: ${#GRANDFATHERED_HITS[@]} (scheduled for Phase F.1-F.3)."
exit 0
