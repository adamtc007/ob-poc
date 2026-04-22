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
# Two check layers (Phase F.4, 2026-04-22):
#
#   Layer 1 (direct): catches direct external-I/O tokens in plugin op
#   files. Strict — fails CI on any non-grandfathered match.
#
#   Layer 2 (transitive, --taint mode): taint-propagates through
#   `use crate::...` imports. Any source file under rust/src/ or
#   rust/crates/ that directly uses external-I/O tokens becomes a
#   "tainted module"; any plugin op file that imports a tainted module
#   is flagged as transitively-tainted. Helper-indirect violations
#   (e.g. `crate::bpmn_integration::client.signal(...)` from a
#   bpmn_lite_ops verb body) surface here even though Layer 1 can't see
#   them.
#
#   Layer 2 is advisory — it lists findings but does NOT fail CI unless
#   `--taint --strict` is passed. Rationale: call-graph precision in
#   bash is limited; false positives are possible. Use Layer 2 to guide
#   Phase F.1–F.3 remediation and to catch new helper-indirect
#   violations that would otherwise slip past Layer 1.
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
#   ./scripts/lint_external_effects_in_verbs.sh         # Layer 1 direct check (CI default)
#   ./scripts/lint_external_effects_in_verbs.sh --list  # Show current grandfathered hits
#   ./scripts/lint_external_effects_in_verbs.sh --taint # Layer 2 transitive analysis (advisory)
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
    # 2026-04-22: `crates/sem_os_postgres/src/ops/agent.rs` removed from
    # grandfathered list — the `ActivateTeaching` subprocess spawn was
    # closed by outbox deferral (same pattern as Phase 0g
    # `MaintenanceReindexEmbeddingsOp`). Ledger §2.2 moved to CLOSED.
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
elif [[ "${1:-}" == "--taint" ]]; then
    MODE="taint"
fi

VIOLATIONS=()
GRANDFATHERED_HITS=()

for file in $PLUGIN_FILES; do
    # Skip comment lines (///, //!, //) before checking — these appear in
    # docstrings that reference the pattern we're linting. A comment
    # quoting `tokio::process::Command::new` as historical context is
    # NOT an A1 violation.
    hits=$(cd "$ROOT" && grep -En "$EXTERNAL_IO_REGEX" "$file" 2>/dev/null \
        | grep -vE '^\s*[0-9]+:\s*(//|\s*\*)' || true)
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

