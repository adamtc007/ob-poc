# Data Extraction Scripts

## GLEIF Scripts

### `gleif_extract_allianz.py`
**Purpose:** Extract complete ownership chain from GLEIF API starting from any LEI.

**Capabilities:**
- Traces ownership chain from starting LEI to apex (ultimate parent)
- Extracts direct parent relationships with corroboration levels  
- Fetches direct children (subsidiaries)
- Fetches managed funds
- Generates DSL commands for ob-poc system
- Outputs JSON for further processing

**Usage:**
```bash
python gleif_extract_allianz.py

# Output files created in:
# - /data/derived/gleif/allianzgi_ownership_chain.dsl
# - /data/derived/gleif/allianzgi_ownership_chain.json
```

### `gleif_allianz_chain.py`
**Purpose:** Simpler chain extraction script for ownership verification.

## Fund Scraping Scripts

### `fetch_allianzgi_funds.py`
**Purpose:** Scrape fund details from Allianz Global Investors website.

### `generate_allianzgi_dsl.py`
**Purpose:** Convert scraped fund data into DSL commands.

## Related Documentation

See `/docs/architecture/OWNERSHIP-DATA-SOURCE-ARCHITECTURE.md` for:
- GLEIF API reference
- Data source hierarchy
- Multi-source reconciliation strategy
- Regulatory framework for LEI requirements

## Data Source Priority

1. **GLEIF** - Entity identity + consolidation chains
2. **Company Registries** - PSC/UBO registers, legal structure
3. **Annual Reports** - Ownership percentages, subsidiary lists
4. **Regulatory Filings** - BaFin >3% notifications, SEC 13F

## Example GLEIF API Calls

```bash
# Get entity by LEI
curl "https://api.gleif.org/api/v1/lei-records/OJ2TIQSVQND4IZYYK658"

# Get direct parent relationship
curl "https://api.gleif.org/api/v1/lei-records/OJ2TIQSVQND4IZYYK658/direct-parent-relationship"

# Get reporting exception (for UBO terminus)
curl "https://api.gleif.org/api/v1/lei-records/529900K9B0N5BT694847/direct-parent-reporting-exception"

# Get managed funds
curl "https://api.gleif.org/api/v1/lei-records/OJ2TIQSVQND4IZYYK658/managed-funds"
```

## Key Findings

### Allianz Ownership Chain (Verified via GLEIF)

```
Allianz SE (DE) - LEI: 529900K9B0N5BT694847
├── UBO TERMINUS: NO_KNOWN_PERSON (publicly traded)
└── 100% (FULLY_CORROBORATED) ──▶ Allianz Global Investors GmbH (DE)
                                   LEI: OJ2TIQSVQND4IZYYK658
                                   └── Managed Funds: 300 (in GLEIF)
```

### UK Companies House PSC Insight

The UK PSC register revealed corporate restructuring:
- May 2023: AllianzGI UK now held directly by Allianz SE
- Previously held through AllianzGI GmbH (2021-2023)  
- Earlier held through AllianzGI Holdings Ltd (2018-2021)

This historical data is not available in GLEIF.
