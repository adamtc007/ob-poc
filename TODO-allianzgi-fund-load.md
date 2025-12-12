# TODO: Load AllianzGI Funds by ManCo as CBU

## Objective
Load complete AllianzGI fund universe using DSL verbs: `cbu.*`, `entity.*`, `fund.*`

## Prerequisites
- [ ] Verify DSL REPL is operational: `./target/release/ob-poc-dsl`
- [ ] Confirm database connection is active
- [ ] Ensure seed data folder exists: `data/seed/allianzgi/`

---

## PHASE 1: Download Fund Data (One-Time)

### 1.1 Setup Playwright Downloader
```bash
cd /Users/adamtc007/Developer/ob-poc/data/allianzgi_seed
npm install
npx playwright install chromium
```

### 1.2 Run Download Script
```bash
node download-allianzgi-funds.mjs
```

Expected outputs in `out/`:
- `LU__AGI_LUX__*.csv` (~1,972 share classes)
- `GB__AGI_UK__*.csv` (~242 share classes)
- `IE__AGI_IE__*.csv` (~780 share classes)
- `DE__AGI_DE__*.csv`
- `CH__AGI_CH__*.csv`

---

## PHASE 2: Generate DSL Scripts

### 2.1 Convert CSV → DSL (per ManCo jurisdiction)
```bash
cd /Users/adamtc007/Developer/ob-poc/data/seed/allianzgi

# Generate per-jurisdiction DSL files
python3 csv_to_dsl.py ../allianzgi_seed/out/LU__AGI_LUX__*.csv > load_lu_funds.dsl
python3 csv_to_dsl.py ../allianzgi_seed/out/GB__AGI_UK__*.csv > load_gb_funds.dsl
python3 csv_to_dsl.py ../allianzgi_seed/out/IE__AGI_IE__*.csv > load_ie_funds.dsl
python3 csv_to_dsl.py ../allianzgi_seed/out/DE__AGI_DE__*.csv > load_de_funds.dsl
python3 csv_to_dsl.py ../allianzgi_seed/out/CH__AGI_CH__*.csv > load_ch_funds.dsl
```

---

## PHASE 3: Load Core Structure (Run Once)

### 3.1 Load Group + ManCos
Execute `03_load_allianzgi.dsl` which creates:
```bash
cd /Users/adamtc007/Developer/ob-poc
./target/release/ob-poc-dsl < data/seed/allianzgi/03_load_allianzgi.dsl
```

**Creates:**
- CBU: `Allianz Global Investors (Group)` [jurisdiction=DE, category=FUND_MANDATE]
- 11 ManCo entities with `MANAGEMENT_COMPANY` role
- 3 Service provider entities

### DSL Verbs Used:
```dsl
# CBU creation
cbu.ensure name="..." jurisdiction=DE client-type="ASSET_MANAGER" -> $cbu_var

# Entity creation  
entity.create-limited-company name="..." jurisdiction=XX company-number="..." cbu-id=$cbu_var -> $entity_var

# Role assignment
cbu.assign-role cbu-id=$cbu_var entity-id=$entity_var role=MANAGEMENT_COMPANY
```

---

## PHASE 4: Load Funds by Jurisdiction

### 4.1 Luxembourg Funds (AGI_LUX)
```bash
./target/release/ob-poc-dsl < data/seed/allianzgi/load_lu_funds.dsl
```

### 4.2 UK Funds (AGI_UK)
```bash
./target/release/ob-poc-dsl < data/seed/allianzgi/load_gb_funds.dsl
```

### 4.3 Ireland Funds (AGI_IE)
```bash
./target/release/ob-poc-dsl < data/seed/allianzgi/load_ie_funds.dsl
```

### 4.4 Germany Funds (AGI_DE)
```bash
./target/release/ob-poc-dsl < data/seed/allianzgi/load_de_funds.dsl
```

### 4.5 Switzerland Funds (AGI_CH)
```bash
./target/release/ob-poc-dsl < data/seed/allianzgi/load_ch_funds.dsl
```

---

## PHASE 5: DSL Verb Reference

