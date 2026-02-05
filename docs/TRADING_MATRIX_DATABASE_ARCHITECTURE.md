# Trading Matrix Database Architecture

**Document Version:** 1.0  
**Last Updated:** 2026-02-05  
**Audience:** Engineering Team  

---

## Executive Summary

The Trading Matrix defines **WHAT a CBU can trade** - the permissioned universe of instruments, markets, currencies, and counterparties. It's the foundation for:
- Trade validation (can this CBU trade EQUITY on XLON in GBP?)
- Settlement routing (which SSI to use for DVP in EUR?)
- OTC counterparty management (which ISDAs/CSAs are in place?)
- Product configuration (what services are enabled per instrument/market?)

**Key Concept:** The trading matrix is a **3-dimensional permission cube**:
```
Instrument Class × Market × Currency = Trading Permission
```

For OTC derivatives, there's a 4th dimension: **Counterparty**.

---

## Architecture Overview

### Schema Organization

| Schema | Purpose | Key Tables |
|--------|---------|------------|
| `custody` | Reference data & settlement | `instrument_classes`, `markets`, `cbu_ssi`, `ssi_booking_rules` |
| `ob-poc` | CBU-specific configuration | `cbu_trading_profiles`, `cbu_instrument_universe`, `cbu_matrix_product_overlay` |

### Core Tables

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        TRADING MATRIX DATA MODEL                             │
│                                                                              │
│  Reference Data (custody)          CBU-Specific (ob-poc / custody)          │
│  ──────────────────────            ───────────────────────────────          │
│  instrument_classes ◄───────────── cbu_instrument_universe                  │
│  markets ◄──────────────────────── cbu_instrument_universe                  │
│  settlement_locations              cbu_trading_profiles                     │
│                                    cbu_matrix_product_overlay               │
│                                                                              │
│  Settlement (custody)              OTC (custody)                            │
│  ────────────────────              ────────────                             │
│  cbu_ssi                           isda_agreements                          │
│  ssi_booking_rules                 csa_agreements                           │
│  cbu_settlement_chains             isda_product_coverage                    │
│  settlement_chain_hops                                                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 1. Reference Data Tables

### 1.1 Instrument Classes (`custody.instrument_classes`)

The master list of tradeable instrument types. Hierarchical with parent/child relationships.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       custody.instrument_classes                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ class_id (PK)              UUID        Unique identifier                    │
│ code                       VARCHAR     EQUITY, FIXED_INCOME, OTC_IRS, etc.  │
│ name                       VARCHAR     Human-readable name                   │
│ default_settlement_cycle   VARCHAR     T+0, T+1, T+2, VARIES                │
│ swift_message_family       VARCHAR     MT5xx family for settlement          │
│ requires_isda              BOOLEAN     True for OTC derivatives             │
│ requires_collateral        BOOLEAN     Needs CSA for margin                 │
│ cfi_category               CHAR(1)     CFI classification (E, D, C, etc.)   │
│ cfi_group                  CHAR(1)     CFI group code                       │
│ smpg_group                 VARCHAR     SMPG classification                  │
│ isda_asset_class           VARCHAR     ISDA taxonomy mapping                │
│ parent_class_id            UUID → instrument_classes (hierarchy)            │
│ is_active                  BOOLEAN     Active flag                          │
│ created_at / updated_at    TIMESTAMPTZ Audit timestamps                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Instrument Class Taxonomy (67 active classes)

| Category | Codes | Settlement | ISDA Required |
|----------|-------|------------|---------------|
| **Equity** | `EQUITY`, `EQUITY_COMMON`, `EQUITY_PREFERRED`, `EQUITY_ETF`, `EQUITY_ADR`, `EQUITY_GDR` | T+1 to T+2 | No |
| **Fixed Income** | `FIXED_INCOME`, `GOVT_BOND`, `CORP_BOND`, `MUNI_BOND`, `COVERED_BOND`, `ABS`, `MBS` | T+1 to T+2 | No |
| **Money Market** | `MONEY_MARKET`, `MMF`, `CD`, `COMMERCIAL_PAPER`, `T_BILL`, `REPO` | T+0 | No |
| **Listed Derivatives** | `EQUITY_FUTURE`, `BOND_FUTURE`, `INDEX_FUTURE`, `EQUITY_OPTION`, `INDEX_OPTION` | T+1 | No |
| **OTC Interest Rates** | `OTC_IRS`, `IRS`, `FRA`, `CAP_FLOOR`, `SWAPTION` | T+0 to T+2 | **Yes** |
| **OTC FX** | `OTC_FX`, `FX_FORWARD`, `FX_SWAP`, `FX_OPTION`, `FX_NDF` | T+2 | **Yes** |
| **OTC Credit** | `OTC_CDS`, `CDS`, `TRS` | T+0 to T+2 | **Yes** |
| **OTC Equity** | `OTC_EQD`, `EQUITY_SWAP`, `VARIANCE_SWAP` | T+2 | **Yes** |
| **Funds** | `CIS`, `MUTUAL_FUND`, `HEDGE_FUND`, `PRIVATE_EQUITY`, `REAL_ESTATE_FUND` | T+2 to T+30 | No |

