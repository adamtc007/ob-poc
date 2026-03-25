# P3-B: Service, Trading & Instrument Schema Review

**Reviewer:** Claude Opus 4.6
**Date:** 2026-03-16
**Scope:** Service layer, trading/instrument tables, settlement/SSI, booking principals, deal lifecycle, fee billing
**Source of truth:** `migrations/master-schema.sql` + numbered migrations (020, 024-027, 045, 067-069, 072)

---

## Table of Contents

1. [Per-Table Scorecard](#per-table-scorecard)
2. [Cross-Table Consistency Findings](#cross-table-consistency-findings)
3. [Severity-Tagged Issue List](#severity-tagged-issue-list)
4. [Index Recommendation List](#index-recommendation-list)

---

## Per-Table Scorecard

Scoring: PASS / WARN / FAIL per dimension.

### 1. Service Layer Tables

#### service_intents (024)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default on `intent_id` |
| FK Integrity | PASS | `cbu_id` FK to cbus (CASCADE), `product_id` FK to products, `service_id` FK to services |
| Index Coverage | PASS | Composite `(cbu_id, product_id, service_id)` + status partial index |
| Constraints | PASS | UNIQUE on `(cbu_id, product_id, service_id)`, status CHECK |
| Temporal | PASS | `created_at`/`updated_at` with defaults |
| Naming | PASS | Consistent snake_case |

#### srdef_discovery_reasons (024)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FKs to service_intents, products, services |
| Index Coverage | PASS | Composite + partial indexes |
| Constraints | PASS | UNIQUE constraint, CHECK on reason_type |
| Temporal | PASS | `created_at`/`updated_at` present |
| Naming | PASS | Consistent |

#### cbu_service_readiness (027)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | Composite PK `(cbu_id, product_id, service_id)` -- appropriate for derived table |
| FK Integrity | PASS | FKs to cbus (CASCADE), products, services |
| Index Coverage | PASS | Status indexes, partial indexes for blocked/ready states |
| Constraints | PASS | Status CHECK (ready/blocked/partial), JSONB defaults |
| Temporal | PASS | `as_of`, `last_recomputed_at` NOT NULL with defaults; `is_stale` column with invalidation trigger |
| Naming | PASS | Consistent |

#### cbu_unified_attr_requirements (025)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | Composite PK `(cbu_id, attr_id)` -- appropriate for derived table |
| FK Integrity | PASS | FKs to cbus (CASCADE) and attributes |
| Index Coverage | PASS | Strength index present |
| Constraints | PASS | CHECK on requirement_strength (required/recommended/optional) |
| Temporal | PASS | `computed_at` NOT NULL |
| Naming | PASS | Consistent |

#### cbu_attr_values (025)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | Composite PK `(cbu_id, attr_id)` |
| FK Integrity | PASS | FK to cbus (CASCADE) |
| Index Coverage | PASS | Source index present |
| Constraints | PASS | CHECK on value_source |
| Temporal | PASS | `collected_at`, `updated_at` present |
| Naming | PASS | Consistent |

#### provisioning_requests (026)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FKs to cbus (CASCADE), services |
| Index Coverage | PASS | Status + cbu composite indexes |
| Constraints | PASS | Status CHECK, idempotency_key UNIQUE, **immutability trigger** on UPDATE/DELETE |
| Temporal | PASS | `created_at` NOT NULL, append-only enforced |
| Naming | PASS | Consistent |

#### provisioning_events (026)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to provisioning_requests (CASCADE) |
| Index Coverage | PASS | Request + event_type composite index |
| Constraints | PASS | event_type CHECK |
| Temporal | PASS | `occurred_at` NOT NULL |
| Naming | PASS | Consistent |

### 2. Trading & Instrument Tables

#### cbu_trading_profiles (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default on `profile_id` |
| FK Integrity | PASS | `cbu_id` FK to cbus |
| Index Coverage | PASS | `idx_cbu_trading_profiles_cbu` on cbu_id, partial index `idx_cbu_trading_profiles_active` WHERE status = 'ACTIVE' |
| Constraints | PASS | Status CHECK (DRAFT/VALIDATED/PENDING_REVIEW/ACTIVE/SUPERSEDED/ARCHIVED), `document_hash` for change detection |
| Temporal | PASS | `created_at` NOT NULL with default, `materialized_at`, `validated_at` |
| Naming | PASS | Consistent |

#### trading_profile_materializations (020)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | WARN | `gen_random_uuid()` -- should be `uuidv7()` for time-ordered audit trail |
| FK Integrity | PASS | FK to cbu_trading_profiles |
| Index Coverage | PASS | Profile-level index present |
| Constraints | PASS | JSONB for materialized data |
| Temporal | WARN | Has `materialized_at` but no standard `created_at`/`updated_at` pair |
| Naming | PASS | Consistent |

#### instrument_classes (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | Self-referential `parent_class_id` FK for taxonomy hierarchy |
| Index Coverage | WARN | No explicit indexes visible beyond PK; taxonomy lookups by parent_class_id may need index |
| Constraints | WARN | No NOT NULL on `created_at`/`updated_at`; CFI/SMPG/ISDA classification columns are nullable (appropriate for reference data) |
| Temporal | FAIL | `created_at`/`updated_at` lack NOT NULL despite having DEFAULT NOW() |
| Naming | PASS | Consistent |

#### instrument_lifecycles (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FKs to instrument_classes and lifecycle events |
| Index Coverage | WARN | Likely needs composite index on `(instrument_class_id, lifecycle_event)` |
| Constraints | WARN | No status CHECK despite being a junction table with lifecycle semantics |
| Temporal | FAIL | `created_at` lacks NOT NULL; no `updated_at` column at all |
| Naming | PASS | Consistent |

### 3. SSI & Settlement Tables

#### cbu_ssi (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FKs to cbus, markets |
| Index Coverage | PASS | Indexes on cbu_id, market_id; partial index on active SSIs |
| Constraints | FAIL | **No status CHECK constraint** despite having a `status` column; 12+ account fields are all nullable (intentional for sparse SSIs) |
| Temporal | FAIL | `created_at`/`updated_at` both lack NOT NULL |
| Naming | PASS | Consistent |

#### cbu_ssi_agent_override (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to cbu_ssi (CASCADE delete) |
| Index Coverage | WARN | May need index on `ssi_id` for override lookups |
| Constraints | WARN | Sequence ordering field present but no UNIQUE constraint on `(ssi_id, sequence)` |
| Temporal | FAIL | `created_at` lacks NOT NULL |
| Naming | PASS | Consistent |

#### ssi_booking_rules (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to cbu_ssi (CASCADE) |
| Index Coverage | PASS | Multi-column composite index on `(ssi_id, instrument_class, security_type, market, currency, counterparty_id)` + `specificity_score DESC` for rule matching |
| Constraints | PASS | **Excellent design**: `specificity_score` is `GENERATED ALWAYS AS` stored column using bit-weighted scoring from nullable filter columns |
| Temporal | FAIL | `created_at`/`updated_at` both lack NOT NULL |
| Naming | PASS | Consistent |

#### settlement_chain_hops (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to settlement_chains (CASCADE) |
| Index Coverage | PASS | Chain_id + sequence composite index |
| Constraints | PASS | Role CHECK (CUSTODIAN/SUBCUSTODIAN/AGENT/CSD/ICSD), NOT NULL on timestamps |
| Temporal | PASS | `created_at`/`updated_at` both NOT NULL |
| Naming | PASS | Consistent |

#### settlement_locations (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | Appropriate FKs present |
| Index Coverage | PASS | Location type and code indexes |
| Constraints | PASS | location_type CHECK (CSD/ICSD/CUSTODIAN), JSONB for operating_hours/cycles |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### settlement_types / ssi_types (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | VARCHAR PK (`code`) -- appropriate for reference/lookup tables |
| FK Integrity | N/A | Reference data, no outbound FKs |
| Index Coverage | PASS | PK is the lookup key |
| Constraints | PASS | Minimal columns, appropriate for reference data |
| Temporal | WARN | No timestamps at all (acceptable for static reference data, but seed changes would be untracked) |
| Naming | PASS | Consistent |

#### entity_settlement_identity (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to entities |
| Index Coverage | WARN | May need index on `(entity_id, identity_type)` for lookups |
| Constraints | WARN | No CHECK on identity_type (BIC/LEI/ALERT/CTM are documented but not constrained) |
| Temporal | FAIL | `created_at`/`updated_at` lack NOT NULL |
| Naming | PASS | Consistent |

#### entity_ssi (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to entities |
| Index Coverage | WARN | Multi-dimensional matching columns (instrument_class, security_type, market, currency) likely need composite index |
| Constraints | WARN | No UNIQUE constraint preventing duplicate SSI entries for same entity/dimensions |
| Temporal | FAIL | `created_at`/`updated_at` lack NOT NULL |
| Naming | PASS | Consistent |

#### cbu_settlement_chains (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to cbus |
| Index Coverage | PASS | Multi-dimensional matching columns indexed |
| Constraints | PASS | Appropriate nullable dimensions for cascading specificity |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### cbu_settlement_location_preferences (master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` default |
| FK Integrity | PASS | FK to cbus, settlement_locations |
| Index Coverage | PASS | CBU + priority index |
| Constraints | PASS | Priority ordering, UNIQUE on `(cbu_id, location_id)` |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

### 4. Legal Contract Tables

#### legal_contracts (045)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | WARN | `gen_random_uuid()` -- should be `uuidv7()` |
| FK Integrity | WARN | `client_label` is NOT a FK to any table -- semantic join key only, no referential enforcement |
| Index Coverage | PASS | Indexes on client_label and status |
| Constraints | PASS | Status CHECK (DRAFT/ACTIVE/TERMINATED/EXPIRED) |
| Temporal | WARN | DEFAULT NOW() on timestamps but no NOT NULL enforcement |
| Naming | PASS | Consistent |

#### contract_products (045)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | Composite PK `(contract_id, product_code)` -- appropriate for junction table |
| FK Integrity | PASS | FK to legal_contracts (CASCADE) |
| Index Coverage | PASS | product_code index |
| Constraints | PASS | Composite PK prevents duplicates |
| Temporal | WARN | DEFAULT NOW() but no NOT NULL; no `updated_at` trigger |
| Naming | PASS | Consistent |

#### rate_cards (045)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | WARN | `gen_random_uuid()` -- should be `uuidv7()` |
| FK Integrity | PASS | Referenced by contract_products |
| Index Coverage | WARN | No explicit indexes beyond PK |
| Constraints | WARN | No status column; `currency` defaults to USD but no CHECK |
| Temporal | WARN | DEFAULT NOW() but no NOT NULL |
| Naming | PASS | Consistent |

#### cbu_subscriptions (045)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | Composite PK `(cbu_id, contract_id, product_code)` |
| FK Integrity | PASS | FK to cbus (CASCADE), **composite FK** to `contract_products(contract_id, product_code)` (CASCADE) -- enforces onboarding gate |
| Index Coverage | PASS | Contract+product composite index |
| Constraints | PASS | Status CHECK (PENDING/ACTIVE/SUSPENDED/TERMINATED) |
| Temporal | WARN | DEFAULT NOW() but no NOT NULL |
| Naming | PASS | Consistent |

### 5. Deal Lifecycle Tables

#### deals (067, final form in master-schema.sql)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` (upgraded in 068) |
| FK Integrity | PASS | `primary_client_group_id` FK to client_group |
| Index Coverage | PASS | Status, client_group, sales_owner, reference indexes |
| Constraints | PASS | Status CHECK with 9 values (068); deal_reference present |
| Temporal | FAIL | `created_at`/`updated_at` lack NOT NULL; **no `updated_at` trigger** |
| Naming | PASS | Consistent |

#### deal_participants (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` (upgraded in 068) |
| FK Integrity | PASS | FKs to deals (CASCADE) and entities |
| Index Coverage | PASS | Deal + entity composite index |
| Constraints | PASS | UNIQUE on `(deal_id, entity_id, role)`, role CHECK |
| Temporal | FAIL | Timestamps lack NOT NULL; no `updated_at` trigger |
| Naming | PASS | Consistent |

#### deal_contracts (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | Composite PK `(deal_id, contract_id)` |
| FK Integrity | PASS | FKs to deals (CASCADE) and legal_contracts |
| Index Coverage | PASS | Contract_id index |
| Constraints | PASS | Composite PK prevents duplicates |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### deal_rate_cards (067 + 068 + 069)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FKs to deals (CASCADE), legal_contracts, products; self-referential `superseded_by` FK |
| Index Coverage | PASS | `idx_deal_rate_cards_one_agreed` partial unique index (only one AGREED per deal/contract/product); deal+status composite |
| Constraints | PASS | Status CHECK (DRAFT/PROPOSED/COUNTER_PROPOSED/AGREED/SUPERSEDED/CANCELLED); **supersession trigger** `trg_validate_rate_card_supersession` enforces chain integrity |
| Temporal | FAIL | No NOT NULL on timestamps; no `updated_at` trigger |
| Naming | WARN | Status value `COUNTER_PROPOSED` in CHECK (068) vs `COUNTER_OFFERED` in 067 COMMENT -- **terminology mismatch** |

#### deal_rate_card_lines (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to deal_rate_cards (CASCADE) |
| Index Coverage | PASS | Rate_card_id + fee_type composite index |
| Constraints | PASS | **Excellent**: 3 domain-specific CHECKs -- BPS requires rate_value + basis_points_on, PER_TRANSACTION requires rate_value, TIERED requires tier_brackets JSONB |
| Temporal | FAIL | `created_at` lacks NOT NULL; **no `updated_at` column at all** |
| Naming | PASS | Consistent |

#### deal_products (069)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FKs to deals (CASCADE) and products |
| Index Coverage | PASS | Indexes on deal_id, product_id, product_status |
| Constraints | PASS | UNIQUE on `(deal_id, product_id)`, status CHECK (PROPOSED/NEGOTIATING/AGREED/DECLINED/REMOVED) |
| Temporal | WARN | `created_at`/`updated_at` have defaults; `added_at` and `agreed_at` are domain timestamps |
| Naming | PASS | Consistent |

#### deal_slas (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to deals (CASCADE) |
| Index Coverage | PASS | deal_id index |
| Constraints | WARN | No CHECK on sla_type or metric_type |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### deal_documents (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to deals (CASCADE) |
| Index Coverage | PASS | deal_id + status composite index |
| Constraints | PASS | Status CHECK (068): DRAFT/UNDER_REVIEW/SIGNED/EXECUTED/SUPERSEDED/ARCHIVED |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### deal_ubo_assessments (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to deals (CASCADE) |
| Index Coverage | WARN | deal_id index; may need index on kyc_case_id for cross-referencing |
| Constraints | WARN | No status CHECK despite having assessment_status |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### deal_onboarding_requests (067 + 068)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to deals (CASCADE) |
| Index Coverage | PASS | deal_id + status composite index |
| Constraints | PASS | Status CHECK (068): PENDING/IN_PROGRESS/BLOCKED/COMPLETED/CANCELLED |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### deal_events (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to deals (CASCADE) |
| Index Coverage | PASS | Composite `(deal_id, event_type, occurred_at)` for time-series queries |
| Constraints | WARN | No CHECK on event_type |
| Temporal | PASS | `occurred_at` present (audit table -- `created_at` is the event time) |
| Naming | PASS | Consistent |

### 6. Fee Billing Tables

#### fee_billing_profiles (067 + 068)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | WARN | FK to deals -- **no CASCADE** (default RESTRICT). Intentional to prevent accidental deal deletion from orphaning billing data, but inconsistent with child deal_* tables which all CASCADE |
| Index Coverage | PASS | deal_id + status composite index |
| Constraints | PASS | Status CHECK (068): DRAFT/ACTIVE/SUSPENDED/CLOSED |
| Temporal | FAIL | No NOT NULL on timestamps; no `updated_at` trigger |
| Naming | PASS | Consistent |

#### fee_billing_account_targets (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to fee_billing_profiles (CASCADE) and cbus |
| Index Coverage | PASS | Profile + CBU composite index |
| Constraints | PASS | UNIQUE on `(profile_id, cbu_id)` prevents duplicate targets |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### fee_billing_periods (067 + 068)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to fee_billing_profiles (CASCADE) |
| Index Coverage | PASS | Profile + period_start composite index |
| Constraints | PASS | calc_status CHECK (068): PENDING/CALCULATED/REVIEWED/APPROVED/DISPUTED/INVOICED |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### fee_billing_period_lines (067)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to fee_billing_periods (CASCADE) |
| Index Coverage | PASS | Period_id index |
| Constraints | WARN | No CHECK on fee_type or line_status |
| Temporal | FAIL | No NOT NULL on timestamps |
| Naming | PASS | Consistent |

### 7. Booking Principal Tables (072)

**Overall assessment:** The 072 migration is the **gold standard** in this schema. Consistent `uuidv7()`, NOT NULL timestamps, CHECK constraints, temporal exclusion constraints via `btree_gist`, and thorough index coverage.

#### legal_entities (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | Appropriate FKs |
| Index Coverage | PASS | Entity type, jurisdiction, name indexes |
| Constraints | PASS | entity_type CHECK, NOT NULL on key fields |
| Temporal | PASS | NOT NULL on `created_at`/`updated_at` |
| Naming | PASS | Consistent |

#### booking_locations (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to legal_entities |
| Index Coverage | PASS | Legal entity, country, active status indexes |
| Constraints | PASS | Country CHECK, status CHECK, NOT NULL |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### booking_principals (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FKs to legal_entities and booking_locations |
| Index Coverage | PASS | Legal entity, location, status, type indexes |
| Constraints | PASS | Status CHECK, type CHECK, NOT NULL on key fields |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### service_availability (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to booking_principals |
| Index Coverage | PASS | Principal + service + lane composite |
| Constraints | PASS | **Temporal exclusion** via `btree_gist` EXCLUDE USING gist -- prevents overlapping availability windows; lane CHECK (regulatory/commercial/operational); status CHECK |
| Temporal | PASS | NOT NULL on timestamps; `effective_from`/`effective_to` for validity ranges |
| Naming | PASS | Consistent |

#### client_principal_relationships (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FKs to cbus and booking_principals |
| Index Coverage | PASS | CBU + principal composite, status index |
| Constraints | PASS | Status CHECK, relationship_type CHECK |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### rulesets (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | Standalone, referenced by rules |
| Index Coverage | PASS | Status, name indexes |
| Constraints | PASS | Status CHECK (draft/published/retired), version NOT NULL |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### rules (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FK to rulesets (CASCADE) |
| Index Coverage | PASS | Ruleset + priority composite |
| Constraints | PASS | Priority ordering, JSONB conditions with defaults |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### rule_fields (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | Standalone reference data |
| Index Coverage | PASS | Field_name UNIQUE, data_type index |
| Constraints | PASS | data_type CHECK, UNIQUE field_name |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

#### contract_packs (072)

| Dimension | Score | Notes |
|-----------|-------|-------|
| PK Strategy | PASS | `uuidv7()` |
| FK Integrity | PASS | FKs to booking_principals and legal_entities |
| Index Coverage | PASS | Principal, entity indexes |
| Constraints | PASS | Status CHECK, pack_type CHECK |
| Temporal | PASS | NOT NULL on timestamps |
| Naming | PASS | Consistent |

---

## Cross-Table Consistency Findings

### F1: UUID Strategy Inconsistency

**Tables using `gen_random_uuid()`** (v4, non-time-ordered):
- `legal_contracts` (045)
- `rate_cards` (045)
- `trading_profile_materializations` (020)

**All other tables use `uuidv7()`** (time-ordered). The 045 and 020 tables predate the 068 migration that standardized on `uuidv7()`.

**Impact:** v4 UUIDs are fine for uniqueness but lose temporal ordering benefits for B-tree index locality and debugging. Low-priority but worth aligning.

### F2: NOT NULL Timestamp Discipline

Two distinct patterns observed:

| Migration Group | `created_at` NOT NULL | `updated_at` NOT NULL | `updated_at` trigger |
|-----------------|:---:|:---:|:---:|
| Service layer (024-027) | YES | YES | YES (readiness invalidation) |
| Legal contracts (045) | NO | NO | NO |
| Deal lifecycle (067) | NO | NO | NO |
| Fee billing (067) | NO | NO | NO |
| Booking principals (072) | YES | YES | NO (but NOT NULL enforced) |

The 067/045 tables have `DEFAULT NOW()` but allow NULL. This means a manual INSERT with an explicit `NULL` bypasses the default. Given these are application-managed (not user-facing), the risk is low but creates a schema hygiene gap.

### F3: CASCADE Delete Strategy

All `deal_*` child tables CASCADE from `deals`. However:
- `fee_billing_profiles` → `deals` uses **default RESTRICT** (no CASCADE specified)
- This is likely intentional -- preventing accidental deal deletion that would orphan billing records with financial audit implications
- But it creates an operational asymmetry: deleting a deal cascades to participants, documents, events, etc., but fails if billing profiles exist

**Recommendation:** If intentional, add a comment. If not, align with CASCADE.

### F4: Status Terminology Mismatch

In `deal_rate_cards`:
- Migration 067 COMMENT says: `COUNTER_OFFERED`
- Migration 068 CHECK constraint says: `COUNTER_PROPOSED`
- The CHECK constraint is authoritative (runtime enforcement), but the COMMENT creates confusion

### F5: Missing Status CHECKs

Tables with `status` columns but **no CHECK constraint**:
- `cbu_ssi.status` -- operational SSI status
- `deal_ubo_assessments.assessment_status`
- `fee_billing_period_lines` (no status/type CHECK)
- `deal_slas` (no sla_type or metric_type CHECK)
- `deal_events.event_type` (audit table -- CHECK may be intentionally omitted for extensibility)

### F6: Duplicate Index

Found in master-schema.sql index definitions:
- `idx_materializations_profile` on `trading_profile_materializations(profile_id)`
- `idx_materializations_profile_id` on `trading_profile_materializations(profile_id)`

Both index the same single column. One should be dropped.

### F7: `updated_at` Auto-Update Triggers

The service layer tables have staleness invalidation triggers, and the provisioning layer has immutability triggers, but **none of the deal/billing/contract tables have `updated_at` auto-update triggers**. Application code must manually set `updated_at = NOW()` on every UPDATE.

Booking principal tables (072) enforce NOT NULL on `updated_at` but also lack triggers -- they rely on application discipline.

### F8: Cross-Domain FK Coherence

The schema has clean FK chains across domains:

```
deals → deal_rate_cards → deal_rate_card_lines
     → deal_contracts → legal_contracts → contract_products
     → deal_products → products
     → deal_onboarding_requests → (links to existing CBU onboarding)
     → fee_billing_profiles → fee_billing_account_targets → cbus
                             → fee_billing_periods → fee_billing_period_lines
```

The booking principal domain has its own FK chain:
```
legal_entities → booking_locations → booking_principals
                                   → service_availability
                                   → client_principal_relationships → cbus
```

No cross-domain FK integrity issues found. The `client_label` join key in `legal_contracts` is the only semantic (non-FK-enforced) join.

### F9: Computed Column Pattern

`ssi_booking_rules.specificity_score` uses `GENERATED ALWAYS AS` with a bit-weighted expression computing specificity from nullable filter columns. This is a well-designed pattern for rule priority ordering. No other tables use generated columns, but the deal rate card precedence uses a partial unique index instead -- also appropriate.

### F10: Temporal Constraint Model

Only the booking principal domain (072) uses `btree_gist` temporal exclusion constraints for overlapping date ranges. The deal/billing tables use simple date columns (`effective_from`, `effective_to`) without overlap protection. For rate cards, the partial unique index + supersession trigger provide equivalent protection, but billing periods could benefit from overlap exclusion.

---

## Severity-Tagged Issue List

### Critical

None identified. The schema is structurally sound with no referential integrity gaps that could cause data corruption.

### High

| ID | Issue | Tables | Recommendation |
|----|-------|--------|----------------|
| H-1 | Missing status CHECK on `cbu_ssi` | `cbu_ssi` | Add CHECK constraint for SSI status values (ACTIVE/INACTIVE/PENDING/SUSPENDED or as appropriate) |
| H-2 | `deal_rate_card_lines` has no `updated_at` column | `deal_rate_card_lines` | Add `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()` column |
| H-3 | Status terminology mismatch | `deal_rate_cards` | Align 067 COMMENT to say COUNTER_PROPOSED (matching 068 CHECK) |

### Medium

| ID | Issue | Tables | Recommendation |
|----|-------|--------|----------------|
| M-1 | `created_at`/`updated_at` allow NULL | deals, deal_participants, deal_contracts, deal_rate_cards, deal_slas, deal_documents, deal_ubo_assessments, deal_onboarding_requests, fee_billing_profiles, fee_billing_account_targets, fee_billing_periods, fee_billing_period_lines, legal_contracts, contract_products, rate_cards, cbu_subscriptions, cbu_ssi, cbu_ssi_agent_override, ssi_booking_rules, entity_settlement_identity, entity_ssi, instrument_classes, instrument_lifecycles | Batch migration to add NOT NULL with `DEFAULT NOW()` backfill |
| M-2 | No `updated_at` trigger on deal/billing tables | deals, deal_participants, deal_rate_cards, deal_slas, deal_documents, deal_onboarding_requests, fee_billing_profiles, fee_billing_periods | Create shared `set_updated_at()` trigger function; apply to all mutable tables |
| M-3 | Duplicate index | `trading_profile_materializations` | Drop `idx_materializations_profile_id` (keep `idx_materializations_profile`) |
| M-4 | Missing status CHECK on `deal_ubo_assessments` | `deal_ubo_assessments` | Add CHECK constraint for assessment_status |
| M-5 | `gen_random_uuid()` on pre-068 tables | legal_contracts, rate_cards, trading_profile_materializations | Migrate defaults to `uuidv7()` for consistency |
| M-6 | `instrument_lifecycles` missing `updated_at` column | `instrument_lifecycles` | Add column |

### Low

| ID | Issue | Tables | Recommendation |
|----|-------|--------|----------------|
| L-1 | No CHECK on `deal_events.event_type` | `deal_events` | Consider adding if event types are finite; skip if extensible by design |
| L-2 | No CHECK on `fee_billing_period_lines` fee_type | `fee_billing_period_lines` | Add CHECK matching rate card line pricing models |
| L-3 | No CHECK on `deal_slas.sla_type` / `metric_type` | `deal_slas` | Add CHECK constraints |
| L-4 | No CHECK on `entity_settlement_identity.identity_type` | `entity_settlement_identity` | Add CHECK (BIC/LEI/ALERT/CTM) |
| L-5 | `cbu_ssi_agent_override` lacks UNIQUE on `(ssi_id, sequence)` | `cbu_ssi_agent_override` | Add UNIQUE constraint to prevent duplicate sequence numbers |
| L-6 | Reference data tables lack timestamps | `ssi_types`, `settlement_types` | Add `created_at`/`updated_at` for seed change tracking |
| L-7 | `legal_contracts.client_label` has no FK enforcement | `legal_contracts` | Consider adding FK to `client_group.canonical_name` or a dedicated alias table; alternatively document as intentional semantic join |
| L-8 | `fee_billing_profiles` → `deals` FK lacks CASCADE | `fee_billing_profiles` | Add comment documenting intentional RESTRICT behavior |
| L-9 | `fee_billing_periods` lacks temporal overlap protection | `fee_billing_periods` | Consider `btree_gist` EXCLUDE constraint on `(profile_id, period_start, period_end)` |

---

## Index Recommendation List

### Missing Indexes (Add)

| Priority | Table | Proposed Index | Rationale |
|----------|-------|---------------|-----------|
| High | `instrument_classes` | `idx_instrument_classes_parent` on `parent_class_id` | Taxonomy hierarchy traversal |
| High | `entity_ssi` | `idx_entity_ssi_dimensions` on `(entity_id, instrument_class, market, currency)` | Multi-dimensional SSI matching |
| Medium | `entity_settlement_identity` | `idx_entity_settlement_identity_lookup` on `(entity_id, identity_type)` | Identity type lookups |
| Medium | `instrument_lifecycles` | `idx_instrument_lifecycles_class` on `(instrument_class_id)` | Class-level lifecycle queries |
| Medium | `deal_ubo_assessments` | `idx_deal_ubo_assessments_case` on `kyc_case_id` | Cross-referencing KYC cases to deals |
| Low | `cbu_ssi_agent_override` | `idx_cbu_ssi_agent_override_ssi` on `ssi_id` | Override lookups by parent SSI |
| Low | `rate_cards` | `idx_rate_cards_name` on `name` | Name-based lookups if rate cards are searched by name |

### Redundant Indexes (Drop)

| Table | Index to Drop | Reason |
|-------|--------------|--------|
| `trading_profile_materializations` | `idx_materializations_profile_id` | Duplicate of `idx_materializations_profile` on same column |

### Existing Indexes Confirmed Appropriate

- `idx_deal_rate_cards_one_agreed` -- partial unique index for rate card precedence (excellent)
- `ssi_booking_rules` composite + specificity_score DESC -- rule matching with priority (excellent)
- `cbu_trading_profiles` partial index WHERE status = 'ACTIVE' -- active profile lookups (good)
- `deal_events(deal_id, event_type, occurred_at)` -- time-series audit queries (good)
- `service_availability` btree_gist exclusion -- temporal overlap prevention (excellent)

---

## Summary

**Strongest areas:**
- Booking principal tables (072): exemplary constraint discipline, temporal exclusion, consistent patterns
- SSI booking rules: generated specificity_score column is elegant
- Deal rate card supersession: partial unique index + validation trigger + atomic supersede function
- Service layer (024-027): clean derived table patterns with composite PKs and staleness tracking

**Weakest areas:**
- Deal/billing tables (067): systematic NOT NULL gaps on timestamps, no `updated_at` triggers, inconsistent status terminology
- Legal contract tables (045): oldest migration in scope, pre-dates `uuidv7()` standardization, semantic `client_label` join
- Instrument reference tables: minimal constraint coverage

**Recommended migration priority:**
1. **Quick win (single migration):** Add NOT NULL to timestamps across deal/billing tables + add missing `updated_at` columns + create shared `set_updated_at()` trigger
2. **Quick win:** Add status CHECK to `cbu_ssi`, `deal_ubo_assessments`, and `fee_billing_period_lines`
3. **Quick win:** Drop duplicate `idx_materializations_profile_id`
4. **Medium effort:** Add missing indexes (instrument_classes parent, entity_ssi dimensions, etc.)
5. **Low priority:** Migrate `gen_random_uuid()` defaults to `uuidv7()` on 045/020 tables
