#!/usr/bin/env bash
# check-no-widening.sh — anti-widening guard for the dead-code /
# dual-routing sweep (see docs/research/control-plane-ownership-ledger.md,
# "Dead code & dual-routing static sweep" entry).
#
# Purpose: prove that every deletion made during this sweep is actually a
# deletion — not a visibility widening (pub(crate) -> pub, a new `pub use`,
# etc.) used to dodge Phase 1's `dead_code = "deny"` gate, and not a new
# #[allow(dead_code)] / #[allow(unreachable_pub)] suppression added instead
# of deleting the flagged item. Zero-suppressions policy: this script is
# the enforcement mechanism for that policy, not just a description of it.
#
# Two checks, both asymmetric on purpose (growth fails, shrinkage passes):
#
#   1. Public API growth — for every crate with a committed
#      audits/surface/<crate>.txt baseline (the same baseline
#      check-public-api-surface.sh ratchets against exactly), the
#      current `cargo +nightly public-api --all-features` output must
#      not ADD any line vs that baseline. Removed lines (shrinkage) are
#      expected during this sweep and do not fail this check — that's
#      the actual deliverable. This deliberately does NOT replace
#      check-public-api-surface.sh's exact-match ratchet; it isolates
#      "did *this* work widen anything" independent of whether/when the
#      committed baseline itself gets refreshed to reflect the
#      deletions.
#
#   2. Suppression count — workspace-wide count of #[allow(dead_code)]
#      plus #[allow(unreachable_pub)] must not exceed the count recorded
#      in the baseline file. Any new allow site added during this sweep
#      to route around Phase 1's deny gate (instead of deleting the
#      flagged item) trips this.
#
# Usage:
#   scripts/check-no-widening.sh --record-baseline
#       Snapshot the current #[allow(...)] count as the "before any
#       change in this PR" baseline. Run this ONCE, before Phase 1.
#
#   scripts/check-no-widening.sh
#       Run both checks against that baseline. Run after every phase.
#
# Requires: nightly toolchain + cargo-public-api (same prerequisites as
# check-public-api-surface.sh — see that script / the CI workflow at
# .github/workflows/public-api-surface.yml for how CI provisions both).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

BASE=audits/surface
ALLOW_BASELINE="${BASE}/_no-widening-allow-baseline.txt"

count_allows() {
  # Sum of #[allow(dead_code)] + #[allow(unreachable_pub)] across the
  # workspace. Matches Phase 0's stated scope: these two lints are the
  # ones Phase 1 denies workspace-wide, so they're the two suppressions
  # a "delete it, don't gate it" violation would show up as.
  local dc unp
  dc=$(grep -rn '#\[allow(dead_code)\]' --include='*.rs' rust 2>/dev/null | wc -l | tr -d ' ')
  unp=$(grep -rn '#\[allow(unreachable_pub)\]' --include='*.rs' rust 2>/dev/null | wc -l | tr -d ' ')
  echo $((dc + unp))
}

if [ "${1:-}" = "--record-baseline" ]; then
  mkdir -p "$BASE"
  count_allows > "$ALLOW_BASELINE"
  echo "Recorded allow-count baseline: $(cat "$ALLOW_BASELINE") (dead_code + unreachable_pub allows)"
  echo "Baseline written to $ALLOW_BASELINE — commit this file."
  exit 0
fi

if [ ! -f "$ALLOW_BASELINE" ]; then
  echo "No baseline found at $ALLOW_BASELINE — run with --record-baseline first (once, before Phase 1)."
  exit 1
fi

fail=0

# cargo-public-api's rustdoc-JSON output orders auto-trait bounds
# (Send/Sync on downcast-helper methods like `into_any_arc`)
# non-deterministically across toolchain/version drift — confirmed
# 2026-07-15 this already silently breaks
# check-public-api-surface.sh's exact-match ratchet for sem_os_postgres,
# unrelated to any change in this sweep. Canonicalize both orderings so
# this known artifact doesn't drown out real signal.
normalize_bounds() {
  sed -E \
    -e 's/core::marker::Sync \+ core::marker::Send/core::marker::Send + core::marker::Sync/g'
}

echo "== 1. Public API growth (additions-only vs committed audits/surface/ baselines) =="
CRATES=$(cd rust && cargo metadata --format-version 1 --no-deps 2>/dev/null | jq -r '.packages[].name' | sort)
for pkg in $CRATES; do
  snap="${BASE}/${pkg}.txt"
  [ -f "$snap" ] || continue
  head -1 "$snap" | grep -q "MEASUREMENT FAILED" && continue

  now=$(cd rust && cargo +nightly public-api -p "$pkg" --all-features 2>/dev/null | normalize_bounds)
  if [ -z "$now" ]; then
    echo "  MEASUREMENT FAILED: $pkg (cargo public-api errored — check build)"
    fail=1
    continue
  fi

  base=$(tail -n +2 "$snap" | normalize_bounds)
  added=$(comm -13 <(echo "$base" | sort) <(echo "$now" | sort))
  if [ -n "$added" ]; then
    echo "  WIDENING: $pkg — new public item(s) not in baseline $snap:"
    echo "$added" | sed 's/^/    + /'
    fail=1
  fi
done

echo ""
echo "== 2. Suppression count (#[allow(dead_code)] + #[allow(unreachable_pub)]) =="
baseline_count=$(cat "$ALLOW_BASELINE")
current_count=$(count_allows)
echo "  baseline: $baseline_count  current: $current_count"
if [ "$current_count" -gt "$baseline_count" ]; then
  echo "  SUPPRESSION GROWTH: $((current_count - baseline_count)) new allow(s) added — delete the flagged item instead of suppressing it."
  fail=1
fi

echo ""
if [ "$fail" -eq 0 ]; then
  echo "== No-widening guard PASSED =="
else
  echo "== No-widening guard FAILED =="
fi
exit "$fail"
