# Allianz Fund Import for ob-poc

## Data Summary (Luxembourg)

| Metric | Count |
|--------|-------|
| **Total Funds** | 205 |
| **Total Share Classes** | 1,977 |

### By Asset Class
- Equity: 76 funds
- Multi Asset: 70 funds
- Fixed Income: 58 funds
- Private Markets: 1 fund

### By SFDR Classification
- Article 8: 131 funds (sustainable investment)
- Article 6: 68 funds (no sustainability claims)
- Article 9: 6 funds (dark green / impact)

### By Legal Structure
- SICAV: 168 funds
- FCP: 35 funds
- Other: 2 funds

---

## Import into ob-poc

### Option 1: DSL Template (Recommended)

Create a template for fund onboarding:

```yaml
# templates/fund-onboard.yaml
name: fund-onboard
description: Onboard a fund with ManCo relationship
params:
  - name: fund_name
    type: string
  - name: isin
    type: string
  - name: manco_name
    type: string
  - name: jurisdiction
    type: string
  - name: asset_class
    type: string
  - name: sfdr
    type: string
body: |
  (block
    ;; Ensure ManCo exists
    (entity.ensure-company
      :name "$manco_name"
      :jurisdiction "$jurisdiction")
    
    ;; Create fund as CBU
    (cbu.create
      :name "$fund_name"
      :type "FUND"
      :jurisdiction "$jurisdiction"
      :isin "$isin"
      :attributes {
        :asset_class "$asset_class"
        :sfdr "$sfdr"
      })
    
    ;; Assign ManCo role
    (cbu.assign-role
      :cbu (cbu "$fund_name")
      :entity (company "$manco_name")
      :role MANCO)
    
    ;; Create standard KYC case
    (kyc-case.create
      :cbu (cbu "$fund_name")
      :case-type "fund-onboard"))
```

### Option 2: Bulk Import Script

Use the scraped JSON directly:

```typescript
// scripts/import-allianz-funds.ts
import funds from '../scrapers/allianz/output/allianz-lu-2025-12-17.json';

async function importFunds() {
  const { manco, funds: fundList } = funds;
  
  // 1. Create ManCo entity
  await createEntity({
    type: 'LIMITED_COMPANY',
    name: manco.name,
    jurisdiction: manco.jurisdiction,
    roles: manco.roles
  });
  
  // 2. Import each fund
  for (const fund of fundList) {
    await createCBU({
      name: fund.name,
      type: 'FUND',
      jurisdiction: fund.jurisdiction,
      attributes: {
        fundId: fund.fundId,
        assetClass: fund.assetClass,
        legalStructure: fund.legalStructure,
        sfdr: fund.sfdr,
        shareClassCount: fund.shareClassCount
      }
    });
    
    // Link to ManCo
    await assignRole({
      cbu: fund.name,
      entity: manco.name,
      role: 'MANCO'
    });
    
    // Import share classes as sub-entities or attributes
    for (const sc of fund.shareClasses) {
      await createShareClass({
        fundName: fund.name,
        isin: sc.isin,
        wkn: sc.wkn,
        className: sc.className,
        currency: sc.currency,
        launchDate: sc.launchDate
      });
    }
  }
}
```

### Option 3: Agent Natural Language

Use the agent REPL:

```
User: Load the Allianz Luxembourg fund data and create CBUs for all Article 8 funds

Agent: I'll load the scraped data and filter for Article 8 funds...
       Found 131 Article 8 funds. Generating DSL...
       
       (block
         ;; ManCo first
         (entity.create-company 
           :name "Allianz Global Investors GmbH"
           :jurisdiction "DE"
           :type LIMITED_COMPANY)
         
         ;; Then 131 funds...
         (cbu.create :name "Allianz Green Bond" :isin "LU0057107152" ...)
         (cbu.create :name "Allianz Europe Equity Growth Select" ...)
         ...
       )
       
       Ready to execute. This will create 1 ManCo + 131 CBUs + 131 KYC cases.
       [Execute] [Review]
```

---

## Files Location

```
ob-poc/scrapers/allianz/
├── package.json
├── scrape.js          # Main scraper
├── debug.js           # Debug utilities
├── README.md
└── output/
    ├── allianz-lu-2025-12-17.json      # Full structured data
    ├── allianz-lu-2025-12-17.csv       # Quick reference
    └── allianz-lu-2025-12-17-raw.json  # Raw API response
```

---

## Re-running the Scrape

```bash
cd ob-poc/scrapers/allianz

# Luxembourg (default)
npm run scrape

# Ireland
npm run scrape -- --jurisdiction=ie

# Germany
npm run scrape -- --jurisdiction=de

# UK
npm run scrape -- --jurisdiction=uk
```

---

## Next Steps

1. **Entity Resolution** - Map ManCo names to existing entities in ob-poc
2. **Fund Detail Scrape** - Get investment objectives, benchmarks, KIIDs
3. **Officers Scrape** - Get fund directors from prospectus PDFs
4. **Automate** - Set up scheduled re-scrape for new fund launches
