# Ownership Data Quick Reference

## When To Use Each Source

| Question | Primary Source | Backup Source |
|----------|----------------|---------------|
| "Who is the legal entity?" | GLEIF Level 1 | Company Registry |
| "Who consolidates this entity?" | GLEIF Level 2 | Annual Report |
| "What % does parent own?" | Annual Report | Commercial data |
| "Who are the >25% UBOs?" | PSC Register (UK) | BaFin (DE) |
| "Who are the >3% shareholders?" | BaFin / FCA filings | SEC 13F |
| "Has ownership changed recently?" | UK PSC History | Registry filings |
| "Is this entity trading legitimately?" | GLEIF LEI status | Regulator check |

## Key URLs

```
GLEIF API:       https://api.gleif.org/api/v1
GLEIF Search:    https://search.gleif.org/
UK Companies:    https://find-and-update.company-information.service.gov.uk/
DE Registry:     https://www.handelsregister.de/
BaFin Voting:    https://portal.mvp.bafin.de/database/AnzeigeStimmrechte/
OpenCorporates:  https://opencorporates.com/
```

## UBO Terminus Detection

An entity is a UBO terminus when:

| GLEIF Exception | Meaning | Action |
|-----------------|---------|--------|
| `NO_KNOWN_PERSON` | Public company, dispersed ownership | Mark as terminus |
| `NATURAL_PERSONS` | Owned by individuals | Get names from PSC/UBO register |
| `NON_CONSOLIDATING` | Parent doesn't consolidate | Check registry for actual owner |
| `NO_LEI` | Parent has no LEI | Look up parent in registry |

## Confidence Levels by Source

```
HIGH CONFIDENCE (use for compliance)
├── GLEIF FULLY_CORROBORATED
├── UK PSC Register
├── Audited Annual Reports
└── Regulatory filings (BaFin, FCA, SEC)

MEDIUM CONFIDENCE (verify independently)
├── GLEIF PARTIALLY_CORROBORATED  
├── OpenCorporates
└── Commercial data providers

LOW CONFIDENCE (corroborate required)
├── Client self-declarations
├── Website disclosures
└── GLEIF UNCORROBORATED
```

## DSL Source Attribution Pattern

```clojure
(cbu.role:assign-ownership
    :owner-entity-id @parent
    :owned-entity-id @child
    :percentage 100.0
    :source "GLEIF"              ;; or UK_COMPANIES_HOUSE, ANNUAL_REPORT_2024, etc.
    :source-date "2025-07-01"
    :corroboration :fully-corroborated)
```
