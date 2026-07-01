#!/usr/bin/env bash
# check-public-api-surface.sh — public API surface ratchet.
# Run from the repo root (ob-poc/). Requires nightly toolchain + cargo-public-api.
#
# Three checks, one script, because they're the same measurement viewed
# three ways:
#
#   1. RATCHET — each crate's `--all-features` surface must exactly match
#      the committed baseline in audits/surface/<crate>.txt (header line
#      skipped). Any diff — growth OR shrink — fails. Surface changes are
#      real architecture decisions; a snapshot refresh in the same PR is
#      how a reviewer sees them instead of them landing silently.
#
#   2. MEMBRANE (charter-reconciliation-v1.md §6.2) — the crates with an
#      optional `database` feature must show zero sqlx/PgPool/PgConnection
#      symbols in their DEFAULT-features surface. The membrane may exist;
#      it must not be on by default.
#
#   3. TEST-DOUBLE LEAK (§8) — no crate's DEFAULT-features surface may
#      expose a Stub/Mock/Fake-prefixed type unconditionally. Test doubles
#      belong behind `#[cfg(test)]` or a dedicated feature, not shipped in
#      the crate's normal public API.
#
# Failure in any of the three fails the gate (exit 1).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

BASE=audits/surface
fail=0

CRATES=$(cd rust && cargo metadata --format-version 1 --no-deps 2>/dev/null | jq -r '.packages[].name' | sort)

MEMBRANE_CRATES="ob-poc-derived-attributes ob-poc-diagnostics ob-poc-entity-linking ob-poc-taxonomy ob-poc-trading-profile ob-poc-authoring ob-poc-sage"

echo "== 1. Ratchet: --all-features surface vs committed baseline =="
for pkg in $CRATES; do
  snap="${BASE}/${pkg}.txt"
  if [ ! -f "$snap" ]; then
    echo "  (no baseline for $pkg — skipping; new crate, needs a snapshot added)"
    continue
  fi
  if head -1 "$snap" | grep -q "MEASUREMENT FAILED"; then
    continue  # binary-only crate (no [lib] target) — expected, not a gate finding
  fi
  now=$(cd rust && cargo +nightly public-api -p "$pkg" --all-features 2>/dev/null)
  if [ -z "$now" ]; then
    echo "  MEASUREMENT FAILED: $pkg (cargo public-api errored — check build)"
    fail=1
    continue
  fi
  base=$(tail -n +2 "$snap")
  if [ "$now" != "$base" ]; then
    echo "  RATCHET TRIP: $pkg — public API changed vs ${snap}"
    diff <(echo "$base") <(echo "$now") | head -20
    echo "  (refresh with: cargo +nightly public-api -p $pkg --all-features | (echo \"# $pkg | features=all | HEAD=\$(git rev-parse HEAD) | \$(date -u +%Y-%m-%dT%H:%M:%SZ)\"; cat) > $snap)"
    fail=1
  fi
done

echo ""
echo "== 2. Membrane check (§6.2): default-features must carry no sqlx/PgPool =="
for pkg in $MEMBRANE_CRATES; do
  now=$(cd rust && cargo +nightly public-api -p "$pkg" 2>/dev/null)
  if [ -z "$now" ]; then
    echo "  BUILD FAILED AT DEFAULT FEATURES: $pkg (crate does not compile in isolation with its own default feature set — a Cargo.toml feature-declaration gap, not a membrane leak)"
    fail=1
    continue
  fi
  hits=$(echo "$now" | grep -E "sqlx::|PgPool|PgConnection" || true)
  if [ -n "$hits" ]; then
    echo "  MEMBRANE LEAK: $pkg default-features surface exposes DB types:"
    echo "$hits" | sed 's/^/    /'
    fail=1
  fi
done

echo ""
echo "== 3. Test-double leak check (§8): no Stub/Mock/Fake in default surface =="
for pkg in $CRATES; do
  now=$(cd rust && cargo +nightly public-api -p "$pkg" 2>/dev/null)
  [ -z "$now" ] && continue
  hits=$(echo "$now" | grep -E '(::|^pub (struct|enum|fn|use|trait) )(Stub|Mock|Fake)[A-Za-z0-9_]*\b' || true)
  if [ -n "$hits" ]; then
    echo "  TEST-DOUBLE LEAK: $pkg default-features surface exposes:"
    echo "$hits" | sed 's/^/    /'
    fail=1
  fi
done

echo ""
if [ "$fail" -eq 0 ]; then
  echo "== Public API surface gate PASSED =="
else
  echo "== Public API surface gate FAILED =="
fi
exit "$fail"
