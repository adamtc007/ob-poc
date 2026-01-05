# TODO: Trading View & Instrument Matrix - The Foundation of Onboarding

## Executive Summary

The Trading View is broken - it shows only the CBU node because:
1. `ConfigDrivenGraphBuilder` incorrectly maps trading edges to "services" layer
2. No `load_trading_layer()` function exists
3. `VisualizationRepository` has no trading data queries

More critically: **The Instrument Matrix is the master configuration that drives ALL onboarding requirements.**

What you trade → What products you need → What services those products require → What resources to provision

Without the instrument matrix visualization, onboarding makes no sense.

---

## Domain Model Clarification

### Three Orthogonal CBU Views

| View | Root | Contains | Purpose |
|------|------|----------|---------|
| **UBO** | CBU | Entities → Ownership chains → Natural persons | Who owns/controls |
| **Trading** | CBU | Trading Profile → Instrument Matrix → Markets/ISDA | What they're authorized to trade |
| **Onboarding** | CBU | Products → Services → Resources | What they subscribe to (DERIVED from Trading) |

**Critical Insight**: Onboarding view shows Products/Services/Resources - but these are DRIVEN BY the instrument matrix. You don't subscribe to "Custody" generically - you subscribe to "Custody for US Equities on NYSE" because your instrument matrix says you trade EQUITY on XNYS.

### The Instrument Matrix as Requirements Generator

```
INSTRUMENT MATRIX (what you trade)
        ↓
    PRODUCT REQUIREMENTS (what you need)
        ↓
    SERVICE REQUIREMENTS (what those products require)
        ↓
    RESOURCE PROVISIONING (accounts, SSIs, ISDAs)
```

Example flows:

```
Trade OTC Interest Rate Swaps
  → Requires: ISDA Master Agreement with counterparty
  → Requires: CSA (VM + potentially IM)
  → Requires: Collateral Management product
  → Requires: Tri-party custody accounts
  → Requires: Margin call processing service
  → Requires: Eligible collateral schedule

Trade US Equities on NYSE
  → Requires: Custody product
  → Requires: Settlement service (T+1)
  → Requires: DTCC participant setup
  → Requires: Subcustodian account (BNYM)
  → Requires: SSI for XNYS/USD
  → Requires: Corporate actions service
  → Requires: Proxy voting service (optional)

Trade Japanese Government Bonds
  → Requires: Fixed Income custody
  → Requires: JASDEC participant
  → Requires: Subcustodian in Tokyo
  → Requires: SSI for XTKS/JPY
  → Requires: Income collection service
  → Requires: Withholding tax handling
```

---

## Current State Analysis

### What Exists

#### 1. Trading Profile Document Types (`rust/src/trading_profile/types.rs`)

Complete JSONB document structure (747 lines):

```rust
TradingProfileDocument {
    universe: Universe {
        base_currency: String,
        allowed_currencies: Vec<String>,
        allowed_markets: Vec<MarketConfig>,      // MIC-based
        instrument_classes: Vec<InstrumentClassConfig>,  // CFI, ISDA taxonomy
    },
    investment_managers: Vec<InvestmentManagerMandate> {
        manager: EntityRef,        // BIC or LEI
        scope: ManagerScope {
            mics: Vec<String>,     // Which markets
            instrument_classes: Vec<String>,
        },
        can_trade: bool,
        can_settle: bool,
    },
    isda_agreements: Vec<IsdaAgreementConfig> {
        counterparty: EntityRef,   // LEI
        governing_law: String,
        product_coverage: Vec<ProductCoverage>,
        csa: Option<CsaConfig> {
            csa_type: String,      // VM, VM_IM
            threshold_amount,
            eligible_collateral: Vec<EligibleCollateral>,
            initial_margin: Option<InitialMarginConfig>,
        },
    },
    settlement_config: SettlementConfig {
        matching_platforms: Vec<MatchingPlatform>,  // CTM, ALERT
        subcustodian_network: Vec<SubcustodianEntry>,
    },
    booking_rules: Vec<BookingRule>,
    standing_instructions: HashMap<String, Vec<StandingInstruction>>,
    pricing_matrix: Vec<PricingRule>,
}
```

#### 2. Database Schema (custody schema)

Reference tables:
- `custody.instrument_classes` - Hierarchical! Has `parent_class_id`
  - code, name, CFI category/group, ISDA asset class
- `custody.markets` - MIC, name, country, CSD BIC, timezone
- `custody.security_types` - SMPG/ALERT codes under instrument classes

