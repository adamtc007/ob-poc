# Allianz Fund Scraper

Scrapes fund data from Allianz Global Investors regulatory sites for import into ob-poc.

## Quick Start

```bash
cd scrapers
npm install
npx playwright install chromium

# Quick test (5 funds, no detail pages)
npm run scrape:test

# Full Luxembourg scrape with detail pages
npm run scrape:full
```

## Data Captured

### From Fund List Page
- Fund name
- ISIN
- Share class type (A, I, W, AT, IT, etc.)
- Currency
- Asset class (Equity, Fixed Income, Multi Asset, etc.)
- NAV and NAV date
- SFDR category (if displayed)

### From Detail Pages (with `--details` flag)
- Investment objective/mandate
- SFDR classification (Article 6/8/9)
- Sustainability indicators (SRI/ESG)
- Total Expense Ratio (TER)
- Inception date
- Benchmark
- Document links:
  - KIID (Key Investor Information Document)
  - Prospectus
  - Factsheet
  - Annual/Semi-annual reports

## Commands

| Command | Description |
|---------|-------------|
| `npm run scrape` | Scrape Luxembourg fund list |
| `npm run scrape:lu` | Scrape Luxembourg |
| `npm run scrape:ie` | Scrape Ireland |
| `npm run scrape:de` | Scrape Germany |
| `npm run scrape:details` | Include detail page scraping |
| `npm run scrape:full` | Luxembourg with full detail scraping |
| `npm run scrape:test` | Test mode (5 funds with details) |
| `npm run to-dsl` | Convert JSON to ob-poc DSL |

## CLI Options

```bash
node scrape-allianz-enhanced.js [options]

Options:
  -j, --jurisdiction <code>  Jurisdiction (LU, IE, DE) [default: LU]
  -d, --details              Scrape individual fund detail pages
  -t, --test                 Test mode (first 5 funds only)
  -m, --max <n>              Maximum funds to scrape
  -v, --verbose              Verbose output
  --headed                   Run browser in headed mode (visible)
```

## Output Format

```json
{
  "metadata": {
    "scrapedAt": "2024-12-17T10:30:00.000Z",
    "jurisdiction": "LU",
    "manco": {
      "name": "Allianz Global Investors GmbH",
      "jurisdiction": "DE",
      "regNumber": "B-159495"
    }
  },
  "umbrellas": [
    {
      "name": "Allianz Global Investors Fund",
      "legalStructure": "SICAV",
      "jurisdiction": "LU",
      "funds": [...]
    }
  ],
  "funds": [
    {
      "name": "Allianz Income and Growth",
      "assetClass": "Multi Asset",
      "sfdrCategory": "Article 8",
      "shareClasses": [
        {
          "isin": "LU0820561818",
          "type": "AM",
          "currency": "USD"
        }
      ]
    }
  ],
  "rawEntries": [...]
}
```

## Pipeline: Scrape → Import

### 1. Scrape fund data
```bash
npm run scrape:full
# Output: output/allianz-lu-2024-12-17.json
```

### 2. Convert to DSL
```bash
npm run to-dsl output/allianz-lu-2024-12-17.json > ../examples/allianz-import.dsl
```

### 3. Run in ob-poc Agent REPL
```
> load ../examples/allianz-import.dsl
> execute
```

## Management Company Details

### Luxembourg (LU)
- **ManCo**: Allianz Global Investors GmbH, Luxembourg Branch
- **Reg Number**: B-159495
- **Regulator**: CSSF
- **Depositary**: State Street Bank International GmbH, Luxembourg Branch

### Ireland (IE)
- **ManCo**: Allianz Global Investors Ireland Limited
- **Reg Number**: 332926
- **Regulator**: Central Bank of Ireland

### Germany (DE)
- **ManCo**: Allianz Global Investors GmbH
- **Reg Number**: HRB 9340
- **Regulator**: BaFin

## Troubleshooting

### Browser doesn't launch
```bash
npx playwright install chromium
```

### Consent dialogs blocking
Use `--headed` flag to see what's happening:
```bash
node scrape-allianz-enhanced.js --headed --test
```

### Timeouts on slow network
The scraper has 30s page load timeout. If needed, edit `CONFIG.delays` in the script.

### Missing data in output
Detail pages may have different layouts per fund. Check the `errors` array in output.

## Re-running Scrapes

The scraper outputs timestamped files (`allianz-lu-2024-12-17.json`), so you can:
1. Run scrapes periodically
2. Compare outputs for changes
3. Track fund launches/mergers/liquidations

## Data Usage in ob-poc

The scraped data provides:

| Allianz Data | ob-poc Entity | Notes |
|--------------|---------------|-------|
| ManCo | `entity` (MANCO role) | One per jurisdiction |
| Fund (sub-fund) | `cbu` | Main onboarding unit |
| Share Class | CBU attribute | Multiple per fund |
| SFDR Category | CBU attribute | Regulatory classification |
| Documents | Linked references | For KYC/due diligence |
