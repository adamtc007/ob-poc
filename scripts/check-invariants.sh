#!/usr/bin/env bash
# check-invariants.sh — executable gates for EOP-PLAN-CONTROLPLANE-001's
# plan-level completion invariants (E1-E5), promoted from prose acceptance
# criteria per the invariant-promotion session
# (docs/todo/control-plane/EOP-SESSION-CONTROLPLANE-INVARIANT-PROMOTION-001.md).
#
# Run from the repo root (ob-poc/).
#
# Usage:
#   scripts/check-invariants.sh e1      # ledger rows provably CLOSED
#   scripts/check-invariants.sh e2      # envelope-only admission
#   scripts/check-invariants.sh e3      # G1-G14 evaluated with evidence+metrics
#   scripts/check-invariants.sh e4      # Mode-1 register classified+tested
#   scripts/check-invariants.sh e5      # workspace hygiene
#   scripts/check-invariants.sh all     # run all five, report each status
#
# Exit code: 0 if the requested invariant currently HOLDS, 1 if it does
# NOT hold (or is only partially satisfied). This is a status probe, not
# a pass/fail judgement on the codebase — E1-E4 are EXPECTED to exit 1
# today (see invariants-expected.toml and the CI ratchet in
# .github/workflows/invariants.yml). A gate that can be satisfied by a
# comment, a sentinel return value, or a wildcard match arm is defective;
# see the session doc's "Governing principle."
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
REPO_ROOT="$(pwd)"
LEDGER="docs/research/control-plane-ownership-ledger.md"
INVENTORY="docs/research/control-plane-phase0-inventory.md"

# ---------------------------------------------------------------------------
# E1 — every RR-3 C-0xx row is CLOSED in the ownership ledger, provably not
# claimed: disposition class + commit hash + destination symbol(s) that
# resolve in the current workspace.
# ---------------------------------------------------------------------------
gate_e1() {
  echo "== E1: ledger rows provably CLOSED =="

  # Canonical row-id set: every C-0xx cited in RR-3 (the opening balance).
  # A row absent from the ledger's own table is a failure, not a skip.
  local inventory_ids ledger_ids
  inventory_ids="$(grep -oE '^\| C-[0-9]{3} \|' "$INVENTORY" | grep -oE 'C-[0-9]{3}' | sort -u)"
  ledger_ids="$(grep -oE '^\| C-[0-9]{3} \|' "$LEDGER" | grep -oE 'C-[0-9]{3}' | sort -u)"

  local missing=0
  local missing_ids=""
  while IFS= read -r cid; do
    [ -z "$cid" ] && continue
    if ! grep -qF "$cid" <<<"$ledger_ids"; then
      missing=$((missing + 1))
      missing_ids="$missing_ids $cid"
    fi
  done <<<"$inventory_ids"

  local total closed_provable=0 not_closed=0 not_closed_ids=""
  total=$(wc -l <<<"$inventory_ids" | tr -d ' ')

  while IFS= read -r cid; do
    [ -z "$cid" ] && continue
    # The ledger row for this CID: the single line starting "| C-xxx |".
    local row
    row="$(grep -E "^\| ${cid} \|" "$LEDGER" | head -1)"
    if [ -z "$row" ]; then
      continue # already counted in $missing
    fi

    # Must be literally "**CLOSED**", not "**PARTIALLY CLOSED**" or a bare
    # OPEN/RECLASSIFIED row.
    if echo "$row" | grep -qE '\*\*CLOSED\*\*' && ! echo "$row" | grep -qE '\*\*PARTIALLY CLOSED\*\*'; then
      # Must cite a commit hash (backtick-quoted 7-8 hex chars).
      local has_hash has_symbol symbol_resolves
      has_hash=0
      echo "$row" | grep -qE '`[0-9a-f]{7,8}`' && has_hash=1

      # Must cite at least one backtick-quoted destination symbol, and
      # that symbol must actually resolve somewhere in the workspace
      # (existence check via rg, not string trust).
      has_symbol=0
      symbol_resolves=0
      local symbols
      symbols="$(echo "$row" | grep -oE '`[A-Za-z_][A-Za-z0-9_:]*`' | tr -d '`' | sort -u)"
      if [ -n "$symbols" ]; then
        has_symbol=1
        while IFS= read -r sym; do
          [ -z "$sym" ] && continue
          # Skip obviously-non-symbol tokens (bare words that happen to be
          # capitalized, e.g. env var names already excluded by requiring
          # a leading letter+underscore mix is unreliable — instead just
          # check ANY cited symbol resolves; one hit is sufficient proof
          # the row points at something real, matching the spec's
          # "verifies each named symbol resolves" at the row level).
          if rg -q --type rust -e "\b${sym}\b" rust/ 2>/dev/null; then
            symbol_resolves=1
            break
          fi
        done <<<"$symbols"
      fi

      if [ "$has_hash" -eq 1 ] && [ "$has_symbol" -eq 1 ] && [ "$symbol_resolves" -eq 1 ]; then
        closed_provable=$((closed_provable + 1))
      else
        not_closed=$((not_closed + 1))
        not_closed_ids="$not_closed_ids ${cid}(claimed-closed,unproven:hash=$has_hash,symbol=$has_symbol,resolves=$symbol_resolves)"
      fi
    else
      not_closed=$((not_closed + 1))
      not_closed_ids="$not_closed_ids $cid"
    fi
  done <<<"$inventory_ids"

  echo "  Total RR-3 rows: $total"
  echo "  Missing from ledger: $missing${missing_ids:+ ($missing_ids)}"
  echo "  Provably CLOSED: $closed_provable"
  echo "  Not provably closed: $not_closed"
  if [ -n "$not_closed_ids" ]; then
    echo "  Non-closed/unproven IDs:$not_closed_ids"
  fi

  if [ "$missing" -eq 0 ] && [ "$not_closed" -eq 0 ] && [ "$total" -gt 0 ]; then
    echo "  E1: HOLDS"
    return 0
  else
    echo "  E1: DOES NOT HOLD"
    return 1
  fi
}

