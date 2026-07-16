#!/usr/bin/env ruby
# 10_apply_remediation.rb
#
# Applies the verb->pack remediation agreed in the recon conversation.
# DRY RUN BY DEFAULT — prints every planned change and a summary, writes
# nothing, unless you pass --apply. Use --sample to only actually touch
# one representative file per change-category on the first --apply run,
# so you can eyeball the diff before letting it loose on everything.
#
#   ruby 10_apply_remediation.rb                # dry run, full plan
#   ruby 10_apply_remediation.rb --apply --sample   # apply to 1 file/category
#   ruby 10_apply_remediation.rb --apply            # apply everything
#
# IMPORTANT: this is a TEXT patcher, not a YAML re-serializer. It finds
# existing `key:` blocks by indentation and inserts new `- item` lines
# next to them, leaving every comment and everything else byte-for-byte
# untouched. That's deliberate (a real YAML.dump would nuke every comment
# in your hand-authored DAG files) but it means it can be fooled by
# unusual formatting. ALWAYS review the diff (git diff) after --apply
# before committing, especially on the first run.
#
# Run from repo root.

require "yaml"
require "set"
require "fileutils"

APPLY = ARGV.include?("--apply")
SAMPLE = ARGV.include?("--sample")
SKIP_RENAME = ARGV.include?("--skip-rename")
# --skip-rename: added after 00_impact_scan.rb found 37 hardcoded refs to
# "product-service-taxonomy" in ACP boundary/envelope code and byte-
# equality tests (rust/src/acp_runtime_context.rs, ob-poc-boundary's
# acp_dag_semantic.rs / acp_pack_context_envelope_v2.rs /
# acp_registry_projection.rs, rust/tests/catalogue_pack_seam.rs,
# rust/xtask/src/acp_envelope_byte_equality.rs). Byte-equality tests on an
# envelope suggest this id may also be baked into PERSISTED state (session
# snapshots, event logs) that a source-code grep can't see — a text-patch
# rename could leave stored data holding the old id while source moves to
# the new one. Scope that as its own deliberate migration, not this one.

LOAD = ->(f) { YAML.safe_load(File.read(f), permitted_classes: [], permitted_symbols: [], aliases: true) }

CHANGE_LOG = [] # [category, path, description]

# ---------------------------------------------------------------------------
# 0. Load current state (same loaders as the reconciliation scripts)
# ---------------------------------------------------------------------------
verbs = {}
Dir["rust/config/verbs/**/*.yaml"].sort.each do |f|
  next if File.basename(f) == "_meta.yaml"
  doc = LOAD.call(f) || {}
  (doc["domains"] || {}).each do |domain, dconf|
    (dconf["verbs"] || {}).each { |action, vconf| verbs["#{domain}.#{action}"] = { file: f, domain: domain, config: vconf } }
  end
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
  domain_packs[y["pack_id"]] = { file: f, owned_packs: Array(y["owned_packs"]), owned_dags: Array(y["owned_dags"]) }
end

dags = {}
Dir["rust/config/sem_os_seeds/dag_taxonomies/*.yaml"].sort.each do |f|
  y = LOAD.call(f) || {}
  next unless y["dag_id"]
  dags[y["dag_id"]] = { file: f }
end

dag_owner = Hash.new { |h, k| h[k] = [] }
domain_packs.each { |pid, dp| dp[:owned_dags].each { |d| dag_owner[d] << pid } }
jpack_owner = Hash.new { |h, k| h[k] = [] } # journey_pack_id -> [domain_pack_ids]
domain_packs.each { |pid, dp| dp[:owned_packs].each { |jp| jpack_owner[jp] << pid } }

def primary_dag_for_journey_pack(jp_id, jpack_owner, domain_packs)
  dpid = jpack_owner[jp_id]&.first
  return nil unless dpid
  domain_packs.dig(dpid, :owned_dags)&.first
end

# ---------------------------------------------------------------------------
# 1. Text-patch helpers
# ---------------------------------------------------------------------------

