# Instrument Matrix & Trading Universe — Design Paper

**Version:** 1.0  
**Date:** 2026-02-11  
**Status:** For Peer Review  
**Audience:** Engineering, Product, Domain Architects  

---

## 1. Purpose

This paper describes the **Instrument Matrix** — the complete model for declaring, configuring, and operating a CBU's trading universe. It covers:

- What instruments, markets, currencies, and counterparties a CBU is permissioned to trade
- How standing settlement instructions (SSIs) route trades to the correct accounts
- How ISDA master agreements and CSAs govern OTC derivatives
- How booking rules match trades to SSIs using ALERT-style priority logic
- How the entire configuration lives as a versioned JSONB document and materializes to operational tables
- The 60+ DSL verb set that drives the full lifecycle

**Key thesis:** The trading universe is not a flat list — it is a **multi-dimensional permission cube** (Instrument × Market × Currency × Counterparty) with settlement routing, collateral management, corporate actions policy, and pricing configuration layered on top. The single source of truth is a versioned JSONB document per CBU that materializes deterministically to 15+ operational tables.

---

## 2. The Permission Cube

Every CBU has a trading universe defined by the intersection of four dimensions:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    THE PERMISSION CUBE                                        │
│                                                                              │
│     Instrument Class                                                         │
│         │                                                                    │
│         ├── EQUITY                                                           │
│         ├── GOVT_BOND         Market (MIC)                                   │
│         ├── CORP_BOND            │                                           │
│         ├── OTC_IRS              ├── XNYS (NYSE)                            │
│         ├── FX_FORWARD           ├── XLON (LSE)                             │
│         └── ...                  ├── XETR (Deutsche Börse)                  │
│                                  └── ...                                     │
│                                                                              │
│     Currency                  Counterparty (4th dim, OTC only)              │
│         │                        │                                           │
│         ├── EUR                  ├── Goldman Sachs                           │
│         ├── USD                  ├── Morgan Stanley                          │
│         ├── GBP                  ├── JP Morgan                              │
│         └── ...                  └── ...                                     │
│                                                                              │
│     For each cell in the cube:                                               │
│     ┌───────────────────────────────────────────────────┐                   │
│     │  - Is this combination permitted? (universe)       │                   │
│     │  - Which SSI settles it? (booking rules → SSI)    │                   │
│     │  - What products are overlaid? (overlay)           │                   │
│     │  - What pricing source? (pricing matrix)           │                   │
│     │  - What CA policy? (corporate actions)             │                   │
│     └───────────────────────────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

For listed instruments (equities, bonds, ETFs), the cube is 3D: **Instrument × Market × Currency**.

For OTC derivatives, a 4th dimension appears: **Counterparty** — because each OTC trade requires a bilateral agreement (ISDA) with a specific counterparty, and collateral flows (CSA) are per-counterparty.

---

## 3. Document-First Architecture

### 3.1 Design Philosophy

The trading profile uses a **document-first** pattern:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DOCUMENT-FIRST ARCHITECTURE                                │
│                                                                              │
│  TradingProfileDocument (JSONB)                                             │
│  ═══════════════════════════════                                            │
│  Single source of truth.                                                    │
│  Versioned. Immutable once activated.                                       │
│  Human-readable YAML seed format.                                           │
│                                                                              │
│         │ trading-profile.materialize                                        │
│         │ (deterministic projection)                                        │
│         ▼                                                                    │
│  ┌──────────────────────────────────────────────────────────────────┐       │
│  │  OPERATIONAL TABLES (15+ tables in custody schema)               │       │
│  │                                                                   │       │
│  │  cbu_instrument_universe  │  cbu_ssi  │  ssi_booking_rules      │       │
│  │  isda_agreements          │  csa_agreements  │  isda_product_*   │       │
│  │  cbu_im_assignments       │  cbu_pricing_config                  │       │
│  │  cbu_ca_preferences       │  cbu_ca_instruction_windows          │       │
│  │  cbu_ca_ssi_mappings      │  cbu_cash_sweep_config               │       │
│  │  subcustodian_network     │  cbu_settlement_chains               │       │
│  └──────────────────────────────────────────────────────────────────┘       │
│                                                                              │
│  Why not write directly to operational tables?                               │
│  - Atomicity: a profile change may touch 10+ tables                         │
│  - Versioning: you can diff v3 vs v4 at the document level                 │
│  - Rollback: revert to v3 = activate v3 + re-materialize                   │
│  - Audit: the document IS the audit record                                  │
│  - Import/export: YAML seed files are just the document format              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Storage

| Table | Schema | Purpose |
|-------|--------|---------|
| `cbu_trading_profiles` | `ob-poc` | Versioned JSONB documents with status state machine |
| `trading_profile_materializations` | `ob-poc` | Audit log of each materialization run |

```sql
CREATE TABLE "ob-poc".cbu_trading_profiles (
    profile_id    UUID PRIMARY KEY,
    cbu_id        UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    version       INTEGER NOT NULL,
    status        VARCHAR(20) NOT NULL,   -- DRAFT, VALIDATED, PENDING_REVIEW, ACTIVE, SUPERSEDED, ARCHIVED
    document      JSONB NOT NULL,         -- TradingProfileDocument
    document_hash VARCHAR(64) NOT NULL,   -- SHA-256 of canonical JSON
    created_by    VARCHAR(100),
    activated_at  TIMESTAMPTZ,
    activated_by  VARCHAR(100),
    notes         TEXT,
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    updated_at    TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (cbu_id, version)
);
```

### 3.3 Document Lifecycle State Machine