### 1.3 Markets (`custody.markets`)

The master list of trading venues with settlement infrastructure details.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           custody.markets                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ market_id (PK)             UUID        Unique identifier                    │
│ mic                        VARCHAR     ISO 10383 Market Identifier Code     │
│ name                       VARCHAR     Full market name                     │
│ country_code               VARCHAR     ISO 3166-1 alpha-2                   │
│ operating_mic              VARCHAR     Operating MIC (for segments)         │
│ primary_currency           VARCHAR     Main trading currency                │
│ supported_currencies       ARRAY       All currencies supported             │
│ csd_bic                    VARCHAR     Central Securities Depository BIC    │
│ timezone                   VARCHAR     Market timezone (IANA)               │
│ cut_off_time               TIME        Daily settlement cut-off             │
│ is_active                  BOOLEAN     Active flag                          │
│ created_at / updated_at    TIMESTAMPTZ Audit timestamps                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.4 Key Markets (50+ active)

| Region | MIC Codes | Primary Currencies |
|--------|-----------|-------------------|
| **North America** | `XNYS`, `XNAS`, `ARCX`, `BATS`, `IEXG`, `XTSE` | USD, CAD |
| **Europe** | `XLON`, `XETR`, `XPAR`, `XAMS`, `XBRU`, `XMAD`, `XMIL` | GBP, EUR, CHF |
| **Asia Pacific** | `XHKG`, `XTKS`, `XASX`, `XKRX`, `XSES`, `XBOM` | HKD, JPY, AUD, KRW, SGD, INR |
| **Emerging** | `BVMF`, `XJSE`, `XMEX`, `XIDX` | BRL, ZAR, MXN, IDR |

---

## 2. CBU Trading Configuration

### 2.1 Trading Profiles (`ob-poc.cbu_trading_profiles`)

The master configuration document for what a CBU can trade. Stored as versioned JSONB.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      ob-poc.cbu_trading_profiles                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ profile_id (PK)            UUID        Unique profile identifier            │
│ cbu_id                     UUID → cbus                                      │
│ version                    INTEGER     Version number (increments)          │
│ status                     VARCHAR     DRAFT | SUBMITTED | ACTIVE | SUPERSEDED │
│ document                   JSONB       The trading matrix definition        │
│ document_hash              TEXT        SHA-256 for change detection         │
│ created_by / created_at    VARCHAR / TIMESTAMPTZ                            │
│ submitted_at / submitted_by TIMESTAMPTZ / VARCHAR                           │
│ validated_at / validated_by TIMESTAMPTZ / VARCHAR                           │
│ activated_at / activated_by TIMESTAMPTZ / VARCHAR                           │
│ rejected_at / rejected_by  TIMESTAMPTZ / VARCHAR                            │
│ rejection_reason           TEXT        If rejected                          │
│ superseded_at              TIMESTAMPTZ When replaced by new version         │
│ superseded_by_version      INTEGER     The replacing version                │
│ materialization_status     VARCHAR     PENDING | COMPLETE | FAILED          │
│ materialized_at            TIMESTAMPTZ When expanded to universe            │
│ materialization_hash       TEXT        Hash of materialized state           │
│ sla_profile_id             UUID        SLA configuration                    │
│ source_document_id         UUID → document_catalog                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Profile Status Lifecycle

```
DRAFT ──► SUBMITTED ──► VALIDATED ──► ACTIVE ──► SUPERSEDED
  │           │             │           │
  │           │             │           └─► New version created
  │           │             │
  │           │             └─► (Compliance approved)
  │           │
  │           └─► (Under review)
  │
  └─► (Work in progress)

                REJECTED ◄── (Validation failed)
```

### 2.3 Trading Profile Document Schema (JSONB)

The `document` field contains the full trading matrix definition:

