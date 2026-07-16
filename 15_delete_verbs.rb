#!/usr/bin/env ruby
# 15_delete_verbs.rb
#
# Deletes confirmed-dead verb definitions from rust/config/verbs/**/*.yaml.
# Separate from 10_apply_remediation.rb on purpose: deletion is a
# different risk class than insertion (it's destructive, and the real
# blast radius depends on what references the verb, not just where it's
# declared) so it gets its own narrow tool and its own dry-run.
#
# Before deleting anything, checks for references in:
#   - journey pack allowed_verbs/forbidden_verbs
#   - DAG dsl_verb_reconciliation / progression_verbs / transitions[].via
#   - macro expands-to targets (a macro pointing at a deleted verb would
#     break at expansion time — this is treated as a hard stop, not a
#     warning)
#   - Rust source (rust/src, rust/crates, rust/xtask, rust/tests)
#   - rust/config/agent/*.yaml (known-stale secondary NLU mapping files
#     per earlier recon — reported if found, but doesn't block, since
#     those files are already flagged as needing their own cleanup pass)
#
# A verb with a macro-expansion reference or a live DAG/pack reference is
# NOT deleted even with --apply — it's reported as BLOCKED so you can
# decide by hand. Only genuinely reference-free verbs get removed.
#
#   ruby 15_delete_verbs.rb                # dry run / reference check only
#   ruby 15_delete_verbs.rb --apply        # deletes anything that's clean

require "yaml"
require "date"

APPLY = ARGV.include?("--apply")

VERBS_TO_DELETE = %w[]
# billing.revenue-summary was proposed for deletion (2026-07) on the
# premise "declared, never implemented, no data behind it" — this
# script's own reference check disproved that premise before anything
# was deleted: RevenueSummary is a fully implemented, registered
# SemOsVerbOp (rust/crates/sem_os_postgres/src/ops/billing.rs, real SQL
# against fee_billing_periods/fee_billing_period_lines, registered in
# sem_os_postgres::ops::mod.rs). Not dead — reassigned to deal-lifecycle
# via RULES in 10_apply_remediation.rb instead. Left empty here rather
# than removed as a worked example of the check doing its job; add a
# future confirmed-dead verb only with the same kind of explicit,
# re-verified confirmation — don't trust a "never implemented" claim
# without running this script against it first.

LOAD = ->(f) { YAML.safe_load(File.read(f), permitted_classes: [Date], permitted_symbols: [], aliases: true) }

def rg(term, dirs)
  dirs = dirs.select { |d| Dir.exist?(d) }
  return [] if dirs.empty?
  out = IO.popen(["rg", "-n", "--no-heading", "-F", term, *dirs], err: [:child, :out]) { |io| io.read }
  out.lines.map(&:chomp)
rescue Errno::ENOENT
  warn "ripgrep (rg) not found on PATH"
  []
end

# Delete a verb's `<action>:` block from its domain file, preserving
# every other line and comment verbatim (text patch, not a YAML re-dump —
# same approach as 10_apply_remediation.rb's insertions, for the same
# reason: don't destroy hand-authored comments elsewhere in the file).
def remove_verb_block(path, domain, action)
  lines = File.readlines(path)
  domain_idx = nil
  lines.each_with_index do |line, i|
    if line.strip == "#{domain}:" && line[/^ */].size.positive?
      domain_idx = i
      break
    end
  end
  return false unless domain_idx

  verbs_idx = nil
  domain_indent = lines[domain_idx][/^ */].size
  (domain_idx + 1...lines.size).each do |i|
    line = lines[i]
    next if line.strip.empty?
    this_indent = line[/^ */].size
    break if this_indent <= domain_indent
    if this_indent == domain_indent + 2 && line.strip == "verbs:"
      verbs_idx = i
      break
    end
  end
  return false unless verbs_idx

  verbs_indent = lines[verbs_idx][/^ */].size
  action_idx = nil
  (verbs_idx + 1...lines.size).each do |i|
    line = lines[i]
    next if line.strip.empty?
    this_indent = line[/^ */].size
    break if this_indent <= verbs_indent
    if this_indent == verbs_indent + 2 && line.strip == "#{action}:"
      action_idx = i
      break
    end
  end
  return false unless action_idx

  action_indent = lines[action_idx][/^ */].size
  end_idx = action_idx + 1
  while end_idx < lines.size
    line = lines[end_idx]
    if line.strip.empty?
      end_idx += 1
      next
    end
    break if line[/^ */].size <= action_indent
    end_idx += 1
  end

  new_lines = lines[0...action_idx] + lines[end_idx..-1]
  File.write(path, new_lines.join) if APPLY

  # Note if this leaves the domain's verbs block empty — worth a manual
  # look, not auto-cleaned here.
  remaining_verb_line = new_lines[(verbs_idx)...(verbs_idx + 3)].find { |l| l && l.strip.end_with?(":") && l[/^ */].size == verbs_indent + 2 }
  warn "  NOTE: '#{domain}' domain may now have no remaining verbs in #{path} — check whether the whole domain block should be removed too." unless remaining_verb_line

  true
