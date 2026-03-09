# OB-POC Schema ↔ DSL ↔ SemOS Reconciliation Audit

**Date:** 2026-03-08  
**Source:** `master-schema.sql` (DDL), `OB_POC_SCHEMA_ENTITY_OVERVIEW_refocused.md`, `OB_POC_SCHEMA_ENTITY_OVERVIEW_appendix.md`, `OB-POC_Data_Strategy`, GitHub repo `adamtc007/ob-poc` (verb YAMLs + SemOS seeds)

---

## Executive Summary

The ob-poc platform has grown to a substantial scale: **344 DDL tables** across 14 schemas, **1,123 DSL verbs** across 134 domains, and **229 tables** with SemOS metadata. The two overview docs are high quality and structurally accurate for the three load-bearing aggregates they cover — but they represent only the tip of a much larger schema iceberg. The main gaps are not errors but **coverage gaps** between what the docs describe, what the DDL contains, and what SemOS metadata governs.

| Layer | Count | Coverage vs DDL |
|-------|-------|-----------------|
| DDL tables (all schemas) | 344 | — |
| DDL tables (`ob-poc` only) | 281 | — |
| SemOS `domain_metadata` tables | 229 (126 ob-poc bare, 103 schema-prefixed) | 45% of ob-poc DDL |
| Verb YAML verbs (non-template) | 1,123 across 134 domains | — |
| Verbs with SemOS data footprints | 184 of 1,123 | **16% footprint coverage** |
| SemOS footprint verbs not in YAML | 50 orphans | — |

**Bottom line:** The overview docs are solid mental models. The DSL verb surface has grown far beyond what SemOS metadata tracks, creating a governance gap where 84% of verbs have no declared data footprint. The SemOS domain_metadata uses schema-prefixed table names (`kyc.cases`, `custody.cbu_ssi`) that don't match the actual DDL schema (`"ob-poc".cases`, `"ob-poc".cbu_ssi`), producing 103 phantom table entries.

---

## 1. Overview Doc Quality Assessment

### Refocused doc (3 taxonomies)

**Strengths:**
- The Deal Map → Onboarding Request → KYC/UBO narrative is clean and accurate. The three aggregates map directly to real DDL tables with correct PKs/FKs.
- Mermaid ERDs are correct. Every relationship shown (`deals ←→ deal_participants ←→ entities`, `deal_onboarding_requests → cases`, etc.) exists in the DDL with matching FK constraints.
- The "cross-taxonomy join points" table is accurate and implementable — all seven join paths are backed by real FK constraints.
- State machine semantics (deal lifecycle, onboarding status) match the DDL CHECK constraints.

**Weaknesses / gaps:**
- Covers roughly 25–30 tables out of 281 in `ob-poc`. Entire subsystems are explicitly excluded (custody trading profiles at 47 verbs, fund registry at 26 verbs, settlement chains, CSA agreements, ISDA, instrument universe).
- The `client` schema (client_types, client_classification, client_profile, clients, credentials) is absent — these are load-bearing for portal/auth but invisible in the overview.
- `investor_role_profiles` (the largest table definition in the DDL at ~3,400 lines) gets zero mention.

### Appendix doc (cross-cutting systems)

**Strengths:**
- Correctly maps Documents, SemReg, Agent, Runbooks/BPMN, and Events/Feedback back to the three taxonomies. The join keys section (Section G) is accurate.
- Good recognition that `ubo_evidence` and `kyc_ubo_evidence` are two overlapping evidence models.
- The SemReg object_type enumeration matches the DDL enum values.

**Weaknesses / gaps:**
- Claims `sem_reg.object_type` includes `verb_contract`, `taxonomy_def`, etc. — these are conceptual but the actual DDL stores `object_type` as a TEXT column on `sem_reg.snapshots`, not as a PostgreSQL enum. The appendix should note this isn't enforced at the DDL level.
- The `stewardship` schema (9 tables: basis_claims, basis_records, conflict_records, events, focus_states, idempotency_keys, templates, verb_implementation_bindings, viewport_manifests) is not mentioned despite being a fully-wired governance subsystem.
- The `teams` schema (7 tables including access_review_campaigns, access_attestations) is absent — relevant for CBU access control and delegation.
- `ob_kyc.entity_regulatory_registrations` and the entire `ob_ref` schema (regulators, request_types, role_types, standards_mappings, tollgate_definitions) get no coverage.