CBU-specific tables:
- `custody.cbu_instrument_universe` - CBU × InstrumentClass × Market × Counterparty
- `custody.isda_agreements` + `isda_product_coverage`
- `custody.cbu_ssi` - Standing settlement instructions
- `custody.ssi_booking_rules` - ALERT-style matching rules
- `custody.cbu_settlement_chains` + `settlement_chain_hops`
- `custody.cbu_pricing_config`
- `custody.cbu_tax_status`

Document storage:
- `"ob-poc".cbu_trading_profiles` - Versioned JSONB documents with materialization status

#### 3. Trading Matrix API (`rust/src/api/trading_matrix_routes.rs`)

`GET /api/cbu/{cbu_id}/trading-matrix` returns hierarchical tree:

```
CBU
├── Instrument Classes (category)
│   ├── EQUITY
│   │   ├── Markets
│   │   │   ├── XNYS → Universe Entries → SSIs, Booking Rules
│   │   │   └── XLON → Universe Entries → SSIs
│   │   └── Resources: Settlement Chains, Tax Config
│   └── DERIVATIVES
│       ├── Exchange Traded
│       │   └── Markets...
│       └── OTC
│           └── Counterparties → ISDA → CSA
└── Settlement Config
    └── Subcustodians, Matching Platforms
```

This API already loads all the data - but it's for a dedicated panel, not the main graph.

#### 4. UI Panel (`ob-poc-ui/src/panels/trading_matrix.rs`)

Renders the trading matrix as expandable tree browser.

### What's Broken

#### ConfigDrivenGraphBuilder (`rust/src/graph/config_driven_builder.rs`)

```rust
// CURRENT (WRONG):
fn get_included_layers(&self) -> Vec<String> {
    // ...
    if self.visible_edge_types.contains("CBU_HAS_TRADING_PROFILE") {
        layers.push("services".to_string());  // ← WRONG! Maps to services
    }
}

// In build():
if included_layers.contains(&"services".to_string()) {
    self.load_services_layer(&mut graph, repo).await?;  // Loads Products/Services
}
// NO trading layer loading!
```

Result: Trading view tries to load "services" layer which has Products/Services/Resources but NO trading data.

---

## Implementation Plan

### Phase 1: Fix Layer Mapping

**File**: `rust/src/graph/config_driven_builder.rs`

```rust
fn get_included_layers(&self) -> Vec<String> {
    let mut layers = vec!["core".to_string()];

    // UBO layer
    if self.visible_edge_types.contains("OWNERSHIP")
        || self.visible_edge_types.contains("CONTROL")
    {
        layers.push("ubo".to_string());
    }

    // TRADING layer (NEW - separate from services!)
    if self.visible_edge_types.contains("CBU_HAS_TRADING_PROFILE")
        || self.visible_edge_types.contains("TRADING_PROFILE_HAS_MATRIX")
        || self.visible_edge_types.contains("MATRIX_INCLUDES_CLASS")
        || self.visible_edge_types.contains("CLASS_TRADED_ON_MARKET")
        || self.visible_edge_types.contains("OTC_COVERED_BY_ISDA")
    {
        layers.push("trading".to_string());
    }

    // ONBOARDING layer (renamed from services)
    if self.visible_edge_types.contains("CBU_USES_PRODUCT")
        || self.visible_edge_types.contains("PRODUCT_PROVIDES_SERVICE")
        || self.visible_edge_types.contains("SERVICE_USES_RESOURCE")
    {
        layers.push("onboarding".to_string());
    }

    layers
}
```

In `build()`:
```rust
if included_layers.contains(&"trading".to_string()) {
    self.load_trading_layer(&mut graph, repo).await?;
}

if included_layers.contains(&"onboarding".to_string()) {
    self.load_onboarding_layer(&mut graph, repo).await?;  // renamed
}
```

### Phase 2: Add Trading Layer Loader

**File**: `rust/src/graph/config_driven_builder.rs`

