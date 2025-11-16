# SQL Directory - OB-POC Database Schema

**Last Updated:** 2025-11-16  
**Schema Version:** 3.0 (Consolidated)

## 📁 Directory Structure

```
sql/
├── README.md                              # This file
├── 00_MASTER_SCHEMA_CONSOLIDATED.sql      # ✅ CURRENT: Complete schema (67 tables)
├── 01_SEED_DATA_CONSOLIDATED.sql          # ✅ CURRENT: Essential seed data
├── CURRENT_SCHEMA_DUMP.sql                # Auto-generated from live DB
├── migrations/                            # Active migrations
│   ├── 009_complete_taxonomy.sql          # Taxonomy system (2025-11-16)
│   └── 010_seed_taxonomy_data.sql         # Taxonomy seed data
├── refactor-migrations/                   # Document attribute refactoring
│   ├── 001_document_attribute_mappings.sql
│   └── 002_seed_document_mappings.sql
└── archive/                               # Historical/deprecated files
    ├── 00_master_schema_OLD.sql
    ├── 01_seed_data_OLD.sql
    └── migrations/                        # Old migration history
```

## 🚀 Quick Start

### Fresh Database Setup
```bash
# 1. Create schema with all tables
psql $DATABASE_URL -f 00_MASTER_SCHEMA_CONSOLIDATED.sql

# 2. Load seed data
psql $DATABASE_URL -f 01_SEED_DATA_CONSOLIDATED.sql

# 3. Verify
psql $DATABASE_URL -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'ob-poc';"
```

### Expected Result
- **67 tables** created in `ob-poc` schema
- **Essential seed data** loaded (entity types, roles, products, services, etc.)

## 📊 Schema Overview

### Database Statistics
| Category | Count | Description |
|----------|-------|-------------|
| **Total Tables** | 67 | Complete production schema |
| **Entity Types** | 6 | Person, Company, Partnership, Trust, etc. |
| **Products** | 3 | Custody, Prime Brokerage, Fund Admin |
| **Services** | 4 | Settlement, Safekeeping, Corporate Actions, Reporting |
| **Resources** | 3 | DTCC, Euroclear, APAC Clearinghouse |
| **DSL Domains** | 8 | case, kyc, entity, products, services, etc. |

### Major Table Groups

#### 1. **Core CBU** (4 tables)
- `cbus` - Client Business Units
- `cbu_creation_log` - CBU creation audit trail
- `roles` - Business roles
- `cbu_entity_roles` - Entity-role assignments

#### 2. **Attributes & Dictionary** (5 tables)
- `dictionary` - Universal attribute dictionary
- `attribute_registry` - Attribute definitions
- `attribute_values` - Runtime attribute values
- `attribute_values_typed` - Typed attribute storage
- Document attribute mappings

#### 3. **Entities** (16 tables)
- `entities` - Core entity table
- `entity_types` - Entity classifications
- `entity_proper_persons` - Individual persons
- `entity_limited_companies` - Companies
- `entity_partnerships` - Partnerships  
- `entity_trusts` - Trust structures
- `ubo_registry` - Ultimate beneficial ownership
- Supporting tables for relationships, validation, lifecycle

#### 4. **Products & Services** (14 tables)
- `products` - Product catalog
- `services` - Service catalog
- `product_services` - Product-service mappings
- `prod_resources` - Production resources
- `service_option_definitions` - Service options
- `service_option_choices` - Option values
- `service_resource_capabilities` - Resource capabilities
- `resource_attribute_requirements` - Attribute requirements
- Supporting tables for workflows, requirements

#### 5. **Onboarding Workflow** (5 tables)
- `onboarding_requests` - Workflow state machine
- `onboarding_products` - Product selections
- `onboarding_service_configs` - Service configurations
- `onboarding_resource_allocations` - Resource assignments
- `service_discovery_cache` - Performance cache

#### 6. **Documents** (7 tables)
- `document_catalog` - Document repository
- `document_types` - Document classifications
- `document_metadata` - Extracted metadata
- `document_relationships` - Document links
- `document_attribute_mappings` - Type-attribute mappings

#### 7. **DSL Management** (7 tables)
- `dsl_domains` - Domain definitions
- `dsl_versions` - Version control
- `dsl_instances` - DSL instances
- `parsed_asts` - Compiled ASTs
- `dsl_execution_log` - Execution history
- `dsl_examples` - Example DSLs
- `dsl_ob` - DSL objects