```json
{
  "profile_version": "2.0",
  "effective_date": "2024-01-15",
  
  "instruments": [
    {
      "class": "EQUITY",
      "markets": ["XLON", "XNYS", "XPAR", "XETR"],
      "currencies": ["GBP", "USD", "EUR"],
      "settlement_types": ["DVP", "FOP"],
      "restrictions": {
        "exclude_markets": [],
        "exclude_currencies": [],
        "max_position_size": null
      }
    },
    {
      "class": "FIXED_INCOME",
      "sub_classes": ["GOVT_BOND", "CORP_BOND", "COVERED_BOND"],
      "markets": ["XLON", "XFRA"],
      "currencies": ["GBP", "EUR", "USD"],
      "settlement_types": ["DVP"],
      "min_rating": "BBB-"
    },
    {
      "class": "OTC_IRS",
      "counterparties": [
        {
          "name": "Goldman Sachs International",
          "entity_id": "uuid-...",
          "isda_ref": "GSI-2024-001"
        },
        {
          "name": "JP Morgan Chase Bank",
          "entity_id": "uuid-...",
          "isda_ref": "JPM-2024-001"
        }
      ],
      "currencies": ["USD", "EUR", "GBP"],
      "governing_law": "ISDA_NY",
      "max_tenor_years": 30,
      "notional_limits": {
        "single_trade": 100000000,
        "aggregate": 500000000
      }
    },
    {
      "class": "FX_SPOT",
      "currencies": ["USD", "EUR", "GBP", "JPY", "CHF"],
      "settlement_types": ["GROSS", "NET"]
    }
  ],
  
  "default_settlement": {
    "EQUITY": {
      "cycle": "T+2",
      "method": "DVP",
      "partial_settlement": true
    },
    "FIXED_INCOME": {
      "cycle": "T+2", 
      "method": "DVP",
      "partial_settlement": false
    },
    "OTC_IRS": {
      "cycle": "T+0",
      "method": "NET",
      "collateral_required": true
    }
  },
  
  "global_restrictions": {
    "blocked_countries": ["KP", "IR", "SY"],
    "sanctions_screening": true,
    "esg_exclusions": ["coal_mining", "weapons"]
  }
}
```

### 2.4 Instrument Universe (`custody.cbu_instrument_universe`)

The **materialized** trading permissions. When a trading profile is activated, it's expanded into this table for efficient querying.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     custody.cbu_instrument_universe                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ universe_id (PK)           UUID        Unique entry identifier              │
│ cbu_id                     UUID → ob-poc.cbus                               │
│ instrument_class_id        UUID → instrument_classes                        │
│ market_id                  UUID → markets (NULL for OTC)                    │
│ currencies                 ARRAY       Permitted currencies                 │
│ settlement_types           ARRAY       DVP, FOP, etc.                       │
│ counterparty_entity_id     UUID → entities (for OTC)                        │
│ counterparty_key           UUID        Derived key for uniqueness           │
│ is_held                    BOOLEAN     Can hold positions                   │
│ is_traded                  BOOLEAN     Can execute trades                   │
│ is_active                  BOOLEAN     Active flag                          │
│ effective_date             DATE        When permission starts               │
│ created_at                 TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.5 Matrix Expansion Example

**Profile Definition:**
```json
{
  "class": "EQUITY",
  "markets": ["XLON", "XNYS"],
  "currencies": ["GBP", "USD", "EUR"]
}
```

**Materialized Universe Entries:**
| cbu_id | instrument_class | market | currencies | is_traded |
|--------|------------------|--------|------------|-----------|
| abc123 | EQUITY | XLON | [GBP, USD, EUR] | true |
| abc123 | EQUITY | XNYS | [GBP, USD, EUR] | true |

---

## 3. Settlement Infrastructure

### 3.1 Standing Settlement Instructions (`custody.cbu_ssi`)