### Data Strategy doc

Accurate as a positioning document. The "three taxonomies as load-bearing aggregates" framing matches reality. The "Slices 1–4" delivery sequencing is pragmatic. No factual errors found.

---

## 2. DDL ↔ SemOS Domain Metadata Reconciliation

### The naming mismatch problem

SemOS `domain_metadata.yaml` uses **logical schema prefixes** (`kyc.cases`, `custody.cbu_ssi`, `feedback.failures`) that don't correspond to actual PostgreSQL schema names. In reality, most of these tables live in `"ob-poc".*`. This creates **103 phantom table entries** — tables that exist in both DDL and SemOS but can't be matched programmatically because the names differ.

Examples of the mismatch:

| SemOS metadata name | Actual DDL location |
|---------------------|-------------------|
| `kyc.cases` | `"ob-poc".cases` |
| `custody.cbu_ssi` | `"ob-poc".cbu_ssi` |
| `custody.cbu_settlement_chains` | `"ob-poc".cbu_settlement_chains` |
| `feedback.failures` | `feedback.failures` ✓ (this one matches) |
| `sem_reg.changesets` | `sem_reg.changesets` ✓ |

**Impact:** Any runtime code that does `SELECT * FROM {sem_os_table}` using the domain_metadata names will break for 70%+ of the schema-prefixed entries. The AffinityGraph and verb-data navigation rely on these names being resolvable.

**Recommendation:** Either (a) use the real DDL schema.table names in domain_metadata, or (b) add a `ddl_schema` field to each domain that maps the logical prefix to the physical schema.

### Tables missing from SemOS metadata entirely

155 `ob-poc` tables have no SemOS metadata coverage at all. Key functional groups:

- **Attribute system** (3 tables): `attribute_observations`, `attribute_registry`, `attribute_values_typed` — core to the evidence/extraction pipeline documented in the appendix but ungoverned by SemOS
- **KYC core** (8+ tables): `cases`, `entity_workstreams`, `doc_requests`, `tollgate_evaluations`, `case_events`, `case_evaluation_snapshots`, `case_import_runs`, `case_types` — the entire KYC case system is missing from SemOS
- **Custody/settlement** (~15 tables): `cbu_ssi`, `cbu_settlement_chains`, `settlement_chain_hops`, `csa_agreements`, `isda_agreements`, etc.
- **Fund/share registry** (~10 tables): `fund_compartments`, `share_classes`, `share_class_supply`, `share_class_identifiers`, `investor_role_profiles`
- **Client/portal** (~8 tables): `clients`, `client_types`, `client_classification`, `client_portal_sessions`, `credentials`
- **DSL/session** (~8 tables): `dsl_ob`, `dsl_session_locks`, `dsl_snapshots`, `dsl_verb_categories`

### Tables in SemOS but not in DDL (non-phantom)

After accounting for the schema prefix mismatch, a few genuine phantoms may exist where SemOS references tables that were planned but never created (e.g., `client_portal.escalations`). These need a manual audit pass.

---

## 3. DSL Verb ↔ Data Coverage

### Scale of the verb surface

| Metric | Count |
|--------|-------|
| Total verb YAML domains | 134 |
| Total verb definitions | 1,123 |
| Verb template files (compound verbs) | 14 |
| Domains with 20+ verbs | 7 (trading-profile:47, deal:42, registry:26, ubo:25, client-group:23, entity:21, fund:20) |

### Verb table lookup references

54 tables are referenced in verb argument `lookup` blocks. These are the tables verbs resolve entities against at runtime. Key observations:

