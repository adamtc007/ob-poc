#!/usr/bin/env ruby
# 00_impact_scan.rb
#
# RUN THIS FIRST, before 10_apply_remediation.rb.
#
# The verb-pack remediation itself only moves verb FQNs between
# allowlists/reconciliation blocks in YAML — no verb FQN ever changes, so
# reassigning verb ownership carries no rename risk by construction. The
# only genuine hardcoding risk in this remediation is:
#   (a) the product-service-taxonomy -> product-maintenance id rename
#   (b) the brand-new platform-admin pack id (collision check)
#   (c) confirming allowed_verbs / owned_entity_kinds / owned_packs /
#       dsl_verb_reconciliation are read dynamically from YAML by Rust,
#       not hardcoded per-pack anywhere
#
# This scans rust/src, rust/crates, rust/xtask, rust/tests, rust/examples —
# deliberately EXCLUDING rust/config and rust/dsl-source, since those ARE
# the taxonomy/config files this remediation edits on purpose.
#
# Run from repo root:
#   ruby 00_impact_scan.rb

SEARCH_DIRS = %w[rust/src rust/crates rust/xtask rust/tests rust/examples]

RENAME_RISK_TERMS = %w[product-service-taxonomy ob-poc.product-service-taxonomy]
NEW_ID_TERMS = %w[platform-admin ob-poc.platform-admin]
STRUCTURAL_FIELD_TERMS = %w[dsl_verb_reconciliation owned_entity_kinds owned_packs allowed_verbs]

def rg(term, dirs)
  dirs = dirs.select { |d| Dir.exist?(d) }
  return [] if dirs.empty?
  out = IO.popen(["rg", "-n", "--no-heading", "-F", term, *dirs], err: [:child, :out]) { |io| io.read }
  out.lines.map(&:chomp)
rescue Errno::ENOENT
  warn "ripgrep (rg) not found on PATH — install it, or swap `rg -n --no-heading -F` for `grep -rn -F` below."
  []
end

puts "=" * 72
puts "IMPACT SCAN — hardcoded reference check"
puts "=" * 72

puts "\n## 1. RENAME RISK: product-service-taxonomy -> product-maintenance"
puts "Every hit below is a place that will silently keep referring to the"
puts "OLD id — the rename step only touches 2 files (the journey-pack yaml"
puts "and the domain-pack manifest). Anything found here needs a manual fix."
any = false
RENAME_RISK_TERMS.each do |term|
  hits = rg(term, SEARCH_DIRS)
  next if hits.empty?
  any = true
  puts "\n  term: #{term.inspect}  (#{hits.size} hit(s))"
  hits.each { |h| puts "    #{h}" }
end
puts "\n  -> CLEAN: no source-code references found." unless any

puts "\n## 2. NEW-ID COLLISION CHECK: platform-admin"
any = false
NEW_ID_TERMS.each do |term|
  hits = rg(term, SEARCH_DIRS)
  next if hits.empty?
  any = true
  puts "\n  term: #{term.inspect}  (#{hits.size} hit(s)) — investigate before using this id"
  hits.each { |h| puts "    #{h}" }
end
puts "\n  -> CLEAN: 'platform-admin' is not already in use anywhere in source." unless any

puts "\n## 3. STRUCTURAL FIELD CONSUMERS (informational)"
puts "Confirms these fields are read dynamically from YAML at runtime, not"
puts "hardcoded per-pack in Rust. If any count looks surprisingly low (e.g."
puts "0 hits for allowed_verbs), pack admission may be enforced somewhere"
puts "this scan didn't catch — grep manually before trusting that the"
puts "remediation's pack-admission changes have real runtime effect."
STRUCTURAL_FIELD_TERMS.each do |term|
  hits = rg(term, SEARCH_DIRS)
  puts "  #{term}: #{hits.size} reference(s) in source"
end

puts "\n## 4. VERB FQN SPOT-CHECK"
puts "A blind grep for all 1,257 FQNs across all source is too expensive"
puts "and noisy to run automatically. If you suspect a SPECIFIC verb is"
puts "hardcoded somewhere (a match/switch statement, a special-cased"
puts "handler), check it directly, e.g.:"
puts '  rg -n "session\\.load-cluster" rust/src rust/crates'
puts "\nKnown existing hotspots from prior research (already reviewed,"
puts "unrelated to this remediation, not touched by it):"
puts "  - rust/crates/dsl-migrate/src/verb_resolver.rs — Camunda topic->verb"
puts "    string map for BPMN migration; legacy/unrelated verb names."
puts "  - rust/xtask/src/main.rs PACK001/PACK002 lints — read pack/macro"
puts "    YAML dynamically at lint time, no hardcoded verb lists."

puts "\n" + "=" * 72
puts "If section 1 or 2 show ANY hits, resolve them manually (or pick a"
puts "different new pack id) BEFORE running 10_apply_remediation.rb --apply."
puts "=" * 72
