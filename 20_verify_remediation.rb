#!/usr/bin/env ruby
# 20_verify_remediation.rb
#
# RUN THIS AFTER 10_apply_remediation.rb --apply.
#
# Re-runs the same classification logic used throughout the recon against
# the current (post-remediation) state and asserts specific expectations,
# rather than just printing a count and hoping it looks right. Exits 1 on
# any FAIL so it's usable as a CI/pre-commit gate later if you want.
#
# This only checks YAML-level consistency. It does NOT replace running:
#   cargo build
#   cargo run --manifest-path xtask/Cargo.toml -- verbs lint
#   cargo test -p dsl-analysis -p dsl-resolution -p dsl-runtime
# — those catch Rust-level issues (compile errors, runtime wiring) this
# script structurally cannot see. Run them too. See the summary at the end.
#
# Run from repo root:
#   ruby 20_verify_remediation.rb

require "yaml"
require "set"
require "date"

LOAD = ->(f) { YAML.safe_load(File.read(f), permitted_classes: [Date], permitted_symbols: [], aliases: true) }
FAILURES = []
fail_check = ->(msg) { FAILURES << msg; puts "  FAIL: #{msg}" }
pass_check = ->(msg) { puts "  PASS: #{msg}" }

# Pre-existing, broken before this remediation touched anything — not
# caused by us, not fixed by us. Confirmed by Zed against the real repo
# (2026-07): both are constellation-map files with a real YAML syntax
# error (unexpected `:` at line 20 col 27), unrelated to DSL/pack work.
KNOWN_PREEXISTING_PARSE_FAILURES = [
  /struct_pe_cross_border\.yaml$/,
  /struct_hedge_cross_border\.yaml$/,
]

puts "=" * 72
puts "VERIFY REMEDIATION"
puts "=" * 72

# ---------------------------------------------------------------------------
# 0. Every touched YAML file still parses
# ---------------------------------------------------------------------------
puts "\n## 0. YAML parse check"
all_yaml = Dir["rust/config/**/*.yaml"]
bad = []
known = []
all_yaml.each do |f|
  begin
    LOAD.call(f)
  rescue => e
    if KNOWN_PREEXISTING_PARSE_FAILURES.any? { |pat| f =~ pat }
      known << [f, e.message]
    else
      bad << [f, e.message]
    end
  end
end
if bad.empty?
  pass_check.call("all #{all_yaml.size - known.size} in-scope config YAML files parse")
else
  bad.each { |f, msg| fail_check.call("#{f} does not parse: #{msg}") }
end
known.each { |f, msg| puts "  KNOWN (pre-existing, out of scope): #{f} — #{msg}" }

# ---------------------------------------------------------------------------
# 1. Reload everything (same loaders as prior scripts)
# ---------------------------------------------------------------------------
verbs = {}
Dir["rust/config/verbs/**/*.yaml"].sort.each do |f|
  next if File.basename(f) == "_meta.yaml"
  doc = LOAD.call(f) || {}
  (doc["domains"] || {}).each do |domain, dconf|
    (dconf["verbs"] || {}).each { |action, vconf| verbs["#{domain}.#{action}"] = { file: f, domain: domain, config: vconf } }
  end
end

macros = {}
Dir["rust/config/verb_schemas/macros/*.yaml"].sort.each do |f|
  (LOAD.call(f) || {}).each { |k, c| macros[k] = true if c.is_a?(Hash) && c["kind"] == "macro" }
end

journey_packs = {}
Dir["rust/config/packs/*.yaml"].sort.each do |f|
  y = LOAD.call(f) || {}
  next unless y["id"]
  journey_packs[y["id"]] = { file: f, allowed_verbs: Array(y["allowed_verbs"]).uniq }
end

domain_packs = {}
Dir["rust/config/sem_os_seeds/domain_packs/*.yaml"].sort.each do |f|
  y = LOAD.call(f) || {}
  next unless y["pack_id"]
  domain_packs[y["pack_id"]] = { file: f, owned_packs: Array(y["owned_packs"]), owned_entity_kinds: Array(y["owned_entity_kinds"]), owned_dags: Array(y["owned_dags"]) }