# Insert missing items into a YAML block-sequence found by descending
# key_path (array of key names) from the top of the file. Creates the
# tail of the path as a new nested block if it doesn't fully exist yet.
# Returns the list of items actually added (empty if all already present
# or if dry-run).
def patch_yaml_sequence(path, key_path, new_items)
  return [] if new_items.empty?
  lines = File.readlines(path)

  key_line_idx = nil
  indent = -2
  found_depth = 0
  search_from = 0

  key_path.each_with_index do |key, depth|
    target_indent = depth.zero? ? 0 : indent + 2
    found = nil
    i = search_from
    while i < lines.size
      line = lines[i]
      stripped = line.strip
      unless stripped.empty? || stripped.start_with?("#")
        this_indent = line[/^ */].size
        break if depth.positive? && this_indent < target_indent
        if this_indent == target_indent && stripped.start_with?("#{key}:")
          found = i
          break
        end
        break if this_indent < target_indent
      end
      i += 1
    end
    if found
      key_line_idx = found
      indent = target_indent
      found_depth = depth + 1
      search_from = found + 1
    else
      break
    end
  end

  raise "could not locate top-level key #{key_path.first.inspect} in #{path}" if found_depth.zero?

  if found_depth == key_path.size
    seq_indent = indent + 2
    existing = []
    end_idx = key_line_idx + 1
    while end_idx < lines.size
      line = lines[end_idx]
      if line.strip.empty?
        end_idx += 1
        next
      end
      this_indent = line[/^ */].size
      break if this_indent < seq_indent
      if this_indent == seq_indent && line.lstrip.start_with?("- ")
        existing << line.lstrip.sub(/^- /, "").strip
        end_idx += 1
      elsif this_indent > seq_indent
        end_idx += 1
      else
        break
      end
    end
    missing = new_items.reject { |it| existing.include?(it) }.uniq
    return [] if missing.empty?
    insertion = missing.sort.map { |it| "#{' ' * seq_indent}- #{it}\n" }
    lines = lines[0...end_idx] + insertion + lines[end_idx..-1]
  else
    parent_indent = indent
    child_indent = parent_indent + 2
    end_idx = key_line_idx + 1
    while end_idx < lines.size
      line = lines[end_idx]
      if line.strip.empty?
        end_idx += 1
        next
      end
      this_indent = line[/^ */].size
      break if this_indent < child_indent
      end_idx += 1
    end
    remaining = key_path[found_depth..]
    block = []
    cur_indent = child_indent
    remaining.each { |k| block << "#{' ' * cur_indent}#{k}:\n"; cur_indent += 2 }
    new_items.sort.each { |it| block << "#{' ' * cur_indent}- #{it}\n" }
    missing = new_items.dup
    lines = lines[0...end_idx] + block + lines[end_idx..-1]
  end

  File.write(path, lines.join) if APPLY
  missing
end

# Remove a single "- value" line from a block found under key_path.
# If the block becomes empty, collapses "key:\n" to "key: []\n" rather
# than leaving a dangling key with nothing under it (parses as YAML null,
# not an empty sequence — harmless to the one known reader today, since
# sem_os_obpoc_adapter's string_vec() treats non-sequence as empty, but
# ambiguous YAML worth avoiding on principle).
def remove_yaml_sequence_item(path, key_path, value)
  lines = File.readlines(path)
  removed = false
  target_key = key_path.last
  in_block = false
  block_indent = nil
  key_line_idx = nil
  items_kept = 0
  new_lines = []
  lines.each do |line|
    stripped = line.strip
    if !in_block && stripped.start_with?("#{target_key}:")
      in_block = true
      block_indent = line[/^ */].size
      key_line_idx = new_lines.size
      new_lines << line
      next
    end
    if in_block
      this_indent = line[/^ */].size
      if stripped.empty?
        new_lines << line
        next
      end
      if this_indent <= block_indent && !stripped.start_with?("- ")
        in_block = false
        new_lines << line
        next
      end
      if stripped == "- #{value}"
        removed = true
        next # drop this line
      end
      items_kept += 1
    end
    new_lines << line
  end
  if removed && items_kept.zero?
    new_lines[key_line_idx] = "#{' ' * block_indent}#{target_key}: []\n"
  end
  File.write(path, new_lines.join) if APPLY && removed
  removed
end

def log(category, path, desc)
  CHANGE_LOG << [category, path, desc]
end

def sample_gate(category, seen)
  return true unless SAMPLE
  if seen.include?(category)
    false
  else
    seen << category
    true
  end
end

seen_categories = Set.new

