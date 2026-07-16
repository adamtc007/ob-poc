#!/usr/bin/env bash
# EOP-PLAN-CONTROLPLANE-001 T6 exit criterion: "grep/CI gate — no call
# site constructs VerbExecutionContext outside control-plane-issued
# paths (allowlist file, CI-checked)". No such gate existed until this
# script — the independent adversarial review (docs/research/control-plane-pir-001.md,
# PIR-D-005) found T6's own literal exit criterion unmet and enumerated
# every construction site by hand (P3.1). This automates that enumeration
# as a standing CI gate.
#
# What this proves: every non-test `VerbExecutionContext::new(` call
# site in the workspace is either on the allowlist (a known, categorised,
# ledger-tracked site) or the build fails. It does NOT prove every
# allowlisted site is admission-wired — KNOWN-BYPASS entries are
# deliberately, honestly still bypasses (see the allowlist file's own
# comments and the ownership ledger). This is source scanning, not
# call-graph analysis, matching the same caveat scripts/lint_write_paths.sh
# already carries for a similar reason: it catches a NEW unlisted site
# appearing, it does not prove an EXISTING listed site's downstream
# behaviour.
#
# Test-only classification: brace-depth tracking, not a line-proximity
# heuristic — a construction site is test-only iff it falls inside the
# body of an item (mod or fn) annotated `#[cfg(test)]` / `#[cfg(any(test, ...))]`
# / `#[cfg(all(test, ...))]` on the nearest preceding attribute line,
# tracked by nesting depth so code appended AFTER a `#[cfg(test)] mod
# tests { ... }` block's closing brace is correctly classified as
# production, not test (an earlier nearest-preceding-marker-line
# heuristic got exactly this case wrong — verified by probe during
# development, see the wiring commit). This is still not a real parser:
# it counts braces textually, so a `{`/`}` inside a string literal or
# doc-comment code block could in principle confuse the depth counter.
#
# Bug found and fixed 2026-07-16 (G14/RR-2 closure sweep): the
# `#[cfg(all(test, feature = "database"))]` form (used by every
# db-integration test module gated on a live DATABASE_URL, e.g.
# `dsl-runtime/src/crud_executor.rs`'s `db_integration_tests`,
# `sem_os_runtime/verb_executor_adapter.rs`'s two feature-gated test
# modules) was NOT recognised by either substring check — only bare
# `#[cfg(test)]` and `#[cfg(any(test` matched — so those modules'
# `VerbExecutionContext::new(` test fixtures were misclassified as
# production and flagged as false-positive UNLISTED CONSTRUCTION SITEs.
# Added an `#[cfg(all(test` match alongside the other two. No such other
# case was found in this workspace's actual construction-site files at
# the time of writing; if a new `#[cfg(...)]` spelling appears, prefer
# restructuring the code over gaming the heuristic.

set -uo pipefail

cd "$(dirname "$0")/.."

ALLOWLIST_FILE="audits/surface/_verb-execution-context-allowlist.txt"
fail=0

echo "== VerbExecutionContext construction-site allowlist gate (T6 exit criterion, PIR-D-005) =="
echo ""

# sem_os_harness is a standalone integration-test-harness crate with zero
# production consumers workspace-wide (nothing depends on it as a
# package) — excluded by directory, not by heuristic, since the whole
# crate's purpose is test scaffolding.
hits=$(grep -rln "VerbExecutionContext::new(" rust/src rust/crates 2>/dev/null \
  | grep -v "^rust/crates/sem_os_harness/" \
  | sort -u)

for file in $hits; do
  # Character-by-character brace counting (not a regex gsub on literal
  # `{`/`}`): some awk implementations (notably macOS's BSD awk) mishandle
  # brace characters embedded in regex literals within the program source
  # itself, unrelated to the target text being scanned. Comparing
  # substr() output against a plain string is unambiguous everywhere.
  production_hit_count=$(awk -v ob="{" -v cb="}" '
    BEGIN { depth = 0; test_depth = -1; pending = 0; prod_hits = 0 }
    {
      line = $0
      if (test_depth == -1 && index(line, "#[cfg(test)") > 0) { pending = 1 }
      if (test_depth == -1 && index(line, "#[cfg(any(test") > 0) { pending = 1 }
      if (test_depth == -1 && index(line, "#[cfg(all(test") > 0) { pending = 1 }
      if (index(line, "VerbExecutionContext::new(") > 0 && test_depth == -1) {
        prod_hits++
      }
      n = length(line)
      for (i = 1; i <= n; i++) {
        c = substr(line, i, 1)
        if (c == ob) {
          depth++
          if (pending == 1 && test_depth == -1) {
            test_depth = depth
            pending = 0
          }
        } else if (c == cb) {
          if (test_depth == depth) test_depth = -1
          depth--
        }
      }
    }
    END { print prod_hits }
  ' "$file")

  [ "$production_hit_count" -eq 0 ] && continue

  rel="${file#rust/}"
  if grep -q "^${rel}:" "$ALLOWLIST_FILE"; then
    category=$(grep "^${rel}:" "$ALLOWLIST_FILE" | head -1 | cut -d: -f2)
    echo "  ALLOWLISTED ($category): $rel ($production_hit_count production site(s))"
  else
    echo "  UNLISTED CONSTRUCTION SITE: $rel ($production_hit_count production site(s))"
    echo "    A non-test VerbExecutionContext::new( call exists here with no"
    echo "    entry in $ALLOWLIST_FILE. Add one (ADMISSION-WIRED or"
    echo "    KNOWN-BYPASS, with a ledger row if it's a new bypass) or route"
    echo "    the call through execute_verb_admitting_envelope first."
    fail=1
  fi
done

echo ""
if [ "$fail" -eq 0 ]; then
  echo "== VerbExecutionContext allowlist gate PASSED =="
else
  echo "== VerbExecutionContext allowlist gate FAILED =="
fi
exit "$fail"
