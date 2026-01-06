# Client-Side Rendering for Trading View

## The Gap

When server returns trading node types, client doesn't know how to style them:

```rust
// Server sends:
GraphNodeData {
    node_type: "trading_profile",  // or "instrument_class", "market", "isda", "csa"
    ...
}

// Client parses:
entity_type: node.node_type.parse().unwrap_or_default()
// → EntityType::Unknown (no match!)
// → Default gray styling
```

## Fix 1: Extend EntityType Enum

**File**: `rust/crates/ob-poc-graph/src/graph/types.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    #[default]
    Unknown,
    // Entity types
    ProperPerson,
    LimitedCompany,
    Partnership,
    Trust,
    Fund,
    // Service layer types
    Product,
    Service,
    Resource,
    // Trading layer types (NEW)
    TradingProfile,
    InstrumentMatrix,
    InstrumentClass,
    Market,
    Counterparty,
    IsdaAgreement,
    CsaAgreement,
    InvestmentManager,
}

impl std::str::FromStr for EntityType {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        Ok(if lower.contains("person") {
            Self::ProperPerson
        } else if lower.contains("company") || lower.contains("limited") {
            Self::LimitedCompany
        } else if lower.contains("partner") {
            Self::Partnership
        } else if lower.contains("trust") {
            Self::Trust
        } else if lower.contains("fund") {
            Self::Fund
        } else if lower == "product" {
            Self::Product
        } else if lower == "service" {
            Self::Service
        } else if lower == "resource" {
            Self::Resource
        // Trading types (NEW)
        } else if lower == "trading_profile" || lower == "tradingprofile" {
            Self::TradingProfile
        } else if lower == "instrument_matrix" || lower == "instrumentmatrix" {
            Self::InstrumentMatrix
        } else if lower == "instrument_class" || lower == "instrumentclass" {
            Self::InstrumentClass
        } else if lower == "market" || lower == "exchange" {
            Self::Market
        } else if lower == "counterparty" {
            Self::Counterparty
        } else if lower == "isda" || lower == "isda_agreement" {
            Self::IsdaAgreement
        } else if lower == "csa" || lower == "csa_agreement" {
            Self::CsaAgreement
        } else if lower == "investment_manager" || lower == "im" {
            Self::InvestmentManager
        } else {
            Self::Unknown
        })
    }
}
```

## Fix 2: Add Colors for Trading Types

**File**: `rust/crates/ob-poc-graph/src/graph/colors.rs`

```rust
/// Get fill color for entity type (neutral palette)
pub fn entity_type_fill(entity_type: EntityType) -> Color32 {
    match entity_type {
        // Existing...
        EntityType::ProperPerson => Color32::from_rgb(100, 181, 246),
        EntityType::LimitedCompany => Color32::from_rgb(144, 164, 174),
        EntityType::Partnership => Color32::from_rgb(129, 199, 132),
        EntityType::Trust => Color32::from_rgb(206, 147, 216),
        EntityType::Fund => Color32::from_rgb(178, 223, 219),
        EntityType::Product => Color32::from_rgb(168, 85, 247),
        EntityType::Service => Color32::from_rgb(96, 165, 250),
        EntityType::Resource => Color32::from_rgb(74, 222, 128),
        
        // Trading layer (NEW) - matches TODO color specs
        EntityType::TradingProfile => Color32::from_rgb(14, 165, 233),  // Sky blue #0EA5E9
        EntityType::InstrumentMatrix => Color32::from_rgb(2, 132, 199), // Cyan #0284C7
        EntityType::InstrumentClass => Color32::from_rgb(124, 58, 237), // Purple #7C3AED
        EntityType::Market => Color32::from_rgb(5, 150, 105),           // Green #059669
        EntityType::Counterparty => Color32::from_rgb(234, 88, 12),     // Orange #EA580C
        EntityType::IsdaAgreement => Color32::from_rgb(220, 38, 38),    // Red #DC2626
        EntityType::CsaAgreement => Color32::from_rgb(234, 88, 12),     // Orange #EA580C
        EntityType::InvestmentManager => Color32::from_rgb(139, 92, 246), // Violet #8B5CF6
        
        EntityType::Unknown => Color32::from_rgb(176, 190, 197),
    }
}

/// Get border color for entity type
pub fn entity_type_border(entity_type: EntityType) -> Color32 {
    match entity_type {
        // Existing...
        EntityType::ProperPerson => Color32::from_rgb(25, 118, 210),
        EntityType::LimitedCompany => Color32::from_rgb(69, 90, 100),
        EntityType::Partnership => Color32::from_rgb(56, 142, 60),
        EntityType::Trust => Color32::from_rgb(142, 36, 170),
        EntityType::Fund => Color32::from_rgb(0, 137, 123),
        EntityType::Product => Color32::from_rgb(88, 28, 135),
        EntityType::Service => Color32::from_rgb(30, 58, 138),
        EntityType::Resource => Color32::from_rgb(20, 83, 45),
        
        // Trading layer (NEW) - darker versions
        EntityType::TradingProfile => Color32::from_rgb(3, 105, 161),   // Sky dark
        EntityType::InstrumentMatrix => Color32::from_rgb(8, 51, 68),   // Cyan dark
        EntityType::InstrumentClass => Color32::from_rgb(76, 29, 149),  // Purple dark
        EntityType::Market => Color32::from_rgb(6, 78, 59),             // Green dark
        EntityType::Counterparty => Color32::from_rgb(154, 52, 18),     // Orange dark
        EntityType::IsdaAgreement => Color32::from_rgb(153, 27, 27),    // Red dark
        EntityType::CsaAgreement => Color32::from_rgb(154, 52, 18),     // Orange dark
        EntityType::InvestmentManager => Color32::from_rgb(91, 33, 182),// Violet dark
        
        EntityType::Unknown => Color32::from_rgb(96, 125, 139),
    }
}
```

