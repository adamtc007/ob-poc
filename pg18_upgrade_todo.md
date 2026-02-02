# PostgreSQL 17 → 18 Upgrade TODO

## Context
- Current: PostgreSQL 17.7_1 via Homebrew on Mac M4
- Target: PostgreSQL 18 with native UUIDv7 support
- Data directory is fresh (Jan 28, 2025) - safe to nuke and rebuild from migrations

## Phase 1: Backup & Teardown

```bash
# 1. Backup existing data (safety net)
/opt/homebrew/opt/postgresql@17/bin/pg_dumpall > ~/pg17_backup_$(date +%Y%m%d).sql

# 2. Stop PostgreSQL 17
brew services stop postgresql@17

# 3. Uninstall PostgreSQL 17
brew uninstall postgresql@17

# 4. Remove data directory
rm -rf /opt/homebrew/var/postgres*
```

## Phase 2: Install PostgreSQL 18

```bash
# 1. Install PostgreSQL 18
brew install postgresql@18

# 2. Start the service
brew services start postgresql@18

# 3. Verify installation
/opt/homebrew/opt/postgresql@18/bin/psql --version

# 4. Test native UUIDv7
/opt/homebrew/opt/postgresql@18/bin/psql -c "SELECT uuidv7();"
```

## Phase 3: Update Shell PATH

Add to `~/.zshrc`:
```bash
export PATH="/opt/homebrew/opt/postgresql@18/bin:$PATH"
```

Then reload:
```bash
source ~/.zshrc
```

## Phase 4: Rebuild ob-poc Database

```bash
# From the ob-poc project directory
cd ~/path/to/ob-poc  # UPDATE THIS PATH

# Create database
sqlx database create

# Run migrations
sqlx migrate run
```

## Phase 5: UUID Migration in Codebase

### 5a. Update Cargo.toml
Ensure uuid crate has v7 feature:
```toml
uuid = { version = "1.10", features = ["v4", "v7", "serde"] }
```

### 5b. Update SQL Migrations
Find and replace in migration files:
- `gen_random_uuid()` → `uuidv7()`
- `uuid_generate_v4()` → `uuidv7()`

### 5c. Update Rust Code
Find and replace in source files:
- `Uuid::new_v4()` → `Uuid::now_v7()`

## Phase 6: Verification

```bash
# 1. Confirm version
psql -c "SELECT version();"

# 2. Confirm UUIDv7 works
psql -d ob_poc -c "SELECT uuidv7();"

# 3. Run tests
cargo test

# 4. Check a sample table default
psql -d ob_poc -c "\d+ kyc_applications" | grep -i uuid
```

## Post-Upgrade: Index Audit (Run Later)

After the system has been running for a few days with real usage stats, run the index audit script to identify skip-scan consolidation opportunities.

---

## Notes
- SQLx 0.8+ wire protocol is stable across PG versions
- UUIDv7 is time-prefixed → sequential inserts → better B-tree performance
- No need for pgcrypto or uuid-ossp extensions anymore