```
DRAFT ──► VALIDATED ──► PENDING_REVIEW ──► ACTIVE ──► SUPERSEDED
  │           │              │                            │
  │           │              │                            └──► ARCHIVED
  │           │              └──► (reject) ──► DRAFT
  │           └──► ARCHIVED
  └──► ARCHIVED

State Transitions:
  DRAFT → VALIDATED          validate-go-live-ready passes
  VALIDATED → PENDING_REVIEW submit (sent to client for approval)
  PENDING_REVIEW → ACTIVE    approve (client approves, auto-materialize)
  PENDING_REVIEW → DRAFT     reject (client rejects with reason)
  ACTIVE → SUPERSEDED        new version activated for same CBU
  any → ARCHIVED             archive (soft delete)
```

---

## 4. TradingProfileDocument Structure

The JSONB document is a typed Rust struct (`TradingProfileDocument`) with 11 top-level sections:

```
TradingProfileDocument
├── universe                    What the CBU can trade
│   ├── base_currency           EUR, USD, etc.
│   ├── allowed_currencies      [EUR, USD, GBP, CHF, JPY, ...]
│   ├── allowed_markets[]       MIC + currencies + settlement types
│   └── instrument_classes[]    Class code + CFI prefixes + ISDA asset classes
│
├── investment_managers[]       IM mandates with priority + scope
│   ├── manager (EntityRef)     LEI, BIC, Name, or UUID
│   ├── role                    INVESTMENT_MANAGER, SUB_ADVISOR, etc.
│   ├── scope                   All, or by markets/instrument classes
│   └── instruction_method      SWIFT, CTM, FIX, API, ALERT, MANUAL
│
├── isda_agreements[]           ISDA master agreements (one per counterparty)
│   ├── counterparty (EntityRef)
│   ├── governing_law           NY or ENGLISH
│   ├── product_coverage[]      Asset class + base products
│   └── csa (CsaConfig)        Collateral terms
│       ├── csa_type            VM, VM_IM, IM
│       ├── thresholds          Amount, currency, MTA, rounding
│       ├── eligible_collateral[] Type, currencies, haircuts
│       └── collateral_ssi_ref  → SSI in standing_instructions.OTC_COLLATERAL
│
├── settlement_config           Settlement infrastructure
│   ├── matching_platforms[]    CTM, ALERT with participant IDs + rules
│   ├── settlement_identities[] BIC, LEI, ALERT/CTM participant IDs
│   ├── subcustodian_network[]  Market-specific subcustodians
│   └── instruction_preferences[] SWIFT/ISO20022 message types
│
├── booking_rules[]             ALERT-style SSI selection rules
│   ├── name, priority
│   ├── match                   Instrument, market, currency, counterparty, etc.
│   └── ssi_ref                 → SSI name in standing_instructions
│
├── standing_instructions{}     SSIs by category
│   ├── SECURITIES[]            Custody accounts (safekeeping + cash)
│   ├── CASH[]                  Cash accounts
│   ├── OTC_COLLATERAL[]        Collateral accounts for CSA
│   └── FUND_ACCOUNTING[]       NAV/accounting feeds
│
├── pricing_matrix[]            Pricing source hierarchy
│   ├── scope                   Instrument classes + markets
│   ├── source                  BLOOMBERG, REUTERS, MARKIT, etc.
│   └── fallback + staleness    Fallback source, max age, tolerance
│
├── valuation_config            NAV/valuation settings
│   ├── frequency, cutoff, timezone
│   └── swing_pricing           Threshold-based swing factor
│
├── constraints                 Trading limits
│   ├── short_selling           PROHIBITED, RESTRICTED, PERMITTED
│   └── leverage                Max gross/net leverage ratios
│
├── corporate_actions           CA policy (authored via ca.* verbs)
│   ├── enabled_event_types[]   DVCA, DVOP, RHTS, TEND, MRGR, etc.
│   ├── notification_policy     Channels, SLA hours, escalation
│   ├── election_policy         Who elects, evidence required
│   ├── default_options[]       Default election per event type
│   ├── cutoff_rules[]          Deadline rules per market/depository
│   └── proceeds_ssi_mappings[] Which SSI receives CA proceeds
│
└── metadata                    Provenance
    ├── source, source_ref
    ├── created_by
    └── regulatory_framework    UCITS, AIFMD, SEC, etc.
```

### 4.1 Entity Reference Pattern

Throughout the document, counterparties and managers are referenced using `EntityRef`:

```rust
pub struct EntityRef {
    pub ref_type: EntityRefType,  // LEI, BIC, NAME, UUID
    pub value: String,
}
```

This allows the document to be portable — references are resolved to UUIDs at materialization time via the entity linking service.

---

## 5. Materialization Pipeline

Materialization is the deterministic projection from JSONB document to operational tables. It is triggered by the `trading-profile.materialize` verb.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  MATERIALIZATION PIPELINE                                                    │
│                                                                              │
│  (trading-profile.materialize :profile-id @profile :sections all)           │
│                                                                              │
│  1. Load document from cbu_trading_profiles                                 │
│  2. Resolve EntityRefs → UUIDs (via entity linking)                         │
│  3. For each section (or all):                                              │
│                                                                              │
│  universe ──────────► custody.cbu_instrument_universe                       │
│    Cross-product: instrument_class × market × currency                      │
│    OTC: instrument_class × counterparty (no market)                         │
│                                                                              │
│  standing_instructions ──► custody.cbu_ssi                                  │
│    One row per SSI name per category                                        │
│                                                                              │
│  booking_rules ─────────► custody.ssi_booking_rules                         │
│    Priority-based with specificity_score (GENERATED column)                 │
│                                                                              │
│  isda_agreements ───────► custody.isda_agreements                           │
│                         + custody.isda_product_coverage                     │
│                         + custody.csa_agreements                            │
│                                                                              │
│  investment_managers ───► custody.cbu_im_assignments                        │
│                                                                              │
│  pricing_matrix ────────► custody.cbu_pricing_config                        │
│                                                                              │
│  corporate_actions ─────► custody.cbu_ca_preferences                        │
│                         + custody.cbu_ca_instruction_windows                │
│                         + custody.cbu_ca_ssi_mappings                       │
│                                                                              │
│  settlement_config ─────► custody.subcustodian_network                      │
│                         + custody.cbu_settlement_chains                     │
│                                                                              │
│  4. Write MaterializationResult audit record                                │
│     (sections, records created/updated/deleted, errors, duration)           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.1 Idempotency