```rust
/// Load trading layer data
/// 
/// Structure:
/// CBU → TradingProfile → InstrumentMatrix
///                           ├── InstrumentClass nodes
///                           │     ├── Market nodes (exchange traded)
///                           │     └── Counterparty nodes (OTC)
///                           │           └── ISDA → CSA
///                           └── IM links (entities doing the trading)
async fn load_trading_layer(
    &self,
    graph: &mut CbuGraph,
    repo: &VisualizationRepository,
) -> Result<()> {
    // 1. Load active trading profile
    let Some(profile) = repo.get_active_trading_profile(self.cbu_id).await? else {
        return Ok(()); // No trading profile = no trading layer
    };

    // 2. Add Trading Profile node
    let profile_node_id = format!("profile-{}", profile.profile_id);
    if self.is_node_type_visible("TRADING_PROFILE") {
        graph.add_node(GraphNode {
            id: profile_node_id.clone(),
            node_type: NodeType::TradingProfile,
            layer: LayerType::Trading,
            label: "Trading Profile".into(),
            sublabel: Some(format!("v{} - {}", profile.version, profile.status)),
            status: match profile.status.as_str() {
                "ACTIVE" => NodeStatus::Active,
                "DRAFT" => NodeStatus::Pending,
                _ => NodeStatus::Inactive,
            },
            ..Default::default()
        });

        // Edge: CBU → Trading Profile
        if self.is_edge_type_visible("CBU_HAS_TRADING_PROFILE") {
            graph.add_edge(GraphEdge {
                source: self.cbu_id.to_string(),
                target: profile_node_id.clone(),
                edge_type: EdgeType::HasTradingProfile,
                ..Default::default()
            });
        }
    }

    // 3. Add Instrument Matrix node (container)
    let matrix_node_id = format!("matrix-{}", profile.profile_id);
    if self.is_node_type_visible("INSTRUMENT_MATRIX") {
        graph.add_node(GraphNode {
            id: matrix_node_id.clone(),
            node_type: NodeType::InstrumentMatrix,
            layer: LayerType::Trading,
            label: "Instrument Matrix".into(),
            ..Default::default()
        });

        graph.add_edge(GraphEdge {
            source: profile_node_id.clone(),
            target: matrix_node_id.clone(),
            edge_type: EdgeType::HasMatrix,
            ..Default::default()
        });
    }

    // 4. Load universe entries grouped by instrument class
    let universe = repo.get_cbu_instrument_universe(self.cbu_id).await?;
    
    // Group by instrument class
    let mut by_class: HashMap<Uuid, Vec<UniverseEntryView>> = HashMap::new();
    for entry in universe {
        by_class.entry(entry.instrument_class_id)
            .or_default()
            .push(entry);
    }

    // 5. Add instrument class nodes
    for (class_id, entries) in by_class {
        let class_code = entries.first().map(|e| e.class_code.clone()).unwrap_or_default();
        let class_node_id = format!("class-{}", class_id);

        if self.is_node_type_visible("INSTRUMENT_CLASS") {
            let requires_isda = entries.iter().any(|e| e.counterparty_id.is_some());
            
            graph.add_node(GraphNode {
                id: class_node_id.clone(),
                node_type: NodeType::InstrumentClass,
                layer: LayerType::Trading,
                label: class_code.clone(),
                sublabel: Some(if requires_isda { "OTC" } else { "Exchange" }.into()),
                data: serde_json::json!({
                    "class_id": class_id,
                    "is_otc": requires_isda,
                    "market_count": entries.iter().filter(|e| e.market_id.is_some()).count(),
                    "counterparty_count": entries.iter().filter(|e| e.counterparty_id.is_some()).count(),
                }),
                ..Default::default()
            });

            // Edge: Matrix → Class
            graph.add_edge(GraphEdge {
                source: matrix_node_id.clone(),
                target: class_node_id.clone(),
                edge_type: EdgeType::IncludesClass,
                ..Default::default()
            });
        }

        // 6. Add market nodes (exchange traded)
        for entry in entries.iter().filter(|e| e.market_id.is_some()) {
            let market_id = entry.market_id.unwrap();
            let market_node_id = format!("market-{}", market_id);

            if !graph.has_node(&market_node_id) && self.is_node_type_visible("MARKET") {
                graph.add_node(GraphNode {
                    id: market_node_id.clone(),
                    node_type: NodeType::Market,
                    layer: LayerType::Trading,
                    label: entry.mic.clone().unwrap_or_default(),
                    sublabel: entry.market_name.clone(),
                    data: serde_json::json!({
                        "market_id": market_id,
                        "mic": entry.mic,
                        "currencies": entry.currencies,
                    }),
                    ..Default::default()
                });
            }

            // Edge: Class → Market
            if self.is_edge_type_visible("CLASS_TRADED_ON_MARKET") {
                graph.add_edge(GraphEdge {
                    source: class_node_id.clone(),
                    target: market_node_id.clone(),
                    edge_type: EdgeType::TradedOn,
                    label: Some(entry.currencies.join(", ")),
                    ..Default::default()
                });
            }
        }

        // 7. Add counterparty nodes (OTC) with ISDA links
        for entry in entries.iter().filter(|e| e.counterparty_id.is_some()) {
            let counterparty_id = entry.counterparty_id.unwrap();
            let cp_node_id = format!("counterparty-{}", counterparty_id);

            // Counterparty is an entity - may already exist in graph from UBO
            if !graph.has_node(&cp_node_id) && self.is_node_type_visible("ENTITY_COMPANY") {
                graph.add_node(GraphNode {
                    id: cp_node_id.clone(),
                    node_type: NodeType::Entity,
                    layer: LayerType::Trading,
                    label: entry.counterparty_name.clone().unwrap_or("Unknown".into()),
                    data: serde_json::json!({
                        "entity_id": counterparty_id,
                        "role": "OTC_COUNTERPARTY",
                    }),
                    ..Default::default()
                });
            }

            // Edge: Class → Counterparty (OTC trading relationship)
            if self.is_edge_type_visible("OTC_WITH_COUNTERPARTY") {
                graph.add_edge(GraphEdge {
                    source: class_node_id.clone(),
                    target: cp_node_id.clone(),
                    edge_type: EdgeType::OtcCounterparty,
                    ..Default::default()
                });
            }
        }
    }

    // 8. Load ISDA agreements and link to counterparties
    let isdas = repo.get_cbu_isda_agreements(self.cbu_id).await?;
    for isda in isdas {
        let isda_node_id = format!("isda-{}", isda.isda_id);
        let cp_node_id = format!("counterparty-{}", isda.counterparty_entity_id);

        if self.is_node_type_visible("ISDA_AGREEMENT") {
            graph.add_node(GraphNode {
                id: isda_node_id.clone(),
                node_type: NodeType::IsdaAgreement,
                layer: LayerType::Trading,
                label: format!("ISDA {}", isda.governing_law.as_deref().unwrap_or("?")),
                sublabel: isda.counterparty_name.clone(),
                data: serde_json::json!({
                    "isda_id": isda.isda_id,
                    "governing_law": isda.governing_law,
                    "agreement_date": isda.agreement_date,
                }),
                ..Default::default()
            });

            // Edge: Counterparty → ISDA
            if self.is_edge_type_visible("OTC_COVERED_BY_ISDA") {
                graph.add_edge(GraphEdge {
                    source: cp_node_id,
                    target: isda_node_id.clone(),
                    edge_type: EdgeType::CoveredByIsda,
                    ..Default::default()
                });
            }
        }

        // 9. Load CSA under ISDA
        if let Some(csa) = repo.get_isda_csa(isda.isda_id).await? {
            let csa_node_id = format!("csa-{}", csa.csa_id);

            if self.is_node_type_visible("CSA_AGREEMENT") {
                graph.add_node(GraphNode {
                    id: csa_node_id.clone(),
                    node_type: NodeType::CsaAgreement,
                    layer: LayerType::Trading,
                    label: format!("CSA ({})", csa.csa_type),
                    data: serde_json::json!({
                        "csa_id": csa.csa_id,
                        "csa_type": csa.csa_type,
                        "threshold_amount": csa.threshold_amount,
                        "threshold_currency": csa.threshold_currency,
                    }),
                    ..Default::default()
                });

                graph.add_edge(GraphEdge {
                    source: isda_node_id,
                    target: csa_node_id,
                    edge_type: EdgeType::HasCsa,
                    ..Default::default()
                });
            }
        }
    }

    // 10. Load Investment Managers and link to CBU
    let ims = repo.get_cbu_investment_managers(self.cbu_id).await?;
    for im in ims {
        // IM is an entity - link via role
        let im_entity_node_id = format!("entity-{}", im.entity_id);
        
        // Edge: CBU → IM Entity with trading role
        if self.is_edge_type_visible("CBU_IM_MANDATE") {
            graph.add_edge(GraphEdge {
                source: self.cbu_id.to_string(),
                target: im_entity_node_id,
                edge_type: EdgeType::ImMandate,
                label: Some(format!("IM: {}", im.scope_description)),
                data: Some(serde_json::json!({
                    "can_trade": im.can_trade,
                    "can_settle": im.can_settle,
                    "scope_mics": im.scope_mics,
                })),
                ..Default::default()
            });
        }
    }

    Ok(())
}
```