# ---------------------------------------------------------------------------
# 2. Manifest cleanup — false owned_entity_kinds claims + duplicate owned_packs
# ---------------------------------------------------------------------------
ENTITY_KIND_CLEANUP = {
  "ob-poc.book-setup" => %w[cbu cbu_service_option_binding cbu_disposition cbu_resource_instance_option_lineage cbu_evidence],
  "ob-poc.instrument-matrix" => %w[cbu cbu_service_option_binding cbu_disposition cbu_resource_instance_option_lineage cbu_evidence],
  "ob-poc.session-bootstrap" => %w[cbu cbu_service_option_binding cbu_disposition cbu_resource_instance_option_lineage cbu_evidence],
}
OWNED_PACKS_CLEANUP = {
  # ob-poc.cbu falsely co-claims book-setup and kyc-case, which are
  # independently owned by ob-poc.book-setup and ob-poc.kyc respectively.
  "ob-poc.cbu" => %w[book-setup kyc-case],
}

puts "== Manifest cleanup =="
ENTITY_KIND_CLEANUP.each do |pack_id, kinds|
  next unless sample_gate("entity_kind_cleanup", seen_categories)
  dp = domain_packs[pack_id]
  next unless dp
  kinds.each do |k|
    removed = remove_yaml_sequence_item(dp[:file], %w[owned_entity_kinds], k)
    if removed
      log("manifest-cleanup", dp[:file], "removed false owned_entity_kinds claim: #{k}")
    end
  end
end
OWNED_PACKS_CLEANUP.each do |pack_id, jps|
  next unless sample_gate("owned_packs_cleanup", seen_categories)
  dp = domain_packs[pack_id]
  next unless dp
  jps.each do |jp|
    removed = remove_yaml_sequence_item(dp[:file], %w[owned_packs], jp)
    log("manifest-cleanup", dp[:file], "removed duplicate owned_packs claim: #{jp}") if removed
  end
end

# ---------------------------------------------------------------------------
# 3. session_bootstrap_dag.yaml — REMOVED (2026-07). Confirmed by Zed
# against the real repo: this file already exists, and is deliberately
# scoped to exactly session.load-cluster/session.load-galaxy, with its own
# `out_of_scope` note pointing the rest of session.*/pack.* at "Layer 3
# REPL V2 ContextStack". The original assumption that this DAG was
# missing (from an earlier recon pass that failed to match it) was wrong.
# Nothing to create or wire here — see chat for the open question on
# where session.set-case/set-mandate/set-persona/set-structure/start/
# undo/redo and pack.answer/pack.select should actually live.
# ---------------------------------------------------------------------------


# ---------------------------------------------------------------------------
# 4. New pack trio: platform-admin (refdata + access control + team admin)
# ---------------------------------------------------------------------------
PLATFORM_ADMIN_VERBS = %w[
  identifier.attach identifier.attach-clearstream identifier.attach-lei
  identifier.find-by-clearstream identifier.find-by-isin identifier.find-by-lei
  identifier.invalidate identifier.list-by-entity identifier.remove
  identifier.update-lei-status identifier.validate
  admin.regulators.create admin.regulators.deactivate admin.regulators.list
  admin.regulators.read admin.regulators.update
  admin.role-types.create admin.role-types.deactivate admin.role-types.list
  admin.role-types.read admin.role-types.update
  role.delete role.ensure role.list role.read
  refdata.deactivate refdata.ensure refdata.list refdata.read
  refdata.load-all refdata.load-instrument-classes refdata.load-markets
  refdata.load-sla-templates refdata.load-subcustodians
  team.add-cbu-access team.add-governance-member team.add-member team.archive
  team.create team.grant-service team.read team.remove-cbu-access team.remove-member
  team.revoke-service team.transfer-member team.update-member
  user.create user.offboard user.reactivate user.suspend
  access-review.attest access-review.audit-report access-review.bulk-confirm
  access-review.campaign-status access-review.confirm-access access-review.confirm-all-clean
  access-review.create-campaign access-review.escalate-item access-review.extend-access
  access-review.launch-campaign access-review.list-flagged access-review.list-items
  access-review.my-pending access-review.populate-campaign access-review.process-deadline
  access-review.revoke-access access-review.send-reminders
]

