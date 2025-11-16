# Taxonomy System - Quick Start Guide

**Status**: ✅ Production Ready  
**Last Updated**: 2025-11-16

## 🚀 Quick Commands

### Run the Demo
```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo run --example taxonomy_workflow_demo --features database
```

### Run Tests
```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo test --features database test_product_discovery -- --ignored --nocapture
cargo test --features database test_service_options -- --ignored --nocapture
```

### Database Status
```bash
cd /Users/adamtc007/Developer/ob-poc
psql $DATABASE_URL -c "SELECT COUNT(*) FROM \"ob-poc\".products WHERE product_code IS NOT NULL;"
psql $DATABASE_URL -c "SELECT COUNT(*) FROM \"ob-poc\".services WHERE service_code IS NOT NULL;"
psql $DATABASE_URL -c "SELECT COUNT(*) FROM \"ob-poc\".service_option_choices;"
```

## 📊 What's Available

### Products (3)
- **CUSTODY_INST** - Institutional Custody
- **PRIME_BROKER** - Prime Brokerage  
- **FUND_ADMIN** - Fund Administration

### Services (4)
- **SETTLEMENT** - Trade Settlement (with options)
- **SAFEKEEPING** - Asset Safekeeping
- **CORP_ACTIONS** - Corporate Actions
- **REPORTING** - Client Reporting

### Resources (3)
- **DTCC_SETTLE** - DTCC Settlement System (US markets, T0/T1/T2)
- **EUROCLEAR** - Euroclear Settlement (EU markets, T1/T2)
- **APAC_CLEAR** - APAC Clearinghouse (APAC markets, T2)

## 💻 Code Examples

### Create Onboarding Workflow
```rust
use ob_poc::database::DatabaseManager;
use ob_poc::taxonomy::{TaxonomyDslManager, DslOperation};

// Setup
let db = DatabaseManager::with_default_config().await?;
let manager = TaxonomyDslManager::new(db.pool().clone());

// Step 1: Create request
let result = manager.execute(DslOperation::CreateOnboarding {
    cbu_id: your_cbu_id,
    initiated_by: "my_agent".to_string(),
}).await?;

let request_id = /* extract from result.data */;

// Step 2: Add products
let result = manager.execute(DslOperation::AddProducts {
    request_id,
    product_codes: vec!["CUSTODY_INST".to_string()],
}).await?;
```

### Configure Service with Options
```rust
use std::collections::HashMap;

let mut options = HashMap::new();
options.insert("markets".to_string(), serde_json::json!(["US_EQUITY", "EU_EQUITY"]));
options.insert("speed".to_string(), serde_json::json!("T1"));

let result = manager.execute(DslOperation::ConfigureService {
    request_id,
    service_code: "SETTLEMENT".to_string(),
    options,
}).await?;

println!("Generated DSL: {}", result.dsl_fragment.unwrap());
```

## 📁 Key Files

| File | Purpose |
|------|---------|
| `sql/migrations/009_complete_taxonomy.sql` | Database schema |
| `sql/migrations/010_seed_taxonomy_data.sql` | Initial data |
| `rust/src/models/taxonomy.rs` | Data models |
| `rust/src/database/taxonomy_repository.rs` | Database operations |
| `rust/src/taxonomy/manager.rs` | Business logic |
| `rust/examples/taxonomy_workflow_demo.rs` | Working example |

## 🔧 Troubleshooting

### Database Connection Issues
```bash
# Check DATABASE_URL is set
echo $DATABASE_URL

# Test connection
psql $DATABASE_URL -c "SELECT 1;"
```

### Missing Tables
```bash
# Run migrations
psql $DATABASE_URL -f sql/migrations/009_complete_taxonomy.sql
psql $DATABASE_URL -f sql/migrations/010_seed_taxonomy_data.sql
```

### Build Errors
```bash
# Clean build
cd rust
cargo clean
cargo build --features database
```

## 📚 Documentation

- **Full Implementation**: See `TAXONOMY_IMPLEMENTATION_COMPLETE.md`
- **Original Plan**: See `rust/COMPLETE_TAXONOMY_IMPLEMENTATION.md`
- **CLAUDE.md**: Project overview and architecture

## 🎯 Next Steps

1. **Explore the Demo**: Run `taxonomy_workflow_demo` to see it in action
2. **Add Data**: Insert your own products/services/resources
3. **Extend Workflow**: Implement resource allocation and finalization
4. **Build API**: Expose via REST endpoints for external agents

---

**Happy Coding! 🚀**