end

puts "=" * 72
puts "DELETE CONFIRMED-DEAD VERBS"
puts "=" * 72

VERBS_TO_DELETE.each do |fqn|
  domain, action = fqn.split(".", 2)
  puts "\n## #{fqn}"

  blocking = []

  Dir["rust/config/packs/*.yaml"].each do |f|
    y = LOAD.call(f) || {}
    if Array(y["allowed_verbs"]).include?(fqn) || Array(y["forbidden_verbs"]).include?(fqn)
      blocking << "referenced in journey pack #{f}"
    end
  end

  Dir["rust/config/sem_os_seeds/dag_taxonomies/*.yaml"].each do |f|
    y = LOAD.call(f) || {}
    (y["dsl_verb_reconciliation"] || {}).each_value { |list| blocking << "referenced in dsl_verb_reconciliation of #{f}" if Array(list).include?(fqn) }
    Array(y.dig("overall_lifecycle", "phases")).each { |p| blocking << "referenced in progression_verbs of #{f}" if Array(p["progression_verbs"]).include?(fqn) }
    Array(y["slots"]).each do |s|
      [s["state_machine"], *Array(s["dual_lifecycle"])].each do |sm|
        next unless sm.is_a?(Hash)
        Array(sm["transitions"]).each do |t|
          via = t["via"]
          vias = via.is_a?(Array) ? via : [via]
          blocking << "referenced as a transition via in #{f}" if vias.include?(fqn)
        end
      end
    end
  end

  Dir["rust/config/verb_schemas/macros/*.yaml"].each do |f|
    (LOAD.call(f) || {}).each do |mk, mc|
      next unless mc.is_a?(Hash) && mc["kind"] == "macro"
      steps = Array(mc.dig("expands_to")) + Array(mc["steps"])
      text = steps.to_s
      blocking << "referenced in macro '#{mk}' expansion in #{f}" if text.include?(fqn)
    end
  end

  source_hits = rg(fqn, %w[rust/src rust/crates rust/xtask rust/tests])
  blocking.concat(source_hits.map { |h| "referenced in source: #{h}" })

  agent_hits = rg(fqn, %w[rust/config/agent])
  agent_hits.each { |h| puts "  INFO (non-blocking, known-stale NLU files): #{h}" }

  if blocking.empty?
    if APPLY
      files = Dir["rust/config/verbs/**/*.yaml"]
      deleted = false
      files.each do |f|
        next if File.basename(f) == "_meta.yaml"
        if remove_verb_block(f, domain, action)
          puts "  DELETED from #{f}"
          deleted = true
          break
        end
      end
      puts "  NOT FOUND in any verb file — nothing to delete (already gone?)" unless deleted
    else
      puts "  CLEAN — no references found. Would delete on --apply."
    end
  else
    puts "  BLOCKED — not deleting. References found:"
    blocking.uniq.each { |b| puts "    #{b}" }
  end
end

puts "\n" + "=" * 72
puts APPLY ? "APPLIED" : "DRY RUN (pass --apply to actually delete clean verbs)"
puts "=" * 72