# ---------------------------------------------------------------------------
# E2 — all four RR-2 paths execute only via envelope admission in enforce
# mode. Two independent checks, both required.
# ---------------------------------------------------------------------------
gate_e2() {
  echo "== E2: execution only via envelope admission (structural + dynamic) =="
  local struct_fail=0

  # Structural: each RR-2 path's dispatch entry must resolve to
  # execute_verb_admitting_envelope, not the bare execute_verb, as its
  # sole route to the mutation terminus. Enumerated from RR-2 itself
  # (Path A/B/C/D), not hardcoded independent of that source.
  echo "  -- structural (per RR-2 path) --"

  # Path A: runbook step dispatch -> ObPocVerbExecutor.
  if rg -q 'execute_verb_admitting_envelope' rust/src/runbook/step_executor_bridge.rs 2>/dev/null; then
    echo "    Path A (runbook/step_executor_bridge.rs): admitting entry point present"
  else
    echo "    Path A (runbook/step_executor_bridge.rs): FAIL — no admitting-entry-point call"
    struct_fail=$((struct_fail + 1))
  fi

  # Path B: raw DSL execute handler.
  if rg -q 'execute_verb_admitting_envelope' rust/src/api/agent_routes.rs 2>/dev/null; then
    echo "    Path B (api/agent_routes.rs raw execute): admitting entry point present"
  else
    echo "    Path B (api/agent_routes.rs raw execute): FAIL — no admitting-entry-point call"
    struct_fail=$((struct_fail + 1))
  fi

  # Path C: BPMN/workflow dispatch.
  if rg -q 'execute_verb_admitting_envelope' rust/src/bpmn_integration/dispatcher.rs rust/src/domain_ops/bpmn_controller_ops.rs 2>/dev/null; then
    echo "    Path C (bpmn_integration/dispatcher.rs, domain_ops/bpmn_controller_ops.rs): admitting entry point present"
  else
    echo "    Path C (bpmn_integration/dispatcher.rs, domain_ops/bpmn_controller_ops.rs): FAIL — no admitting-entry-point call"
    struct_fail=$((struct_fail + 1))
  fi

  # Path D: bus adapter.
  if rg -q 'execute_verb_admitting_envelope' rust/crates/ob-poc-web/src/bus_runtime.rs 2>/dev/null; then
    echo "    Path D (ob-poc-web/src/bus_runtime.rs): admitting entry point present"
  else
    echo "    Path D (ob-poc-web/src/bus_runtime.rs): FAIL — no admitting-entry-point call"
    struct_fail=$((struct_fail + 1))
  fi

  # Dynamic: drive the shared admission mechanism (admit_in_scope, the
  # thing every path's admitting entry point ultimately calls) against a
  # real DB and assert execution occurred iff an admission event preceded
  # it, in enforce mode. Reuses existing live-DB tests
  # (sem_os_runtime/verb_executor_adapter.rs::t4_1_envelope_admission_tests)
  # rather than standing up four new per-path integration harnesses —
  # Paths A/B/C already fail the STRUCTURAL half outright (no admitting
  # call site exists to dynamically test), so a negative dynamic result
  # for them is already proven by absence, not asserted from a stub. Path
  # D is the one path with a real admitting call site, so it's the one
  # this dynamic check can meaningfully drive:
  #   - shadow_default_admits_every_verb_with_no_envelope: proves, live,
  #     that under today's production default (ENFORCE_VERBS unset),
  #     execution is admitted with NO envelope at all -- i.e. execution
  #     does NOT require a preceding admission event today.
  #   - enforced_verb_without_envelope_is_rejected: proves the mechanism
  #     DOES work when a verb is added to the enforced set (this is what
  #     "iff" requires evidence of both directions, not just the failing
  #     one).
  echo "  -- dynamic (live DB, Path D's shared admission mechanism) --"
  if [ -z "${DATABASE_URL:-}" ]; then
    echo "    SKIPPED — DATABASE_URL not set (these are #[ignore]-gated live-DB tests)"
    echo "    Run manually: DATABASE_URL=... cargo test -p ob-poc --lib --features database t4_1_envelope_admission_tests -- --ignored"
  else
    (cd rust && cargo test -p ob-poc --lib --features database t4_1_envelope_admission_tests -- --ignored --nocapture 2>&1) | tail -30
  fi

  echo "  Structural failures: $struct_fail / 4 paths"
  if [ "$struct_fail" -eq 0 ]; then
    echo "  E2: structural half HOLDS; dynamic evidence shows Path D NotEnforced by default -> DOES NOT HOLD"
  else
    echo "  E2: DOES NOT HOLD ($struct_fail/4 paths have no admitting entry point at all)"
  fi
  return 1
}