PACK_YAML_PATH = "rust/config/packs/platform-admin.yaml"
PACK_YAML_TEMPLATE = <<~YAML
  # Platform Admin Journey Pack (scaffold, generated #{Time.now.strftime('%Y-%m-%d')})
  #
  # Reference-data vocabulary (roles, regulators, identifiers, general
  # refdata) combined with access/entitlement administration (teams, users,
  # access-review campaigns). Housekeeping-tier pack, not tied to a single
  # business entity's operational lifecycle.
  #
  # NOTE: role.* and admin.role-types.* look like near-duplicate concepts
  # (both are "role reference data with a deactivate transition"). Kept as
  # separate verb families here per the original recon — flagged for a
  # follow-up decision on whether they should collapse into one.

  id: platform-admin
  name: Platform Admin
  version: "1.0"
  description: >
    Reference-data vocabulary (roles, regulators, identifiers) and
    access/entitlement administration (teams, users, access-review
    campaigns). Scaffold — review naming and verb split before treating
    as final.

  invocation_phrases:
    - "manage roles"
    - "manage team access"
    - "run access review"

  required_context: []
  optional_context: []
  workspaces:
    - platform_admin

  allowed_verbs:
  #{PLATFORM_ADMIN_VERBS.sort.map { |v| "  - #{v}" }.join("\n")}

  forbidden_verbs: []
  risk_policy:
    require_confirm_before_execute: true
    max_steps_without_confirm: 3
  required_questions: []
  optional_questions: []
  stop_rules: []
  templates: []
  section_layout:
    - title: "Reference Data"
      verb_prefixes: ["identifier.", "admin.", "role.", "refdata."]
    - title: "Access & Entitlements"
      verb_prefixes: ["team.", "user.", "access-review."]
  definition_of_done: []
  progress_signals: []
YAML

MANIFEST_TEMPLATE = <<~YAML
  pack_id: ob-poc.platform-admin
  name: ob-poc Platform Admin Domain Pack
  version: 0.1.0
  implementation_mode: native_compiled
  compatibility_tier: dry_run_only
  owned_dags: [platform_admin_dag]
  owned_packs: [platform-admin]
  owned_state_machines: []
  owned_constellation_maps: []
  owned_constellation_families: []
  owned_universes: []
  owned_verb_prefixes:
    - identifier.
    - admin.
    - role.
    - refdata.
    - team.
    - user.
    - access-review.
  owned_entity_kinds:
    - identifier
    - role
    - team
    - access_review
  business_crates: []
  owned_constellations: []
  allowed_transitions: []
  discovery_probes: []
  projection_catalog: []
  mention_namespaces: []
  declared_modes: []
  workflow_phases: []
  acp_personas: []
  resource_uri_schemes: []
  external_mcp_transports: []
  typed_extension_points: []
  classification_policy:
    max_prompt_classification: internal
    allow_external_llm: false
    required_redactions: [pii]
YAML

DAG_TEMPLATE = <<~YAML
  # Platform Admin Workspace — DAG Taxonomy (scaffold, generated #{Time.now.strftime('%Y-%m-%d')})
  #
  # STATUS: scaffold. Verbs below are declared node-global to this
  # workspace via dsl_verb_reconciliation only — no real state machines
  # have been authored for the `role`, `team`, `user`, or `access_review`
  # slots yet (e.g. team.create -> active -> archived is plausible from
  # the verb names but was NOT guessed at here; author it explicitly as
  # follow-up work rather than trusting an inferred state machine).

  version: "1.0"
  workspace: platform_admin
  dag_id: platform_admin_dag

  dsl_verb_reconciliation:
    reference_data_surface:
  #{PLATFORM_ADMIN_VERBS.select { |v| v.start_with?("identifier.", "admin.", "role.", "refdata.") }.sort.map { |v| "      - #{v}" }.join("\n")}
    access_entitlement_surface:
  #{PLATFORM_ADMIN_VERBS.select { |v| v.start_with?("team.", "user.", "access-review.") }.sort.map { |v| "      - #{v}" }.join("\n")}

  slots:
    - id: workspace_root
      stateless: true
      rationale: "Aggregation root for platform admin domain."

    - id: role
      stateless: true
      rationale: "Reference-data vocabulary. State machine not yet authored (scaffold)."

    - id: team
      stateless: false
      state_machine: "(scaffold — team.create/archive suggest a real lifecycle; not authored here, follow-up work.)"

    - id: access_review
      stateless: false
      state_machine: "(scaffold — campaign create/launch/confirm/revoke/close suggest a real lifecycle; not authored here, follow-up work.)"

  cross_slot_constraints: []
  cross_workspace_constraints: []
  derived_cross_workspace_state: []
  product_module_gates:
    always_on:
      - workspace_root
      - role
      - team
      - access_review
    conditionally_on: []
  out_of_scope: []
  prune_cascade_rules: []
  prune_pre_validation:
    required_verbs: []
    abort_conditions: []