Materialization is idempotent — running it twice with the same document produces the same operational state. The pipeline uses:
- `INSERT ... ON CONFLICT DO UPDATE` for upserts
- Deletion of orphaned rows (removed from document but still in operational table)
- `document_hash` comparison to skip unchanged profiles

### 5.2 Selective Materialization

The `sections` argument allows materializing only specific sections:

```clojure
;; Materialize only SSIs and booking rules (fast, after SSI change)
(trading-profile.materialize :profile-id @profile :sections ["ssis", "booking_rules"])

;; Dry run to preview changes
(trading-profile.materialize :profile-id @profile :dry-run true)
```

---

## 6. Settlement Routing — The Three-Layer Model

Settlement routing follows a three-layer architecture:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    THREE-LAYER SETTLEMENT MODEL                              │
│                                                                              │
│  Layer 1: UNIVERSE (What can be traded)                                     │
│  ─────────────────────────────────────                                      │
│  custody.cbu_instrument_universe                                            │
│  Declares: instrument_class × market × currency × counterparty             │
│  Drives: SSI completeness checks, trade validation                         │
│                                                                              │
│  Layer 2: SSI (Where to settle)                                             │
│  ──────────────────────────────                                             │
│  custody.cbu_ssi                                                            │
│  Pure account data: safekeeping account, BIC, cash account                 │
│  No routing logic — just the destination                                    │
│                                                                              │
│  Layer 3: BOOKING RULES (How to route)                                      │
│  ─────────────────────────────────────                                      │
│  custody.ssi_booking_rules                                                  │
│  ALERT-style priority matching:                                             │
│  Given a trade's attributes → which SSI to use?                            │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │  Trade: EQUITY on XLON in GBP, DVP                               │      │
│  │         │                                                         │      │
│  │         ▼                                                         │      │
│  │  Rule 10: EQUITY + XLON + GBP → SSI "UK-EQUITY-DVP"     ← match │      │
│  │  Rule 20: EQUITY + any  + GBP → SSI "GBP-EQUITY"                │      │
│  │  Rule 50: any    + any  + any → SSI "DEFAULT"                    │      │
│  │                                                                   │      │
│  │  Winner: Rule 10 (highest specificity at lowest priority number)  │      │
│  └──────────────────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.1 Specificity Score

The `specificity_score` column is a **GENERATED** column computed from which criteria are populated:

```sql
specificity_score INTEGER GENERATED ALWAYS AS (
    CASE WHEN counterparty_entity_id IS NOT NULL THEN 32 ELSE 0 END +
    CASE WHEN instrument_class_id    IS NOT NULL THEN 16 ELSE 0 END +
    CASE WHEN security_type_id       IS NOT NULL THEN  8 ELSE 0 END +
    CASE WHEN market_id              IS NOT NULL THEN  4 ELSE 0 END +
    CASE WHEN currency               IS NOT NULL THEN  2 ELSE 0 END +
    CASE WHEN settlement_type        IS NOT NULL THEN  1 ELSE 0 END
) STORED
```

| Criterion | Weight | Rationale |
|-----------|--------|-----------|
| Counterparty | 32 | OTC-specific, highest specificity |
| Instrument class | 16 | Asset class is primary discriminator |
| Security type | 8 | Sub-class refinement |
| Market | 4 | Market-specific routing |
| Currency | 2 | Currency-specific accounts |
| Settlement type | 1 | DVP vs FOP vs RVP |

A rule matching on `counterparty + instrument_class + market` scores 52 (32+16+4), beating a rule matching only `instrument_class` (score 16).

### 6.2 SSI Lookup Function

The `custody.find_ssi_for_trade()` function implements the ALERT-style lookup:

```sql
SELECT ssi_id, ssi_name, rule_id, rule_name, rule_priority, specificity_score
FROM custody.ssi_booking_rules r
JOIN custody.cbu_ssi s ON r.ssi_id = s.ssi_id
WHERE r.cbu_id = p_cbu_id
  AND r.is_active = true
  AND s.status = 'ACTIVE'
  AND (r.expiry_date IS NULL OR r.expiry_date > CURRENT_DATE)
  -- NULL = wildcard (matches anything)
  AND (r.instrument_class_id IS NULL OR r.instrument_class_id = p_instrument_class_id)
  AND (r.security_type_id    IS NULL OR r.security_type_id    = p_security_type_id)
  AND (r.market_id           IS NULL OR r.market_id           = p_market_id)
  AND (r.currency            IS NULL OR r.currency            = p_currency)
  AND (r.settlement_type     IS NULL OR r.settlement_type     = p_settlement_type)
  AND (r.counterparty_entity_id IS NULL OR r.counterparty_entity_id = p_counterparty_entity_id)
ORDER BY r.priority ASC
LIMIT 1;
```

**Key design:** NULL in a rule column means "matches anything" — this is the wildcard/catch-all pattern from the ALERT settlement matching standard.

---

## 7. ISDA & CSA — OTC Derivatives Infrastructure

### 7.1 Why ISDA/CSA Are Part of the Trading Universe