### Phase 3: Add VisualizationRepository Methods

**File**: `rust/src/database/visualization_repository.rs`

```rust
// =============================================================================
// TRADING DATA QUERIES
// =============================================================================

#[derive(Debug, Clone)]
pub struct TradingProfileView {
    pub profile_id: Uuid,
    pub version: i32,
    pub status: String,
    pub activated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct UniverseEntryView {
    pub universe_id: Uuid,
    pub instrument_class_id: Uuid,
    pub class_code: String,
    pub class_name: String,
    pub market_id: Option<Uuid>,
    pub mic: Option<String>,
    pub market_name: Option<String>,
    pub counterparty_id: Option<Uuid>,
    pub counterparty_name: Option<String>,
    pub currencies: Vec<String>,
    pub is_otc: bool,
}

#[derive(Debug, Clone)]
pub struct IsdaAgreementView {
    pub isda_id: Uuid,
    pub counterparty_entity_id: Uuid,
    pub counterparty_name: Option<String>,
    pub governing_law: Option<String>,
    pub agreement_date: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct CsaView {
    pub csa_id: Uuid,
    pub csa_type: String,
    pub threshold_amount: Option<f64>,
    pub threshold_currency: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InvestmentManagerView {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub bic: Option<String>,
    pub can_trade: bool,
    pub can_settle: bool,
    pub scope_mics: Vec<String>,
    pub scope_classes: Vec<String>,
    pub scope_description: String,
}

impl VisualizationRepository {
    /// Get active trading profile for CBU
    pub async fn get_active_trading_profile(&self, cbu_id: Uuid) -> Result<Option<TradingProfileView>> {
        let row = sqlx::query!(
            r#"SELECT profile_id, version, status, activated_at
               FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = $1 AND status = 'ACTIVE'
               ORDER BY version DESC
               LIMIT 1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| TradingProfileView {
            profile_id: r.profile_id,
            version: r.version,
            status: r.status,
            activated_at: r.activated_at,
        }))
    }

    /// Get instrument universe entries for CBU
    pub async fn get_cbu_instrument_universe(&self, cbu_id: Uuid) -> Result<Vec<UniverseEntryView>> {
        let rows = sqlx::query!(
            r#"SELECT 
                u.universe_id,
                u.instrument_class_id,
                ic.code as class_code,
                ic.name as class_name,
                u.market_id,
                m.mic,
                m.name as market_name,
                u.counterparty_entity_id as counterparty_id,
                e.name as counterparty_name,
                u.currencies,
                ic.requires_isda as is_otc
               FROM custody.cbu_instrument_universe u
               JOIN custody.instrument_classes ic ON ic.class_id = u.instrument_class_id
               LEFT JOIN custody.markets m ON m.market_id = u.market_id
               LEFT JOIN "ob-poc".entities e ON e.entity_id = u.counterparty_entity_id
               WHERE u.cbu_id = $1 AND u.is_active = true
               ORDER BY ic.code, m.mic, e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| UniverseEntryView {
            universe_id: r.universe_id,
            instrument_class_id: r.instrument_class_id,
            class_code: r.class_code,
            class_name: r.class_name,
            market_id: r.market_id,
            mic: r.mic,
            market_name: r.market_name,
            counterparty_id: r.counterparty_id,
            counterparty_name: r.counterparty_name,
            currencies: r.currencies,
            is_otc: r.is_otc.unwrap_or(false),
        }).collect())
    }

    /// Get ISDA agreements for CBU
    pub async fn get_cbu_isda_agreements(&self, cbu_id: Uuid) -> Result<Vec<IsdaAgreementView>> {
        let rows = sqlx::query!(
            r#"SELECT 
                i.isda_id,
                i.counterparty_entity_id,
                e.name as counterparty_name,
                i.governing_law,
                i.agreement_date
               FROM custody.isda_agreements i
               JOIN "ob-poc".entities e ON e.entity_id = i.counterparty_entity_id
               WHERE i.cbu_id = $1 AND i.is_active = true
               ORDER BY e.name"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| IsdaAgreementView {
            isda_id: r.isda_id,
            counterparty_entity_id: r.counterparty_entity_id,
            counterparty_name: r.counterparty_name,
            governing_law: r.governing_law,
            agreement_date: r.agreement_date,
        }).collect())
    }

    /// Get CSA for ISDA agreement
    pub async fn get_isda_csa(&self, isda_id: Uuid) -> Result<Option<CsaView>> {
        let row = sqlx::query!(
            r#"SELECT 
                csa_id,
                csa_type,
                threshold_amount::float8 as threshold_amount,
                threshold_currency
               FROM custody.csa_agreements
               WHERE isda_id = $1 AND is_active = true"#,
            isda_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| CsaView {
            csa_id: r.csa_id,
            csa_type: r.csa_type,
            threshold_amount: r.threshold_amount,
            threshold_currency: r.threshold_currency,
        }))
    }

    /// Get investment managers for CBU (from trading profile document)
    pub async fn get_cbu_investment_managers(&self, cbu_id: Uuid) -> Result<Vec<InvestmentManagerView>> {
        // Investment managers are stored in the JSONB document
        // Extract and join with entities table
        let rows = sqlx::query!(
            r#"SELECT 
                im->>'manager' as manager_ref,
                im->>'role' as role,
                (im->>'can_trade')::boolean as can_trade,
                (im->>'can_settle')::boolean as can_settle,
                im->'scope'->'mics' as scope_mics,
                im->'scope'->'instrument_classes' as scope_classes
               FROM "ob-poc".cbu_trading_profiles p,
                    jsonb_array_elements(p.document->'investment_managers') as im
               WHERE p.cbu_id = $1 AND p.status = 'ACTIVE'"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // TODO: Resolve manager refs (BIC/LEI) to entity IDs
        // For now, return parsed data
        Ok(rows.into_iter().filter_map(|r| {
            // Parse manager ref and resolve to entity
            // This is simplified - real implementation needs BIC/LEI lookup
            Some(InvestmentManagerView {
                entity_id: Uuid::nil(), // TODO: resolve from manager_ref
                entity_name: r.manager_ref.unwrap_or_default(),
                bic: None,
                can_trade: r.can_trade.unwrap_or(true),
                can_settle: r.can_settle.unwrap_or(true),
                scope_mics: serde_json::from_value(r.scope_mics.unwrap_or_default()).unwrap_or_default(),
                scope_classes: serde_json::from_value(r.scope_classes.unwrap_or_default()).unwrap_or_default(),
                scope_description: r.role.unwrap_or_default(),
            })
        }).collect())
    }
}
```