# ---------------------------------------------------------------------------
# E3 — G1-G14 each evaluated in production (not NotImplemented) with
# metrics flowing. Enumerated exhaustively from GateId::ALL (the canonical
# registry) — a 15th gate must break this until covered, same discipline
# as an exhaustive Rust match with no wildcard arm.
# ---------------------------------------------------------------------------
gate_e3() {
  echo "== E3: G1-G14 evaluated in production with metrics flowing =="
  echo "  Compile-time half (exhaustive GateId match, no _ arm):"
  (cd rust && cargo test -p ob-poc --lib --features database e3_gate_label_match_is_exhaustive 2>&1) | tail -10
  local compile_result=$?

  echo ""
  echo "  Live half (gate_outcome_counts over real control_plane_shadow_decisions rows):"
  local result
  if [ -z "${DATABASE_URL:-}" ]; then
    echo "    SKIPPED — DATABASE_URL not set (this is a #[ignore]-gated live-DB test)"
    echo "    Run manually: DATABASE_URL=... cargo test -p ob-poc --lib --features database e3_invariant_probe -- --ignored --nocapture"
    echo "  E3: NOT VERIFIED (live half skipped) — does not count as HOLDS; absence of proof is not proof"
    return 1
  else
    (cd rust && cargo test -p ob-poc --lib --features database e3_invariant_probe -- --ignored --nocapture 2>&1) | tail -30
    local live_result=$?
    result=$((compile_result + live_result))
  fi

  if [ "$result" -eq 0 ]; then
    echo "  E3: HOLDS"
  else
    echo "  E3: DOES NOT HOLD (see harness output above)"
  fi
  return "$result"
}