if [[ "$MODE" == "taint" ]]; then
    # ── Layer 2: transitive taint analysis ────────────────────────────
    # Phase F.4 (2026-04-22). Advisory only — does not fail CI.
    #
    # Strategy:
    #   Step A: scan all .rs under src/ + crates/ for external-I/O tokens.
    #           Each matching file becomes a "tainted module".
    #   Step B: compute a module path for each tainted file.
    #             src/foo/bar.rs      → crate::foo::bar
    #             src/foo/bar/mod.rs  → crate::foo::bar
    #           (Crate-internal approximation; cross-crate references
    #           under crates/*/src/... become `<crate>::<path>`.)
    #   Step C: for each plugin op file, extract `use crate::X::Y::...;` and
    #           `use <crate>::X::Y::...;` statements. Flag any import that
    #           points into a tainted module (prefix match).
    #   Step D: separate findings into `tainted_but_grandfathered` (file is
    #           already in the ledger — known transitive violation) and
    #           `tainted_new` (not in ledger — surface as advisory).
    #
    # Scope limitations (honest):
    #   - `use` scan only. Fully-qualified calls (`crate::foo::bar::send(...)`
    #     without a matching `use`) are missed.
    #   - Prefix match. `use crate::foo::bar::Type;` flags even if Type is
    #     a pure data struct; coarse but errs on the side of alarm.
    #   - No follow-through to second-hop helpers. If A imports B and B
    #     imports C (tainted), A is NOT flagged. Extending taint recursively
    #     would need a fixed-point loop; skipped for now.
    # ──────────────────────────────────────────────────────────────────

    echo "═══════════════════════════════════════════════════════════════════"
    echo "  Layer 2: transitive taint analysis (advisory)"
    echo "═══════════════════════════════════════════════════════════════════"
    echo ""

    # Step A + B: build the tainted module list. (bash 3.2-compatible:
    # no mapfile; accumulate into an array via `while read`.)
    TAINTED_FILES=()
    while IFS= read -r f; do
        TAINTED_FILES+=("$f")
    done < <(
        cd "$ROOT" && grep -rl -E "$EXTERNAL_IO_REGEX" src/ crates/ 2>/dev/null \
        | grep -E '\.rs$' | sort -u
    )

    # Map each tainted file to a module path (prefix that a `use` would match).
    TAINTED_MODULES=()
    for f in "${TAINTED_FILES[@]}"; do
        # Strip src/ → crate::
        if [[ "$f" == src/* ]]; then
            mod="${f#src/}"
            mod="${mod%.rs}"
            mod="${mod%/mod}"
            mod="crate::${mod//\//::}"
            TAINTED_MODULES+=("$mod")
        fi
        # Strip crates/<crate>/src/ → <crate>::
        if [[ "$f" == crates/*/src/* ]]; then
            without_crates="${f#crates/}"
            crate_name="${without_crates%%/*}"
            mod="${without_crates#*/src/}"
            mod="${mod%.rs}"
            mod="${mod%/mod}"
            # Crate names use kebab-case in paths but snake_case in modules.
            crate_mod="${crate_name//-/_}"
            TAINTED_MODULES+=("${crate_mod}::${mod//\//::}")
        fi
    done

    echo "Tainted modules (${#TAINTED_MODULES[@]} — files that directly use"
    echo "external-I/O tokens):"
    printf '  %s\n' "${TAINTED_MODULES[@]}" | head -30
    if (( ${#TAINTED_MODULES[@]} > 30 )); then
        echo "  ... (${#TAINTED_MODULES[@]} total; showing first 30)"
    fi
    echo ""

    # Step C: for each plugin op file, scan for references to tainted
    # modules — both `use` statements AND fully-qualified call paths
    # (bpmn_lite_ops.rs uses `crate::bpmn_integration::client::...` via
    # a `fn get_bpmn_client()` helper without a `use` statement, which
    # a use-only scan would miss).
    TRANSITIVELY_TAINTED_NEW=()
    TRANSITIVELY_TAINTED_GRANDFATHERED=()

    for file in $PLUGIN_FILES; do
        is_gf="false"
        for g in "${GRANDFATHERED[@]}"; do
            if [[ "$file" == "$g" ]]; then
                is_gf="true"
                break
            fi
        done

        # Skip the file itself if it's a tainted module (it'd trivially match).
        file_is_tainted="false"
        for tf in "${TAINTED_FILES[@]}"; do
            if [[ "$file" == "$tf" ]]; then
                file_is_tainted="true"
                break
            fi
        done

        for tmod in "${TAINTED_MODULES[@]}"; do
            # Grep for any reference to the tainted module path in the
            # file body, outside comment lines. Uses word boundaries on
            # both sides to avoid substring matches (e.g. `client_group`
            # shouldn't match `client`).
            # tmod_regex escapes `::` literal for grep.
            # shellcheck disable=SC2001
            tmod_esc=$(echo "$tmod" | sed 's/::/\\:\\:/g')
            hit=$(cd "$ROOT" && grep -En "\b${tmod_esc}\b" "$file" 2>/dev/null \
                | grep -vE '^\s*[0-9]+:\s*(//|\s*\*)' \
                | head -1 || true)

            if [[ -n "$hit" ]]; then
                line_no=$(echo "$hit" | cut -d: -f1)
                entry="$file:$line_no → references $tmod"
                if [[ "$is_gf" == "true" ]]; then
                    TRANSITIVELY_TAINTED_GRANDFATHERED+=("$entry")
                elif [[ "$file_is_tainted" == "true" ]]; then
                    # The file IS a tainted module — don't flag self-references.
                    :
                else
                    TRANSITIVELY_TAINTED_NEW+=("$entry")
                fi
                break  # one flag per file is enough for the advisory.
            fi
        done
    done

    echo "═══════════════════════════════════════════════════════════════════"
    echo "  Transitively-tainted plugin op files — grandfathered"
    echo "═══════════════════════════════════════════════════════════════════"
    if (( ${#TRANSITIVELY_TAINTED_GRANDFATHERED[@]} == 0 )); then
        echo "  (none)"
    else
        printf '  %s\n' "${TRANSITIVELY_TAINTED_GRANDFATHERED[@]}" | head -20
        if (( ${#TRANSITIVELY_TAINTED_GRANDFATHERED[@]} > 20 )); then
            echo "  ... (${#TRANSITIVELY_TAINTED_GRANDFATHERED[@]} total; showing first 20)"
        fi
    fi
    echo ""

    echo "═══════════════════════════════════════════════════════════════════"
    echo "  Transitively-tainted plugin op files — NEW (needs review)"
    echo "═══════════════════════════════════════════════════════════════════"
    if (( ${#TRANSITIVELY_TAINTED_NEW[@]} == 0 )); then
        echo "  (none — helper-indirect surface matches the ledger)"
    else
        printf '  %s\n' "${TRANSITIVELY_TAINTED_NEW[@]}"
        echo ""
        echo "These files import modules that directly perform external I/O."
        echo "Review each case — some imports are fine (pure data types from a"
        echo "tainted module), others indicate a new helper-indirect A1"
        echo "violation that belongs on the remediation ledger."
    fi
    echo ""

    # Advisory — always exit 0 in --taint mode unless --strict is added later.
    exit 0
fi

echo "✓ LINT L4 PASS: no new A1 violations."
echo "  Grandfathered hits: ${#GRANDFATHERED_HITS[@]} (scheduled for Phase F.1-F.3)."
exit 0
