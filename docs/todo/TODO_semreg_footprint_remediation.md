# SemOS Verb Footprint + Metadata Remediation

**Codebase:** `ob-poc` (Rust, `rust/` directory)  
**File under edit:** `rust/config/sem_os_seeds/domain_metadata.yaml`  
**Build check:** `RUSTC_WRAPPER= cargo check -p ob-poc`  
**Seed loader test:** `RUSTC_WRAPPER= cargo test -p ob-poc -- domain_metadata`  
**Scope:** YAML-only changes — zero Rust code modifications  

---

## Context

The `domain_metadata.yaml` file is the SemOS metadata dictionary. It maps every table to a governance domain, and maps every verb to the tables it reads/writes (`verb_data_footprint`). The AffinityGraph, ECIR, and governed query systems all depend on these footprint declarations being correct and complete.

**Current state of the `sem-reg` + `stewardship` domains:**
- 83 DSL verbs defined in `rust/config/verbs/sem-reg/*.yaml` across 7 sub-domains
- Only **1 of 83** verbs has a matching footprint in `domain_metadata.yaml`
- 14 orphan footprints use **stale names** (`stew.compose-changeset`, `changeset.propose`, etc.) that no longer match any YAML verb FQN
- `sem_reg_pub` (4 tables) and `sem_reg_authoring` (6 tables) are completely absent from domain_metadata
- `sem_reg.classification_levels` and `sem_reg.disambiguation_prompts` are missing from domain_metadata

**Target state:** Every sem-reg/stewardship YAML verb has a `verb_data_footprint` entry. Every sem_reg/sem_reg_pub/sem_reg_authoring table has a metadata entry.

---

## Task 1: Remove Orphan Footprints

In `domain_metadata.yaml`, find and **delete** these 14 stale footprint entries. They use old verb names that don't match any current YAML verb FQN. They will be replaced with correct entries in Tasks 3-4.

### In the `sem-reg` domain `verb_data_footprint:` section, delete:

```yaml
# DELETE these 6 — they use old names
changeset.propose:      # → replaced by changeset.compose
changeset.validate:     # → replaced by governance.validate
changeset.publish:      # → replaced by governance.publish
registry.describe:      # → replaced by registry.describe-object
registry.list-verbs:    # → replaced by registry.list-objects (with type filter)
registry.list-attributes: # → same
```

### In the `stewardship` domain `verb_data_footprint:` section, delete:

```yaml
# DELETE these 8 — they use stew.* prefix, verbs now live in changeset/governance/focus domains
stew.compose-changeset:   # → changeset.compose
stew.submit-for-review:   # → governance.submit-for-review
stew.approve-changeset:   # → governance.record-review
stew.publish-changeset:   # → governance.publish
stew.attach-basis:        # → changeset.attach-basis
stew.resolve-conflict:    # → changeset.resolve-conflict
stew.set-focus:           # → focus.set
stew.show:                # → focus.get
```

**After this task:** 0 footprints remain in sem-reg/stewardship. Clean slate.

---

## Task 2: Add Missing Tables to Domain Metadata

### 2A: Add `sem_reg_pub` tables to `sem-reg` domain

In the `sem-reg` domain `tables:` section, add:

```yaml
    sem_reg_pub.active_verb_contracts:
      description: "Flattened active verb contract projections for runtime consumption"
      governance_tier: operational
      classification: internal
      pii: false
    sem_reg_pub.active_entity_types:
      description: "Flattened active entity type definition projections for runtime consumption"
      governance_tier: operational
      classification: internal
      pii: false
    sem_reg_pub.active_taxonomies:
      description: "Flattened active taxonomy definition projections for runtime consumption"
      governance_tier: operational
      classification: internal
      pii: false
    sem_reg_pub.projection_watermark:
      description: "Outbox dispatcher progress tracker — last processed sequence per projection"
      governance_tier: operational
      classification: internal
      pii: false
```

### 2B: Add `sem_reg_authoring` tables to `sem-reg` domain

```yaml
    sem_reg_authoring.validation_reports:
      description: "Append-only validation results per changeset (Stage 1 artifact + Stage 2 dry-run)"
      governance_tier: operational
      classification: internal
      pii: false
    sem_reg_authoring.governance_audit_log:
      description: "Permanent audit trail for all governance verbs (propose/validate/publish/rollback)"
      governance_tier: governed
      classification: internal
      pii: false
    sem_reg_authoring.publish_batches:
      description: "Atomic batch publish records with topologically-sorted changeset IDs"
      governance_tier: governed
      classification: internal
      pii: false
    sem_reg_authoring.change_set_artifacts:
      description: "Stored artifacts (SQL, YAML, JSON) associated with changesets"
      governance_tier: operational
      classification: internal
      pii: false
    sem_reg_authoring.change_sets_archive:
      description: "Archive for expired/orphan changesets (rejected >90d, orphan draft >30d)"
      governance_tier: operational
      classification: internal
      pii: false
    sem_reg_authoring.change_set_artifacts_archive:
      description: "Archive for artifacts belonging to archived changesets"
      governance_tier: operational
      classification: internal
      pii: false
```

