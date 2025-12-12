# AllianzGI Fund Data Seed Package

This package contains everything needed to load AllianzGI's fund structure into the CBU system.

## Quick Start

### Option 1: Sample Data (Immediate)

The sample data covers the ManCo structure and a representative set of Luxembourg funds:

```bash
# From ob-poc root, run the DSL loader
cd /Users/adamtc007/Developer/ob-poc
./target/release/ob-poc-dsl < data/seed/allianzgi/03_load_allianzgi.dsl
```

This creates:
- 1 CBU (AllianzGI Group)  
- 11 ManCo entities (DE, LU, UK, IE, CH, HK, SG, JP, TW, CN, ID)
- 3 Service provider entities
- 2 Umbrella funds (Luxembourg SICAVs)
- 6 Sub-funds
- 15 Share classes

### Option 2: Full Data (Requires Browser Download)

To get the complete fund universe (~3,000+ share classes across all jurisdictions):

```bash
# 1. Install Playwright
cd data/seed/allianzgi
npm init -y
npm i -D playwright
npx playwright install chromium

# 2. Download fund CSVs from AllianzGI
node download_funds.mjs

# 3. Convert CSV to DSL
python3 csv_to_dsl.py out/LU__AGI_LUX__*.csv > load_lu_funds.dsl
python3 csv_to_dsl.py out/GB__AGI_UK__*.csv > load_gb_funds.dsl
python3 csv_to_dsl.py out/IE__AGI_IE__*.csv > load_ie_funds.dsl
python3 csv_to_dsl.py out/DE__AGI_DE__*.csv > load_de_funds.dsl
python3 csv_to_dsl.py out/CH__AGI_CH__*.csv > load_ch_funds.dsl

# 4. Load into CBU
cat load_lu_funds.dsl | ../../target/release/ob-poc-dsl
```

## File Structure

```
allianzgi/
├── 01_group_structure.yaml   # ManCo entities, service providers (reference)
├── 02_lu_sicav_sample.yaml   # Sample Luxembourg fund structure (reference)
├── 03_load_allianzgi.dsl     # DSL script to load sample data
├── download_funds.mjs        # Playwright script to download full data
├── csv_to_dsl.py             # Convert CSV exports to DSL commands
└── README.md                 # This file
```

## Data Sources

| Jurisdiction | ManCo Code | URL | Expected Volume |
|--------------|------------|-----|-----------------|
| Luxembourg | AGI_LUX | [Fund Explorer](https://regulatory.allianzgi.com/en-gb/facilities-services/luxemburg-en/funds/mutual-funds) | ~206 funds, ~1,972 share classes |
| UK | AGI_UK | [Fund Explorer](https://regulatory.allianzgi.com/en-gb/b2c/united-kingdom-en/funds/mutual-funds) | ~70 funds, ~242 share classes |
| Ireland | AGI_IE | [Fund Explorer](https://regulatory.allianzgi.com/en-ie/b2c/ireland-en/funds/mutual-funds) | ~50 funds, ~780 share classes |
| Germany | AGI_DE | [Fund Explorer](https://regulatory.allianzgi.com/de-de/b2c/deutschland-de/funds/mutual-funds) | German domiciled funds |
| Switzerland | AGI_CH | [Fund Explorer](https://regulatory.allianzgi.com/de-ch/b2c/schweiz-de/funds/mutual-funds) | Swiss registered funds |

## Fund Structure Hierarchy

```
CBU: AllianzGI Group
├── ManCo: Allianz Global Investors GmbH (DE)
│   ├── Umbrella: Allianz Global Investors Fund (SICAV, LU)
│   │   ├── Sub-fund: Allianz Global Artificial Intelligence
│   │   │   ├── Share Class: A - EUR (Retail, ACC)
│   │   │   ├── Share Class: IT - USD (Institutional, ACC)
│   │   │   ├── Share Class: AT (H2-EUR) - EUR (Retail, ACC, Hedged)
│   │   │   └── ...
│   │   ├── Sub-fund: Allianz Emerging Markets Equity
│   │   │   └── ...
│   │   └── ...
│   └── Service Providers
│       ├── Depositary: State Street Bank (LU)
│       └── Auditor: PwC (LU)
├── ManCo: Allianz Global Investors UK Limited
│   └── Umbrella: AllianzGI UK OEIC (OEIC, GB)
└── ...
```

## DSL Verbs Used

| Domain | Verb | Purpose |
|--------|------|---------|
| `cbu` | `ensure` | Create or update CBU |
| `cbu` | `set-category` | Set CBU category (FUND_MANDATE) |
| `cbu` | `assign-role` | Assign ManCo/Depositary/Auditor roles |
| `entity` | `create-limited-company` | Create ManCo legal entities |
| `fund` | `create-umbrella` | Create SICAV/ICAV/OEIC structures |
| `fund` | `create-subfund` | Create compartments within umbrellas |
| `fund` | `create-share-class` | Create share class entities |
| `fund` | `show-structure` | Display fund hierarchy |

## Verb Coverage Check

The current verb definitions support all operations needed:

✅ **cbu.yaml**
- `cbu.ensure` - Create CBU with upsert semantics
- `cbu.assign-role` - Link entities to CBU with roles
- `cbu.set-category` - Set FUND_MANDATE category

✅ **entity.yaml**  
- `entity.create-limited-company` - Create ManCo entities

✅ **fund.yaml**
- `fund.create-umbrella` - SICAV/ICAV/OEIC with full metadata
- `fund.create-subfund` - Compartments with umbrella linkage
- `fund.create-share-class` - Share classes with ISIN, fees, etc.
- `fund.show-structure` - Visualize hierarchy

✅ **delegation.yaml**
- `delegation.add` - For sub-advisor relationships (if needed)

### Missing Verbs (Enhancement Opportunities)

The current verbs handle the core load scenario. For full production use, consider adding:

1. **`fund.assign-manco`** - Explicit ManCo assignment to fund (currently done via `cbu.assign-role`)
2. **`fund.assign-depositary`** - Depositary assignment  
3. **`fund.assign-auditor`** - Auditor assignment
4. **`fund.set-sfdr-category`** - SFDR Article 6/8/9 classification
5. **`fund.set-benchmark`** - Benchmark assignment

These could be added to `fund.yaml` if needed for stricter modeling.

## Notes

### US Business
AllianzGI transferred its US investment management business to Voya Investment Management on July 25, 2022. US fund coverage should be handled via a separate Voya seed package if needed.

### Data Freshness
The AllianzGI fund lists are updated regularly. Re-run `download_funds.mjs` periodically to capture:
- New fund launches
- Share class additions
- Fund terminations
- Fee changes

### LEI Codes
LEI (Legal Entity Identifier) codes are not included in the CSV exports. For production use, source LEIs from:
- GLEIF (Global LEI Foundation): https://www.gleif.org/en/lei-data/global-lei-index
- Bloomberg/Reuters terminals
- AllianzGI prospectuses

## Troubleshooting

### Download Fails
- AllianzGI sites use investor type gating - the script handles this
- If downloads fail, try running with `headless: false` in `download_funds.mjs` to debug

### CSV Parsing Errors
- AllianzGI exports vary by region (CSV vs XLSX, `;` vs `,` delimiter)
- The `csv_to_dsl.py` script auto-detects format
- Check column mapping if fields are missing

### DSL Execution Errors
- Ensure the CBU and umbrella exist before creating sub-funds
- Share class ISINs must be unique
- Check jurisdiction codes match `master_jurisdictions` table
