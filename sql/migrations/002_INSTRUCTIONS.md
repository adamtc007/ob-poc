# Database Migration Instructions for Claude Code

## Overview
These migrations complete the persistence layer for DSL verb domains (screening, decision, monitoring, attribute).

**Important**: Migration 002 is designed to be ADDITIVE to the existing `017_kyc_investigation_tables.sql` which already created `kyc_investigations`, `screenings`, `kyc_decisions`, `decision_conditions`, and `document_requests`.

## Schema Audit Findings

Before creating these migrations, we audited for existing tables with similar functionality:

### Partial Overlaps Identified

| New Table | Existing Table | Relationship |
|-----------|----------------|--------------|
| `ownership_relationships` | `ubo_registry` | **Different purpose**: `ubo_registry` stores *identified UBOs* (result). New table stores *ownership chain* (input for UBO calculation). |
| `ownership_relationships` | `partnership_interests` | **Subset**: Partnership-specific only. New table is general entity→entity. |
| `risk_ratings` | `ubo_registry.risk_rating` | **Different level**: `ubo_registry` has UBO-level. New table is CBU-level with history. |
| `screening_hit_resolutions` | `ubo_registry.screening_result` | **Different detail**: `ubo_registry` has status only. New table has full workflow. |

### Conclusion
All new tables are needed - no true duplicates. Added FKs and comments to clarify relationships.

## Files
- `001_schema_drift_fixes.sql` - Pre-flight fixes for existing column drift (210 lines)
- `002_kyc_screening_decision_monitoring_tables.sql` - New tables + bridge views (543 lines)

## Migration Strategy

### Existing Tables (from 017)
| Table | crud_asset | Status |
|-------|-----------|--------|
| kyc_investigations | INVESTIGATION | ✅ Exists, view bridges naming |
| screenings | SCREENING_RESULT | ✅ Exists, view bridges naming |
| kyc_decisions | DECISION | ✅ Exists, view bridges naming |
| decision_conditions | DECISION_CONDITION | ✅ Exists |
| document_requests | DOCUMENT_REQUEST | ✅ Exists |
| risk_assessments | RISK_ASSESSMENT_CBU | ✅ Exists |

### New Tables (from 002)
| Table | crud_asset |
|-------|-----------|
| ownership_relationships | OWNERSHIP |
| document_entity_links | DOCUMENT_LINK |
| risk_ratings | RISK_RATING |
| screening_hit_resolutions | SCREENING_HIT_RESOLUTION |
| screening_batches | SCREENING_BATCH |
| monitoring_cases | MONITORING_CASE |
| monitoring_reviews | MONITORING_REVIEW |
| monitoring_alert_rules | MONITORING_ALERT_RULE |
| monitoring_activities | MONITORING_ACTIVITY |

### Bridge Views Created
- `investigations` → points to `kyc_investigations`
- `screening_results` → points to `screenings`  
- `decisions` → points to `kyc_decisions`

## Steps to Execute

### Step 0: Pre-flight Schema Drift Fixes
```bash
cd /Users/adamtc007/Developer/ob-poc
psql $DATABASE_URL -f sql/migrations/001_schema_drift_fixes.sql
```
Fixes column mismatches causing SQLX compile errors.

### Step 1: Verify Existing Tables from 017
```bash
psql $DATABASE_URL -c "SELECT tablename FROM pg_tables WHERE schemaname = 'ob-poc' AND tablename IN ('kyc_investigations', 'screenings', 'kyc_decisions', 'document_requests', 'risk_assessments');"
```
If these exist, 017 has been applied. If not, run:
```bash
psql $DATABASE_URL -f sql/migrations/017_kyc_investigation_tables.sql
```

### Step 2: Run Migration 002
```bash
psql $DATABASE_URL -f sql/migrations/002_kyc_screening_decision_monitoring_tables.sql
```

### Step 3: Verify New Tables
```bash
psql $DATABASE_URL -c "SELECT tablename FROM pg_tables WHERE schemaname = 'ob-poc' AND tablename IN ('ownership_relationships', 'monitoring_cases', 'screening_hit_resolutions', 'risk_ratings');"
```

### Step 4: Verify Bridge Views
```bash
psql $DATABASE_URL -c "SELECT viewname FROM pg_views WHERE schemaname = 'ob-poc' AND viewname IN ('investigations', 'screening_results', 'decisions');"
```

### Step 5: Generate SQLX Cache
```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo sqlx prepare --database-url $DATABASE_URL
```

### Step 6: Verify Rust Compilation
```bash
cargo check --features database
```

## Troubleshooting

### If 017 wasn't applied
```bash
psql $DATABASE_URL -f sql/migrations/017_kyc_investigation_tables.sql
psql $DATABASE_URL -f sql/migrations/002_kyc_screening_decision_monitoring_tables.sql
```

### If SQLX errors persist
Check these columns exist:
```bash
psql $DATABASE_URL -c "\d \"ob-poc\".document_metadata"
psql $DATABASE_URL -c "\d \"ob-poc\".attribute_values_typed"
```

### Full reset (nuclear option)
```bash
psql $DATABASE_URL -f sql/CURRENT_SCHEMA_DUMP.sql
psql $DATABASE_URL -f sql/migrations/017_kyc_investigation_tables.sql
psql $DATABASE_URL -f sql/migrations/002_kyc_screening_decision_monitoring_tables.sql
```

## Complete Verb→Table Mapping After Migration

| Domain | Verb | crud_asset | Table |
|--------|------|-----------|-------|
| CBU | cbu.* | CBU | cbus |
| CBU | cbu.attach-entity | CBU_ENTITY_ROLE | cbu_entity_roles |
| Entity | entity.create-* | LIMITED_COMPANY, etc. | entity_limited_companies, etc. |
| Entity | entity.ensure-ownership | OWNERSHIP | ownership_relationships |
| Document | document.request | DOCUMENT_REQUEST | document_requests |
| Document | document.link | DOCUMENT_LINK | document_entity_links |
| Document | document.* | DOCUMENT | document_catalog |
| KYC | investigation.* | INVESTIGATION | kyc_investigations (via view) |
| KYC | risk.assess-cbu | RISK_ASSESSMENT_CBU | risk_assessments |
| KYC | risk.set-rating | RISK_RATING | risk_ratings |
| Screening | screening.pep/sanctions/adverse-media | SCREENING_RESULT | screenings (via view) |
| Screening | screening.resolve-hit | SCREENING_HIT_RESOLUTION | screening_hit_resolutions |
| Screening | screening.batch | SCREENING_BATCH | screening_batches |
| Decision | decision.* | DECISION | kyc_decisions (via view) |
| Decision | decision.add-condition | DECISION_CONDITION | decision_conditions |
| Monitoring | monitoring.close-case | MONITORING_CASE | monitoring_cases |
| Monitoring | monitoring.schedule-review | MONITORING_REVIEW | monitoring_reviews |
| Monitoring | monitoring.add-alert-rule | MONITORING_ALERT_RULE | monitoring_alert_rules |
| Monitoring | monitoring.record-activity | MONITORING_ACTIVITY | monitoring_activities |
| Attribute | attribute.* | ATTRIBUTE_VALUE | attribute_values_typed |