### Phase 4: Add Edge Type and Node Type Config

**File**: Append to existing migration or new migration

```sql
-- =============================================================================
-- TRADING VIEW NODE TYPES
-- =============================================================================

INSERT INTO "ob-poc".node_types (
    node_type_code, display_name, description,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    icon, default_color, default_shape,
    default_width, default_height, can_be_container, default_tier
) VALUES
('TRADING_PROFILE', 'Trading Profile', 'CBU trading authorization configuration',
 false, true, false, false, false,
 'file-cog', '#0EA5E9', 'RECTANGLE',
 180.0, 60.0, true, 1),

('INSTRUMENT_MATRIX', 'Instrument Matrix', 'Authorized instruments and markets',
 false, true, false, false, false,
 'grid-3x3', '#0284C7', 'RECTANGLE',
 200.0, 80.0, true, 2),

('INSTRUMENT_CLASS', 'Instrument Class', 'Asset class category (Equity, Fixed Income, Derivatives)',
 false, true, false, false, false,
 'layers', '#7C3AED', 'RECTANGLE',
 160.0, 50.0, true, 3),

('MARKET', 'Market', 'Exchange or trading venue (MIC)',
 false, true, false, false, false,
 'landmark', '#059669', 'RECTANGLE',
 140.0, 50.0, false, 4),

('ISDA_AGREEMENT', 'ISDA Agreement', 'OTC Master Agreement',
 false, true, false, false, false,
 'file-signature', '#DC2626', 'RECTANGLE',
 160.0, 50.0, true, 4),

('CSA_AGREEMENT', 'CSA', 'Credit Support Annex (collateral)',
 false, true, false, false, false,
 'shield-check', '#EA580C', 'RECTANGLE',
 140.0, 40.0, false, 5)

ON CONFLICT (node_type_code) DO UPDATE SET
    show_in_trading_view = EXCLUDED.show_in_trading_view,
    display_name = EXCLUDED.display_name;

-- =============================================================================
-- TRADING VIEW EDGE TYPES
-- =============================================================================

INSERT INTO "ob-poc".edge_types (
    edge_type_code, display_name, description,
    from_node_types, to_node_types,
    show_in_ubo_view, show_in_trading_view, show_in_fund_structure_view,
    show_in_service_view, show_in_product_view,
    edge_style, edge_color, edge_width, arrow_style,
    layout_direction, tier_delta, is_hierarchical,
    is_trading, sort_order
) VALUES
('CBU_HAS_TRADING_PROFILE', 'Has Trading Profile', 'CBU trading configuration',
 '["CBU"]'::JSONB, '["TRADING_PROFILE"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#0EA5E9', 2.0, 'SINGLE',
 'DOWN', 1, true,
 true, 200),

('TRADING_PROFILE_HAS_MATRIX', 'Has Matrix', 'Trading profile includes instrument matrix',
 '["TRADING_PROFILE"]'::JSONB, '["INSTRUMENT_MATRIX"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#0284C7', 1.5, 'SINGLE',
 'DOWN', 1, true,
 true, 201),

('MATRIX_INCLUDES_CLASS', 'Includes Class', 'Matrix includes instrument class',
 '["INSTRUMENT_MATRIX"]'::JSONB, '["INSTRUMENT_CLASS"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#7C3AED', 1.5, 'SINGLE',
 'DOWN', 1, true,
 true, 202),

('CLASS_TRADED_ON_MARKET', 'Traded On', 'Instrument class traded on market',
 '["INSTRUMENT_CLASS"]'::JSONB, '["MARKET"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#059669', 1.0, 'SINGLE',
 'DOWN', 1, true,
 true, 203),

('OTC_WITH_COUNTERPARTY', 'OTC With', 'OTC trading relationship',
 '["INSTRUMENT_CLASS"]'::JSONB, '["ENTITY_COMPANY"]'::JSONB,
 false, true, false, false, false,
 'DASHED', '#DC2626', 1.5, 'SINGLE',
 'DOWN', 1, true,
 true, 204),

('OTC_COVERED_BY_ISDA', 'Covered By ISDA', 'OTC trades governed by ISDA',
 '["ENTITY_COMPANY"]'::JSONB, '["ISDA_AGREEMENT"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#DC2626', 1.5, 'SINGLE',
 'RIGHT', 0, false,
 true, 205),

('ISDA_HAS_CSA', 'Has CSA', 'ISDA includes collateral annex',
 '["ISDA_AGREEMENT"]'::JSONB, '["CSA_AGREEMENT"]'::JSONB,
 false, true, false, false, false,
 'SOLID', '#EA580C', 1.0, 'SINGLE',
 'DOWN', 1, true,
 true, 206),

('CBU_IM_MANDATE', 'IM Mandate', 'Investment manager trading mandate',
 '["CBU"]'::JSONB, '["ENTITY_COMPANY"]'::JSONB,
 false, true, false, false, false,
 'DASHED', '#8B5CF6', 2.0, 'DOUBLE',
 'RIGHT', 0, false,
 true, 210)

ON CONFLICT (edge_type_code) DO UPDATE SET
    show_in_trading_view = EXCLUDED.show_in_trading_view,
    display_name = EXCLUDED.display_name;
```

