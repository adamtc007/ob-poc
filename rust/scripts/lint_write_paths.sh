#!/usr/bin/env bash
# CI lint: verify no new raw SQL write statements appear outside allowlisted paths.
#
# SemOS is the hub. All agent-triggered DB mutations flow through DSL verbs.
# This lint catches accidental raw SQL writes in new code.
#
# Usage:
#   ./scripts/lint_write_paths.sh          # Check for violations
#   ./scripts/lint_write_paths.sh --update  # Regenerate baseline

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$SCRIPT_DIR/.."
BASELINE="$SCRIPT_DIR/write_path_baseline.txt"

# ─── Allowlisted directories and files ───────────────────────────────────────
# These paths are permitted to contain raw SQL INSERT/UPDATE/DELETE statements.
# They fall into three categories:
#
# 1. Verb execution layer (DSL verbs are the gate)
#    - domain_ops/          Plugin verb handlers (SemOsVerbOp impls)
#    - dsl_v2/              CRUD verb executor (generic_executor, sheet_executor)
#
# 2. Canonical persistence engines (SemOS-governed)
#    - derived_attributes/  Below-the-line derived value persistence
#    - sem_reg/             SemOS snapshot store + stewardship + projections
#
# 3. Infrastructure services (called only from verb handlers or internal pipelines)
#    - database/            Centralized DB access layer
#    - service_resources/   Pipeline service (pub(crate) guarded)
#    - services/            Attribute executor, board control, document catalog
#    - repl/                Session state (not entity data)
#    - session/             Unified session state
#    - bpmn_integration/    Workflow orchestration internals
#    - runbook/             Plan executor
#    - events/              Append-only event log
#    - calibration/         Calibration DB (diagnostic)
#    - agent/               Learning/telemetry (append-only)
#    - gleif/               External data import repository
#    - bods/                External data import repository
#    - sage/                Session context (internal state)
#    - state_reducer/       State machine transitions
#    - traceability/        Audit trail
#    - trading_profile/     Trading profile persistence
#    - entity_linking/      Entity linking compiler
#    - document_bundles/    Document bundle service
#    - research/            Research agent controller
#    - feedback/            Feedback inspector
#    - mcp/handlers/        MCP tool handlers (learning, scope)
#    - mcp/scope_resolution.rs  Scope resolution
#    - api/                 Legacy REST routes (being migrated to verb pipeline)
#    - bin/                 CLI tools

ALLOWED_DIRS=(
    "src/domain_ops/"
    "src/dsl_v2/"
    "src/derived_attributes/"
    "src/sem_reg/"
    "src/database/"
    "src/service_resources/"
    "src/services/"
    "src/repl/"
    "src/session/"
    "src/bpmn_integration/"
    "src/runbook/"
    "src/events/"
    "src/calibration/"
    "src/agent/"
    "src/gleif/"
    "src/bods/"
    "src/sage/"
    "src/state_reducer/"
    "src/traceability/"
    "src/trading_profile/"
    "src/entity_linking/"
    "src/document_bundles/"
    "src/research/"
    "src/feedback/"
    "src/mcp/"
    "src/api/"
    "src/bin/"
)

# Build grep exclusion pattern for allowed dirs
EXCLUDE_ARGS=()
for dir in "${ALLOWED_DIRS[@]}"; do
    EXCLUDE_ARGS+=(--glob "!${dir}**")
done

# ─── Scan for raw SQL writes ─────────────────────────────────────────────────

cd "$ROOT"

VIOLATIONS=$(rg --no-heading -n \
    'INSERT\s+INTO|UPDATE\s+.*\s+SET|DELETE\s+FROM' \
    src/ \
    --glob '*.rs' \
    "${EXCLUDE_ARGS[@]}" \
    2>/dev/null || true)

if [ "${1:-}" = "--update" ]; then
    echo "$VIOLATIONS" | sort > "$BASELINE"
    COUNT=$(echo "$VIOLATIONS" | grep -c . || true)
    echo "Baseline updated: $COUNT entries written to $BASELINE"
    exit 0
fi

if [ -z "$VIOLATIONS" ]; then
    echo "✓ No SQL write statements found outside allowlisted paths."
    exit 0
fi

# Compare against baseline if it exists
if [ -f "$BASELINE" ]; then
    CURRENT=$(echo "$VIOLATIONS" | sort)
    BASELINE_CONTENT=$(cat "$BASELINE")

    NEW_VIOLATIONS=$(comm -13 <(echo "$BASELINE_CONTENT") <(echo "$CURRENT"))

    if [ -z "$NEW_VIOLATIONS" ]; then
        BASELINE_COUNT=$(echo "$BASELINE_CONTENT" | grep -c . || true)
        echo "✓ No NEW SQL write statements. ($BASELINE_COUNT known baseline entries)"
        exit 0
    else
        echo "✗ NEW raw SQL write statements found outside allowlisted paths:"
        echo ""
        echo "$NEW_VIOLATIONS"
        echo ""
        echo "If these are intentional, run: ./scripts/lint_write_paths.sh --update"
        exit 1
    fi
else
    echo "⚠ No baseline file found. Run: ./scripts/lint_write_paths.sh --update"
    echo ""
    echo "Current violations:"
    echo "$VIOLATIONS"
    exit 1
fi