SSIs define WHERE to settle - the accounts and intermediaries for each market/currency combination.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           custody.cbu_ssi                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ ssi_id (PK)                UUID        Unique SSI identifier                │
│ cbu_id                     UUID → ob-poc.cbus                               │
│ ssi_name                   VARCHAR     Human-readable name                  │
│ ssi_type                   VARCHAR     SECURITIES | CASH | COLLATERAL       │
│ market_id                  UUID → markets                                   │
│                                                                              │
│ Safekeeping (Securities):                                                   │
│ safekeeping_account        VARCHAR     Account number at custodian          │
│ safekeeping_bic            VARCHAR     Custodian BIC                        │
│ safekeeping_account_name   VARCHAR     Account name                         │
│                                                                              │
│ Cash:                                                                        │
│ cash_account               VARCHAR     Cash account number                  │
│ cash_account_bic           VARCHAR     Cash correspondent BIC               │
│ cash_currency              VARCHAR     Currency of cash account             │
│                                                                              │
│ Collateral:                                                                  │
│ collateral_account         VARCHAR     Margin account number                │
│ collateral_account_bic     VARCHAR     Collateral agent BIC                 │
│                                                                              │
│ Settlement Chain:                                                            │
│ pset_bic                   VARCHAR     Place of Settlement BIC              │
│ receiving_agent_bic        VARCHAR     Receiving agent BIC                  │
│ delivering_agent_bic       VARCHAR     Delivering agent BIC                 │
│                                                                              │
│ Status & Dates:                                                              │
│ status                     VARCHAR     ACTIVE | PENDING | EXPIRED           │
│ effective_date             DATE        When SSI becomes valid               │
│ expiry_date                DATE        When SSI expires (null = no expiry)  │
│ source                     VARCHAR     Manual, SWIFT, OMGEO, etc.           │
│ source_reference           VARCHAR     External reference                   │
│ created_at / updated_at    TIMESTAMPTZ                                      │
│ created_by                 VARCHAR                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 SSI Types

| Type | Purpose | Key Fields |
|------|---------|------------|
| `SECURITIES` | Hold securities positions | `safekeeping_account`, `safekeeping_bic`, `pset_bic` |
| `CASH` | Cash settlement | `cash_account`, `cash_account_bic`, `cash_currency` |
| `COLLATERAL` | OTC margin/collateral | `collateral_account`, `collateral_account_bic` |

### 3.3 SSI Booking Rules (`custody.ssi_booking_rules`)

Rules that determine WHICH SSI to use for a given trade. Priority-ordered, most specific wins.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       custody.ssi_booking_rules                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ rule_id (PK)               UUID        Unique rule identifier               │
│ cbu_id                     UUID → ob-poc.cbus                               │
│ ssi_id                     UUID → cbu_ssi (the target SSI)                  │
│ rule_name                  VARCHAR     Human-readable name                  │
│ priority                   INTEGER     Lower = higher priority              │
│                                                                              │
│ Matching Criteria (all optional, NULL = wildcard):                          │
│ instrument_class_id        UUID → instrument_classes                        │
│ security_type_id           UUID → security_types                            │
│ market_id                  UUID → markets                                   │
│ currency                   VARCHAR     Settlement currency                  │
│ settlement_type            VARCHAR     DVP | FOP | FREE                     │
│ counterparty_entity_id     UUID → entities (for OTC)                        │
│ isda_asset_class           VARCHAR     ISDA taxonomy match                  │
│ isda_base_product          VARCHAR     ISDA product match                   │
│                                                                              │
│ specificity_score          INTEGER     Computed: more criteria = higher     │
│ is_active                  BOOLEAN     Active flag                          │
│ effective_date             DATE        Rule start date                      │
│ expiry_date                DATE        Rule end date                        │
│ created_at / updated_at    TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Rule Matching Algorithm

When routing a trade to an SSI, the system:

1. **Filter** rules where all non-NULL criteria match the trade
2. **Sort** by `priority` ASC, then `specificity_score` DESC
3. **Select** the first matching rule's SSI

**Example Rules (priority order):**

| Priority | Rule Name | Criteria | SSI |
|----------|-----------|----------|-----|
| 10 | Goldman OTC IRS | counterparty=Goldman, class=OTC_IRS | SSI-GSI-COLL |
| 20 | XLON GBP DVP | market=XLON, currency=GBP, settlement=DVP | SSI-XLON-GBP |
| 30 | XLON Catchall | market=XLON | SSI-XLON-DEFAULT |
| 100 | Default | (none) | SSI-DEFAULT |

### 3.5 Settlement Chains (`custody.cbu_settlement_chains`)

For complex multi-hop settlements, chains define the intermediary path.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      custody.cbu_settlement_chains                           │
├─────────────────────────────────────────────────────────────────────────────┤
│ chain_id (PK)              UUID                                             │
│ cbu_id                     UUID → ob-poc.cbus                               │
│ chain_name                 VARCHAR     e.g., "XLON via Euroclear"           │
│ market_id                  UUID → markets                                   │
│ instrument_class_id        UUID → instrument_classes                        │
│ currency                   VARCHAR                                          │
│ settlement_type            VARCHAR     DVP | FOP                            │
│ is_default                 BOOLEAN     Default chain for this combination   │
│ is_active                  BOOLEAN                                          │
│ effective_date             DATE                                             │
│ notes                      TEXT                                             │
│ created_at / updated_at    TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.6 Settlement Chain Hops (`custody.settlement_chain_hops`)

