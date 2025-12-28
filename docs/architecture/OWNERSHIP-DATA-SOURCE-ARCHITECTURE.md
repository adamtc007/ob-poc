# Ownership Data Source Architecture

## Purpose

This document provides a comprehensive technical reference for data sources used to verify corporate ownership chains, Ultimate Beneficial Ownership (UBO), and entity identity in the context of KYC/AML compliance. It covers regulatory frameworks, API access, data quality characteristics, and practical integration patterns.

**Key Insight:** No single data source provides complete ownership information. A multi-source strategy is essential for:
- Verifying 100% ownership at each level of the chain
- Identifying intermediate holding companies
- Detecting corporate restructuring
- Reconciling conflicting data

---

## Data Source Hierarchy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DATA SOURCE PRIORITY                                │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 1: Regulatory Mandated (High Confidence)                               │
│ ├── GLEIF LEI Database          - Entity identity + consolidation chains    │
│ ├── Company Registries          - Legal structure, PSC/UBO registers        │
│ └── Stock Exchange Filings      - Major shareholder notifications           │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 2: Audited Disclosures (Medium-High Confidence)                        │
│ ├── Annual Reports              - Subsidiary lists, ownership percentages   │
│ ├── Fund Prospectuses           - ManCo, depositary, auditor details        │
│ └── SEC Filings (US)            - 10-K, 13F, proxy statements               │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 3: Aggregated Commercial Data (Variable Confidence)                    │
│ ├── OpenCorporates              - 200M+ companies, multi-registry           │
│ ├── Bureau van Dijk (Orbis)     - Complete corporate structures             │
│ └── Bloomberg/Refinitiv         - Ownership trees with percentages          │
├─────────────────────────────────────────────────────────────────────────────┤
│ TIER 4: Self-Reported (Lower Confidence)                                    │
│ ├── Client declarations         - Requires corroboration                    │
│ └── Website disclosures         - May be outdated                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## GLEIF: The Foundation Layer

### What Is GLEIF?

The Global Legal Entity Identifier Foundation operates the global LEI system, providing a unique identifier for every legal entity participating in financial transactions.

### Regulatory Framework

| Regulation | Jurisdiction | LEI Requirement | Effective Date |
|------------|--------------|-----------------|----------------|
| **MiFID II / MiFIR** | EU | MANDATORY - "No LEI, No Trade" | Jan 2018 |
| **EMIR** | EU | MANDATORY for derivative counterparties | Nov 2017 |
| **SFTR** | EU | MANDATORY for securities financing | Apr 2020 |
| **MAR** | EU | MANDATORY for issuers on regulated markets | Jul 2016 |
| **Dodd-Frank** | US | MANDATORY for swaps reporting | 2012 |

**Key Point:** If an entity trades derivatives, issues securities on a regulated market, or is a counterparty to reportable financial transactions, it MUST have an LEI.

### LEI Data Levels

#### Level 1: "Who Is Who" (Entity Identity)

| Field | Description | Verification |
|-------|-------------|--------------|
| LEI | 20-character ISO 17442 identifier | Guaranteed unique |
| Legal Name | Official registered name | Verified vs registry |
| Jurisdiction | Country of registration | Commercial registry |
| Registration Authority | Local registry (e.g., HRB, Companies House) | Verified |
| Registration Number | Local company number | Verified |
| Legal Address | Registered office | Verified |
| HQ Address | Operational headquarters | Self-reported |
| Entity Status | ACTIVE, INACTIVE, etc. | Verified |
| Entity Category | FUND, BRANCH, GENERAL, etc. | Verified |

#### Level 2: "Who Owns Whom" (Relationship Data)

**Critical Understanding:** Level 2 captures **accounting consolidation**, NOT shareholding percentages.

| Field | Description | What It Means |
|-------|-------------|---------------|
| Direct Parent LEI | Immediate consolidating parent | 100% ownership for consolidation |
| Ultimate Parent LEI | Apex consolidating entity | Top of consolidation chain |
| Relationship Type | IS_DIRECTLY_CONSOLIDATED_BY | Binary: consolidated or not |
| Relationship Status | ACTIVE, INACTIVE | Current validity |
| Corroboration Level | FULLY_CORROBORATED, PARTIALLY, etc. | Verification quality |
| Accounting Period | Start/end dates | Reporting period |