### 2C: Add missing `sem_reg` tables

```yaml
    sem_reg.classification_levels:
      description: "Security classification reference data (Public, Internal, Confidential, Restricted)"
      governance_tier: governed
      classification: internal
      pii: false
    sem_reg.disambiguation_prompts:
      description: "Disambiguation questions with options for agent plan decision-making"
      governance_tier: operational
      classification: internal
      pii: false
```

**After this task:** All sem_reg/sem_reg_pub/sem_reg_authoring tables have metadata entries.

---

## Task 3: Add Verb Data Footprints — `changeset` Domain (14 verbs)

In the `sem-reg` domain `verb_data_footprint:` section, add all 14 changeset verbs.

**Rule:** if `side_effects: state_write` → has `writes`. If `side_effects: facts_only` → reads only.

```yaml
    verb_data_footprint:
      # === changeset domain (14 verbs) ===
      changeset.compose:
        reads: []
        writes: [sem_reg.changesets, stewardship.events]
      changeset.add-item:
        reads: [sem_reg.changesets]
        writes: [sem_reg.changeset_entries]
      changeset.remove-item:
        reads: [sem_reg.changesets, sem_reg.changeset_entries]
        writes: [sem_reg.changeset_entries]
      changeset.refine-item:
        reads: [sem_reg.changesets, sem_reg.changeset_entries]
        writes: [sem_reg.changeset_entries]
      changeset.suggest:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: []
      changeset.apply-template:
        reads: [sem_reg.changesets, stewardship.templates]
        writes: [sem_reg.changeset_entries]
      changeset.attach-basis:
        reads: [sem_reg.changesets, sem_reg.changeset_entries]
        writes: [stewardship.basis_records, stewardship.basis_claims]
      changeset.validate-edit:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: []
      changeset.cross-reference:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: []
      changeset.impact-analysis:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots, sem_reg.derivation_edges]
        writes: []
      changeset.resolve-conflict:
        reads: [stewardship.conflict_records]
        writes: [stewardship.conflict_records]
      changeset.list:
        reads: [sem_reg.changesets]
        writes: []
      changeset.get:
        reads: [sem_reg.changesets, sem_reg.changeset_entries]
        writes: []
      changeset.diff:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: []
```

---

## Task 4: Add Verb Data Footprints — `governance` Domain (9 verbs)

```yaml
      # === governance domain (9 verbs) ===
      governance.gate-precheck:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg_authoring.validation_reports]
        writes: []
      governance.submit-for-review:
        reads: [sem_reg.changesets]
        writes: [sem_reg.changesets, stewardship.events]
      governance.record-review:
        reads: [sem_reg.changesets]
        writes: [sem_reg.changesets, sem_reg.changeset_reviews, stewardship.events]
      governance.validate:
        reads: [sem_reg.changesets, sem_reg.changeset_entries]
        writes: [sem_reg.changesets, sem_reg_authoring.validation_reports, sem_reg_authoring.governance_audit_log]
      governance.dry-run:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: [sem_reg.changesets, sem_reg_authoring.validation_reports, sem_reg_authoring.governance_audit_log]
      governance.plan-publish:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots, sem_reg.snapshot_sets]
        writes: []
      governance.publish:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: [sem_reg.snapshots, sem_reg.snapshot_sets, sem_reg.changesets, sem_reg.outbox_events, sem_reg_authoring.publish_batches, sem_reg_authoring.governance_audit_log]
      governance.publish-batch:
        reads: [sem_reg.changesets, sem_reg.changeset_entries, sem_reg.snapshots]
        writes: [sem_reg.snapshots, sem_reg.snapshot_sets, sem_reg.changesets, sem_reg.outbox_events, sem_reg_authoring.publish_batches, sem_reg_authoring.governance_audit_log]
      governance.rollback:
        reads: [sem_reg.snapshot_sets, sem_reg.snapshots]
        writes: [sem_reg.snapshot_sets, sem_reg_authoring.governance_audit_log]
```

---

## Task 5: Add Verb Data Footprints — `focus` Domain (6 verbs)

