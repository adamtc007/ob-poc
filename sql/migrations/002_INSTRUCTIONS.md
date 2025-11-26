# Database Migration Instructions for Claude Code

## Overview
This migration adds 14+ tables required by the new DSL verb domains (screening, decision, monitoring, attribute).

## Files
- `sql/migrations/001_schema_drift_fixes.sql` - Pre-flight fixes for existing column drift (210 lines)
- `sql/migrations/002_kyc_screening_decision_monitoring_tables.sql` - Main migration (664 lines)

## Steps to Execute

### 0. Pre-flight: Fix Existing Schema Drift
```bash
cd /Users/adamtc007/Developer/ob-poc
psql $DATABASE_URL -f sql/migrations/001_schema_drift_fixes.sql
```
This fixes column mismatches that cause SQLX compile errors (doc_id, entity_id, value_text, etc.)

### 1. Verify Database Connection
```bash
# Test connection
psql $DATABASE_URL -c "SELECT current_database(), current_schema();"
```

### 2. Run the Migration
```bash
cd /Users/adamtc007/Developer/ob-poc
psql $DATABASE_URL -f sql/migrations/002_kyc_screening_decision_monitoring_tables.sql
```

### 3. Verify Tables Created
```bash
psql $DATABASE_URL -c "SELECT tablename FROM pg_tables WHERE schemaname = 'ob-poc' AND tablename IN ('document_requests', 'investigations', 'screening_results', 'decisions', 'monitoring_cases') ORDER BY tablename;"
```

Expected output:
```
     tablename      
--------------------
 decisions
 document_requests
 investigations
 monitoring_cases
 screening_results
(5 rows)
```

### 4. Generate SQLX Cache (for offline compilation)
```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo sqlx prepare --database-url $DATABASE_URL
```

### 5. Verify Rust Compilation
```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo check --features database
```

## Tables Added

| Domain | Table | Purpose |
|--------|-------|---------|
| Document | document_requests | Track document requests |
| Document | document_entity_links | Link documents to entities |
| Entity | ownership_relationships | UBO ownership chains |
| KYC | investigations | KYC investigations |
| KYC | risk_assessments | Risk assessment results |
| KYC | risk_ratings | Historical risk ratings |
| Screening | screening_results | PEP/sanctions/adverse media results |
| Screening | screening_hit_resolutions | Hit resolution decisions |
| Screening | screening_batches | Batch screening jobs |
| Screening | screening_batch_results | Batch-to-result links |
| Decision | decisions | Approval/rejection/escalation records |
| Decision | decision_conditions | Conditional approval conditions |
| Monitoring | monitoring_cases | Ongoing monitoring cases |
| Monitoring | monitoring_reviews | Periodic/triggered reviews |
| Monitoring | monitoring_alert_rules | Custom alert rules |
| Monitoring | monitoring_activities | Activity audit log |
| Monitoring | risk_rating_changes | Risk change audit trail |

## Views Added

- `active_investigations` - Open KYC investigations
- `pending_screening_hits` - Unresolved screening hits
- `overdue_reviews` - Past-due monitoring reviews
- `blocking_conditions` - Unsatisfied blocking conditions

## Constraints Updated

- `crud_operations.asset_type_check` - Expanded to include all verb crud_asset values
- `dsl_examples.asset_type_check` - Same expansion

## Troubleshooting

### If migration fails
```bash
# Check for existing tables that might conflict
psql $DATABASE_URL -c "SELECT tablename FROM pg_tables WHERE schemaname = 'ob-poc' AND tablename LIKE '%investigation%';"

# Rollback if needed (manual - identify and drop new tables)
```

### If SQLX check still fails after migration
The issue may be column drift in *existing* tables. Check:
- `document_metadata.doc_id` exists
- `attribute_values_typed.entity_id` exists
- `attribute_values_typed.value_text` exists

```bash
psql $DATABASE_URL -c "\d \"ob-poc\".attribute_values_typed"
```