**What GLEIF Does NOT Provide:**
- Shareholding percentages (e.g., "BlackRock owns 6.9%")
- Shareholder registers for public companies
- Beneficial ownership below 100% control
- Entities without LEIs in the ownership chain

### GLEIF Reporting Exceptions

When a parent relationship cannot be reported, these exceptions apply:

| Exception Code | Meaning | KYC Implication |
|----------------|---------|-----------------|
| `NO_KNOWN_PERSON` | Dispersed public ownership | UBO TERMINUS - no single >25% owner |
| `NATURAL_PERSONS` | Owned by individuals directly | Look up PSC/UBO register |
| `NON_CONSOLIDATING` | Parent doesn't consolidate | Check actual ownership separately |
| `NON_PUBLIC` | Legal obstacles to disclosure | Requires manual investigation |
| `NO_LEI` | Parent refuses to get LEI | Check registry for parent identity |

### GLEIF API Reference

**Base URL:** `https://api.gleif.org/api/v1`

```bash
# Get entity by LEI
GET /lei-records/{lei}

# Get direct parent entity
GET /lei-records/{lei}/direct-parent

# Get parent relationship with corroboration
GET /lei-records/{lei}/direct-parent-relationship

# Get reporting exception reason
GET /lei-records/{lei}/direct-parent-reporting-exception

# Get subsidiaries (direct children)
GET /lei-records/{lei}/direct-children

# Get managed funds
GET /lei-records/{lei}/managed-funds

# Search by name
GET /lei-records?filter[entity.legalName]=Allianz

# Search by jurisdiction
GET /lei-records?filter[entity.jurisdiction]=DE&page[size]=100
```

**Rate Limits:** No authentication required. Be respectful (max ~1 req/sec).

### GLEIF Data Quality

| Aspect | Quality | Notes |
|--------|---------|-------|
| Entity Identity | HIGH | Verified against commercial registries |
| Consolidation Chain | HIGH | Based on audited financial statements |
| Ownership % | NOT AVAILABLE | Never provided |
| Timeliness | GOOD | Annual renewal required |
| Completeness | VARIABLE | Only entities with LEIs included |

---

## Company Registries

### UK Companies House

**Gold standard** for beneficial ownership transparency.

**Coverage:** All UK-incorporated companies + overseas companies with UK presence

**Access:** Free API + bulk data downloads

**Key Data:**

| Endpoint | Data Provided | Access |
|----------|---------------|--------|
| `/company/{number}` | Basic profile | Free |
| `/company/{number}/filing-history` | All filed documents | Free |
| `/company/{number}/officers` | Directors, secretaries | Free |
| `/company/{number}/persons-with-significant-control` | **UBO Register** | Free |

**PSC (Persons with Significant Control) Register:**

UK law requires disclosure of anyone with >25% shares OR voting rights OR control.

```json
{
  "kind": "corporate-entity-person-with-significant-control",
  "name": "Allianz Se",
  "address": {
    "premises": "28 Koeniginstrasse",
    "locality": "Munich",
    "country": "Germany"
  },
  "natures_of_control": [
    "ownership-of-shares-75-to-100-percent",
    "voting-rights-75-to-100-percent",
    "right-to-appoint-and-remove-directors"
  ],
  "notified_on": "2023-05-23",
  "ceased_on": null
}
```

**Critical Feature:** PSC register shows HISTORY of ownership changes with dates.

**API Registration:** https://developer.company-information.service.gov.uk/

### German Handelsregister

**Coverage:** All German GmbH, AG, KG, etc.

**Access:** Pay-per-document (€4.50), no bulk API

**Key Data:**
- Articles of association
- Shareholder lists (for GmbH)
- Management board appointments
- Commercial register extracts

**Access Point:** https://www.handelsregister.de/

**Limitation:** No free API. Must purchase individual documents.

### Luxembourg Business Registers (RCSL)

**Coverage:** All Luxembourg entities including SICAVs, SARLs

**Access:** Free search, document fees apply

**URL:** https://www.lbr.lu/

### OpenCorporates (Aggregator)

**Coverage:** 200M+ companies across 140+ jurisdictions

**Access:** Free search, paid API for bulk access

**Value:** Single interface across multiple registries