```yaml
      # === focus domain (6 verbs) ===
      focus.get:
        reads: [stewardship.focus_states]
        writes: []
      focus.set:
        reads: []
        writes: [stewardship.focus_states]
      focus.render:
        reads: [stewardship.focus_states, sem_reg.snapshots]
        writes: []
      focus.viewport:
        reads: [stewardship.focus_states, stewardship.viewport_manifests]
        writes: []
      focus.diff:
        reads: [stewardship.focus_states, stewardship.viewport_manifests, sem_reg.snapshots]
        writes: []
      focus.capture-manifest:
        reads: [stewardship.focus_states]
        writes: [stewardship.viewport_manifests]
```

---

## Task 6: Add Verb Data Footprints — `audit` Domain (8 verbs)

```yaml
      # === audit domain (8 verbs) ===
      audit.create-plan:
        reads: []
        writes: [sem_reg.agent_plans]
      audit.add-plan-step:
        reads: [sem_reg.agent_plans]
        writes: [sem_reg.plan_steps]
      audit.validate-plan:
        reads: [sem_reg.agent_plans, sem_reg.plan_steps]
        writes: []
      audit.execute-plan-step:
        reads: [sem_reg.agent_plans, sem_reg.plan_steps]
        writes: [sem_reg.plan_steps, sem_reg.run_records]
      audit.record-decision:
        reads: [sem_reg.plan_steps]
        writes: [sem_reg.decision_records]
      audit.record-escalation:
        reads: [sem_reg.decision_records]
        writes: [sem_reg.escalation_records]
      audit.record-disambiguation:
        reads: [sem_reg.decision_records]
        writes: [sem_reg.disambiguation_prompts]
      audit.record-observation:
        reads: [sem_reg.snapshots]
        writes: [sem_reg.derivation_edges, sem_reg.run_records]
```

---

## Task 7: Add Verb Data Footprints — `registry` Domain (26 verbs)

All registry verbs are `facts_only` — read-only queries against the snapshot store.

```yaml
      # === registry domain (26 verbs) — all read-only ===
      registry.describe-object:
        reads: [sem_reg.snapshots]
        writes: []
      registry.search:
        reads: [sem_reg.snapshots, sem_reg.embedding_records]
        writes: []
      registry.list-objects:
        reads: [sem_reg.snapshots]
        writes: []
      registry.resolve-context:
        reads: [sem_reg.snapshots]
        writes: []
      registry.verb-surface:
        reads: [sem_reg.snapshots, sem_reg_pub.active_verb_contracts]
        writes: []
      registry.attribute-producers:
        reads: [sem_reg.snapshots]
        writes: []
      registry.lineage:
        reads: [sem_reg.snapshots, sem_reg.derivation_edges, sem_reg.run_records]
        writes: []
      registry.regulation-trace:
        reads: [sem_reg.snapshots]
        writes: []
      registry.taxonomy-tree:
        reads: [sem_reg.snapshots]
        writes: []
      registry.taxonomy-members:
        reads: [sem_reg.snapshots]
        writes: []
      registry.classify:
        reads: [sem_reg.snapshots]
        writes: []
      registry.describe-view:
        reads: [sem_reg.snapshots]
        writes: []
      registry.apply-view:
        reads: [sem_reg.snapshots]
        writes: []
      registry.describe-policy:
        reads: [sem_reg.snapshots]
        writes: []
      registry.coverage-report:
        reads: [sem_reg.snapshots]
        writes: []
      registry.evidence-freshness:
        reads: [sem_reg.snapshots]
        writes: []
      registry.evidence-gaps:
        reads: [sem_reg.snapshots]
        writes: []
      registry.snapshot-history:
        reads: [sem_reg.snapshots]
        writes: []
      registry.snapshot-diff:
        reads: [sem_reg.snapshots]
        writes: []
      registry.active-manifest:
        reads: [sem_reg.snapshot_sets, sem_reg.snapshots]
        writes: []
      registry.adjacent-verbs:
        reads: [sem_reg.snapshots]
        writes: []
      registry.data-for-verb:
        reads: [sem_reg.snapshots]
        writes: []
      registry.discover-dsl:
        reads: [sem_reg.snapshots, sem_reg_pub.active_verb_contracts]
        writes: []
      registry.governance-gaps:
        reads: [sem_reg.snapshots]
        writes: []
      registry.verbs-for-attribute:
        reads: [sem_reg.snapshots]
        writes: []
      registry.verbs-for-table:
        reads: [sem_reg.snapshots]
        writes: []
```

---

## Task 8: Add Verb Data Footprints — `schema` Domain (13 verbs)

All schema verbs are `facts_only` — read-only introspection against PostgreSQL `information_schema` and the snapshot store.