### Phase 5: Add GraphNode/GraphEdge Types

**File**: `rust/src/graph/types.rs`

```rust
// Add to NodeType enum:
pub enum NodeType {
    // ... existing
    TradingProfile,
    InstrumentMatrix,
    InstrumentClass,
    Market,
    IsdaAgreement,
    CsaAgreement,
}

// Add to EdgeType enum:
pub enum EdgeType {
    // ... existing
    HasTradingProfile,
    HasMatrix,
    IncludesClass,
    TradedOn,
    OtcCounterparty,
    CoveredByIsda,
    HasCsa,
    ImMandate,
}
```

### Phase 6: Update ViewMode Enum

**File**: `rust/crates/ob-poc-graph/src/graph/mod.rs`

```rust
pub enum ViewMode {
    KycUbo,
    Trading,          // ← Already exists
    ServiceDelivery,  // Rename to Onboarding?
    ProductsOnly,
}

impl ViewMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC_UBO",
            ViewMode::Trading => "TRADING",
            ViewMode::ServiceDelivery => "ONBOARDING",  // or keep as SERVICE_DELIVERY
            ViewMode::ProductsOnly => "PRODUCTS_ONLY",
        }
    }
}
```

---

## Visual Design Specification

### Trading View Layout

```
                         ┌─────────────────┐
                         │      CBU        │
                         │  Allianz Lux    │
                         └────────┬────────┘
                                  │ has trading profile
                         ┌────────▼────────┐
                         │ Trading Profile │
                         │   v3 ACTIVE     │
                         └────────┬────────┘
                                  │ has matrix
                         ┌────────▼────────┐
                         │Instrument Matrix│
                         │ 12 classes      │
                         └────────┬────────┘
           ┌──────────────────────┼──────────────────────┐
           │                      │                      │
    ┌──────▼──────┐       ┌──────▼──────┐       ┌──────▼──────┐
    │   EQUITY    │       │FIXED_INCOME │       │ DERIVATIVES │
    │  Exchange   │       │  Exchange   │       │   OTC/Exch  │
    └──────┬──────┘       └──────┬──────┘       └──────┬──────┘
    ┌──────┴──────┐       ┌──────┴──────┐       ┌──────┴──────┐
    │             │       │             │       │             │
┌───▼───┐    ┌───▼───┐    ▼            ...     │             │
│ XNYS  │    │ XLON  │  Markets               ▼             ▼
│ NYSE  │    │ LSE   │              ┌─────────────┐  ┌──────────┐
└───────┘    └───────┘              │  Goldman    │  │  XCME    │
                                    │(counterparty)│  │ (exch)   │
                                    └──────┬──────┘  └──────────┘
                                           │ covered by
                                    ┌──────▼──────┐
                                    │  ISDA NY    │
                                    │  2024-01-15 │
                                    └──────┬──────┘
                                           │ has CSA
                                    ┌──────▼──────┐
                                    │  CSA VM+IM  │
                                    │  $10M USD   │
                                    └─────────────┘

═══════════════════════════════════════════════════════════════
Separate overlay (non-hierarchical):

    ┌─────────────────┐          ┌─────────────────┐
    │      CBU        │ ─ ─ ─ ─ ▶│   BlackRock     │
    │  Allianz Lux    │IM Mandate│   BLKIUS33      │
    └─────────────────┘          │ can_trade: ✓    │
                                 │ scope: all mkts │
                                 └─────────────────┘
```