ISDA master agreements and CSAs are not separate from the trading matrix — they are the **4th dimension**. Without an ISDA in place with a counterparty, a CBU cannot trade OTC derivatives with them. Without a CSA, collateral cannot flow.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ISDA/CSA IN THE PERMISSION CUBE                           │
│                                                                              │
│  Listed Instruments:                                                        │
│    EQUITY × XLON × GBP = permitted (3D check)                              │
│                                                                              │
│  OTC Derivatives:                                                           │
│    IRS × (no market) × USD × Goldman Sachs = permitted                     │
│    ONLY IF:                                                                 │
│      ✓ ISDA master agreement exists with Goldman Sachs                      │
│      ✓ ISDA product coverage includes RATES/IRS                            │
│      ✓ CSA is in place (for margined products)                             │
│      ✓ Collateral SSI is configured                                        │
│      ✓ Booking rule exists for OTC + Goldman → collateral SSI              │
│                                                                              │
│  The ISDA is the "market access agreement" for OTC —                        │
│  analogous to exchange membership for listed instruments.                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 ISDA Data Model

```
┌────────────────────────────────────────────────────────┐
│  custody.isda_agreements                                │
│  ──────────────────────                                 │
│  isda_id (PK)                                           │
│  cbu_id → cbus                                          │
│  counterparty_entity_id → entities                      │
│  agreement_date, governing_law (NY/ENGLISH)             │
│  effective_date, termination_date                       │
│  is_active                                              │
│                                                         │
│         │ 1:N                                           │
│         ▼                                               │
│  custody.isda_product_coverage                          │
│  ─────────────────────────────                          │
│  coverage_id (PK)                                       │
│  isda_id → isda_agreements                              │
│  instrument_class_id → instrument_classes               │
│  isda_taxonomy_id → isda_product_taxonomy               │
│                                                         │
│         │ 1:0..1                                        │
│         ▼                                               │
│  custody.csa_agreements                                 │
│  ──────────────────────                                 │
│  csa_id (PK)                                            │
│  isda_id → isda_agreements                              │
│  csa_type: VM, VM_IM, IM                                │
│  threshold_amount, threshold_currency                   │
│  minimum_transfer_amount, rounding_amount               │
│  collateral_ssi_id → cbu_ssi                           │
│  is_active                                              │
└────────────────────────────────────────────────────────┘
```

### 7.3 CSA Collateral Flow

```
1. Margin call triggered (threshold breached)
2. CSA determines:
   - Threshold amount (below which no call needed)
   - Minimum transfer amount (MTA)
   - Rounding
   - Eligible collateral (CASH, GOVT_BOND, etc. with haircuts)
3. Collateral transferred to SSI referenced by collateral_ssi_ref
4. The SSI lives in standing_instructions.OTC_COLLATERAL
5. Booking rules route collateral flows to the correct account
```

### 7.4 ISDA Product Taxonomy

The `custody.isda_product_taxonomy` table maps ISDA's 5-level classification to our instrument classes:

| Asset Class | Base Product | Sub Product | Maps To |
|-------------|-------------|-------------|---------|
| RATES | IRS | Fixed-Float | `OTC_IRS` / `IRS` |
| RATES | FRA | — | `FRA` |
| RATES | SWAPTION | — | `SWAPTION` |
| FX | FORWARD | — | `FX_FORWARD` |
| FX | SWAP | — | `FX_SWAP` |
| CREDIT | CDS | Single-Name | `CDS` |
| EQUITY | SWAP | Total-Return | `TRS` |

This enables regulatory reporting (UPI codes) and product coverage validation.

---

## 8. Product Overlay System

Products (CUSTODY, PRIME_BROKERAGE, FUND_ACCOUNTING, etc.) add attributes to matrix entries — they don't define the trading universe. The overlay system layers product-specific services on top of the base matrix.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRODUCT OVERLAY ARCHITECTURE                                                │
│                                                                              │
│  Base Matrix (from TradingProfileDocument):                                 │
│  ┌─────────────────────────────────────────────────────────┐               │
│  │ EQUITY × XLON × GBP = permitted                        │               │
│  │ EQUITY × XNYS × USD = permitted                        │               │
│  │ IRS × Goldman × USD = permitted (ISDA in place)         │               │
│  └─────────────────────────────────────────────────────────┘               │
│                                                                              │
│  Product Overlays (from cbu_matrix_product_overlay):                        │
│  ┌─────────────────────────────────────────────────────────┐               │
│  │ CUSTODY on EQUITY × XLON:                               │               │
│  │   + settlement service                                  │               │
│  │   + corporate actions processing                        │               │
│  │   + income collection                                   │               │
│  │                                                          │               │
│  │ PRIME_BROKERAGE on EQUITY × XNYS:                       │               │
│  │   + margin lending                                      │               │
│  │   + short selling                                       │               │
│  │   + synthetic prime                                     │               │
│  │                                                          │               │
│  │ CUSTODY on IRS × Goldman (NULL market, NULL currency):  │               │
│  │   + OTC clearing support                                │               │
│  │   + collateral management                               │               │
│  └─────────────────────────────────────────────────────────┘               │
│                                                                              │
│  Effective Matrix (v_cbu_matrix_effective view):                            │
│  ┌─────────────────────────────────────────────────────────┐               │
│  │ EQUITY × XLON × GBP                                    │               │
│  │   products: [CUSTODY]                                   │               │
│  │   services: [settlement, CA processing, income]         │               │
│  │   slas: [T+2 settlement, CA notify 24h]                │               │
│  └─────────────────────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 8.1 Overlay Table