### CBU Verbs (`cbu.yaml`)
| Verb | Purpose | Example |
|------|---------|---------|
| `cbu.ensure` | Upsert CBU | `cbu.ensure name="X" jurisdiction=DE` |
| `cbu.set-category` | Set CBU category | `cbu.set-category cbu-id=$x category=FUND_MANDATE` |
| `cbu.assign-role` | Link entity to CBU | `cbu.assign-role cbu-id=$x entity-id=$y role=MANAGEMENT_COMPANY` |
| `cbu.show` | Display CBU tree | `cbu.show cbu-id=$x` |

### Entity Verbs (`entity.yaml`)
| Verb | Purpose | Example |
|------|---------|---------|
| `entity.create-limited-company` | Create ManCo | `entity.create-limited-company name="X" jurisdiction=DE` |

### Fund Verbs (`fund.yaml`)
| Verb | Purpose | Example |
|------|---------|---------|
| `fund.create-umbrella` | Create SICAV/OEIC | `fund.create-umbrella name="X" fund-structure-type=SICAV` |
| `fund.create-subfund` | Create compartment | `fund.create-subfund name="X" umbrella-id=$umbrella` |
| `fund.create-share-class` | Create share class | `fund.create-share-class name="X" isin="LUxxxx"` |
| `fund.show-structure` | Display fund tree | `fund.show-structure fund-id=$x` |

---

## PHASE 6: Connection Types

### Entity Roles for CBU-Entity Links
| Role | Category | Usage |
|------|----------|-------|
| `MANAGEMENT_COMPANY` | OWNERSHIP_CONTROL | ManCo entities |
| `DEPOSITARY` | TRADING_EXECUTION | Custody providers |
| `AUDITOR` | TRADING_EXECUTION | PwC, KPMG, etc. |
| `ADMINISTRATOR` | TRADING_EXECUTION | Fund admin |
| `PRIME_BROKER` | TRADING_EXECUTION | Prime services |

### Fund Structure Types
| Type | Jurisdiction | Usage |
|------|--------------|-------|
| `SICAV` | LU | Luxembourg investment company |
| `FCP` | LU | Luxembourg contractual fund |
| `OEIC` | GB | UK open-ended investment company |
| `ICAV` | IE | Irish collective asset vehicle |
| `AIF` | * | Alternative investment fund |

---

## PHASE 7: Verification

### 7.1 Count Loaded Entities
```sql
-- CBU count
SELECT COUNT(*) FROM cbus WHERE name LIKE '%AllianzGI%';

-- ManCo count  
SELECT COUNT(*) FROM entities e
JOIN cbu_entity_roles r ON e.entity_id = r.entity_id
WHERE r.role_type_code = 'MANAGEMENT_COMPANY';

-- Fund umbrella count
SELECT COUNT(*) FROM fund_umbrellas WHERE cbu_id = (
  SELECT cbu_id FROM cbus WHERE name LIKE '%AllianzGI%'
);

-- Share class count
SELECT COUNT(*) FROM share_classes sc
JOIN fund_subfunds sf ON sc.subfund_id = sf.subfund_id
JOIN fund_umbrellas fu ON sf.umbrella_id = fu.umbrella_id
WHERE fu.cbu_id = (SELECT cbu_id FROM cbus WHERE name LIKE '%AllianzGI%');
```

### 7.2 Visualize Structure
```dsl
cbu.show cbu-id=$agi_group_cbu
```

---

## Notes

### US Business
AllianzGI US business transferred to Voya IM (July 2022). Treat as separate universe.

### Data Refresh
Re-run download + load periodically to capture:
- New fund launches
- Share class additions
- Fee changes
- Fund terminations

### LEI Codes
Not in CSV exports. Source from GLEIF or Bloomberg for production use.

---

## Files

| File | Purpose |
|------|---------|
| `data/allianzgi_seed/download-allianzgi-funds.mjs` | Playwright downloader |
| `data/seed/allianzgi/01_group_structure.yaml` | ManCo reference data |
| `data/seed/allianzgi/03_load_allianzgi.dsl` | Core structure DSL |
| `data/seed/allianzgi/csv_to_dsl.py` | CSV → DSL converter |
| `data/seed/allianzgi/load_*.dsl` | Generated fund DSL (per jurisdiction) |
