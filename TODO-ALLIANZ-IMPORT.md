# TODO: Allianz Fund Data Import

## Current State

**Data Already Scraped** (as of 2025-12-16):

| Region | Funds | Share Classes | File |
|--------|-------|---------------|------|
| LU | 205 | 1,977 | `lu_comprehensive.json` |
| DE | ? | ? | `de_comprehensive.json` |
| GB | ? | ? | `gb_comprehensive.json` |
| IE | ? | ? | `ie_comprehensive.json` |
| CH | ? | ? | `ch_comprehensive.json` |

**Fields Currently Captured:**
- ✅ Fund name, ID
- ✅ Asset class (Fixed Income, Equity, Multi Asset)
- ✅ Legal structure (SICAV, FCP)
- ✅ SFDR category (Article 6, 8, 9)
- ✅ Morningstar rating
- ✅ ManCo with LEI, regulator
- ✅ Share classes with ISIN, WKN, currency
- ✅ NAV, NAV date
- ✅ Performance (YTD, 1Y, 3Y, 5Y)
- ✅ Launch date

**Fields NOT Yet Captured (need `--details` flag):**
- ❌ Investment objective/mandate
- ❌ Investment policy
- ❌ Risk profile
- ❌ Sustainability approach
- ❌ Portfolio manager
- ❌ Custodian/Depositary
- ❌ Benchmark
- ❌ Ongoing charges (TER)
- ❌ Management fee
- ❌ Document links (KID, Prospectus, Annual Report)

---

## Phase 1: Run Detail Scrape

### 1.1 Install Dependencies

```bash
cd /Users/adamtc007/Developer/ob-poc/data/allianzgi_seed
npm install
```

### 1.2 Run Comprehensive Scrape with Details

```bash
# Luxembourg only (fastest test)
node scrape-allianzgi.mjs --region LU --details

# All regions with details
node scrape-allianzgi.mjs --details

# With document links too
node scrape-allianzgi.mjs --details --docs
```

**Outputs:** `data/external/allianzgi/<region>_comprehensive.json`

### 1.3 Verify Detail Data

Check that `fund.details` object is populated:
```bash
cat data/external/allianzgi/lu_comprehensive.json | jq '.funds[0].details'
```

Expected:
```json
{
  "investmentObjective": "The Fund aims to...",
  "investmentPolicy": "The Fund invests primarily in...",
  "sfdrClassification": "Article 8",
  "portfolioManager": "Name",
  "custodian": "State Street Bank International GmbH",
  "benchmark": "MSCI World Index"
}
```

---

## Phase 2: Enhanced Scraper (Additional Fields)

If detail page scraping misses fields, enhance `scrape-allianzgi.mjs`:

### 2.1 Additional Regulatory Data

Add to `fetchFundDetails()`:

```javascript
// SFDR Pre-contractual disclosure
const sfdrPreContract = await page.evaluate(() => {
  const link = document.querySelector('a[href*="sfdr"], a[href*="precontractual"]');
  return link?.href || null;
});

// PRIIPS KID link
const kidLink = await page.evaluate(() => {
  const link = document.querySelector('a[href*="kid"], a[href*="kiid"]');
  return link?.href || null;
});

// Prospectus link
const prospectusLink = await page.evaluate(() => {
  const link = document.querySelector('a[href*="prospectus"]');
  return link?.href || null;
});
```

### 2.2 Investment Mandate Parsing

The investment objective often contains the mandate. Parse for key terms:

```javascript
function extractMandate(objective) {
  const mandates = [];
  
  // Equity mandates
  if (/equit(y|ies)/i.test(objective)) mandates.push('EQUITY');
  if (/fixed income|bond/i.test(objective)) mandates.push('FIXED_INCOME');
  if (/multi[- ]?asset/i.test(objective)) mandates.push('MULTI_ASSET');
  if (/money market/i.test(objective)) mandates.push('MONEY_MARKET');
  if (/alternative/i.test(objective)) mandates.push('ALTERNATIVE');
  
  // Geographic mandates
  if (/emerg(ing|ent) market/i.test(objective)) mandates.push('EMERGING_MARKETS');
  if (/europe/i.test(objective)) mandates.push('EUROPE');
  if (/asia|pacific/i.test(objective)) mandates.push('ASIA_PACIFIC');
  if (/u\.?s\.?|united states|america/i.test(objective)) mandates.push('US');
  if (/global|world/i.test(objective)) mandates.push('GLOBAL');
  
  // Strategy mandates  
  if (/growth/i.test(objective)) mandates.push('GROWTH');
  if (/value/i.test(objective)) mandates.push('VALUE');
  if (/income|dividend|yield/i.test(objective)) mandates.push('INCOME');
  if (/small[- ]?cap/i.test(objective)) mandates.push('SMALL_CAP');
  if (/sustainable|sri|esg/i.test(objective)) mandates.push('SUSTAINABLE');
  
  return mandates;
}
```

---

## Phase 3: DSL Generator Enhancement

Update `scripts/generate_allianzgi_dsl.py` to use comprehensive data:

### 3.1 Enhanced Fund Template

```python
def render_fund(fund):
    lines = []
    region = fund['jurisdiction']
    name = fund['fundName']
    cbu_sym = binding_slug(name, region)
    
    # Create CBU with extended attributes
    attrs = [
        f':name "{name}"',
        f':jurisdiction "{region}"',
        f':legal-structure "{fund.get("legalStructure", "SICAV")}"',
        f':sfdr-category "{fund.get("sfdrCategory", "Article 6")}"',
        f':asset-class "{fund.get("assetClass", "")}"',
    ]
    
    if fund.get('details', {}).get('benchmark'):
        attrs.append(f':benchmark "{fund["details"]["benchmark"]}"')
    
    lines.append(f'(cbu.ensure {" ".join(attrs)} :as @{cbu_sym})')
    
    # Share classes as products/variants
    for sc in fund.get('shareClasses', []):
        if sc.get('isin'):
            lines.append(
                f'(cbu.add-share-class :cbu @{cbu_sym} '
                f':isin "{sc["isin"]}" '
                f':name "{sc.get("shareClassName", "")}" '
                f':currency "{sc.get("currency", "EUR")}")'
            )
    
    # Assign ManCo
    lines.append(
        f'(cbu.assign-role :cbu @{cbu_sym} '
        f':entity (company "{fund["managementCompany"]["name"]}") '
        f':role "MANCO")'
    )
    
    lines.append('')
    return lines
```

### 3.2 Entity Pre-creation

Create ManCos and IM entities before CBUs:

```python
def render_mancos(funds):
    mancos = {}
    for fund in funds:
        mc = fund.get('managementCompany', {})
        key = (mc.get('name'), mc.get('jurisdiction'))
        if key[0] and key not in mancos:
            mancos[key] = mc
    
    lines = [';; Management Companies']
    for (name, jur), mc in sorted(mancos.items()):
        lei = mc.get('lei', '')
        lines.append(
            f'(entity.ensure-limited-company '
            f':name "{name}" '
            f':jurisdiction "{jur}" '
            f'{f":lei {lei}" if lei else ""}'
            f')'
        )
    lines.append('')
    return lines
```

---

## Phase 4: Import into ob-poc

### 4.1 Run Generator

```bash
cd /Users/adamtc007/Developer/ob-poc
python scripts/generate_allianzgi_dsl.py

# Output: data/derived/dsl/allianzgi.dsl
```

### 4.2 Load via Agent Session

```
User: "Load the Allianz fund book"
Agent: [reads allianzgi.dsl]
       [triggers entity resolution for ManCos]
       [creates CBUs]
       [assigns roles]
```

### 4.3 Verify

```bash
# Check created entities
curl http://localhost:3000/api/entities?search=allianz | jq '.[] | .name'

# Check CBUs
curl http://localhost:3000/api/cbus?jurisdiction=LU | jq 'length'
```