#### 8. **Vocabularies & Grammar** (4 tables)
- `domain_vocabularies` - Domain verbs
- `verb_registry` - Verb definitions
- `vocabulary_audit` - Change tracking
- `grammar_rules` - EBNF rules

#### 9. **Orchestration** (4 tables)
- `orchestration_sessions` - Multi-domain sessions
- `orchestration_domain_sessions` - Domain execution
- `orchestration_tasks` - Task queue
- `orchestration_state_history` - State snapshots

#### 10. **Reference Data** (2 tables)
- `master_jurisdictions` - Jurisdiction reference
- `master_entity_xref` - External ID mappings

#### 11. **Audit & Misc** (3 tables)
- `crud_operations` - CRUD audit log
- `rag_embeddings` - Vector embeddings
- `schema_changes` - Schema change log

## 🔄 Migration Strategy

### Current Approach (Post-Consolidation)
1. **No more incremental migrations** - Schema is stable
2. **Use consolidated files** for new databases
3. **Document changes** in git commits
4. **Periodic regeneration** of CURRENT_SCHEMA_DUMP.sql from live DB

### Historical Migrations (Archived)
All historical migrations moved to `archive/migrations/`:
- `001` through `008` - Original phased development
- Attribute refactoring series
- Various fixes and enhancements

**Note:** These are for reference only. Do not run on new databases.

## 📝 Seed Data Contents

### Entity Types
- PERSON, LIMITED_COMPANY, PARTNERSHIP, TRUST, FOUNDATION, LLC

### Roles
- Beneficial Owner, Director, Shareholder, Trustee, Partner, etc.

### Products
- **CUSTODY_INST** - Institutional Custody
- **PRIME_BROKER** - Prime Brokerage
- **FUND_ADMIN** - Fund Administration

### Services
- **SETTLEMENT** - Trade Settlement (with multi-market options)
- **SAFEKEEPING** - Asset Safekeeping
- **CORP_ACTIONS** - Corporate Actions
- **REPORTING** - Client Reporting

### Production Resources
- **DTCC_SETTLE** - US markets (T0/T1/T2)
- **EUROCLEAR** - European markets (T1/T2)
- **APAC_CLEAR** - APAC markets (T2)

### DSL Domains
- case, kyc, entity, products, services, ubo, document, compliance

### Jurisdictions (Sample)
- US, GB, DE, FR, SG, HK, CH, LU, KY, BM

### Document Types (Sample)
- Passport, Drivers License, National ID, Proof of Address
- Certificate of Incorporation, Articles of Association
- Trust Deed, Partnership Agreement, UBO Certificate

## 🔧 Maintenance

### Regenerate Schema Dump
```bash
pg_dump $DATABASE_URL --schema-only --schema="ob-poc" > CURRENT_SCHEMA_DUMP.sql
```

### Verify Schema
```bash
psql $DATABASE_URL -c "
SELECT tablename 
FROM pg_tables 
WHERE schemaname = 'ob-poc' 
ORDER BY tablename;"
```

### Check Seed Data
```bash
psql $DATABASE_URL -f 01_SEED_DATA_CONSOLIDATED.sql
# Look for verification output at end
```

## 📚 Related Documentation

- **CLAUDE.md** - Project overview and architecture
- **TAXONOMY_IMPLEMENTATION_COMPLETE.md** - Taxonomy system details
- **TAXONOMY_QUICK_START.md** - Quick reference guide
- **rust/COMPLETE_TAXONOMY_IMPLEMENTATION.md** - Original Opus plan

## ⚠️ Important Notes

1. **Always use consolidated files** for new setups
2. **Archive old migrations** - don't delete (git history)
3. **Document schema changes** in commit messages
4. **Test migrations** on dev database first
5. **Backup before schema changes** in production

## 🎯 Version History

| Version | Date | Description |
|---------|------|-------------|
| **3.0** | 2025-11-16 | Consolidated schema (67 tables) + taxonomy system |
| 2.x | 2025-11 | Phased development, attribute refactoring |
| 1.x | 2025-10 | Initial schema development |

---

**Schema Status:** ✅ Production Ready  
**Last Schema Dump:** 2025-11-16  
**Total Tables:** 67