Individual steps in a settlement chain.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     custody.settlement_chain_hops                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ hop_id (PK)                UUID                                             │
│ chain_id                   UUID → cbu_settlement_chains                     │
│ hop_sequence               INTEGER     Order in chain (1, 2, 3...)          │
│ role                       VARCHAR     CUSTODIAN | SUB_CUSTODIAN | CSD | AGENT │
│ intermediary_entity_id     UUID → entities                                  │
│ intermediary_bic           VARCHAR     SWIFT BIC                            │
│ intermediary_name          VARCHAR     Name for display                     │
│ account_number             VARCHAR     Account at this intermediary         │
│ ssi_id                     UUID → cbu_ssi (SSI at this hop)                 │
│ instructions               TEXT        Special instructions                 │
│ created_at / updated_at    TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.7 Settlement Chain Example

**Chain:** "XLON Equities via Euroclear"

| Hop | Role | Intermediary | BIC | Account |
|-----|------|--------------|-----|---------|
| 1 | CUSTODIAN | State Street Global Custody | SBOSUS3N | 123456 |
| 2 | SUB_CUSTODIAN | State Street UK | SBOSUS3NLND | 789012 |
| 3 | CSD | Euroclear UK & International | CABORB2L | CREST-001 |

---

## 4. OTC Derivatives Infrastructure

### 4.1 ISDA Agreements (`custody.isda_agreements`)

Master agreements for OTC derivative trading.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        custody.isda_agreements                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ isda_id (PK)               UUID        Unique ISDA identifier               │
│ cbu_id                     UUID → ob-poc.cbus                               │
│ counterparty_entity_id     UUID → ob-poc.entities                           │
│ agreement_date             DATE        Date agreement signed                │
│ governing_law              VARCHAR     NY | ENGLISH | GERMAN | FRENCH       │
│ is_active                  BOOLEAN     Active flag                          │
│ effective_date             DATE        When agreement becomes effective     │
│ termination_date           DATE        Termination date (if any)            │
│ created_at / updated_at    TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 CSA Agreements (`custody.csa_agreements`)

Credit Support Annexes for collateral/margin.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         custody.csa_agreements                               │
├─────────────────────────────────────────────────────────────────────────────┤
│ csa_id (PK)                UUID                                             │
│ isda_id                    UUID → isda_agreements (parent ISDA)             │
│ csa_type                   VARCHAR     VM | IM | BILATERAL                  │
│ threshold_amount           NUMERIC     Unsecured exposure threshold         │
│ threshold_currency         VARCHAR     Currency for threshold               │
│ minimum_transfer_amount    NUMERIC     MTA for margin calls                 │
│ rounding_amount            NUMERIC     Rounding for transfers               │
│ collateral_ssi_id          UUID → cbu_ssi (where to post collateral)        │
│ is_active                  BOOLEAN                                          │
│ effective_date             DATE                                             │
│ created_at / updated_at    TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 ISDA Product Coverage (`custody.isda_product_coverage`)

Which instrument classes are covered under each ISDA.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      custody.isda_product_coverage                           │
├─────────────────────────────────────────────────────────────────────────────┤
│ coverage_id (PK)           UUID                                             │
│ isda_id                    UUID → isda_agreements                           │
│ instrument_class_id        UUID → instrument_classes                        │
│ isda_taxonomy_id           UUID → isda_product_taxonomy                     │
│ is_active                  BOOLEAN                                          │
│ created_at                 TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.4 OTC Trading Validation Flow

```
Trade Request: CBU wants to trade OTC_IRS with Goldman Sachs in USD
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 1: Check cbu_instrument_universe                                        │
│ ✓ CBU has OTC_IRS with counterparty_entity_id = Goldman                     │
│ ✓ USD is in currencies array                                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 2: Check isda_agreements                                                │
│ ✓ Active ISDA exists for CBU + Goldman                                      │
│ ✓ Governing law matches (NY)                                                │
└─────────────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 3: Check isda_product_coverage                                          │
│ ✓ OTC_IRS (instrument_class_id) is covered under this ISDA                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 4: Check csa_agreements (if collateral required)                        │
│ ✓ CSA exists with thresholds                                                │
│ ✓ Collateral SSI is valid                                                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                         TRADE PERMITTED ✓
```