```sql
CREATE TABLE "ob-poc".cbu_matrix_product_overlay (
    overlay_id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id                  UUID NOT NULL,
    subscription_id         UUID NOT NULL REFERENCES cbu_product_subscriptions,
    instrument_class_id     UUID REFERENCES custody.instrument_classes,  -- NULL = all
    market_id               UUID REFERENCES custody.markets,             -- NULL = all
    currency                VARCHAR(3),                                   -- NULL = all
    counterparty_entity_id  UUID REFERENCES entities,                    -- NULL = all
    additional_services     JSONB DEFAULT '[]',
    additional_slas         JSONB DEFAULT '[]',
    additional_resources    JSONB DEFAULT '[]',
    product_specific_config JSONB DEFAULT '{}',
    status                  VARCHAR(20) DEFAULT 'ACTIVE',
    -- UNIQUE NULLS NOT DISTINCT for deterministic wildcard matching
    UNIQUE NULLS NOT DISTINCT (cbu_id, subscription_id, instrument_class_id,
                               market_id, currency, counterparty_entity_id)
);
```

**Key design:** `UNIQUE NULLS NOT DISTINCT` means NULL values are treated as equal for uniqueness — so there can be only one overlay per CBU × subscription × (instrument, market, currency, counterparty) combination, even when some dimensions are NULL (wildcard).

---

## 9. Reference Data

### 9.1 Instrument Class Taxonomy (67 Active Classes)

```
EQUITY
├── EQUITY_COMMON
├── EQUITY_PREFERRED
├── EQUITY_ETF
├── EQUITY_ADR
├── EQUITY_GDR
├── EQUITY_REIT
├── EQUITY_WARRANT
├── EQUITY_RIGHT
└── EQUITY_CONVERTIBLE

FIXED_INCOME
├── GOVT_BOND
├── CORP_BOND
├── MUNI_BOND
├── COVERED_BOND
├── INFLATION_LINKED_BOND
├── CONVERTIBLE_BOND
├── HIGH_YIELD_BOND
├── SUKUK
├── ABS / MBS / CDO / CLO

MONEY_MARKET
├── MMF / CD / COMMERCIAL_PAPER / T_BILL / REPO

LISTED_DERIVATIVE
├── EQUITY_FUTURE / BOND_FUTURE / INDEX_FUTURE / COMMODITY_FUTURE / FX_FUTURE
├── EQUITY_OPTION / INDEX_OPTION / COMMODITY_OPTION / FX_OPTION_LISTED

OTC_DERIVATIVE
├── OTC_IRS (IRS, FRA, CAP_FLOOR, SWAPTION, XCCY_SWAP)
├── OTC_FX  (FX_FORWARD, FX_SWAP, FX_OPTION, FX_NDF)
├── OTC_CDS (CDS, TRS, CDX, ITRAXX)
└── OTC_EQD (EQUITY_SWAP, VARIANCE_SWAP, DIVIDEND_SWAP)

CIS (Collective Investment Schemes)
├── MUTUAL_FUND / ETF / HEDGE_FUND / PRIVATE_EQUITY / REAL_ESTATE_FUND
```

Each class carries:
- `default_settlement_cycle` — T+0 to T+2 (listed) or VARIES (funds)
- `swift_message_family` — MT5xx (securities), MT3xx (FX/derivatives)
- `requires_isda` — true for OTC derivatives
- `requires_collateral` — true for margined products
- `cfi_category` / `cfi_group` — ISO 10962 classification
- `isda_asset_class` — ISDA taxonomy mapping (RATES, CREDIT, FX, EQUITY, COMMODITY)

### 9.2 Markets

Markets are identified by **MIC** (Market Identifier Code, ISO 10383):

| MIC | Name | Country | Primary Currency | CSD BIC |
|-----|------|---------|------------------|---------|
| XNYS | New York Stock Exchange | US | USD | — |
| XNAS | NASDAQ | US | USD | — |
| XLON | London Stock Exchange | GB | GBP | CABORB — |
| XETR | Deutsche Börse (Xetra) | DE | EUR | — |
| XPAR | Euronext Paris | FR | EUR | — |
| XHKG | Hong Kong Exchange | HK | HKD | — |
| XTKS | Tokyo Stock Exchange | JP | JPY | — |

---

## 10. Corporate Actions Policy

The CA policy is authored directly in the JSONB document via `trading-profile.ca.*` verbs, then materialized to operational tables for execution.

### 10.1 Event Type Taxonomy (53 ISO Event Types)

| Category | Events | Examples |
|----------|--------|---------|
| INCOME | Dividend, Interest | DVCA (cash dividend), INTR (interest payment) |
| REORGANIZATION | Merger, Split, Reverse Split | MRGR, SPLF, SPLR |
| VOLUNTARY | Tender Offer, Rights, Exchange | TEND, RHTS, EXOF |
| MANDATORY | Name Change, Conversion | CHAN, CONV |
| INFORMATION | Notice, Meeting | MEET, REDO |

### 10.2 Processing Modes

| Mode | Behavior |
|------|----------|
| `AUTO_INSTRUCT` | System makes election automatically using default |
| `MANUAL` | Human intervention required for every election |
| `DEFAULT_ONLY` | Use default if simple, escalate if complex |
| `THRESHOLD` | Auto below threshold, manual above |

### 10.3 Cutoff Rules

Deadline management with market/depository-specific internal cutoffs:

```yaml
cutoff_rules:
  - event_type: TEND
    market_code: XLON
    days_before: 5        # Internal cutoff: 5 days before market deadline
    warning_days: 3       # Warning notification 3 days before cutoff
    escalation_days: 1    # Escalation 1 day before cutoff
```

---

## 11. Additional Operational Tables

### 11.1 Settlement Chains

Multi-hop settlement chains define the intermediary path for complex markets:

```
Trade → Custodian → Subcustodian → Local Agent → CSD
```

| Table | Purpose |
|-------|---------|
| `custody.cbu_settlement_chains` | Chain definition per market/instrument/currency |
| `custody.settlement_chain_hops` | Individual intermediaries in sequence |

### 11.2 Tax Infrastructure

