#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3002}"
FIXTURES="${FIXTURES:-slice-2-fixtures-v1.jsonl}"
OUT_DIR="${OUT_DIR:-baseline-runs/slice2-runtime-$(date -u +%Y%m%dT%H%M%SZ)}"
STRICT="${STRICT:-1}"

mkdir -p "$OUT_DIR/raw"

summary="$OUT_DIR/run-summary.jsonl"
: > "$summary"

while IFS= read -r fixture_json; do
  [ -n "$fixture_json" ] || continue

  fixture_id="$(ruby -rjson -e 'puts JSON.parse(ARGV[0]).fetch("id")' "$fixture_json")"
  utterance="$(ruby -rjson -e 'puts JSON.parse(ARGV[0]).fetch("utterance")' "$fixture_json")"

  session_start_ms="$(ruby -e 'puts (Time.now.to_f * 1000).to_i')"
  session_json="$(curl -sS -X POST "$BASE_URL/api/session" \
    -H 'Content-Type: application/json' \
    -d '{}')"
  session_id="$(printf '%s' "$session_json" | ruby -rjson -e 'puts JSON.parse(STDIN.read).fetch("session_id")')"

  payload="$(ruby -rjson -e 'puts JSON.generate({kind: "utterance", message: ARGV[0]})' "$utterance")"
  response_json="$(curl -sS -X POST "$BASE_URL/api/session/$session_id/input" \
    -H 'Content-Type: application/json' \
    -d "$payload")"
  end_ms="$(ruby -e 'puts (Time.now.to_f * 1000).to_i')"

  printf '%s\n' "$session_json" > "$OUT_DIR/raw/${fixture_id}-session.json"
  printf '%s\n' "$response_json" > "$OUT_DIR/raw/${fixture_id}-response.json"

  ruby -rjson -e '
    fixture = JSON.parse(ARGV[0])
    response = JSON.parse(ARGV[1])
    session_id = ARGV[2]
    start_ms = ARGV[3].to_i
    end_ms = ARGV[4].to_i

    chat = response["response"] || {}
    trace = chat["acp_trace"] || {}
    raw = JSON.generate(response)
    expected_pack = fixture.fetch("expected_pack")
    ghost = fixture.fetch("group") == "S2-GHOST"
    required_runtime_trace = [
      "runtime_schema_version",
      "runtime_pack_id",
      "runtime_snapshot_id",
      "runtime_hash",
      "runtime_redaction_policy",
      "runtime_freshness_policy",
      "runtime_static_envelope_hash",
      "runtime_projection_hash",
      "runtime_verified",
      "runtime_redacted_count"
    ]

    runtime_values = required_runtime_trace.to_h { |field| [field, trace[field]] }
    runtime_present = runtime_values.values.any? { |value| !value.nil? }
    runtime_trace_fields_present = required_runtime_trace.all? { |field| !trace[field].nil? }
    runtime_pack_hit = expected_pack == "none" ? trace["runtime_pack_id"].nil? : trace["runtime_pack_id"] == expected_pack
    runtime_verified = trace["runtime_verified"] == true
    ghost_runtime_null = !ghost || !runtime_present
    ghost_no_dsl = !ghost || chat["dsl"].nil?
    pack_hit = expected_pack == "none" ? trace["pack_id"].nil? : trace["pack_id"] == expected_pack
    forbidden_field_names_absent = fixture.fetch("forbidden_runtime_fields").all? do |field|
      !raw.include?("\"#{field}\"") && !raw.include?("forbidden-value-for-#{field}")
    end

    runtime_trace_ok = if ghost
      ghost_runtime_null
    elsif expected_pack == "none"
      !runtime_present
    else
      runtime_trace_fields_present && runtime_pack_hit && runtime_verified
    end

    passed = pack_hit && runtime_trace_ok && ghost_no_dsl && forbidden_field_names_absent
    row = {
      fixture_id: fixture.fetch("id"),
      runner: "slice2-runtime-http",
      session_id: session_id,
      group: fixture.fetch("group"),
      utterance: fixture.fetch("utterance"),
      expected_pack: expected_pack,
      actual_pack: trace["pack_id"],
      expected_outcome: fixture.fetch("expected_outcome"),
      actual_status: trace["status"],
      runtime_trace_expected: !ghost && expected_pack != "none",
      runtime_trace_present: runtime_present,
      runtime_trace_fields_present: runtime_trace_fields_present,
      runtime_pack_id: trace["runtime_pack_id"],
      runtime_hash_present: !trace["runtime_hash"].nil?,
      runtime_projection_hash_present: !trace["runtime_projection_hash"].nil?,
      runtime_verified: trace["runtime_verified"],
      runtime_redaction_policy: trace["runtime_redaction_policy"],
      runtime_freshness_policy: trace["runtime_freshness_policy"],
      runtime_redacted_count: trace["runtime_redacted_count"],
      ghost_runtime_null: ghost_runtime_null,
      ghost_no_dsl: ghost_no_dsl,
      forbidden_field_names_absent: forbidden_field_names_absent,
      pack_hit: pack_hit,
      runtime_trace_ok: runtime_trace_ok,
      route_or_fallback_chosen: "POST /api/session/:id/input",
      wall_clock_ms_total: end_ms - start_ms,
      raw_response_path: "raw/#{fixture.fetch("id")}-response.json",
      scoring_status: passed ? "passed" : "failed"
    }
    puts JSON.generate(row)
  ' "$fixture_json" "$response_json" "$session_id" "$session_start_ms" "$end_ms" >> "$summary"

  printf 'captured %s -> %s\n' "$fixture_id" "$session_id" >&2
done < "$FIXTURES"

ruby -rjson -e '
  rows = File.readlines(ARGV[0], chomp: true).map { |line| JSON.parse(line) }
  failed = rows.select { |row| row.fetch("scoring_status") != "passed" }
  groups = Hash.new { |hash, key| hash[key] = { "total" => 0, "passed" => 0 } }
  rows.each do |row|
    groups[row.fetch("group")]["total"] += 1
    groups[row.fetch("group")]["passed"] += 1 if row.fetch("scoring_status") == "passed"
  end
  report = {
    "fixture_count" => rows.length,
    "passed_count" => rows.length - failed.length,
    "failed_count" => failed.length,
    "groups" => groups,
    "failed_fixture_ids" => failed.map { |row| row.fetch("fixture_id") }
  }
  File.write(ARGV[1], JSON.pretty_generate(report) + "\n")
  warn JSON.pretty_generate(report)
  exit(failed.empty? ? 0 : 1)
' "$summary" "$OUT_DIR/score-summary.json" || {
  if [ "$STRICT" = "1" ]; then
    printf 'slice2 runtime baseline failed; see %s\n' "$OUT_DIR/score-summary.json" >&2
    exit 1
  fi
}

printf 'wrote %s\n' "$summary" >&2