- Core entity tables are well-covered: `entities`, `cbus`, `deals`, `client_group`, `cases`, `products`, `services`
- Missing lookups: No verb references `legal_contracts` directly in a lookup (they reference `deals` instead). The `contract_products` table — the "enforcement edge" — has no verb lookup path.
- Reference data gaps: `currencies`, `risk_bands`, `sla_templates` are in DDL but not in any verb lookup.

### Verb-to-SemOS footprint coverage: the 84% gap

Only **184 of 1,123 verbs** (16%) have a declared `verb_data_footprint` in SemOS domain_metadata. The remaining 939 verbs execute without SemOS knowing which tables they read or write.

Domains with ZERO footprint coverage (all verbs ungoverned):

- `admin.regulators` (5 verbs), `admin.role-types` (5 verbs)
- `attribute` (11 verbs) — touches `attribute_registry`, `attribute_values_typed`, `attribute_observations`
- `batch` (4 verbs), `bods` (8 verbs)
- `capital` (7 verbs), `cash-sweep` (7 verbs)
- `delivery` (8 verbs), `delegation` (5 verbs)
- All `kyc/*` sub-domains (~80+ verbs): `kyc-case`, `entity-workstream`, `doc-request`, `evidence`, `tollgate`, `board`, `partnership`, `trust`, `red-flag`, `skeleton-build`, `coverage`, `case-event`, `case-screening`, `request`
- All `refdata/*` domains (~30+ verbs)
- All `reference/*` domains (~15+ verbs)
- All `research/*` domains (~35+ verbs)
- `onboarding` (8 verbs), `product` (6 verbs)
- `screening` (6 verbs), `verification` (16 verbs)
- All `observation/*` domains (15+ verbs)

**Impact:** The AffinityGraph can't build verb→table edges for 84% of the verb surface. This means diagram generation, DSL discovery ("which verbs touch this table?"), and governance ("which verbs can modify this CDE?") are all incomplete.

### Orphan footprints (50)

50 verb names appear in SemOS `verb_data_footprint` but have no corresponding verb YAML definition. Examples:

- `client-group.add-entity`, `client-group.add-alias`, `client-group.add-anchor` — the footprints exist but the actual verb definitions use different naming (possibly composite keys like `client-group.create` with a subcommand)
- `stew.approve-changeset`, `stew.compose-changeset`, `stew.publish-changeset`, `stew.submit-for-review`, `stew.resolve-conflict`, `stew.set-focus`, `stew.show`, `stew.attach-basis` — the stewardship verbs are declared in footprints but not in YAML verb files
- `dsl.execute`, `dsl.generate`, `dsl.validate` — footprints exist for DSL meta-verbs with no YAML
- `deal.counter-offer` — footprint present but verb YAML doesn't define this specific name

**These orphans indicate either renamed verbs or planned verbs that haven't been YAML-defined yet.**

---

## 4. Cross-Layer Alignment Issues

### Issue A: Schema namespace divergence

The platform has three "truths" about table namespacing that don't agree:

1. **DDL:** Most tables are in `"ob-poc"` schema, with separate `sem_reg`, `agent`, `feedback`, `events`, `stewardship`, `teams`, `ob_kyc`, `ob_ref` schemas.
2. **SemOS domain_metadata:** Uses logical domain prefixes (`kyc.`, `custody.`, `client-portal.`) that don't map to DDL schemas.
3. **Overview docs:** Use `"ob-poc".*` notation, matching DDL.

This means the overview docs and DDL agree, but SemOS is the odd one out.

### Issue B: KYC verb surface vs data coverage

KYC is one of the three "load-bearing aggregates" but has the worst coverage:
- 80+ KYC-related verbs across `kyc-case`, `entity-workstream`, `doc-request`, `evidence`, `tollgate`, `board`, `ubo-registry`, etc.
- ZERO of these have SemOS data footprints
- KYC tables (`cases`, `entity_workstreams`, `doc_requests`, etc.) are absent from SemOS domain_metadata entirely
- The overview docs describe KYC tables accurately but SemOS can't govern them