### Node Styling

| Node Type | Shape | Color | Icon |
|-----------|-------|-------|------|
| Trading Profile | Rectangle | Sky Blue (#0EA5E9) | file-cog |
| Instrument Matrix | Rectangle | Cyan (#0284C7) | grid-3x3 |
| Instrument Class | Rectangle | Purple (#7C3AED) | layers |
| Market | Rectangle | Green (#059669) | landmark |
| ISDA Agreement | Rectangle | Red (#DC2626) | file-signature |
| CSA | Rectangle | Orange (#EA580C) | shield-check |

### Edge Styling

| Edge Type | Style | Color | Arrow |
|-----------|-------|-------|-------|
| Has Trading Profile | Solid | Sky Blue | Single |
| Includes Class | Solid | Purple | Single |
| Traded On (exchange) | Solid | Green | Single |
| OTC With Counterparty | Dashed | Red | Single |
| Covered By ISDA | Solid | Red | Single |
| IM Mandate | Dashed | Purple | Double (bidirectional) |

---

## Testing Checklist

### Unit Tests

- [ ] `ViewConfigService::get_view_edge_types("TRADING")` returns trading edge types
- [ ] `ViewConfigService::get_view_node_types("TRADING")` returns trading node types
- [ ] `VisualizationRepository::get_active_trading_profile()` returns correct profile
- [ ] `VisualizationRepository::get_cbu_instrument_universe()` groups correctly

### Integration Tests

- [ ] `GET /api/cbu/{id}/graph?view_mode=TRADING` returns non-empty graph
- [ ] Graph contains CBU → TradingProfile → InstrumentMatrix hierarchy
- [ ] Graph contains InstrumentClass → Market edges for exchange traded
- [ ] Graph contains InstrumentClass → Entity → ISDA edges for OTC

### UI Tests

- [ ] Dropdown selection "Trading" triggers graph refresh with view_mode=TRADING
- [ ] Trading view shows instrument matrix hierarchy
- [ ] ISDA/CSA nodes render correctly
- [ ] IM mandate edges render as overlay (non-hierarchical)

---

## Migration Path

1. **Add node_types/edge_types config** (SQL migration)
2. **Add repository methods** (Rust)
3. **Fix layer mapping** in ConfigDrivenGraphBuilder
4. **Add load_trading_layer()** function
5. **Add GraphNode/GraphEdge types** 
6. **Test via API**
7. **Verify UI dropdown triggers correct view**

---

## Future Enhancements

### Phase 2: Onboarding View Derivation

Show how instrument matrix DRIVES product requirements:

```
EQUITY + XNYS  ─────────▶ Custody Product (US Equities)
                                ├── Settlement Service
                                ├── Corporate Actions Service
                                └── Tax Reclaim Service

DERIVATIVES + OTC ──────▶ Collateral Management Product
                                ├── Margin Call Service
                                └── Tri-party Custody Service
```

### Phase 3: Gap Analysis Visualization

Overlay showing what's MISSING:

```
EQUITY + XTKS ───────────▶ ⚠️ Missing: SSI for XTKS/JPY
                          ⚠️ Missing: Subcustodian Japan
```

### Phase 4: Blade Runner Enhancement

Voice navigation through instrument matrix:
- "Show me OTC derivatives" → Zoom to DERIVATIVES class
- "Enhance Goldman ISDA" → Drill into ISDA details
- "What's the CSA threshold?" → Speak value

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust/src/graph/config_driven_builder.rs` | Fix layer mapping, add `load_trading_layer()` |
| `rust/src/graph/types.rs` | Add NodeType/EdgeType variants |
| `rust/src/database/visualization_repository.rs` | Add trading query methods |
| `rust/migrations/YYYYMMDD_trading_view_config.sql` | Add node/edge type config |
| `rust/crates/ob-poc-graph/src/graph/mod.rs` | Update ViewMode if renaming |

---

## Dependencies

- Existing `custody.*` tables must be populated
- `cbu_trading_profiles` must have ACTIVE profile for CBU
- Entity resolution for Investment Manager BICs (may need GLEIF lookup)