---

## 5. Product Overlay System

### 5.1 Matrix Product Overlay (`ob-poc.cbu_matrix_product_overlay`)

Links trading matrix entries to specific products with additional configuration.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                   ob-poc.cbu_matrix_product_overlay                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ overlay_id (PK)            UUID                                             │
│ cbu_id                     UUID → cbus                                      │
│ subscription_id            UUID → cbu_product_subscriptions                 │
│ instrument_class_id        UUID → instrument_classes (NULL = all)           │
│ market_id                  UUID → markets (NULL = all)                      │
│ currency                   VARCHAR     (NULL = all)                         │
│ counterparty_entity_id     UUID → entities (NULL = all)                     │
│ status                     VARCHAR     ACTIVE | PENDING | DISABLED          │
│ additional_services        JSONB       Extra services for this combination  │
│ additional_slas            JSONB       SLA overrides                        │
│ additional_resources       JSONB       Resource requirements                │
│ product_specific_config    JSONB       Product-specific settings            │
│ created_at / updated_at    TIMESTAMPTZ                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Overlay Matching Logic

Overlays match on **any subset** of criteria:

| Overlay | Matches |
|---------|---------|
| `instrument_class=EQUITY, market=NULL` | All EQUITY regardless of market |
| `instrument_class=NULL, market=XLON` | All instruments on XLON |
| `instrument_class=OTC_IRS, counterparty=Goldman` | IRS with Goldman only |

---

## 6. Key Views

### 6.1 Effective Matrix View (`v_cbu_matrix_effective`)

The unified view combining universe + product overlays.

```sql
-- Simplified structure
SELECT 
    u.cbu_id,
    c.name AS cbu_name,
    ic.code AS instrument_class,
    m.mic AS market,
    u.currencies,
    u.counterparty_entity_id,
    e.name AS counterparty_name,
    u.is_held,
    u.is_traded,
    -- Aggregated product overlays as JSONB array
    COALESCE(jsonb_agg(overlay_details), '[]') AS product_overlays,
    COUNT(overlays) AS overlay_count
FROM custody.cbu_instrument_universe u
JOIN ob-poc.cbus c ON c.cbu_id = u.cbu_id
JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
LEFT JOIN custody.markets m ON m.market_id = u.market_id
LEFT JOIN ob-poc.entities e ON e.entity_id = u.counterparty_entity_id
LEFT JOIN product_overlays po ON (matching_logic)
WHERE u.is_active = true
GROUP BY u.universe_id, ...;
```

---

## 7. Complete Data Flow

### 7.1 Trading Profile Lifecycle

```
1. PROFILE CREATION (DRAFT)
   └── User creates trading profile with instrument/market/currency definitions
   
2. SUBMISSION (SUBMITTED)
   └── Profile submitted for review
   
3. VALIDATION (VALIDATED)
   └── Compliance checks: sanctions, restrictions, ISDA coverage
   
4. ACTIVATION (ACTIVE)
   └── Profile activated, triggers materialization
   
5. MATERIALIZATION
   └── Profile document → Expanded to cbu_instrument_universe
   └── Creates one row per (instrument_class, market, counterparty) combination
   
6. SSI LINKAGE
   └── SSI booking rules evaluated for each universe entry
   └── Settlement routes established
   
7. PRODUCT OVERLAY
   └── Product-specific overlays applied
   └── Additional services/SLAs configured
```

### 7.2 Trade Validation Flow

```
Incoming Trade
      │
      ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Universe Check  │────►│ ISDA Check      │────►│ SSI Resolution  │
│                 │     │ (if OTC)        │     │                 │
│ Is instrument/  │     │                 │     │ Which SSI to    │
│ market/currency │     │ Has valid ISDA? │     │ use for this    │
│ permitted?      │     │ Product covered?│     │ trade?          │
└─────────────────┘     └─────────────────┘     └─────────────────┘
      │                       │                       │
      ▼                       ▼                       ▼
   REJECT              REJECT                  ROUTE TO SSI
   (not permitted)     (no ISDA)               (settlement)
```

---

## 8. Entity Relationship Diagram