### Issue C: Stewardship schema is invisible

The `stewardship` schema has 9 tables and 8 SemOS footprints (via `stew.*` verbs) but:
- No verb YAML files exist for stewardship
- Not mentioned in either overview doc
- The `stewardship.verb_implementation_bindings` table suggests a verb↔implementation binding system that isn't documented anywhere

### Issue D: Trading profile scale vs governance

`trading-profile` is the largest verb domain at 47 verbs, touching only 3 tables in SemOS metadata. This is a 15:1 verb-to-table ratio suggesting either very fine-grained verbs or missing table references. The `trading_profile_materializations`, `cbu_trading_profiles`, and related tables in DDL should be fully footprinted.

---

## 5. Recommendations (Priority Order)

**P0 — Fix the schema naming mismatch** in `domain_metadata.yaml`. Every `kyc.`, `custody.`, `client-portal.` prefixed entry needs to resolve to its actual DDL schema. This is blocking AffinityGraph correctness.

**P1 — Add KYC tables to SemOS metadata.** The entire cases/workstreams/doc_requests/tollgates/UBO cluster needs domain_metadata entries with governance tiers and verb data footprints. This is the most glaring functional gap.

**P2 — Backfill verb data footprints** for at least the high-impact domains: `kyc/*` (~80 verbs), `onboarding` (8), `deal` (42 — only 18 have footprints), `screening` (6), `document` (13).

**P3 — Reconcile orphan footprints.** The 50 orphans suggest either verb renames or planned expansions. Clean them up or create the missing YAML definitions.

**P4 — Add stewardship, teams, and ob_ref** to the overview docs as a "governance infrastructure" section. These are load-bearing for the platform's own operations.

**P5 — Add the attribute system** (`attribute_registry`, `attribute_values_typed`, `attribute_observations`) to SemOS metadata. The appendix doc describes this as the evidence extraction pipeline but SemOS can't see it.

---

## Appendix: Schema Table Inventory

### By schema

| Schema | Tables |
|--------|--------|
| `ob-poc` | 281 |
| `sem_reg` | 16 |
| `stewardship` | 9 |
| `agent` | 8 |
| `teams` | 7 |
| `sem_reg_authoring` | 6 |
| `ob_ref` | 5 |
| `sem_reg_pub` | 4 |
| `feedback` | 3 |
| `sessions` | 1 |
| `events` | 1 |
| `ob_kyc` | 1 |
| `public` | 1 |
| `_sqlx_test` | 1 |
| **Total** | **344** |

### SemOS domain_metadata coverage by domain

| Domain | Tables | Footprints | Gap |
|--------|--------|------------|-----|
| custody | 27 | 8 | 19 tables, many verbs uncovered |
| kyc | 19 | 9 | All tables are phantom (wrong schema prefix) |
| entity | 18 | 14 | Reasonable |
| sem-reg | 14 | 7 | All tables are phantom (schema-prefixed) |
| cbu | 13 | 18 | Good ratio |
| deal | 11 | 18 | Good |
| booking-principal | 11 | 9 | Good |
| client-group | 10 | 8 | Good |
| agent | 10 | 2 | 8 footprint gap |
| ownership | 10 | 6 | Moderate |
| stewardship | 9 | 8 | Good but no YAML verbs |
| fund | 8 | 6 | Good |
| team | 7 | 5 | Good |
| dsl | 7 | 3 | Gap |
| product | 6 | 3 | Gap |
| client-portal | 6 | 0 | Total gap |
| contract | 5 | 10 | Overserved by footprints |
| investor | 4 | 5 | Good |
| document | 4 | 3 | Moderate |
| bpmn | 4 | 4 | Complete |
| billing | 4 | 8 | Good |
| screening | 4 | 2 | Gap |
| session | 4 | 10 | Good |
| trading-profile | 3 | 14 | Fine-grained verbs |
| reference | 10 | 0 | Total gap |
| feedback | 3 | 0 | Total gap |
| gleif | 2 | 4 | Good |
