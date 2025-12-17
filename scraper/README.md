# Allianz Fund Scraper

Scrapes Allianz fund data from regulatory sources for import into ob-poc.

## Data Sources

| Source | Type | Contents |
|--------|------|----------|
| **CSSF Luxembourg** | Official registry | Fund IDs, ISINs, share classes, ManCos |
| **regulatory.allianzgi.com** | Allianz disclosure | SFDR, investment mandates, KIDs, risks |

## Quick Start

```bash
# Install dependencies
npm install

# Install Playwright browsers (for Allianz site scraping)
npx playwright install chromium

# Run full pipeline
npm run scrape
```

## Scripts

### 1. CSSF Download (`npm run cssf`)

Downloads and parses the official CSSF UCI registry.

- **Input**: CSSF ZIP file (auto-downloaded)
- **Output**: `data/cssf-allianz-funds.json`
- **Data**: ~500+ Allianz share classes with ISINs

### 2. Allianz Scraper (`npm run allianz`)

Scrapes detailed fund info from regulatory.allianzgi.com using Playwright.

- **Input**: None (scrapes directly)
- **Output**: `data/allianz-fund-details.json`
- **Data**: SFDR category, investment objective, documents, NAV

Options:
```bash
npm run allianz -- --max=50    # Scrape first 50 funds
npm run allianz -- --all       # Scrape all funds (slow!)
```

### 3. Main Pipeline (`npm run scrape`)

Combines both sources and generates ob-poc import files.

- **Input**: CSSF data + Allianz details
- **Output**: 
  - `data/allianz-import.json` (structured import)
  - `data/allianz-import-sample.dsl` (DSL for agent)

## Output Schema

### allianz-import.json

```json
{
  "entities": [
    {
      "type": "entity",
      "entity_type": "LIMITED_COMPANY",
      "name": "Allianz Global Investors GmbH",
      "jurisdiction": "LU",
      "roles": ["MANCO"]
    }
  ],
  "cbus": [
    {
      "type": "cbu",
      "name": "Allianz Global Artificial Intelligence",
      "isin": "LU1234567890",
      "products": ["custody", "fund-accounting"],
      "manco": "Allianz Global Investors GmbH",
      "share_classes": [
        {
          "name": "CT-EUR",
          "isin": "LU1234567890",
          "currency": "EUR",
          "sfdr_category": "Article 8"
        }
      ],
      "investment_objective": "...",
      "benchmark": "MSCI World",
      "key_risks": ["..."]
    }
  ],
  "relationships": [
    {
      "from_type": "entity",
      "from_name": "Allianz Global Investors GmbH",
      "to_type": "cbu",
      "to_name": "Allianz Global Artificial Intelligence",
      "role": "MANCO"
    }
  ]
}
```

## Data Fields

### From CSSF (Authoritative)

| Field | Description |
|-------|-------------|
| `fund_id` | CSSF fund identifier |
| `fund_name` | Official fund name |
| `isin` | Share class ISIN |
| `currency` | Share class currency |
| `legal_form` | SICAV, FCP, etc. |
| `ucits_type` | Part I (UCITS) or Part II |
| `management_company` | ManCo name |
| `depositary` | Custodian/depositary |
| `launch_date` | Fund inception date |

### From Allianz Site (Enrichment)

| Field | Description |
|-------|-------------|
| `sfdr_category` | Article 6, 8, or 9 |
| `morningstar_rating` | Star rating |
| `investment_objective` | Fund strategy text |
| `benchmark` | Reference index |
| `key_risks` | Risk factors |
| `documents` | KID, prospectus links |
| `nav` | Latest NAV |

## Importing to ob-poc

```bash
# After running scraper
cd ../

# Use agent to import
# In chat: "Import Allianz funds from scraper/data/allianz-import.json"
```

Or use the generated DSL directly:
```bash
cat scraper/data/allianz-import-sample.dsl
```

## Re-running

The scraper is designed to be re-run:

```bash
# Fresh scrape of everything
rm -rf data/*
npm run scrape

# Just refresh CSSF data
npm run cssf

# Just refresh Allianz details (slower)
npm run allianz -- --all
```

## Troubleshooting

### CSSF download fails
- Check if CSSF site is accessible
- ZIP URL may have changed - update in `cssf-download.js`

### Allianz scraper gets blocked
- Increase `DELAY_BETWEEN_REQUESTS` 
- Run with `headless: false` to debug
- May need to handle additional consent dialogs

### Missing SFDR data
- Not all funds have SFDR on the page
- Check individual fund pages manually