```
                                                    ┌──────────────────────┐
                                                    │   instrument_classes │
                                                    │──────────────────────│
                                                    │ class_id (PK)        │
                                                    │ code                 │
                                                    │ requires_isda        │
                                                    └──────────┬───────────┘
                                                               │
                                                               │ N:1
                                                               ▼
┌─────────────┐         ┌─────────────────────────┐         ┌───────────────────────┐
│    cbus     │         │ cbu_trading_profiles    │         │ cbu_instrument_universe│
│─────────────│◄────────│─────────────────────────│         │───────────────────────│
│ cbu_id (PK) │   1:N   │ cbu_id                  │         │ cbu_id                │
│ name        │         │ document (JSONB)        │◄───────►│ instrument_class_id   │
└──────┬──────┘         │ status                  │ expands │ market_id             │
       │                └─────────────────────────┘    to   │ currencies            │
       │                                                    │ counterparty_entity_id│
       │ 1:N                                                └───────────┬───────────┘
       │                                                                │
       ├────────────────────────────────────────────────────────────────┤
       │                                                                │
       ▼                                                                ▼
┌─────────────────┐                                          ┌──────────────────┐
│    cbu_ssi      │◄─────────────────────────────────────────│ ssi_booking_rules│
│─────────────────│                     N:1                  │──────────────────│
│ ssi_id (PK)     │                                          │ ssi_id           │
│ cbu_id          │                                          │ instrument_class │
│ safekeeping_*   │                                          │ market_id        │
│ cash_*          │                                          │ currency         │
│ market_id       │                                          │ priority         │
└─────────────────┘                                          └──────────────────┘
       │
       │ 1:N (for OTC)
       ▼
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│ isda_agreements │◄────────│  csa_agreements │         │isda_product_cov.│
│─────────────────│   1:N   │─────────────────│         │─────────────────│
│ isda_id (PK)    │         │ isda_id         │         │ isda_id         │
│ cbu_id          │         │ threshold_amount│         │ instrument_class│
│ counterparty_id │         │ collateral_ssi  │         └─────────────────┘
│ governing_law   │         └─────────────────┘
└─────────────────┘


                                 ┌───────────────────┐
                                 │      markets      │
                                 │───────────────────│
                                 │ market_id (PK)    │
                                 │ mic               │
                                 │ country_code      │
                                 │ primary_currency  │
                                 │ csd_bic           │
                                 └───────────────────┘
```

---

## 9. Key Queries

### 9.1 Get Full Trading Matrix for a CBU

```sql
SELECT * FROM "ob-poc".v_cbu_matrix_effective
WHERE cbu_id = $1 AND is_traded = true
ORDER BY instrument_class, market;
```

### 9.2 Check if Trade is Permitted

```sql
SELECT EXISTS (
    SELECT 1 FROM custody.cbu_instrument_universe u
    WHERE u.cbu_id = $1
    AND u.instrument_class_id = $2
    AND (u.market_id = $3 OR u.market_id IS NULL)
    AND $4 = ANY(u.currencies)
    AND u.is_active = true
    AND u.is_traded = true
) AS is_permitted;
```

### 9.3 Get SSI for a Trade

```sql
SELECT s.* FROM custody.ssi_booking_rules r
JOIN custody.cbu_ssi s ON s.ssi_id = r.ssi_id
WHERE r.cbu_id = $1
AND r.is_active = true
AND (r.instrument_class_id = $2 OR r.instrument_class_id IS NULL)
AND (r.market_id = $3 OR r.market_id IS NULL)
AND (r.currency = $4 OR r.currency IS NULL)
ORDER BY r.priority ASC, r.specificity_score DESC
LIMIT 1;
```

### 9.4 Get ISDA Coverage for Counterparty

```sql
SELECT ia.*, 
       array_agg(ic.code) AS covered_products
FROM custody.isda_agreements ia
JOIN custody.isda_product_coverage ipc ON ipc.isda_id = ia.isda_id
JOIN custody.instrument_classes ic ON ic.class_id = ipc.instrument_class_id
WHERE ia.cbu_id = $1
AND ia.counterparty_entity_id = $2
AND ia.is_active = true
AND ipc.is_active = true
GROUP BY ia.isda_id;
```

### 9.5 Get Settlement Chain

```sql
SELECT sc.chain_name,
       sch.hop_sequence,
       sch.role,
       sch.intermediary_name,
       sch.intermediary_bic,
       sch.account_number
FROM custody.cbu_settlement_chains sc
JOIN custody.settlement_chain_hops sch ON sch.chain_id = sc.chain_id
WHERE sc.cbu_id = $1
AND sc.market_id = $2
AND sc.is_active = true
ORDER BY sch.hop_sequence;
```

---

## 10. Summary

### Key Design Principles

1. **Separation of Concerns**
   - Reference data (instruments, markets) in `custody` schema
   - CBU-specific config in `ob-poc` schema
   - Settlement infrastructure separate from trading permissions