end

dags = {}
Dir["rust/config/sem_os_seeds/dag_taxonomies/*.yaml"].sort.each do |f|
  y = LOAD.call(f) || {}
  next unless y["dag_id"]
  slots = {}
  Array(y["slots"]).each { |s| slots[s["id"]] = s if s["id"] }
  recon = {}
  (y["dsl_verb_reconciliation"] || {}).each { |surface, list| recon[surface] = Array(list) }
  progression_verbs = []
  Array(y.dig("overall_lifecycle", "phases")).each { |phase| progression_verbs.concat(Array(phase["progression_verbs"])) }
  dags[y["dag_id"]] = { file: f, slots: slots, recon: recon, progression_verbs: progression_verbs.uniq }
end

dag_owner = Hash.new { |h, k| h[k] = [] }
domain_packs.each { |pid, dp| dp[:owned_dags].each { |d| dag_owner[d] << pid } }
entity_kind_claimants = Hash.new { |h, k| h[k] = [] }
domain_packs.each { |pid, dp| dp[:owned_entity_kinds].each { |ek| entity_kind_claimants[ek] << pid } }
jpack_owner = Hash.new { |h, k| h[k] = [] }
domain_packs.each { |pid, dp| dp[:owned_packs].each { |jp| jpack_owner[jp] << pid } }

slot_owning_dag = {}
dags.each { |dag_id, d| d[:slots].each { |slot_id, sconf| slot_owning_dag[slot_id] = dag_id if sconf["state_machine"].is_a?(Hash) || !slot_owning_dag.key?(slot_id) } }
recon_index = Hash.new { |h, k| h[k] = [] }
dags.each { |dag_id, d| d[:recon].each_value { |list| list.each { |fqn| recon_index[fqn] << dag_id } } }
progression_index = Hash.new { |h, k| h[k] = [] }
dags.each { |dag_id, d| d[:progression_verbs].each { |fqn| progression_index[fqn] << dag_id } }

def collect_transitions(sconf, dag_id, slot_id, out)
  [sconf["state_machine"], *Array(sconf["dual_lifecycle"])].each do |sm|
    next unless sm.is_a?(Hash)
    Array(sm["transitions"]).each do |t|
      via = t["via"]; next unless via
      (via.is_a?(Array) ? via : [via]).each { |v| out[v] << { dag_id: dag_id, slot_id: slot_id } if v.is_a?(String) && !v.start_with?("(") }
    end
  end
end
transition_index = Hash.new { |h, k| h[k] = [] }
dags.each { |dag_id, d| d[:slots].each { |slot_id, sconf| collect_transitions(sconf, dag_id, slot_id, transition_index) } }

def resolve_entity_kind_candidates(domain, vconf)
  meta = vconf["metadata"] || {}
  [vconf.dig("transition_args", "target_slot"), meta["noun"], domain.tr("-", "_"), vconf.dig("crud", "table")].compact.uniq
end

status_of = {}
verbs.each do |fqn, v|
  candidates = resolve_entity_kind_candidates(v[:domain], v[:config])
  matched_slot = candidates.find { |c| slot_owning_dag.key?(c) }
  has_transition = transition_index[fqn].any?
  has_recon = recon_index[fqn].any?
  has_progression = progression_index[fqn].any?
  status_of[fqn] = if has_transition then "edge-bound"
                    elsif has_recon then "workspace-surface"
                    elsif has_progression then "lifecycle-progression"
                    elsif matched_slot then "node-global"
                    else "taxonomy-gap"
                    end
end

# ---------------------------------------------------------------------------
# 2. taxonomy-gap set must equal exactly the intentionally-excluded verbs
# ---------------------------------------------------------------------------
puts "\n## 1. Coverage check"
# session/pack added 2026-07: confirmed by Adam as ephemeral REPL
# ContextStack levers, not persisted data state — same exclusion class as
# agent.*/view.*/nav.*. Matches 10_apply_remediation.rb's EXCLUDED_DOMAINS.
EXCLUDED_DOMAINS = %w[agent view nav session pack]
expected_excluded = verbs.keys.select { |fqn| EXCLUDED_DOMAINS.any? { |d| verbs[fqn][:domain] == d || verbs[fqn][:domain].start_with?("#{d}.") } }.to_set
actual_gap = verbs.keys.select { |fqn| status_of[fqn] == "taxonomy-gap" }.to_set

