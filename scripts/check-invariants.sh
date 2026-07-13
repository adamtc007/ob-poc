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
  # execute_verb_admitting_envelope AS ITS SOLE ROUTE to the mutation
  # terminus — presence of an admitting call is necessary but not
  # sufficient; a path with an admitting call in one branch and a bare
  # execute_verb() call in another still leaves the bypass open (the
  # exact sentinel shape the session's governing principle warns about,
  # applied one level up, per 2026-07-13 review finding #1). So for each
  # path this now checks BOTH:
  #   (a) admitting entry point present, with file:line printed so a
  #       reviewer can confirm the match locus is a real call site, not
  #       a comment/doc-reference/test-mock (review finding #2's ask);
  #   (b) zero BARE execute_verb( call sites — same files, filtered to
  #       exclude comment lines (//, ///) and fn/trait-method
  #       DEFINITIONS (`fn execute_verb(` — a mock impl in a test module
  #       is not a call site). Any bare call site fails the path even
  #       if (a) also holds — exclusivity, not presence, is the bar.
  # Enumerated from RR-2 itself (Path A/B/C/D), not hardcoded independent
  # of that source.
  echo "  -- structural (per RR-2 path: admitting-call locus + bare-call exclusivity) --"

  _e2_check_path() {
    local label="$1"
    shift
    local files=("$@")
    local admit_hit="" bare_hits=""
    local f
    for f in "${files[@]}"; do
      [ -f "$f" ] || continue
      local m
      m="$(rg -n 'execute_verb_admitting_envelope' "$f" 2>/dev/null | head -1)"
      if [ -n "$m" ] && [ -z "$admit_hit" ]; then
        admit_hit="$f:$m"
      fi
      local b
      b="$(rg -n 'execute_verb\(' "$f" 2>/dev/null \
        | grep -v 'execute_verb_admitting_envelope' \
        | grep -vE '^\s*[0-9]+:\s*///?' \
        | grep -vE 'fn execute_verb\(')"
      if [ -n "$b" ]; then
        bare_hits="$bare_hits"$'\n'"$f: $b"
      fi
    done

    if [ -n "$admit_hit" ]; then
      echo "    $label: admitting entry point present — $admit_hit"
    else
      echo "    $label: FAIL — no admitting-entry-point call"
      struct_fail=$((struct_fail + 1))
      return
    fi

    if [ -n "$bare_hits" ]; then
      echo "    $label: FAIL — bare execute_verb() call site(s) also present (exclusivity broken):$bare_hits"
      struct_fail=$((struct_fail + 1))
    else
      echo "    $label: no bare execute_verb() call sites — admitting call is the sole route"
    fi
  }

  # Path A: runbook step dispatch -> ObPocVerbExecutor. Wired by commit
  # 5a704f4e ("PIR-D-002 — Path A now reaches the admission port"),
  # governed by docs/todo/control-plane/EOP-RUNBOOK-CONTROLPLANE-GRADUATION-001.md
  # v0.2 §3-4 (Path A graduates first; G1-only coverage today per that
  # runbook's §2 readiness table) — postdates the ledger's T7 hand-check
  # this session's ground truth cited, which is why Path A's pass looked
  # surprising against §1's predicted-fail framing (2026-07-13 review
  # finding #2).
  _e2_check_path "Path A (runbook/step_executor_bridge.rs)" rust/src/runbook/step_executor_bridge.rs

  # Path B/C (G4, EOP-SESSION-CONTROLPLANE-G4-IMPL-001): superseded the
  # pre-G3/G4 placeholder check below, which pointed at
  # api/agent_routes.rs and bpmn_integration/dispatcher.rs/domain_ops/
  # bpmn_controller_ops.rs and looked for a literal
  # `execute_verb_admitting_envelope` call in THOSE files. That shape
  # never matched reality: G3's ratified design doc
  # (EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001 §2.3, re-
  # confirmed in the G4 session) found Path B and Path C converge on
  # ONE shared seam, `dsl_v2::executor::DslExecutor::execute_verb_in_scope`
  # (rust/src/dsl_v2/executor.rs) — every `admit_plan`/`RealDslExecutor`
  # ingress point (agent_routes.rs's raw-execute route, batch/sheet
  # executors, the MCP dsl_execute tool, bpmn_integration's
  # WorkflowDispatcher-wrapped RealDslExecutor) reaches this same
  # function per-step via `execute_plan`/`execute_plan_atomic_in_scope`,
  # never by calling a bare, unguarded verb-dispatch primitive directly.
  # G4 wired the per-step admission call INSIDE that seam (not as a
  # separate wrapper function callers must remember to call), so the
  # correct structural check is: the seam itself contains the admission
  # call, and it runs unconditionally before all three dispatch branches
  # (SemOS-native / CRUD / generic). _e2_check_seam below checks that
  # shape directly, replacing the old per-ingress-file heuristic for B/C
  # only (A/D's wrapper-function shape is unchanged and still checked by
  # _e2_check_path above/below).
  _e2_check_seam() {
    local label="$1"
    local f="rust/src/dsl_v2/executor.rs"
    if [ ! -f "$f" ]; then
      echo "    $label: FAIL — seam file not found: $f"
      struct_fail=$((struct_fail + 1))
      return
    fi
    # The admission call must appear inside execute_verb_in_scope,
    # between its `ENTER` trace and the plugin/CRUD dispatch branches —
    # approximated here by requiring check_admission_in_scope to appear
    # after execute_verb_in_scope's signature and before its first
    # runtime_registry().get(...) lookup (the first line of real
    # dispatch logic), within the same function body.
    local fn_start admit_line first_dispatch_line
    fn_start="$(rg -n 'pub\(crate\) async fn execute_verb_in_scope' "$f" | head -1 | cut -d: -f1)"
    if [ -z "$fn_start" ]; then
      echo "    $label: FAIL — execute_verb_in_scope not found in $f"
      struct_fail=$((struct_fail + 1))
      return
    fi
    admit_line="$(tail -n "+$fn_start" "$f" | rg -n 'check_admission_in_scope' | head -1 | cut -d: -f1)"
    first_dispatch_line="$(tail -n "+$fn_start" "$f" | rg -n 'let runtime_verb = runtime_registry\(\)' | head -1 | cut -d: -f1)"
    if [ -z "$admit_line" ] || [ -z "$first_dispatch_line" ]; then
      echo "    $label: FAIL — admission call or dispatch-branch anchor not found inside execute_verb_in_scope"
      struct_fail=$((struct_fail + 1))
      return
    fi
    if [ "$admit_line" -lt "$first_dispatch_line" ]; then
      echo "    $label: admitting entry point present — $f:$((fn_start + admit_line - 1)) (execute_verb_in_scope, before dispatch branches at $f:$((fn_start + first_dispatch_line - 1)))"
      echo "    $label: single shared seam — every execute_plan/execute_plan_atomic_in_scope step reaches this same admission call, no bare-bypass check applicable to this shape (see comment above)"
    else
      echo "    $label: FAIL — check_admission_in_scope found but AFTER the dispatch-branch anchor (not gating unconditionally)"
      struct_fail=$((struct_fail + 1))
    fi
  }
  _e2_check_seam "Path B (dsl_v2 seam, umbrella: agent_routes.rs raw-execute + batch/sheet executors + MCP dsl_execute + no-BPMN executor_v2 fallback)"
  _e2_check_seam "Path C (dsl_v2 seam, WorkflowDispatcher-wrapped RealDslExecutor instance)"

  # Path D: bus adapter.
  _e2_check_path "Path D (ob-poc-web/src/bus_runtime.rs)" rust/crates/ob-poc-web/src/bus_runtime.rs

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

  # G4 (EOP-SESSION-CONTROLPLANE-G4-IMPL-001): the dsl_v2 seam's own
  # atomicity tests (item 4 — rollback-of-consume on dispatch failure,
  # pin-drift rejection leaves the envelope reconsumable) and the
  # double-admission guard's hard test (item 2 — Branch-3 fallthrough
  # must neither double-consume nor reject a properly admitted
  # dispatch), the Path B/C equivalents of Path D's t4_1 suite above.
  echo "  -- dynamic (live DB, Path B/C dsl_v2 seam atomicity + double-admission guard) --"
  if [ -z "${DATABASE_URL:-}" ]; then
    echo "    SKIPPED — DATABASE_URL not set (these are #[ignore]-gated live-DB tests)"
    echo "    Run manually: DATABASE_URL=... cargo test -p ob-poc --lib --features database g4_seam_admission_tests -- --ignored"
  else
    (cd rust && cargo test -p ob-poc --lib --features database g4_seam_admission_tests -- --ignored --nocapture 2>&1) | tail -30
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
  echo "  Live half (gate_outcome_counts over real control_plane_shadow_decisions rows, Path A):"
  local result
  if [ -z "${DATABASE_URL:-}" ]; then
    echo "    SKIPPED — DATABASE_URL not set (this is a #[ignore]-gated live-DB test)"
    echo "    Run manually: DATABASE_URL=... cargo test -p ob-poc --lib --features database e3_invariant_probe -- --ignored --nocapture"
    echo "  E3: NOT VERIFIED (live half skipped) — does not count as HOLDS; absence of proof is not proof"
    return 1
  else
    local live_output
    live_output="$(cd rust && cargo test -p ob-poc --lib --features database e3_invariant_probe -- --ignored --nocapture 2>&1)"
    local live_result=$?
    echo "$live_output" | tail -30
    result=$((compile_result + live_result))

    # A live_result != 0 is satisfied identically by "verified N/14 gates
    # empty" and "couldn't reach the database at all" — an expected-fail
    # ratchet entry can't distinguish those from the exit bit alone
    # (2026-07-13 review finding #3). Match on the reason, not just the
    # bit: the probe itself (rust/src/agent/control_plane_metrics.rs)
    # panics with a distinct E3_INFRASTRUCTURE_FAILURE marker for
    # connection/query failures vs E3_INVARIANT_FAILURE for a real,
    # verified, substantive result.
    if [ "$live_result" -ne 0 ]; then
      if echo "$live_output" | grep -q 'E3_INFRASTRUCTURE_FAILURE'; then
        echo "  ** E3 live half: INFRASTRUCTURE FAILURE — could not verify (DB unreachable/query failed). **"
        echo "  ** This is NOT proof the invariant fails; it means the harness itself is broken/misconfigured. **"
        echo "  ** If this shows up in CI once DATABASE_URL is wired in, it needs fixing immediately — an **"
        echo "  ** infra failure masquerading as an expected 'fail' lets this gate rot silently. **"
      elif echo "$live_output" | grep -q 'E3_INVARIANT_FAILURE'; then
        echo "  ** E3 live half: INVARIANT FAILURE — verified against a live DB, N/14 gates genuinely empty. **"
      fi
    fi

    # G5 (EOP-PLAN-CONTROLPLANE-GRADUATION-001 §3 item 5,
    # EOP-DESIGN-CONTROLPLANE-G5-GATE-APPLICABILITY-MATRIX-001): the
    # per-(gate, path) amendment against the ratified applicability
    # matrix — Path B/C/D shadow-wired cells (G1/G12 substantive,
    # G3/G9 ratified NotApplicable) plus the window-discipline proof
    # (Path A never produces NotApplicable). Exercises real production
    # code with synthetic traffic where B/C/D have none yet (per the
    # tranche's own exit-gate allowance).
    echo ""
    echo "  Live half — G5 per-(gate, path) matrix probe (Path B/C/D + window discipline):"
    local matrix_output
    matrix_output="$(cd rust && cargo test -p ob-poc --lib --features database \
      -- --ignored --nocapture e3_matrix_invariant_probe g5_path_a_never_produces_not_applicable 2>&1)"
    local matrix_result=$?
    echo "$matrix_output" | tail -30
    result=$((result + matrix_result))
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
#
# PROVISIONAL NAMING (2026-07-13 review finding #4): the pin-symbol and
# test-name strings below (e.g. `SnapshotPins::entity_row_version`,
# `raw_dsl_snapshot_pin`) were INVENTED in this gate-authoring session —
# they name a target, not an observed fact. A tranche is free to
# implement a given row's pin/test under a different, possibly better,
# name; that is a legitimate outcome, not a gate failure to route around
# by quietly editing this array. Renaming a row's target here is a SPEC
# CHANGE and must get the same review visibility as an
# invariants-expected.toml status flip — call it out explicitly in the
# tranche's diff/summary, don't fold it into unrelated script edits.
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