---

## Phase 5: Scheduled Re-scraping

### 5.1 Create Cron Script

```bash
#!/bin/bash
# scripts/refresh-allianz-data.sh

cd /Users/adamtc007/Developer/ob-poc/data/allianzgi_seed

echo "$(date): Starting Allianz fund refresh..."

# Run scraper for all regions
node scrape-allianzgi.mjs --details 2>&1 | tee -a ../logs/allianz-scrape.log

# Generate DSL
cd ../..
python scripts/generate_allianzgi_dsl.py

echo "$(date): Allianz refresh complete"
```

### 5.2 Add to Crontab (Weekly)

```bash
# Sunday 2am
0 2 * * 0 /Users/adamtc007/Developer/ob-poc/scripts/refresh-allianz-data.sh
```

---

## Data Schema Reference

### Comprehensive JSON Structure

```typescript
interface AllianzFundData {
  metadata: {
    source: string;           // "LU"
    sourceName: string;       // "Luxembourg"
    scrapedAt: string;        // ISO timestamp
    fundCount: number;
    shareClassCount: number;
    managementCompany: ManCo;
  };
  funds: Fund[];
}

interface Fund {
  fundName: string;
  fundId: string;
  assetClass: string;         // "Fixed Income", "Equity", "Multi Asset"
  legalStructure: string;     // "SICAV", "FCP"
  jurisdiction: string;       // "LU", "DE", "GB"
  sfdrCategory: string;       // "Article 6", "Article 8", "Article 9"
  morningstarRating: number | null;
  managementCompany: ManCo;
  shareClasses: ShareClass[];
  details?: FundDetails;      // Only if --details flag used
}

interface ShareClass {
  isin: string;
  wkn: string | null;
  shareClassName: string;     // "A (USD)", "IT (EUR)"
  currency: string;
  nav: number;
  navDate: string;
  launchDate: string | null;
  performance: Performance;
}

interface ManCo {
  name: string;
  branch?: string;
  jurisdiction: string;
  regulator: string;          // "CSSF", "BaFin", "FCA"
  lei?: string;
}

interface FundDetails {
  investmentObjective: string | null;
  investmentPolicy: string | null;
  riskProfile: string | null;
  sfdrClassification: string | null;
  sustainabilityApproach: string | null;
  portfolioManager: string | null;
  managementCompany: string | null;
  custodian: string | null;
  ongoingCharges: string | null;
  managementFee: string | null;
  benchmark: string | null;
  documents: string[];
}
```

---

## ob-poc Entity Mapping

| Allianz Field | ob-poc Entity | ob-poc Field |
|---------------|---------------|--------------|
| `managementCompany.name` | `entity` (company) | `name` |
| `managementCompany.jurisdiction` | `entity` | `jurisdiction` |
| `managementCompany.lei` | `entity` | `lei` |
| `fundName` | `cbu` | `name` |
| `jurisdiction` | `cbu` | `jurisdiction` |
| `legalStructure` | `cbu` | `legal_structure` |
| `sfdrCategory` | `cbu` attribute | `sfdr_classification` |
| `assetClass` | `cbu` attribute | `asset_class` |
| `shareClasses[].isin` | `cbu_share_class` | `isin` |
| `shareClasses[].currency` | `cbu_share_class` | `currency` |
| MANCO role | `cbu_entity_role` | `role_type = MANCO` |
| IM role | `cbu_entity_role` | `role_type = INVESTMENT_MANAGER` |

---

## Files

| File | Purpose |
|------|---------|
| `data/allianzgi_seed/scrape-allianzgi.mjs` | Playwright scraper |
| `data/allianzgi_seed/download-allianzgi-funds.mjs` | Download button scraper |
| `scripts/fetch_allianzgi_funds.py` | Simple Python API scraper |
| `scripts/generate_allianzgi_dsl.py` | DSL generator |
| `data/external/allianzgi/*.json` | Scraped data |
| `data/derived/dsl/allianzgi.dsl` | Generated DSL |