```yaml
      # === schema domain (13 verbs) — all read-only ===
      schema.introspect:
        reads: [sem_reg.snapshots]
        writes: []
      schema.domain.describe:
        reads: [sem_reg.snapshots]
        writes: []
      schema.entity.describe:
        reads: [sem_reg.snapshots]
        writes: []
      schema.entity.list-fields:
        reads: [sem_reg.snapshots]
        writes: []
      schema.entity.list-relationships:
        reads: [sem_reg.snapshots]
        writes: []
      schema.entity.list-verbs:
        reads: [sem_reg.snapshots, sem_reg_pub.active_verb_contracts]
        writes: []
      schema.extract-attributes:
        reads: [sem_reg.snapshots]
        writes: []
      schema.extract-entities:
        reads: [sem_reg.snapshots]
        writes: []
      schema.extract-verbs:
        reads: [sem_reg.snapshots]
        writes: []
      schema.cross-reference:
        reads: [sem_reg.snapshots]
        writes: []
      schema.generate-erd:
        reads: [sem_reg.snapshots]
        writes: []
      schema.generate-verb-flow:
        reads: [sem_reg.snapshots, sem_reg.derivation_edges]
        writes: []
      schema.generate-discovery-map:
        reads: [sem_reg.snapshots]
        writes: []
```

---

## Task 9: Add Verb Data Footprints — `maintenance` Domain (7 verbs)

```yaml
      # === maintenance domain (7 verbs) ===
      maintenance.health-pending:
        reads: [sem_reg.outbox_events, sem_reg_pub.projection_watermark]
        writes: []
      maintenance.health-stale-dryruns:
        reads: [sem_reg.changesets, sem_reg_authoring.validation_reports]
        writes: []
      maintenance.cleanup:
        reads: [sem_reg.changesets]
        writes: [sem_reg.changesets, sem_reg_authoring.change_sets_archive, sem_reg_authoring.change_set_artifacts_archive]
      maintenance.bootstrap-seeds:
        reads: [sem_reg.bootstrap_audit]
        writes: [sem_reg.snapshots, sem_reg.snapshot_sets, sem_reg.bootstrap_audit]
      maintenance.drain-outbox:
        reads: [sem_reg.outbox_events]
        writes: [sem_reg.outbox_events, sem_reg_pub.active_verb_contracts, sem_reg_pub.active_entity_types, sem_reg_pub.active_taxonomies, sem_reg_pub.projection_watermark]
      maintenance.reindex-embeddings:
        reads: [sem_reg.snapshots, sem_reg.embedding_records]
        writes: [sem_reg.embedding_records]
      maintenance.validate-schema-sync:
        reads: [sem_reg.snapshots, sem_reg_pub.active_verb_contracts]
        writes: []
```

---

## Verification

After all tasks are complete, run:

```bash
# 1. Build check
RUSTC_WRAPPER= cargo check -p ob-poc

# 2. Seed loader test
RUSTC_WRAPPER= cargo test -p ob-poc -- domain_metadata

# 3. Manual count verification
python3 -c "
import yaml
with open('rust/config/sem_os_seeds/domain_metadata.yaml') as f:
    meta = yaml.safe_load(f)
d = meta['domains']['sem-reg']
tables = len(d.get('tables', {}))
footprints = len(d.get('verb_data_footprint', {}))
# Also check stewardship if still separate
s = meta['domains'].get('stewardship', {})
s_tables = len(s.get('tables', {}))
s_footprints = len(s.get('verb_data_footprint', {}))
print(f'sem-reg: {tables} tables, {footprints} footprints')
print(f'stewardship: {s_tables} tables, {s_footprints} footprints')
print(f'total footprints: {footprints + s_footprints}')
print(f'Expected: 83 footprints, 0 orphans')
"
```

**Expected outcome:**
- 0 orphan footprints (14 deleted in Task 1)
- 83 verb footprints (Tasks 3–9)
- 12 new table entries (Tasks 2A-2C)
- Every verb FQN in `rust/config/verbs/sem-reg/*.yaml` has a matching footprint

---

## Summary

| Task | What | Entries |
|------|------|--------|
| 1 | Delete 14 orphan footprints | -14 |
| 2A | Add sem_reg_pub tables (4) | +4 tables |
| 2B | Add sem_reg_authoring tables (6) | +6 tables |
| 2C | Add missing sem_reg tables (2) | +2 tables |
| 3 | changeset footprints | +14 verbs |
| 4 | governance footprints | +9 verbs |
| 5 | focus footprints | +6 verbs |
| 6 | audit footprints | +8 verbs |
| 7 | registry footprints | +26 verbs |
| 8 | schema footprints | +13 verbs |
| 9 | maintenance footprints | +7 verbs |
| **Total** | | **+12 tables, +83 verb footprints, -14 orphans** |