## Fix 3: Add EdgeType Variants for Trading Edges

**File**: `rust/crates/ob-poc-graph/src/graph/types.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    // Existing
    HasRole,
    Owns,
    Controls,
    UboTerminus,
    Other,
    // Trading layer (NEW)
    HasTradingProfile,
    HasMatrix,
    IncludesClass,
    TradedOn,           // InstrumentClass → Market (exchange)
    OtcCounterparty,    // InstrumentClass → Entity (OTC)
    CoveredByIsda,      // OTC → ISDA
    HasCsa,             // ISDA → CSA
    ImMandate,          // CBU → IM (dashed overlay)
}

impl std::str::FromStr for EdgeType {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().replace('-', "_").as_str() {
            // Existing
            "has_role" | "hasrole" => Self::HasRole,
            "owns" => Self::Owns,
            "controls" => Self::Controls,
            "ubo_terminus" | "uboterminus" => Self::UboTerminus,
            // Trading (NEW)
            "cbu_has_trading_profile" | "has_trading_profile" => Self::HasTradingProfile,
            "trading_profile_has_matrix" | "has_matrix" => Self::HasMatrix,
            "matrix_includes_class" | "includes_class" => Self::IncludesClass,
            "class_traded_on_market" | "traded_on" => Self::TradedOn,
            "otc_with_counterparty" | "otc_counterparty" => Self::OtcCounterparty,
            "otc_covered_by_isda" | "covered_by_isda" => Self::CoveredByIsda,
            "isda_has_csa" | "has_csa" => Self::HasCsa,
            "cbu_im_mandate" | "im_mandate" => Self::ImMandate,
            _ => Self::Other,
        })
    }
}
```

**File**: `rust/crates/ob-poc-graph/src/graph/colors.rs`

```rust
/// Get color for edge type
pub fn edge_color(edge_type: EdgeType) -> Color32 {
    match edge_type {
        // Existing
        EdgeType::HasRole => Color32::from_rgb(107, 114, 128),
        EdgeType::Owns => Color32::from_rgb(34, 197, 94),
        EdgeType::Controls => Color32::from_rgb(251, 191, 36),
        EdgeType::UboTerminus => Color32::from_rgb(239, 68, 68),
        EdgeType::Other => Color32::from_rgb(156, 163, 175),
        // Trading (NEW)
        EdgeType::HasTradingProfile => Color32::from_rgb(14, 165, 233),  // Sky
        EdgeType::HasMatrix => Color32::from_rgb(2, 132, 199),           // Cyan
        EdgeType::IncludesClass => Color32::from_rgb(124, 58, 237),      // Purple
        EdgeType::TradedOn => Color32::from_rgb(34, 197, 94),            // Green (exchange)
        EdgeType::OtcCounterparty => Color32::from_rgb(239, 68, 68),     // Red (OTC)
        EdgeType::CoveredByIsda => Color32::from_rgb(220, 38, 38),       // Red dark
        EdgeType::HasCsa => Color32::from_rgb(234, 88, 12),              // Orange
        EdgeType::ImMandate => Color32::from_rgb(139, 92, 246),          // Violet
    }
}
```

## Fix 4: Edge Style for OTC/IM (Dashed Lines)

**File**: `rust/crates/ob-poc-graph/src/graph/render.rs`

In the edge rendering section, check for dashed styles:

```rust
fn edge_style_for_type(edge_type: EdgeType) -> EdgeStyle {
    match edge_type {
        EdgeType::OtcCounterparty | EdgeType::CoveredByIsda => EdgeStyle {
            color: edge_color(edge_type),
            width: 2.0,
            dashed: true,  // Dashed for OTC
        },
        EdgeType::ImMandate => EdgeStyle {
            color: edge_color(edge_type),
            width: 1.5,
            dashed: true,  // Dashed overlay for IM
        },
        _ => EdgeStyle {
            color: edge_color(edge_type),
            width: 1.5,
            dashed: false,
        },
    }
}
```

## Summary

| File | Changes |
|------|---------|
| `types.rs` | Add 8 EntityType variants, 8 EdgeType variants |
| `colors.rs` | Add fill/border colors for trading types, edge colors |
| `render.rs` | Add dashed line style for OTC/IM edges |

These client-side changes are **required** for the Trading view to render properly once `load_trading_layer()` returns data from the server.

Without these changes:
- All trading nodes render as gray boxes
- All trading edges render as default gray lines
- No visual distinction between exchange/OTC/ISDA/CSA