| Table | Purpose |
|-------|---------|
| `custody.tax_jurisdictions` | Tax jurisdictions with withholding rates and reclaim rules |
| `custody.tax_treaty_rates` | Bilateral treaty rates by income type |
| `custody.cbu_tax_status` | CBU tax status per jurisdiction (FATCA/CRS/QI) |
| `custody.cbu_tax_reporting` | Reporting obligations (FATCA, CRS, DAC6, etc.) |
| `custody.cbu_tax_reclaim_config` | Tax reclaim processing rules |

### 11.3 Cash Sweep

| Table | Purpose |
|-------|---------|
| `custody.cbu_cash_sweep_config` | Idle cash investment rules (STIF, MMF, overnight repo) |

---

## 12. DSL Verb Coverage

### 12.1 Trading Profile Verbs (47 verbs)

| Category | Verbs | Count |
|----------|-------|-------|
| **Lifecycle** | `import`, `read`, `get-active`, `list-versions`, `create-draft`, `clone-to`, `create-new-version`, `activate`, `submit`, `approve`, `reject`, `archive` | 12 |
| **Universe** | `add-instrument-class`, `remove-instrument-class`, `add-market`, `remove-market`, `set-base-currency`, `add-allowed-currency` | 6 |
| **SSI** | `add-standing-instruction`, `remove-standing-instruction` | 2 |
| **Booking Rules** | `add-booking-rule`, `remove-booking-rule` | 2 |
| **ISDA/CSA** | `add-isda-config`, `remove-isda-config`, `add-isda-coverage`, `add-csa-config`, `remove-csa-config`, `add-csa-collateral`, `link-csa-ssi` | 7 |
| **IM Mandates** | `add-im-mandate`, `update-im-scope`, `remove-im-mandate` | 3 |
| **Validation** | `validate-go-live-ready`, `validate-universe-coverage` | 2 |
| **Projection** | `materialize`, `diff` | 2 |
| **CA Policy** | `ca.enable-event-types`, `ca.disable-event-types`, `ca.set-notification-policy`, `ca.set-election-policy`, `ca.set-default-option`, `ca.remove-default-option`, `ca.add-cutoff-rule`, `ca.remove-cutoff-rule`, `ca.link-proceeds-ssi`, `ca.remove-proceeds-ssi`, `ca.get-policy` | 11 |

### 12.2 Product & Matrix Overlay Verbs (14 verbs)

| Category | Verbs | Count |
|----------|-------|-------|
| **Subscription** | `product-subscription.subscribe`, `unsubscribe`, `suspend`, `reactivate`, `list` | 5 |
| **Overlay CRUD** | `matrix-overlay.add`, `remove`, `suspend`, `activate`, `list`, `list-by-subscription` | 6 |
| **Analysis** | `matrix-overlay.effective-matrix`, `unified-gaps`, `compare-products` | 3 |

### 12.3 Total Verb Count: 61

---

## 13. DSL Examples

### 13.1 Create and Configure a Trading Profile

```clojure
;; Create a new draft profile for a CBU
(trading-profile.create-draft :cbu-id <Allianz IE ETF SICAV> :as @profile)

;; Define the universe
(trading-profile.set-base-currency :profile-id @profile :currency "EUR")
(trading-profile.add-allowed-currency :profile-id @profile :currency "USD")
(trading-profile.add-allowed-currency :profile-id @profile :currency "GBP")

(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY"
    :cfi-prefixes ["ES" "EP"])
(trading-profile.add-instrument-class :profile-id @profile :class-code "GOVT_BOND"
    :cfi-prefixes ["DB"])

(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XLON")
(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XNYS")
(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XETR")
```

### 13.2 Configure SSIs and Booking Rules

```clojure
;; Add standing settlement instructions
(trading-profile.add-standing-instruction :profile-id @profile
    :ssi-type "SECURITIES" :ssi-name "UK-EQUITY-DVP"
    :safekeeping-account "12345" :safekeeping-bic "BNYGB2L"
    :cash-account "67890" :cash-bic "BNYGB2L"
    :cash-currency "GBP")

(trading-profile.add-standing-instruction :profile-id @profile
    :ssi-type "SECURITIES" :ssi-name "US-EQUITY-DVP"
    :safekeeping-account "23456" :safekeeping-bic "BNYAUS33"
    :cash-account "78901" :cash-bic "BNYAUS33"
    :cash-currency "USD")

;; Add ALERT-style booking rules
(trading-profile.add-booking-rule :profile-id @profile
    :rule-name "UK Equities" :priority 10 :ssi-ref "UK-EQUITY-DVP"
    :match-instrument-class "EQUITY" :match-mic "XLON" :match-currency "GBP")

(trading-profile.add-booking-rule :profile-id @profile
    :rule-name "US Equities" :priority 20 :ssi-ref "US-EQUITY-DVP"
    :match-instrument-class "EQUITY" :match-mic "XNYS" :match-currency "USD")
```

### 13.3 Add OTC Derivatives (ISDA + CSA)

```clojure
;; Add ISDA master agreement with Goldman Sachs
(trading-profile.add-isda-config :profile-id @profile
    :counterparty-entity-id <Goldman Sachs>
    :counterparty-name "Goldman Sachs International"
    :governing-law "ENGLISH" :agreement-date "2024-01-15")

;; Add product coverage (what can be traded under this ISDA)
(trading-profile.add-isda-coverage :profile-id @profile
    :isda-ref "Goldman Sachs International"
    :asset-class "RATES" :base-products ["IRS" "FRA"])

;; Add CSA (collateral agreement)
(trading-profile.add-csa-config :profile-id @profile
    :isda-ref "Goldman Sachs International"
    :csa-type "VM" :threshold-currency "USD"
    :threshold-amount 10000000 :minimum-transfer-amount 500000)

;; Add collateral SSI
(trading-profile.add-standing-instruction :profile-id @profile
    :ssi-type "OTC_COLLATERAL" :ssi-name "GS-COLLATERAL"
    :safekeeping-account "COL001" :safekeeping-bic "GOLDGB2L"
    :cash-account "COL002" :cash-bic "GOLDGB2L")

;; Link CSA to collateral SSI
(trading-profile.link-csa-ssi :profile-id @profile
    :counterparty-ref "Goldman Sachs International"
    :ssi-name "GS-COLLATERAL")
```

