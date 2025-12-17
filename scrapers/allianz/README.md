# Allianz Fund Scraper

Scrapes fund data from Allianz Global Investors regulatory disclosure site for ob-poc bulk import.

## Data Captured

| Field | Description |
|-------|-------------|
| Fund Name | Sub-fund name |
| ISIN | Per share class |
| ManCo | Management Company |
| Jurisdiction | Fund domicile (LU, IE, DE, UK) |
| SFDR | Article 6/8/9 classification |
| Asset Class | Equity, Fixed Income, Multi Asset, etc. |
| Legal Structure | SICAV, FCP, OEIC |
| Currency | Base currency |
| Share Classes | All share class variants with ISINs |
| Investment Objective | Strategy description |

## Setup

```bash
cd scrapers/allianz
npm install
```

## Usage

```bash
# Scrape Luxembourg funds (default)
npm run scrape

# Scrape specific jurisdiction
npm run scrape -- --jurisdiction=lu   # Luxembourg
npm run scrape -- --jurisdiction=ie   # Ireland  
npm run scrape -- --jurisdiction=de   # Germany
npm run scrape -- --jurisdiction=uk   # UK

# Limit for testing
npm run scrape -- --limit=5

# Custom output directory
npm run scrape -- --output=./data
```

## Output

Creates two files in `./output/`:

1. **JSON** (`allianz-lu-2024-12-17.json`) - Full structured data for ob-poc import
2. **CSV** (`allianz-lu-2024-12-17.csv`) - Quick reference spreadsheet

### JSON Structure

```json
{
  "metadata": {
    "jurisdiction": "LU",
    "manco": "Allianz Global Investors GmbH",
    "scrapedAt": "2024-12-17T...",
    "totalFunds": 150
  },
  "manco": {
    "name": "Allianz Global Investors GmbH",
    "jurisdiction": "DE",
    "type": "LIMITED_COMPANY",
    "roles": ["MANCO", "INVESTMENT_MANAGER"]
  },
  "funds": [
    {
      "name": "Allianz Income and Growth",
      "isin": "LU0264288278",
      "sfdr": "Article 8",
      "assetClass": "Multi Asset",
      "currency": "USD",
      "legalStructure": "SICAV",
      "jurisdiction": "LU",
      "manco": {
        "name": "Allianz Global Investors GmbH",
        "jurisdiction": "DE"
      },
      "shareClasses": [
        { "isin": "LU0264288278", "className": "A", "currency": "USD", "distribution": "Acc" },
        { "isin": "LU0264288351", "className": "I", "currency": "EUR", "distribution": "Dis" }
      ],
      "investmentObjective": "..."
    }
  ]
}
```

## ob-poc Import

Use the DSL template system to bulk import:

```lisp
;; Load scraped data and create CBUs
(for-each $fund in (load-json "allianz-lu-2024-12-17.json" :path "funds")
  (fund-onboard
    :manco "Allianz Global Investors GmbH"
    :fund-name $fund.name
    :isin $fund.isin
    :jurisdiction $fund.jurisdiction
    :products ["custody" "fund-accounting"]))
```

## Notes

- The regulatory site is JavaScript-heavy, hence Playwright
- Be respectful of rate limits - built-in 1s delay between pages
- Re-run periodically to catch new fund launches
- SFDR classification changes over time - re-scrape quarterly