unexpected_gap = actual_gap - expected_excluded
missing_from_gap = expected_excluded - actual_gap # excluded verbs that somehow got resolved — fine, just note it

if unexpected_gap.empty?
  pass_check.call("taxonomy-gap set contains only intentionally-excluded verbs (#{actual_gap.size} total)")
else
  fail_check.call("#{unexpected_gap.size} verb(s) still unresolved and NOT in the excluded list:")
  unexpected_gap.to_a.sort.first(50).each { |fqn| puts "    #{fqn}" }
  puts "    ... and #{unexpected_gap.size - 50} more" if unexpected_gap.size > 50
end
puts "  (info) #{missing_from_gap.size} excluded verb(s) resolved anyway (harmless, just means a rule caught them too)" unless missing_from_gap.empty?

pct = ((verbs.size - actual_gap.size).to_f / verbs.size * 100).round(1)
puts "  Coverage: #{verbs.size - actual_gap.size}/#{verbs.size} (#{pct}%) resolved to a pack or intentionally excluded"

# ---------------------------------------------------------------------------
# 3. Manifest disagreements — only the documented deferred one should remain
# ---------------------------------------------------------------------------
puts "\n## 2. Manifest ownership disagreement check"
KNOWN_DEFERRED = Set["product", "service_resource"] # documented in EXCLUDED_VERBS.md
disagreements = entity_kind_claimants.select { |ek, claimants| claimants.size > 1 }
unexpected_disagreements = disagreements.reject { |ek, _| KNOWN_DEFERRED.include?(ek) }
if unexpected_disagreements.empty?
  pass_check.call("no unexpected owned_entity_kinds disagreements (#{disagreements.size - unexpected_disagreements.size} known/documented exception(s) remain)")
else
  unexpected_disagreements.each { |ek, claimants| fail_check.call("entity_kind=#{ek} claimed by multiple packs: #{claimants.join('|')}") }
end

# owned_packs duplicate check
jpack_dupes = jpack_owner.select { |_, owners| owners.size > 1 }
if jpack_dupes.empty?
  pass_check.call("no journey pack is claimed by more than one domain pack")
else
  jpack_dupes.each { |jp, owners| fail_check.call("journey pack #{jp} claimed by multiple domain packs: #{owners.join('|')}") }
end

# ---------------------------------------------------------------------------
# 4. Pack allowlists still 100% valid against the atomic/macro registry
# ---------------------------------------------------------------------------
puts "\n## 3. Pack allowlist validity check"
total_invalid = 0
journey_packs.each do |jp_id, jp|
  invalid = jp[:allowed_verbs].reject { |v| verbs.key?(v) || macros.key?(v) }
  next if invalid.empty?
  total_invalid += invalid.size
  fail_check.call("#{jp_id} allows #{invalid.size} unknown FQN(s): #{invalid.first(10).join(', ')}")
end
pass_check.call("every pack's allowed_verbs resolves to a real atomic verb or macro") if total_invalid.zero?

# ---------------------------------------------------------------------------
# 5. Summary
# ---------------------------------------------------------------------------
puts "\n" + "=" * 72
if FAILURES.empty?
  puts "ALL CHECKS PASSED"
else
  puts "#{FAILURES.size} CHECK(S) FAILED — see FAIL lines above"
end
puts "=" * 72
puts "\nThis script only checked YAML-level consistency. Run these too before"
puts "trusting the remediation end-to-end:"
puts "  cargo build"
puts "  cargo run --manifest-path xtask/Cargo.toml -- verbs lint"
puts "  cargo test -p dsl-analysis -p dsl-resolution -p dsl-runtime"
puts "  git diff --stat   # confirm the blast radius matches expectations"

exit(FAILURES.empty? ? 0 : 1)