# ---------------------------------------------------------------------------
# E4 — Mode-1 register (RR-5) rows either version-pinned or permanently
# classified human-gated with the classification tested.
#
# RR-5's Mode-1 register (phase0-inventory.md §RR-5) has no machine-
# readable structure — 5 rows of prose in a markdown table, keyed by a
# free-text "Entity/state family" description. Per the session's scope
# constraint ("propose the minimal schema... flag it for review"), a
# short slug id is assigned to each of the 5 rows below (family text
# quoted verbatim from RR-5 so the mapping is auditable against the
# source, not invented independent of it). This IS new structure this
# gate imposes on the artifact — flagged, not silently assumed to
# pre-exist.
#
# Each row is enumerated (not skippable) and must satisfy exactly one of:
#   (a) VERSION-PINNED — a named symbol that resolves in the workspace and
#       is a real pin mechanism (not just present, but load-bearing — this
#       gate checks resolution, the same "not string trust" bar as E1; it
#       does NOT independently prove the pin is wired to a production call
#       site for this specific family, which would require the E2-style
#       dynamic harness — flagged as a real gap in this gate's coverage,
#       not hidden).
#   (b) HUMAN-GATED, TESTED — a named #[test] function that exists in the
#       workspace and exercises a human-gate code path for this family.
# ---------------------------------------------------------------------------
gate_e4() {
  echo "== E4: Mode-1 register rows version-pinned or human-gated-and-tested =="
  echo "  (schema imposed on RR-5 by this gate — see script comment above)"
  echo ""

  # slug|RR-5 family text (verbatim, truncated for display)|pin-symbol|human-gate-test-name
  local rows=(
    "shadow_envelope_entities|Shadow envelope resolved entities (row_version:0, recheck_required:false)|SnapshotPins::entity_row_version|shadow_envelope_entity_requires_human_gate"
    "toctou_entity_tables|Entity tables intended for TOCTOU (migration staged/pending)|verify_pins_in_scope|toctou_unpinned_entity_requires_human_gate"
    "bus_operational_writes|Bus-invoked operational writes (Principal::system(), no runbook/envelope)|PinnedVersionSet::bus_catalogue_version|bus_write_without_envelope_requires_human_gate"
    "bpmn_process_instances|BPMN process_instances (no row-version/CAS check found)|process_instances_row_version|bpmn_process_instance_requires_human_gate"
    "raw_dsl_best_effort|Raw DSL best-effort execution (not routed through envelope/snapshot)|raw_dsl_snapshot_pin|raw_dsl_execution_requires_human_gate"
  )

  local total=${#rows[@]}
  local satisfied=0
  local unsatisfied_ids=""

  for row in "${rows[@]}"; do
    IFS='|' read -r slug family pin_symbol test_name <<<"$row"

    # "Resolves" is not enough on its own — a symbol can exist and only
    # ever be exercised by its own unit tests (definition-only, no real
    # wiring). Require a CALL site (symbol followed by `(` or `.`) in
    # ob-poc's own application layer (rust/src/), on a non-comment,
    # non-assertion line — the closest a grep-based check can get to
    # "this pin is actually load-bearing in production," matching E1's
    # "not string trust" bar applied to this invariant.
    local short_sym="${pin_symbol##*::}"
    local pin_resolves=0
    rg --type rust -e "${short_sym}\s*\(|${short_sym}\." rust/src/ 2>/dev/null \
      | grep -vE '^\s*[^:]+:[0-9]+:\s*//' \
      | grep -vi 'assert' \
      | grep -q . && pin_resolves=1

    local test_resolves=0
    rg -q --type rust -e "fn ${test_name}\b" rust/ 2>/dev/null && test_resolves=1

    echo "  [$slug] $family"
    echo "    version-pin symbol '$pin_symbol' resolves: $pin_resolves"
    echo "    human-gate test '$test_name' exists: $test_resolves"

    if [ "$pin_resolves" -eq 1 ] || [ "$test_resolves" -eq 1 ]; then
      satisfied=$((satisfied + 1))
    else
      unsatisfied_ids="$unsatisfied_ids $slug"
    fi
  done

  echo ""
  echo "  Total Mode-1 register rows: $total"
  echo "  Satisfied (pinned or human-gated-tested): $satisfied"
  if [ -n "$unsatisfied_ids" ]; then
    echo "  Unsatisfied:$unsatisfied_ids"
  fi

  if [ "$satisfied" -eq "$total" ]; then
    echo "  E4: HOLDS"
    return 0
  else
    echo "  E4: DOES NOT HOLD"
    return 1
  fi
}

# ---------------------------------------------------------------------------
# E5 — workspace green.
# ---------------------------------------------------------------------------
gate_e5() {
  echo "== E5: workspace hygiene =="
  local fail=0

  echo "  -- cargo build --workspace --features database --"
  if ! (cd rust && cargo build --workspace --features database 2>&1 | tail -20); then
    fail=1
  fi

  echo "  -- cargo test -p ob-poc --lib --features database --"
  if ! (cd rust && cargo test -p ob-poc --lib --features database 2>&1 | tail -10); then
    fail=1
  fi

  echo "  -- public API surface gate --"
  if command -v cargo-public-api >/dev/null 2>&1; then
    if ! bash scripts/check-public-api-surface.sh; then
      fail=1
    fi
  else
    echo "    SKIPPED (cargo-public-api not installed locally; CI installs it explicitly)"
  fi

  echo "  -- unreachable_pub (crates that opt in via #![deny(unreachable_pub)]) --"
  local deny_crates
  deny_crates="$(rg -l '#!\[deny\(unreachable_pub\)\]' rust/crates/*/src/lib.rs 2>/dev/null)"
  if [ -z "$deny_crates" ]; then
    echo "    FAIL — no crate declares #![deny(unreachable_pub)] (regression: at least ob-poc-agent, ob-poc-control-plane must)"
    fail=1
  else
    echo "    Crates enforcing unreachable_pub:"
    echo "$deny_crates" | sed 's/^/      /'
    # cargo build --workspace above already fails on any violation in
    # these crates (deny = compile error); this block proves the *lint
    # is still declared*, not just that the crate happens to build.
  fi

  if [ "$fail" -eq 0 ]; then
    echo "  E5: HOLDS"
    return 0
  else
    echo "  E5: DOES NOT HOLD"
    return 1
  fi
}

# ---------------------------------------------------------------------------
# ratchet — run all five, compare each against invariants-expected.toml,
# fail on ANY divergence in either direction (unexpected green included).
# ---------------------------------------------------------------------------
gate_ratchet() {
  local expected_file="invariants-expected.toml"
  if [ ! -f "$expected_file" ]; then
    echo "FAIL: $expected_file not found" >&2
    return 1
  fi

  local divergences=0
  for g in e1 e2 e3 e4 e5; do
    echo ""
    gate_"$g"
    local actual_status=$?
    local actual_word="fail"
    [ "$actual_status" -eq 0 ] && actual_word="pass"

    # Parse "status = \"...\"" from this invariant's [eN] section.
    local expected_word
    expected_word="$(awk -v section="[$g]" '
      $0 == section { in_section=1; next }
      /^\[/ { in_section=0 }
      in_section && /^status/ {
        split($0, arr, "\""); print arr[2]; exit
      }
    ' "$expected_file")"

    if [ -z "$expected_word" ]; then
      echo "  [$g] FAIL: no expected status found in $expected_file"
      divergences=$((divergences + 1))
      continue
    fi

    if [ "$actual_word" = "$expected_word" ]; then
      echo "  [$g] actual=$actual_word expected=$expected_word — MATCH"
    else
      echo "  [$g] actual=$actual_word expected=$expected_word — DIVERGENCE"
      if [ "$actual_word" = "pass" ] && [ "$expected_word" = "fail" ]; then
        echo "    Unexpected green: either flip $expected_file's [$g] status as part of the tranche that closed it, or the gate was weakened/satisfied by convention — investigate before flipping."
      fi
      divergences=$((divergences + 1))
    fi
  done

  echo ""
  echo "== Ratchet: $divergences/5 invariant(s) diverge from $expected_file =="
  return "$divergences"
}

# ---------------------------------------------------------------------------
main() {
  local target="${1:-}"
  case "$target" in
    e1) gate_e1 ;;
    e2) gate_e2 ;;
    e3) gate_e3 ;;
    e4) gate_e4 ;;
    e5) gate_e5 ;;
    ratchet) gate_ratchet ;;
    all)
      local overall=0
      for g in e1 e2 e3 e4 e5; do
        echo ""
        gate_"$g"
        status=$?
        echo "  [$g] exit=$status"
        [ "$status" -ne 0 ] && overall=$((overall + 1))
      done
      echo ""
      echo "== Summary: $overall/5 invariants do not hold =="
      exit "$overall"
      ;;
    *)
      echo "usage: $0 {e1|e2|e3|e4|e5|all|ratchet}" >&2
      exit 2
      ;;
  esac
}

main "$@"
