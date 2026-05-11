#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3002}"
FIXTURES="${FIXTURES:-baseline-fixtures-v1.jsonl}"
OUT_DIR="${OUT_DIR:-baseline-runs/current-sage-$(date -u +%Y%m%dT%H%M%SZ)}"

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
    row = {
      fixture_id: fixture.fetch("id"),
      runner: "current-sage",
      session_id: ARGV[1],
      utterance: fixture.fetch("utterance"),
      expected_pack: fixture.fetch("expected_pack"),
      expected_macro_or_template: fixture.fetch("expected_macro_or_template"),
      expected_verb: fixture.fetch("expected_verb"),
      expected_outcome: fixture.fetch("expected_outcome"),
      route_or_fallback_chosen: "POST /api/session/:id/input",
      wall_clock_ms_total: ARGV[3].to_i - ARGV[2].to_i,
      raw_response_path: "raw/#{fixture.fetch("id")}-response.json",
      scoring_status: "unscored"
    }
    puts JSON.generate(row)
  ' "$fixture_json" "$session_id" "$session_start_ms" "$end_ms" >> "$summary"

  printf 'captured %s -> %s\n' "$fixture_id" "$session_id" >&2
done < "$FIXTURES"

printf 'wrote %s\n' "$summary" >&2