### 13.4 Configure Corporate Actions Policy

```clojure
;; Enable CA event types
(trading-profile.ca.enable-event-types :profile-id @profile
    :event-types ["DVCA" "DVOP" "RHTS" "TEND" "MRGR"])

;; Set notification policy
(trading-profile.ca.set-notification-policy :profile-id @profile
    :channels ["email" "portal"] :sla-hours 24
    :escalation-contact "ops@fund.com")

;; Set default elections
(trading-profile.ca.set-default-option :profile-id @profile
    :event-type "DVCA" :default-option "CASH")
(trading-profile.ca.set-default-option :profile-id @profile
    :event-type "DVOP" :default-option "STOCK")

;; Add cutoff rules for UK market
(trading-profile.ca.add-cutoff-rule :profile-id @profile
    :market-code "XLON" :days-before 5 :warning-days 3)
```

### 13.5 Activate and Materialize

```clojure
;; Validate readiness
(trading-profile.validate-go-live-ready :profile-id @profile :strictness "STRICT")

;; Submit for approval
(trading-profile.submit :profile-id @profile :submitted-by "ops-team")

;; Client approves
(trading-profile.approve :profile-id @profile :approved-by "client-pm")

;; Materialize to operational tables
(trading-profile.materialize :profile-id @profile)
```

---

## 14. Entity Relationship Diagram

```mermaid
erDiagram
    cbu_trading_profiles ||--o{ cbu_instrument_universe : "materializes to"
    cbu_trading_profiles ||--o{ cbu_ssi : "materializes to"
    cbu_trading_profiles ||--o{ ssi_booking_rules : "materializes to"
    cbu_trading_profiles ||--o{ isda_agreements : "materializes to"
    cbu_trading_profiles ||--o{ cbu_im_assignments : "materializes to"
    cbu_trading_profiles ||--o{ cbu_pricing_config : "materializes to"
    cbu_trading_profiles ||--o{ cbu_ca_preferences : "materializes to"

    cbu_instrument_universe }o--|| instrument_classes : "references"
    cbu_instrument_universe }o--o| markets : "references"
    cbu_instrument_universe }o--o| entities : "counterparty"

    ssi_booking_rules }o--|| cbu_ssi : "routes to"
    ssi_booking_rules }o--o| instrument_classes : "matches"
    ssi_booking_rules }o--o| security_types : "matches"
    ssi_booking_rules }o--o| markets : "matches"

    isda_agreements ||--o{ isda_product_coverage : "covers"
    isda_agreements ||--o| csa_agreements : "has"
    isda_product_coverage }o--o| instrument_classes : "maps"
    isda_product_coverage }o--o| isda_product_taxonomy : "maps"
    csa_agreements }o--o| cbu_ssi : "collateral account"

    cbu_matrix_product_overlay }o--|| cbu_product_subscriptions : "from"
    cbu_matrix_product_overlay }o--o| instrument_classes : "scoped to"
    cbu_matrix_product_overlay }o--o| markets : "scoped to"

    cbu_settlement_chains ||--o{ settlement_chain_hops : "contains"
    settlement_chain_hops }o--o| settlement_locations : "through"

    cbu_ca_preferences }o--|| ca_event_types : "for"
    cbu_ca_instruction_windows }o--|| ca_event_types : "for"
    cbu_ca_ssi_mappings }o--|| cbu_ssi : "proceeds to"

    cbu_tax_status }o--|| tax_jurisdictions : "in"
    tax_treaty_rates }o--|| tax_jurisdictions : "source"
    tax_treaty_rates }o--|| tax_jurisdictions : "investor"
```

---

## 15. Key Views

### 15.1 v_cbu_matrix_effective

Joins the base universe with product overlays to produce the effective trading matrix:

```sql
-- Simplified structure
WITH matrix_base AS (
    SELECT u.*, ic.code AS instrument_class, m.mic AS market, e.name AS counterparty_name
    FROM custody.cbu_instrument_universe u
    JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
    LEFT JOIN custody.markets m ON m.market_id = u.market_id
    LEFT JOIN entities e ON e.entity_id = u.counterparty_entity_id
    WHERE u.is_active = true
),
product_overlays AS (
    SELECT o.*, p.product_code, p.product_name
    FROM cbu_matrix_product_overlay o
    JOIN cbu_product_subscriptions s ON s.subscription_id = o.subscription_id
    JOIN products p ON p.product_id = s.product_id
    WHERE o.status = 'ACTIVE' AND s.status = 'ACTIVE'
)
SELECT
    mb.*,
    COALESCE(array_agg(DISTINCT po.product_code), '{}') AS products,
    COALESCE(jsonb_agg(po.additional_services), '[]') AS combined_services
FROM matrix_base mb
LEFT JOIN product_overlays po ON mb.cbu_id = po.cbu_id
    AND (po.instrument_class_id IS NULL OR po.instrument_class_id = mb.instrument_class_id)
    AND (po.market_id IS NULL OR po.market_id = mb.market_id)
    AND (po.currency IS NULL OR po.currency = ANY(mb.currencies))
    AND (po.counterparty_entity_id IS NULL OR po.counterparty_entity_id = mb.counterparty_entity_id)
GROUP BY mb.*;
```

---

## 16. Completeness Summary

### 16.1 Schema Coverage