YAML

if sample_gate("new_platform_admin_pack", seen_categories)
  [
    [PACK_YAML_PATH, PACK_YAML_TEMPLATE],
    ["rust/config/sem_os_seeds/domain_packs/ob_poc_platform_admin.yaml", MANIFEST_TEMPLATE],
    ["rust/config/sem_os_seeds/dag_taxonomies/platform_admin_dag.yaml", DAG_TEMPLATE],
  ].each do |path, content|
    if File.exist?(path)
      log("new-file", path, "SKIPPED — already exists")
    else
      File.write(path, content) if APPLY
      log("new-file", path, "created")
    end
  end
end

# ---------------------------------------------------------------------------
# 5. Rename product-service-taxonomy -> product-maintenance (id fields only;
#    dag_id and filenames of the DAG taxonomy file are left untouched to
#    minimize blast radius — only the journey-pack id and domain-pack id
#    change, matching what the DAG's own `workspace:` field already says.)
# ---------------------------------------------------------------------------
if !SKIP_RENAME && sample_gate("rename_product_pack", seen_categories)
  old_pack_file = "rust/config/packs/product-service-taxonomy.yaml"
  new_pack_file = "rust/config/packs/product-maintenance.yaml"
  if File.exist?(old_pack_file)
    text = File.read(old_pack_file)
    text = text.sub(/^id: product-service-taxonomy$/, "id: product-maintenance")
    if APPLY
      File.write(old_pack_file, text)
      FileUtils.mv(old_pack_file, new_pack_file)
    end
    log("rename", new_pack_file, "id: product-service-taxonomy -> product-maintenance, file renamed")
  end

  old_manifest = domain_packs["ob-poc.product-service-taxonomy"]
  if old_manifest
    text = File.read(old_manifest[:file])
    text = text.sub(/^pack_id: ob-poc\.product-service-taxonomy$/, "pack_id: ob-poc.product-maintenance")
    text = text.sub(/^owned_packs: \[product-service-taxonomy\]$/, "owned_packs: [product-maintenance]")
    new_manifest_file = old_manifest[:file].sub("ob_poc_product_service_taxonomy.yaml", "ob_poc_product_maintenance.yaml")
    if APPLY
      File.write(old_manifest[:file], text)
      FileUtils.mv(old_manifest[:file], new_manifest_file)
    end
    log("rename", new_manifest_file, "pack_id + owned_packs updated, file renamed")
  end
end

# ---------------------------------------------------------------------------
# 6. Domain-prefix rule table for currently-unassigned verbs (Track B),
#    plus a generic pass for verbs a journey pack already allows but whose
#    owning DAG never recorded them (Track A), plus a pass for verbs
#    already DAG-resolved but not yet admitted into any journey pack
#    (Track C — e.g. share-class.*).
# ---------------------------------------------------------------------------
RULES = {
  "research.companies-house" => "book-setup", "research.sec-edgar" => "book-setup",
  "research.sources" => "book-setup", "research.outreach" => "book-setup",
  "research.import-run" => "book-setup", "research.generic" => "book-setup",
  "research.workflow" => "book-setup", "gleif" => "book-setup", "bods" => "book-setup",

  "document" => "kyc-case", "docs-bundle" => "kyc-case", "evidence" => "kyc-case",
  "kyc-agreement" => "kyc-case", "request" => "kyc-case", "requirement" => "kyc-case",
  "discrepancy" => "kyc-case", "allegation" => "kyc-case", "observation" => "kyc-case",
  "verify" => "kyc-case", "economic-exposure" => "kyc-case", "issuer-control-config" => "kyc-case",
  "ownership" => "kyc-case",

  "discovery" => "semos-maintenance", "graph" => "semos-maintenance", "focus" => "semos-maintenance",
  "schema" => "semos-maintenance", "state" => "semos-maintenance", "maintenance" => "semos-maintenance",
  "constellation" => "semos-maintenance", "template" => "semos-maintenance", "delegation" => "semos-maintenance",

  # NOTE: "session" / "pack" prefixes deliberately have NO rule here.
  # session-bootstrap is a hand-authored, intentionally narrow pack scoped
  # to exactly session.load-cluster/session.load-galaxy (confirmed by Zed
  # against the real repo, 2026-07). The other session.*/pack.* verbs are
  # a genuine open gap pending an architecture decision — see chat. Do NOT
  # add a blanket prefix rule here without re-checking pack scope first.

  "fund" => "cbu-maintenance", "cbu-group" => "cbu-maintenance", "cbu-role" => "cbu-maintenance",
  "entity-relationship" => "cbu-maintenance", "batch" => "cbu-maintenance", "service-intent" => "cbu-maintenance",

  "onboarding" => "onboarding-request", "readiness" => "onboarding-request",
  "semantic" => "onboarding-request", "provisioning" => "onboarding-request",

  "audit" => "semos-maintenance",
  # billing.* (profile lifecycle + recurring period/invoice cycle) ->
  # deal-lifecycle, matching what the pack's own section_layout already
  # declared intent for. account-targets verbs are excluded from this via
  # the BILLING_ONBOARDING special-case above (checked first).
  "billing" => "deal-lifecycle",
  # regulatory.registration.* / regulatory.status.* are CBU-instance state
  # (is THIS cbu registered/verified with a regulator) — not reference
  # data, so this deliberately does NOT go in platform-admin alongside
  # admin.regulators.* (the regulator catalog itself). Matches the same
  # reference-vs-instance split already applied to identifier/refdata vs
  # the KYC tail. Confirmed correct by Adam directly, 2026-07.
  "regulatory" => "cbu-maintenance",
}
SLA_ONBOARDING = %w[sla.commit sla.bind sla.list-commitments]
SLA_CBU = %w[sla.list-measurements sla.record-measurement sla.list-open-breaches sla.report-breach
             sla.resolve-breach sla.escalate-breach sla.update-remediation sla.suspend-commitment]
