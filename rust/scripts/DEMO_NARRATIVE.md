# GLEIF Research → Onboarding Pivot Demo

## Demo Purpose

Demonstrate how research using the GLEIF (Global LEI Foundation) API can **automatically pivot into onboarding automation** - eliminating manual data entry for fund onboarding.

## The Story

> "We need to onboard Allianz Global Investors. Instead of manually entering data about their funds, investment managers, and corporate structure - we query GLEIF and auto-generate everything."

## Demo Flow (6 Phases)

### Phase 1: Search GLEIF
```clojure
(gleif.search :name "Allianz Global Investors" :limit 5)
```
- Query the global LEI database
- Returns entities matching the search term with their LEIs
- **Key insight**: We're querying real regulatory data, not internal records

### Phase 2: Enrich Entity
```clojure
(gleif.enrich :lei "OJ2TIQSVQND4IZYYK658" :as @allianz-gi)
```
- Fetches full GLEIF record for the entity
- Creates/updates entity in our database with regulatory metadata
- Captures: legal name, jurisdiction, status, legal form, addresses

### Phase 3: Explore Corporate Tree
```clojure
(gleif.get-parent :lei "OJ2TIQSVQND4IZYYK658")
```
- Discovers parent company (Allianz SE)
- Shows the ownership/consolidation chain
- **Demo point**: We can trace UBO chains through GLEIF

### Phase 4: Discover Managed Funds
```clojure
(gleif.get-managed-funds :manager-lei "OJ2TIQSVQND4IZYYK658" :limit 10)
```
- Discovers all funds managed by this investment manager
- Returns fund LEIs, names, jurisdictions
- **Demo point**: One query reveals their entire fund catalog

### Phase 5: THE PIVOT - Auto-Generate Onboarding
```clojure
(gleif.import-managed-funds
  :manager-lei "OJ2TIQSVQND4IZYYK658"
  :create-cbus true
  :limit 5)
```

**This is the key demo moment.**

For each discovered fund, the system automatically:
1. Creates fund entity in `entity_funds` table (with LEI, jurisdiction, GLEIF metadata)
2. Creates CBU (Client Business Unit) 
3. Assigns roles:
   - **ASSET_OWNER**: The fund itself
   - **INVESTMENT_MANAGER**: Allianz Global Investors GmbH
   - **MANAGEMENT_COMPANY**: Allianz Global Investors GmbH
   - **SICAV**: Umbrella fund (if applicable)

### Phase 6: Verify Results
```clojure
(cbu.list :name-contains "Allianz" :limit 10)
```
- Shows the created CBUs
- Each has proper role assignments
- Ready for KYC workflow

## Key Demo Points

### 1. Research → Pivot Pattern
- Start with exploration (search, get-parent, get-managed-funds)
- Pivot to action (import-managed-funds)
- One command creates complete onboarding structure

### 2. Idempotency
- Run the script multiple times - no duplicates
- Entities and CBUs use LEI/name as natural keys
- Safe to re-run during demo

### 3. GLEIF → Our Types Mapping

| GLEIF Data | Our Table | Assigned Roles |
|------------|-----------|----------------|
| Fund (category=FUND) | `entity_funds` | ASSET_OWNER |
| Manager company | `entity_limited_companies` | INVESTMENT_MANAGER, MANAGEMENT_COMPANY |
| Parent company | `entity_limited_companies` | (traced for UBO) |

### 4. Real Data
- All data comes from the live GLEIF API
- 366+ Allianz funds discovered and importable
- Regulatory-grade source of truth

## Running the Demo

```bash
cd rust
./scripts/demo_gleif_research.sh
```

## What's Next After Demo

After import, the created CBUs are ready for:
- KYC case creation
- Document collection
- Screening (PEP, sanctions)
- UBO determination (using the traced ownership chains)

## Technical Details

- **LEI**: Legal Entity Identifier (20-char ISO 17442 code)
- **GLEIF API**: https://api.gleif.org/api/v1/
- **Rate limiting**: Built-in (1 request/sec)
- **Idempotency**: Upsert by LEI for entities, by name for CBUs