| Domain | Tables | Key Tables |
|--------|--------|------------|
| Reference Data | 5 | `instrument_classes`, `markets`, `security_types`, `settlement_locations`, `isda_product_taxonomy` |
| Trading Profile | 2 | `cbu_trading_profiles`, `trading_profile_materializations` |
| Universe | 1 | `cbu_instrument_universe` |
| Settlement | 6 | `cbu_ssi`, `ssi_booking_rules`, `cbu_settlement_chains`, `settlement_chain_hops`, `cbu_settlement_location_preferences`, `subcustodian_network` |
| SSI Overrides | 1 | `cbu_ssi_agent_override` |
| OTC/ISDA | 3 | `isda_agreements`, `isda_product_coverage`, `csa_agreements` |
| IM Assignments | 1 | `cbu_im_assignments` |
| Pricing | 1 | `cbu_pricing_config` |
| Corporate Actions | 4 | `ca_event_types`, `cbu_ca_preferences`, `cbu_ca_instruction_windows`, `cbu_ca_ssi_mappings` |
| Tax | 5 | `tax_jurisdictions`, `tax_treaty_rates`, `cbu_tax_status`, `cbu_tax_reporting`, `cbu_tax_reclaim_config` |
| Cash | 1 | `cbu_cash_sweep_config` |
| Entity Settlement | 2 | `entity_settlement_identity`, `entity_ssi` |
| Cross-Border | 1 | `cbu_cross_border_config` |
| Product Overlay | 2 | `cbu_product_subscriptions`, `cbu_matrix_product_overlay` |
| **Total** | **35** | |

### 16.2 Verb Coverage

| Domain | Verb Count | Coverage |
|--------|-----------|----------|
| `trading-profile` | 47 | Full lifecycle + authoring + CA policy |
| `product-subscription` | 5 | Subscription CRUD |
| `matrix-overlay` | 9 | Overlay CRUD + analysis |
| **Total** | **61** | |

### 16.3 Rust Type Coverage

| Type | File | Purpose |
|------|------|---------|
| `TradingProfileDocument` | `trading_profile/types.rs` | Root document type |
| `Universe` | `trading_profile/types.rs` | Trading universe config |
| `InstrumentClassConfig` | `trading_profile/types.rs` | Instrument class entry |
| `MarketConfig` | `trading_profile/types.rs` | Market entry |
| `BookingRule` | `trading_profile/types.rs` | ALERT-style routing rule |
| `StandingInstruction` | `trading_profile/types.rs` | SSI account data |
| `IsdaAgreementConfig` | `trading_profile/types.rs` | ISDA master agreement |
| `CsaConfig` | `trading_profile/types.rs` | Credit Support Annex |
| `InvestmentManagerMandate` | `trading_profile/types.rs` | IM mandate with scope |
| `PricingRule` | `trading_profile/types.rs` | Pricing source hierarchy |
| `MaterializationResult` | `trading_profile/types.rs` | Audit record |
| `ProfileStatus` | `trading_profile/types.rs` | Status state machine |
| `EntityRef` | `trading_profile/types.rs` | Portable entity reference |

---

## 17. Open Design Questions (For Peer Review)

1. **Tax materialization**: The tax tables (`cbu_tax_status`, `cbu_tax_reporting`, `cbu_tax_reclaim_config`) exist but are not yet included in the `TradingProfileDocument`. Should tax configuration be added as a document section, or remain as direct operational table writes?

2. **Cash sweep materialization**: `cbu_cash_sweep_config` is partially in the document (`pricing_matrix`, `valuation_config`) but the actual sweep vehicle config is not. Should cash sweep be a document section?

3. **Settlement chain authoring**: Settlement chains (`cbu_settlement_chains`, `settlement_chain_hops`) are complex multi-hop structures. Should they be authored in the document or via dedicated verbs that write directly to operational tables?

4. **Cross-border config**: `cbu_cross_border_config` (bridge vs direct vs ICSD routing) is not yet in the document. This is market-pair-specific routing that could be part of settlement_config.

5. **Entity SSI integration**: `custody.entity_ssi` (counterparty-level SSIs) exists separately from CBU SSIs. Should the document reference entity SSIs for OTC booking rules?

6. **Multi-version materialization**: Currently only the ACTIVE profile is materialized. Should we support materializing a DRAFT for "what-if" analysis in a sandbox schema?

---

## Appendix A: Related Architecture Documents

| Document | Location | Coverage |
|----------|----------|----------|
| Trading Matrix Database Architecture v1.1 | `migrations/TRADING_MATRIX_DATABASE_ARCHITECTURE_v1_1.md` | Schema-level detail, ER diagrams, key queries |
| Schema Entity Overview | `migrations/OB_POC_SCHEMA_ENTITY_OVERVIEW.md` | Full schema overview with mermaid diagrams |
| Verb Definition Spec | `docs/verb-definition-spec.md` | YAML verb authoring guide |
| ob-agentic Pipeline | CLAUDE.md §Structured Onboarding Pipeline | Multi-entity onboarding with universe derivation |

## Appendix B: Seed Data

A complete seed profile exists at:
```
rust/config/seed/trading_profiles/allianzgi_complete.yaml
```

This demonstrates a multi-asset global fund with:
- 11 markets (XETR, XLON, XSWX, XPAR, XAMS, XNYS, XNAS, XHKG, XTKS, XASX, XSES)
- 5 instrument classes (EQUITY, GOVT_BOND, CORP_BOND, ETF, OTC_DERIVATIVE)
- 8 currencies (EUR, USD, GBP, CHF, JPY, HKD, SGD, AUD)
- Multiple IM mandates with priority-based scope
- ISDA agreements with CSA and collateral SSI references
- ALERT-style booking rules with priority ordering
- Full pricing matrix and corporate actions policy