SLA_PRODUCT = %w[sla.list-templates sla.read-template]
# Resolves to whichever id is CURRENTLY real. Fixes a bug found by Zed
# (2026-07): hardcoding "product-maintenance" here silently no-op'd both
# sla.* assignments under --skip-rename, since that pack id doesn't exist
# yet. If the rename is ever applied, this picks up the new id automatically.
PRODUCT_PACK_ID = SKIP_RENAME ? "product-service-taxonomy" : "product-maintenance"

EXCLUDED_DOMAINS = %w[agent view nav session pack] # not domain DSL — control-plane / presentation / ephemeral REPL ContextStack. See EXCLUDED_VERBS.md.
# NOTE: session.load-cluster and session.load-galaxy are NOT actually
# affected by this exclusion even though "session" is listed above — they
# already have real status (lifecycle-progression, via progression_verbs)
# from a source this rules table never touches, so they were never
# reached by Track B in the first place. The exclusion only matters for
# the 15 other session.* verbs + pack.answer/pack.select, which had no
# resolution at all before this decision.
# platform-admin domains are handled by the new-pack step above; exclude
# them from the generic rules pass so they aren't double-processed.
PLATFORM_ADMIN_DOMAINS = %w[identifier admin.regulators admin.role-types role refdata team user access-review]

BILLING_ONBOARDING = %w[billing.add-account-target billing.remove-account-target billing.list-account-targets]
# NOTE: add/remove are singular ("...target"), list is plural
# ("...targets") — matches the real verb FQNs in rust/config/verbs/
# billing.yaml exactly (grep-confirmed 2026-07). An earlier version of
# this array had all three plural, which silently misrouted add/remove
# into the "billing" => "deal-lifecycle" catch-all instead of here — no
# WARNING was raised because the catch-all rule matched instead, so this
# would have been a quiet wrong-pack assignment, not a crash. Always
# grep-verify verb FQNs against the actual YAML before hardcoding a list
# like this.
# Linkage/activation per CBU.Product pair — "for each cbu product pair,
# setting up that CBU.Product fee billing" (Adam, 2026-07). Belongs with
# onboarding-request's one-time subscribe/activate handoff, not the
# recurring commercial cycle below.