**URL:** https://opencorporates.com/

**API:** https://api.opencorporates.com/

---

## Regulatory Major Shareholder Notifications

### Germany: BaFin Transparency Register

**Legal Basis:** WpHG §§33-39 (Securities Trading Act)

**Thresholds:** 3%, 5%, 10%, 15%, 20%, 25%, 30%, 50%, 75%

**Requirement:** Notify BaFin + issuer within 4 trading days of crossing threshold

**Search Database:** https://portal.mvp.bafin.de/database/AnsichtStimmrecht/

**Example (Allianz SE shareholders):**
| Shareholder | % Voting Rights | Date |
|-------------|-----------------|------|
| BlackRock Inc. | ~6.9% | Latest notification |
| Eurizon Capital SGR | ~4.5% | Latest notification |

### UK: FCA Disclosure & Transparency Rules

**Legal Basis:** DTR 5.1

**Thresholds:** 3%, 4%, 5%, 6%, 7%, 8%, 9%, 10%, then each 1% up to 100%

### US: SEC 13F Filings

**Legal Basis:** Securities Exchange Act §13(f)

**Requirement:** Institutional managers with >$100M must file quarterly

**Access:** https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&type=13F

---

## Annual Report Disclosures

### German HGB Requirements

Under HGB §313, parent companies must disclose in consolidated accounts:
- List of all subsidiaries
- Ownership percentage in each
- Equity held
- Consolidation method

**Example Statement in Subsidiary Accounts:**
```
"The company is a wholly owned subsidiary of Allianz SE, Munich."
```

This statement is **audited** and provides high confidence.

### Finding Subsidiary Lists

| Company Type | Where to Look |
|--------------|---------------|
| German AG/SE | Annual Report, Note 52 typically |
| UK PLC | Annual Report, Note on subsidiaries |
| US Corporation | 10-K, Exhibit 21 (Subsidiaries) |
| Luxembourg SICAV | Annual Report, Note on sub-funds |

---

## Multi-Source Reconciliation Strategy

### The Problem

Different sources may show different ownership structures:

| Source | Shows |
|--------|-------|
| GLEIF | AllianzGI GmbH → Allianz SE (direct) |
| Seed File | AllianzGI → Allianz AM → Allianz SE |
| Annual Report | Full subsidiary list |

### Resolution Approach

```
1. START with GLEIF (highest authority for consolidation)
   └── Extract consolidation chain

2. SUPPLEMENT with Annual Report
   └── Get actual ownership percentages
   └── Identify intermediate holdings without LEIs

3. VERIFY with Company Registries
   └── UK: PSC register for >25% owners
   └── DE: Handelsregister for shareholder lists

4. CROSS-REFERENCE Regulatory Filings
   └── BaFin for >3% shareholders of German AGs
   └── SEC for US-listed entities

5. RECONCILE Discrepancies
   └── Prefer audited sources over self-reported
   └── Check dates - may reflect restructuring
   └── Document source for each data point
```

### Data Provenance Model

```rust
pub struct OwnershipRecord {
    pub owner_entity_id: EntityId,
    pub owned_entity_id: EntityId,
    pub percentage: f64,
    pub source: DataSource,
    pub source_date: NaiveDate,
    pub corroboration: CorroborationLevel,
    pub notes: Option<String>,
}

pub enum DataSource {
    Gleif,
    UkCompaniesHouse,
    GermanHandelsregister,
    AnnualReport { year: i32 },
    BafinNotification,
    SecFiling { form: String },
    ClientDeclaration,
}

pub enum CorroborationLevel {
    FullyCorroborated,    // Verified against multiple sources
    PartiallyCorroborated, // Single authoritative source
    SelfReported,         // Client declaration only
    Uncorroborated,       // No verification
}
```

---

## Practical Example: Allianz Ownership Verification

### Step 1: GLEIF Query

```bash
# Get AllianzGI GmbH
curl "https://api.gleif.org/api/v1/lei-records/OJ2TIQSVQND4IZYYK658"

# Result: Parent is Allianz SE (LEI: 529900K9B0N5BT694847)
# Corroboration: FULLY_CORROBORATED
# Relationship: IS_DIRECTLY_CONSOLIDATED_BY
```

### Step 2: Check Allianz SE Parent

