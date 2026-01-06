# Trading Matrix AST Refactoring Plan

## Current State

The trading matrix currently has **two separate type hierarchies**:

1. **`TradingProfileDocument`** (`rust/src/trading_profile/types.rs`)
   - YAML document structure (serde-based)
   - Stored as JSONB in `cbu_trading_profiles.document`
   - Materialized to `custody.*` tables by `trading-profile.materialize` verb

2. **`TradingMatrixNodeType`** (`rust/src/api/trading_matrix_routes.rs`)
   - API response type for UI visualization
   - **Reconstructed from SQL queries** against `custody.*` tables
   - Duplicated in client WASM (`rust/crates/ob-poc-graph/src/graph/trading_matrix.rs`)

### Current Data Flow

```
YAML file → serde → TradingProfileDocument → JSONB storage
                                              ↓
                         trading-profile.materialize verb
                                              ↓
                        custody.* tables (SSIs, booking rules, etc.)
                                              ↓
              SQL queries in trading_matrix_routes.rs
                                              ↓
                        TradingMatrixResponse (tree)
                                              ↓
                        Client WASM renders UI
```

**Problems with current approach:**

1. Type duplication between document and API types
2. SQL reconstruction is complex and error-prone
3. Changes require updating three places: document types, SQL queries, client types
4. The document structure doesn't match what the UI renders

## Target State

The trading matrix document **IS** the AST - a typed tree structure that:

1. Is built incrementally by DSL verb execution
2. Uses DB reference data (instrument classes, markets, currencies) for validation
3. Is stored as-is in JSONB
4. Is served directly to UI without SQL reconstruction
5. Is rendered using the same types in client WASM

### Target Data Flow

```
DSL verb execution → TradingMatrixNode → JSONB storage
                                              ↓
                        GET /api/cbu/:id/trading-matrix
                                              ↓
                        Returns document directly (no SQL)
                                              ↓
                        Client WASM renders UI
```

## Unified AST Structure

Create a single canonical type hierarchy in `ob-poc-types`:

```rust
/// Node ID - path-based identifier for tree navigation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct TradingMatrixNodeId(pub Vec<String>);

/// Node type discriminator with type-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TradingMatrixNodeType {
    // Category nodes (virtual groupings)
    Category { name: String },
    
    // Universe layer
    InstrumentClass { class_code: String, cfi_prefix: Option<String>, is_otc: bool },
    Market { mic: String, market_name: String, country_code: String },
    Counterparty { entity_id: String, entity_name: String, lei: Option<String> },
    UniverseEntry { universe_id: String, currencies: Vec<String>, ... },
    
    // SSI layer
    Ssi { ssi_id: String, ssi_name: String, ssi_type: String, ... },
    BookingRule { rule_id: String, rule_name: String, priority: i32, ... },
    
    // Settlement layer
    SettlementChain { chain_id: String, chain_name: String, hop_count: usize, ... },
    SettlementHop { hop_id: String, sequence: i32, ... },
    
    // Tax layer
    TaxJurisdiction { jurisdiction_id: String, jurisdiction_code: String, ... },
    TaxConfig { status_id: String, investor_type: String, tax_exempt: bool, ... },
    
    // OTC layer
    IsdaAgreement { isda_id: String, counterparty_name: String, ... },
    CsaAgreement { csa_id: String, csa_type: String, ... },
}

/// A node in the trading matrix tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixNode {
    pub id: TradingMatrixNodeId,
    pub node_type: TradingMatrixNodeType,
    pub label: String,
    pub sublabel: Option<String>,
    pub children: Vec<TradingMatrixNode>,
    pub status_color: Option<StatusColor>,
}

/// Complete trading matrix document (stored as JSONB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMatrixDocument {
    pub cbu_id: String,
    pub cbu_name: String,
    pub version: i32,
    pub children: Vec<TradingMatrixNode>,  // Top-level category nodes
    pub metadata: TradingMatrixMetadata,
}
```

## DSL Verbs for Incremental Building

Instead of `trading-profile.materialize`, we have verbs that modify the AST directly:

```clojure
;; Add instrument class to universe
(trading-matrix.add-instrument-class 
  :cbu-id @cbu 
  :class-code "EQUITY"
  :cfi-prefix "ES")

;; Add market under instrument class
(trading-matrix.add-market
  :cbu-id @cbu
  :parent-class "EQUITY"
  :mic "XNYS"
  :market-name "New York Stock Exchange"
  :country-code "US")

;; Add SSI
(trading-matrix.add-ssi
  :cbu-id @cbu
  :ssi-name "US Equities SSI"
  :ssi-type "SECURITIES"
  :safekeeping-account "..."
  :safekeeping-bic "...")

;; Add booking rule under SSI
(trading-matrix.add-booking-rule
  :cbu-id @cbu
  :ssi-ref "US Equities SSI"  ; References by name
  :rule-name "US Equity DVP"
  :priority 10
  :match-class "EQUITY"
  :match-market "XNYS"
  :match-currency "USD")

;; Add ISDA agreement
(trading-matrix.add-isda
  :cbu-id @cbu
  :counterparty-lei "..."
  :governing-law "NY"
  :agreement-date "2024-01-15")
```

## Migration Path

1. **Create unified types** in `ob-poc-types`
2. **Add new DSL verbs** for incremental AST building
3. **Create migration verb** that converts existing `custody.*` table data back to AST
4. **Update API** to return document directly
5. **Update client** to import from shared types
6. **Deprecate** `trading-profile.materialize` and SQL reconstruction

## File Changes

| File | Change |
|------|--------|
| `rust/crates/ob-poc-types/src/lib.rs` | Add unified `TradingMatrixNode` types |
| `rust/src/trading_profile/types.rs` | Replace with re-export from ob-poc-types |
| `rust/config/verbs/trading-matrix.yaml` | New verb definitions for AST building |
| `rust/src/dsl_v2/custom_ops/trading_matrix_ops.rs` | New plugin handlers |
| `rust/src/api/trading_matrix_routes.rs` | Simplify to return document directly |
| `rust/crates/ob-poc-graph/src/graph/trading_matrix.rs` | Import from ob-poc-types |
| `rust/src/database/trading_matrix_repository.rs` | JSONB document CRUD |

## Benefits

1. **Single source of truth** - One type hierarchy used everywhere
2. **No SQL reconstruction** - Document IS the tree
3. **Type-safe DSL** - NOM-parsed verbs build validated AST
4. **Simpler API** - Just return the document
5. **Client/server alignment** - Same types via ob-poc-types
6. **Incremental building** - Verbs modify tree, not bulk materialize