def target_journey_pack(fqn, domain)
  return "deal-lifecycle" if fqn == "billing.revenue-summary"
  return "onboarding-request" if BILLING_ONBOARDING.include?(fqn)
  # Everything else under billing.* (profile lifecycle + the recurring
  # period/invoice cycle) falls through to the "billing" => "deal-lifecycle"
  # rule below. Confirmed by Adam, 2026-07: the period cycle runs at the
  # commercial-profile level "applied to all cbu instances" on that
  # profile — a batch/profile-level process, not per-CBU operational
  # history (unlike SLA measurement/breach, which IS per-CBU and lives in
  # cbu-maintenance). Don't apply the SLA pattern here — different shape.
  return "onboarding-request" if SLA_ONBOARDING.include?(fqn)
  return "cbu-maintenance" if SLA_CBU.include?(fqn)
  return PRODUCT_PACK_ID if SLA_PRODUCT.include?(fqn)
  RULES.each { |prefix, pack| return pack if domain == prefix || domain.start_with?("#{prefix}.") }
  nil
end

puts "\n== Track B: rule-based assignment for unassigned verbs ==" if sample_gate("track_b_header", Set.new)
verbs.sort.each do |fqn, v|
  domain = v[:domain]
  next if EXCLUDED_DOMAINS.any? { |d| domain == d || domain.start_with?("#{d}.") }
  next if PLATFORM_ADMIN_DOMAINS.any? { |d| domain == d || domain.start_with?("#{d}.") }
  jp_id = target_journey_pack(fqn, domain)
  next unless jp_id
  jp = journey_packs[jp_id]
  unless jp
    log("WARNING", fqn, "rule resolved to pack id '#{jp_id}' which does not exist in journey_packs — skipped, not applied. Check --skip-rename state or the RULES table for a stale id.")
    next
  end
  next if jp[:allowed_verbs].include?(fqn) # already admitted — nothing to do here
  next unless sample_gate("track_b_#{jp_id}", seen_categories)

  added = patch_yaml_sequence(jp[:file], %w[allowed_verbs], [fqn])
  log("track-b-pack", jp[:file], "admitted #{fqn}") unless added.empty?

  target_dag = primary_dag_for_journey_pack(jp_id, jpack_owner, domain_packs)
  if target_dag && dags[target_dag]
    surface = "#{domain.tr('-', '_').tr('.', '_')}_surface"
    added2 = patch_yaml_sequence(dags[target_dag][:file], ["dsl_verb_reconciliation", surface], [fqn])
    log("track-b-dag", dags[target_dag][:file], "reconciled #{fqn} under #{surface}") unless added2.empty?
  end
end

puts "\n== Track A: verbs a pack already allows but the DAG never recorded ==" if true
verbs.sort.each do |fqn, v|
  domain = v[:domain]
  hitting_packs = journey_packs.select { |_, jp| jp[:allowed_verbs].include?(fqn) }.keys
  next if hitting_packs.empty?
  next unless sample_gate("track_a", seen_categories)
  hitting_packs.each do |jp_id|
    target_dag = primary_dag_for_journey_pack(jp_id, jpack_owner, domain_packs)
    next unless target_dag && dags[target_dag]
    # session_bootstrap_dag.yaml has no dsl_verb_reconciliation block by
    # design (see section 3 note) — leave it untouched this pass, same as
    # Track B. patch_yaml_sequence can extend an existing key path's tail
    # but can't fabricate a brand-new top-level key, so this would
    # otherwise crash.
    next if target_dag == "session_bootstrap_dag"
    surface = "#{domain.tr('-', '_').tr('.', '_')}_surface"
    added = patch_yaml_sequence(dags[target_dag][:file], ["dsl_verb_reconciliation", surface], [fqn])
    log("track-a-dag", dags[target_dag][:file], "reconciled #{fqn} under #{surface} (already allowed by #{jp_id})") unless added.empty?
  end
end