2. **Materialization Pattern**
   - JSONB document → Expanded relational tables
   - Enables efficient querying and indexing
   - Document provides audit trail and version history

3. **Priority-Based Rule Matching**
   - Most specific rule wins
   - Wildcard (NULL) matches any value
   - Clear precedence hierarchy

4. **OTC Special Handling**
   - ISDA/CSA required for derivatives
   - Counterparty dimension adds complexity
   - Collateral SSI separate from trading SSI

### Table Counts

| Category | Tables | Purpose |
|----------|--------|---------|
| Reference Data | 4 | instrument_classes, markets, security_types, cfi_codes |
| Trading Config | 4 | cbu_trading_profiles, cbu_instrument_universe, cbu_matrix_product_overlay |
| Settlement | 5 | cbu_ssi, ssi_booking_rules, cbu_settlement_chains, settlement_chain_hops |
| OTC | 4 | isda_agreements, csa_agreements, isda_product_coverage, isda_product_taxonomy |

### Critical Indexes

| Table | Index | Purpose |
|-------|-------|---------|
| `cbu_instrument_universe` | `(cbu_id, instrument_class_id, market_id)` | Trade validation |
| `ssi_booking_rules` | `(cbu_id, priority)` | SSI resolution |
| `isda_agreements` | `(cbu_id, counterparty_entity_id)` | ISDA lookup |
| `cbu_trading_profiles` | `(cbu_id, status)` | Active profile lookup |

---

## Appendix A: Instrument Class Hierarchy

```
ROOT
├── EQUITY
│   ├── EQUITY_COMMON
│   ├── EQUITY_PREFERRED
│   ├── EQUITY_ADR
│   ├── EQUITY_GDR
│   ├── EQUITY_ETF
│   ├── EQUITY_REIT
│   ├── EQUITY_RIGHTS
│   └── EQUITY_WARRANTS
├── FIXED_INCOME
│   ├── GOVT_BOND
│   ├── CORP_BOND
│   ├── MUNI_BOND
│   ├── COVERED_BOND
│   ├── ABS
│   ├── MBS
│   ├── CDO
│   └── CLO
├── MONEY_MARKET
│   ├── MMF
│   ├── CD
│   ├── COMMERCIAL_PAPER
│   ├── T_BILL
│   └── REPO
├── LISTED_DERIVATIVE
│   ├── EQUITY_FUTURE
│   ├── BOND_FUTURE
│   ├── INDEX_FUTURE
│   ├── EQUITY_OPTION
│   └── INDEX_OPTION
├── OTC_DERIVATIVE
│   ├── OTC_IRS (IRS, FRA, CAP_FLOOR, SWAPTION)
│   ├── OTC_FX (FX_FORWARD, FX_SWAP, FX_OPTION, FX_NDF)
│   ├── OTC_CDS (CDS, TRS)
│   └── OTC_EQD (EQUITY_SWAP, VARIANCE_SWAP)
└── CIS (Collective Investment Schemes)
    ├── MUTUAL_FUND
    ├── ETF
    ├── HEDGE_FUND
    ├── PRIVATE_EQUITY
    └── REAL_ESTATE_FUND
```

---

## Appendix B: Settlement Cycle Reference

| Instrument Class | Default Cycle | Notes |
|------------------|---------------|-------|
| EQUITY | T+2 | Most markets (US moving to T+1) |
| GOVT_BOND | T+1 | Varies by market |
| CORP_BOND | T+2 | |
| MONEY_MARKET | T+0 | Same-day settlement |
| OTC_IRS | T+0 | Trade date |
| OTC_FX | T+2 | Standard FX |
| FX_SPOT | T+2 | Two business days |
| LISTED_DERIVATIVE | T+1 | Exchange-cleared |
| MUTUAL_FUND | T+2 to T+30 | Fund-specific |
| HEDGE_FUND | T+30+ | Monthly/quarterly |

---

## Appendix C: SWIFT Message Types

| Instrument Type | Message Family | Common Messages |
|-----------------|----------------|-----------------|
| Securities | MT5xx | MT540 (Receive Free), MT541 (Receive DVP), MT542 (Deliver Free), MT543 (Deliver DVP) |
| Cash | MT2xx | MT202 (Bank Transfer), MT210 (Notice to Receive) |
| FX | MT3xx | MT300 (FX Confirm), MT320 (FX Option) |
| Derivatives | MT3xx | MT360 (IRS Confirm), MT361 (Cross Currency Swap) |
