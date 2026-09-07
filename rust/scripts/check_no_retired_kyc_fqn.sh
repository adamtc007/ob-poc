#!/usr/bin/env bash
# Standing gate (EOP-DD-UBO-CLEANOUT-001 C4): after the clean start, nothing
# anywhere refers to, resolves to, or was produced by a retired dsl.kyc verb
# FQN. This runs from now on so the next rename cannot leave residue — the
# previous three each did (§2's own audit trail).
#
# Surfaces checked:
#   1. source (rust/src, rust/crates/**/src, rust/crates/**/tests, fuzz,
#      rust/tests — the top-level external test harnesses — and
#      rust/xtask/src) — live (non-comment) lines only; historical notes in
#      `//`/`///`/`//!` comments and change logs are the document's own
#      explicit exemption (§2: "a grep ... returns nothing but deliberate
#      historical notes in change logs"). Missing `tests`/`xtask/src` here
#      once already let real residue in rust/tests/*.rs go undetected
#      (found and fixed EOP-DD-UBO-CLEANOUT-001 T6 P4, 2026-09-07) — a
#      concrete instance of exactly the drift C4 exists to prevent.
#   2. config (rust/config/**/*.yaml)
#   3. the three search-index DB tables (dsl_verbs, verb_pattern_embeddings,
#      verb_centroids) — NOT the event stream itself, which P1 clears
#      separately as data, not vocabulary residue.
#
# The live vocabulary (14 FQNs, `assembly_lexicon()` + `evaluation_lexicon()`
# in crates/ob-poc-kyc-substrate/src/lexicon.rs) is intentionally NOT matched.
#
# Usage: run from the rust/ workspace directory.
#   DATABASE_URL=postgresql:///data_designer ./scripts/check_no_retired_kyc_fqn.sh
#
# Exit 0 = clean. Exit 1 = retired FQN residue found (CI fails).
set -euo pipefail

# Retired-FQN patterns (pre-four-segment names, retired four-segment names,
# and every generation superseded on the way to the current 14).
PATTERN='ubo\.(edge|determination)\.[a-z-]+'
PATTERN+='|kyc\.subject\.(register|classify-structure|assert-type|correct-type)'
PATTERN+='|kyc\.obligation\.(create|satisfy)'
PATTERN+='|kyc\.person\.(approve|reject)'
PATTERN+='|kyc\.role\.(assign|withdraw)'
PATTERN+='|kyc_ubo\.assert\.subject\.(register|structure-class|type|member-withdrawal)'
PATTERN+='|kyc_ubo\.assert\.edge\.(control|reconciliation|supersession|verification)'
PATTERN+='|kyc_ubo\.assert\.obligation\.(creation|satisfaction|waiver)'

FAIL=0

echo "== 1. source (live, non-comment lines) =="
HITS=$(grep -rnE "${PATTERN}" --include="*.rs" src crates tests xtask/src 2>/dev/null \
  | grep -v '/target/' \
  | grep -vE ':\s*(//|///|//!|\*)' || true)
if [ -n "${HITS}" ]; then
    echo "FAIL: retired FQN(s) in live source:"
    echo "${HITS}"
    FAIL=1
else
    echo "PASS: no retired FQN in live source"
fi

echo
echo "== 2. config (yaml, live lines only — # comments exempt, same as source) =="
HITS=$(grep -rnE "${PATTERN}" --include="*.yaml" config 2>/dev/null \
  | grep -vE ':\s*#' || true)
if [ -n "${HITS}" ]; then
    echo "FAIL: retired FQN(s) claimed by config:"
    echo "${HITS}"
    FAIL=1
else
    echo "PASS: no retired FQN in config"
fi

echo
echo "== 3. search-index DB tables (dsl_verbs, verb_pattern_embeddings, verb_centroids) =="
if [ -n "${DATABASE_URL:-}" ]; then
    DB_HITS=$(psql "${DATABASE_URL}" -t -c "
        select 'dsl_verbs:'||full_name from \"ob-poc\".dsl_verbs
          where full_name ~ '${PATTERN}'
        union all
        select 'verb_pattern_embeddings:'||verb_name from \"ob-poc\".verb_pattern_embeddings
          where verb_name ~ '${PATTERN}'
        union all
        select 'verb_centroids:'||verb_name from \"ob-poc\".verb_centroids
          where verb_name ~ '${PATTERN}'
    " 2>&1)
    DB_HITS_TRIMMED=$(echo "${DB_HITS}" | tr -d '[:space:]')
    if [ -n "${DB_HITS_TRIMMED}" ]; then
        echo "FAIL: retired FQN(s) in search-index tables:"
        echo "${DB_HITS}"
        FAIL=1
    else
        echo "PASS: no retired FQN in search-index tables"
    fi
else
    echo "SKIP: DATABASE_URL not set — DB check not run"
fi

echo
if [ "${FAIL}" -eq 0 ]; then
    echo "PASS: no_retired_fqn_anywhere"
    exit 0
else
    exit 1
fi