# ---------------------------------------------------------------------------
# 7. EXCLUDED_VERBS.md — decision record so this isn't rediscovered as a
#    mystery gap in a future audit.
# ---------------------------------------------------------------------------
EXCLUDED_MD = <<~MD
  # Verbs intentionally excluded from domain-pack ownership

  Generated #{Time.now.strftime('%Y-%m-%d')} as part of the verb->pack
  reconciliation. These verb families were deliberately NOT assigned to a
  domain pack, because they have no data state machine to attach to under
  the core rule: "a pack defines a data state-machine taxonomy and
  allocates verbs at its edges."

  ## agent.* (20 verbs)
  Operates the Sage/REPL agent capability itself (teach/unteach, authoring
  mode, execution mode, telemetry, decision confirmation). Control-plane
  runtime configuration, not domain entity state. No entity, no slot, no
  from/to state.

  ## view.* (14 verbs) and nav.* (7 verbs)
  Pure presentation/rendering directives (zoom, pan, layout, breadcrumbs,
  history navigation). Never mutate session or any entity's state — they
  read whatever the session already resolved and render/navigate it.
  Distinguished explicitly from session.* (which DOES mutate real,
  persistent session-scope state and is pack-owned under
  ob-poc.session-bootstrap).

  ## session.* (15 verbs, excluding load-cluster/load-galaxy) and pack.* (2 verbs)
  Confirmed by Adam, 2026-07: these are ephemeral, agent-session-scoped
  ContextStack levers — "levers the agent persona pulls in an active
  session" — not persisted data state. No audit trail, resets with the
  session. Matches the DAG's own `out_of_scope` note pointing at "Layer 3
  REPL V2 ContextStack". Same category as agent.*, for the same reason:
  control-plane, not a data state machine.

  Verbs: session.clear, filter-jurisdiction, info, list, load-system,
  load-universe, redo, set-case, set-client, set-mandate, set-persona,
  set-structure, start, undo, unload-system, pack.answer, pack.select.

  EXCEPTION: session.load-cluster and session.load-galaxy are NOT
  excluded — they establish which cluster/galaxy the session is actually
  bound to (the empty -> scoped transition), which is real scoping state
  other verbs depend on, unlike the REPL-navigation levers above. They
  remain correctly pack-linked via ob-poc.session-bootstrap /
  ob-poc.book-setup respectively.

  ## Resolved this pass (2026-07)
  `audit.*` (8 verbs) -> semos-maintenance, alongside changeset/governance.
  `regulatory.*` (5 verbs) -> cbu-maintenance (CBU-instance registration
  state, not reference data — kept out of platform-admin's
  admin.regulators.* catalog). `billing.*` (17 verbs total): the
  commercial-profile + recurring period/invoice cycle (create/activate/
  suspend/close-profile, get/list-profiles, create/calculate/review/
  approve-period, generate-invoice, dispute-period, period-summary,
  revenue-summary) -> deal-lifecycle, matching the pack's own
  section_layout intent (deal_dag.yaml already owns the billing_period
  slot/lifecycle) — this runs at the commercial-profile level across
  every CBU on that profile, a batch/profile-level process, not per-CBU
  operational history the way an SLA breach is. The account-target
  linkage verbs (add/remove/list-account-targets) -> onboarding-request
  instead: per-CBU.Product activation, a one-time subscribe/activate
  handoff, not the recurring cycle. billing.revenue-summary was nearly
  deleted as "confirmed dead" until 15_delete_verbs.rb's reference check
  found it live, registered, and implemented (sem_os_postgres::ops::
  billing.rs) — a worked example of that script doing its job.

  ## Known deferred question (not excluded, but not fully resolved)
  `product-maintenance`'s manifest declares `owned_entity_kinds: [product,
  service_resource, ...]`, but the `product` and `service_resource` DAG
  slots are physically defined in `instrument_matrix_dag.yaml`, not
  `product_service_taxonomy_dag.yaml`. This remediation does NOT move
  those slots (that's a structural DAG-ownership change, bigger than a
  verb reassignment) — flagged here for a follow-up decision.
MD

if sample_gate("excluded_md", seen_categories)
  File.write("EXCLUDED_VERBS.md", EXCLUDED_MD) if APPLY
  log("doc", "EXCLUDED_VERBS.md", "decision record created")
end

# ---------------------------------------------------------------------------
# 8. Summary
# ---------------------------------------------------------------------------
puts "\n" + "=" * 72
puts APPLY ? "APPLIED#{SAMPLE ? ' (--sample mode: 1 file/category only)' : ''}#{SKIP_RENAME ? ' (--skip-rename: product-service-taxonomy id left unchanged)' : ''}" : "DRY RUN (pass --apply to write; add --sample to preview one file/category first)"
puts "=" * 72
by_cat = CHANGE_LOG.group_by(&:first)
by_cat.each do |cat, entries|
  puts "\n#{cat} (#{entries.size}):"
  entries.first(15).each { |_, path, desc| puts "  #{path}: #{desc}" }
  puts "  ... and #{entries.size - 15} more" if entries.size > 15
end
puts "\nTotal changes: #{CHANGE_LOG.size}"
puts "\nNext: run 20_verify_remediation.rb, then `git diff` to eyeball the" \
     " patched files, especially the first run of each new-file category."