```bash
# Get Allianz SE parent
curl "https://api.gleif.org/api/v1/lei-records/529900K9B0N5BT694847/direct-parent-reporting-exception"

# Result: NO_KNOWN_PERSON
# Meaning: Publicly traded, dispersed ownership, no single >25% holder
# KYC Status: UBO TERMINUS
```

### Step 3: UK Companies House PSC (for UK subsidiary)

```
Company: ALLIANZ GLOBAL INVESTORS UK LIMITED (11516839)

PSC History:
┌──────────────────────────────────┬───────────────────┬───────────────────┐
│ Controller                       │ From              │ To                │
├──────────────────────────────────┼───────────────────┼───────────────────┤
│ Allianz SE                       │ 23 May 2023       │ Current           │
│ Allianz Global Investors GmbH    │ 20 Sep 2021       │ 23 May 2023       │
│ Allianz Global Investors Holdings│ 14 Aug 2018       │ 20 Sep 2021       │
└──────────────────────────────────┴───────────────────┴───────────────────┘

Nature of Control: 75%+ shares, 75%+ votes, right to appoint directors
```

**Insight:** The PSC history reveals corporate restructuring. AllianzGI UK was previously held through intermediaries, now held directly by Allianz SE.

### Step 4: BaFin Major Shareholders (for Allianz SE)

```
Allianz SE Major Shareholders (>3% notifications):

| Shareholder      | % Voting | Source              |
|------------------|----------|---------------------|
| BlackRock Inc.   | ~6.9%    | BaFin notification  |
| Eurizon Capital  | ~4.5%    | BaFin notification  |
| Vanguard Group   | ~3-4%    | SEC 13F             |
| State Street     | ~2-3%    | Institutional filings|
| Other institutional| ~25%   | Estimated           |
| Individual retail| ~59%     | Annual report       |
| TOTAL            | 100%     | ✓                   |
```

**Conclusion:** No single shareholder exceeds 25% → Allianz SE is correctly marked as UBO terminus in GLEIF.

---

## API Integration Scripts

### Location

```
/Users/adamtc007/Developer/ob-poc/scripts/gleif_extract_allianz.py
```

### Capabilities

- Trace ownership chain from any LEI to apex
- Extract parent relationships with corroboration levels
- Fetch direct children (subsidiaries)
- Fetch managed funds
- Generate DSL commands for ob-poc system
- Output JSON for further processing

### Usage

```bash
cd /Users/adamtc007/Developer/ob-poc/scripts
python gleif_extract_allianz.py --lei OJ2TIQSVQND4IZYYK658 --output ../data/derived/gleif/
```

---

## DSL Integration

### Generated DSL Format

```clojure
;; From GLEIF extraction
(cbu.entity:create
    :id @allianz_se
    :legal-name "Allianz SE"
    :lei "529900K9B0N5BT694847"
    :jurisdiction "DE"
    :registration "HRB 164232"
    :legal-form "SGST"
    :status :active
    :source "GLEIF"
    :source-date "2025-07-01")

(cbu.role:assign-ownership
    :owner-entity-id @allianz_se
    :owned-entity-id @allianz_gi
    :percentage 100.0
    :relationship-type :accounting-consolidation
    :source "GLEIF"
    :corroboration :fully-corroborated)

(cbu.ubo:mark-terminus
    :entity-id @allianz_se
    :reason :no-known-person
    :explanation "Publicly traded, dispersed ownership")
```

---

## Appendix: Source URLs

| Source | URL |
|--------|-----|
| GLEIF API | https://api.gleif.org/api/v1 |
| GLEIF Search | https://search.gleif.org/ |
| UK Companies House | https://find-and-update.company-information.service.gov.uk/ |
| UK Companies House API | https://developer.company-information.service.gov.uk/ |
| German Handelsregister | https://www.handelsregister.de/ |
| BaFin Stimmrechte DB | https://portal.mvp.bafin.de/database/AnzeigeStimmrechte/ |
| SEC EDGAR | https://www.sec.gov/edgar/ |
| OpenCorporates | https://opencorporates.com/ |
| Allianz Investor Relations | https://www.allianz.com/en/investor_relations/ |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-07-28 | Claude/Adam | Initial creation from GLEIF deep dive |
