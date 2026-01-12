# TODO: ESPER Navigation + CBU Struct Alignment

> **Status:** PARTIALLY COMPLETE - Reassessed 2026-01-12
> **Priority:** MEDIUM (Most ESPER infrastructure done, CBU category gaps remain)
> **Related:** TODO-ISDA-MATRIX-CONSOLIDATION.md

---

## Issue 1: ESPER UI Infrastructure - ✅ LARGELY COMPLETE

### What's Been Implemented

Claude Code has implemented the core ESPER infrastructure:

| Component | File | Status |
|-----------|------|--------|
| `ViewportState` type | `ob-poc-types/src/viewport.rs` | ✅ Complete |
| `EsperRenderState` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `ViewportRenderState` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `render_viewport_hud()` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `render_focus_ring()` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `render_focus_breadcrumbs()` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `render_enhance_level_indicator()` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `render_confidence_zone_legend()` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| `render_view_type_selector()` | `ob-poc-graph/src/graph/viewport.rs` | ✅ Complete |
| Taxonomy breadcrumbs | `ob-poc-ui/src/panels/taxonomy.rs` | ✅ Complete |
| ViewportState propagation | `ob-poc-ui/src/state.rs` (apply_viewport_state) | ✅ Complete |
| Agent navigation commands | `ob-poc-types/src/lib.rs` (AgentCommand) | ✅ Complete |
| Async pending handlers | `ob-poc-ui/src/state.rs` (take_pending_*) | ✅ Complete |

### EsperRenderState Modes Implemented

All Blade Runner-style visual modes are implemented in `viewport.rs`:

- `xray_enabled` / `xray_alpha` - Make non-focused elements semi-transparent
- `peel_enabled` / `peel_depth` - Hide outer layers progressively  
- `shadow_enabled` / `shadow_alpha` - Dim non-relevant entities
- `illuminate_enabled` / `illuminate_aspect` - Glow specific aspects
- `red_flag_scan_enabled` - Highlight entities with risk indicators
- `black_hole_enabled` - Highlight entities with data gaps
- `depth_indicator_enabled` - Show depth cues
- `cross_section_enabled` - Show slice through structure

### Remaining ESPER Work (Low Priority)

1. **Verify HUD is called** - Confirm `render_viewport_hud()` is invoked in graph rendering
2. **Layer control panel** - Add optional slider UI for xray/shadow alpha (currently toggle-only)
3. **DSL verb binding** - Verify `view.trace`, `view.xray` DSL commands update ViewportState

---

## Issue 2: CBU Struct Misalignment - ⚠️ STILL NEEDS WORK

### Current State

```
Database (cbus table)
├── cbu_category: VARCHAR(50) ✅ EXISTS
│
├── visualization_repository.rs
│   ├── CbuSummaryView: ✅ HAS cbu_category
│   └── CbuBasicView: ✅ HAS cbu_category
│
├── client_routes.rs
│   └── CbuSummary: ✅ HAS cbu_category (local struct)
│
├── graph_routes.rs  
│   └── CbuSummary mapping: ✅ MAPS cbu_category
│
├── ob-poc-types/src/lib.rs (SHARED TYPES)
│   ├── CbuSummary: ❌ MISSING cbu_category  ← FIX NEEDED
│   ├── CbuGraphResponse: ✅ HAS cbu_category
│   └── CbuContext: ❌ MISSING cbu_category  ← FIX NEEDED
│
└── ob-poc-ui/src/panels/context.rs
    └── cbu_category display: ❌ MISSING  ← FIX NEEDED
```

### Fix 2.1: Add cbu_category to CbuSummary in ob-poc-types

**File:** `crates/ob-poc-types/src/lib.rs` (~line 823)

```rust
/// CBU summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: String,
    pub name: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub client_type: Option<String>,
    #[serde(default)]
    pub cbu_category: Option<String>,  // ADD THIS
}
```

Also update the `CbuSummary::new()` constructor (~line 1672):

```rust
impl CbuSummary {
    pub fn new(
        cbu_id: Uuid,
        name: String,
        jurisdiction: Option<String>,
        client_type: Option<String>,
        cbu_category: Option<String>,  // ADD THIS
    ) -> Self {
        Self {
            cbu_id: cbu_id.to_string(),
            name,
            jurisdiction,
            client_type,
            cbu_category,  // ADD THIS
        }
    }
}
```

### Fix 2.2: Add cbu_category to CbuContext

**File:** `crates/ob-poc-types/src/lib.rs` (~line 1600)

```rust
/// CBU-specific context with summary info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuContext {
    /// CBU UUID
    pub id: String,
    /// CBU name
    pub name: String,
    /// Jurisdiction code (e.g., "LU", "US")
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Client type (e.g., "FUND", "CORPORATE")
    #[serde(default)]
    pub client_type: Option<String>,
    /// Template discriminator: FUND_MANDATE, CORPORATE_GROUP, etc.
    #[serde(default)]
    pub cbu_category: Option<String>,  // ADD THIS
    /// Number of linked entities
    #[serde(default)]
    pub entity_count: i32,
    // ... rest of fields unchanged
}
```

### Fix 2.3: Update context_discovery_service to populate cbu_category

**File:** `src/database/context_discovery_service.rs`

Find where `CbuContext` is constructed and add cbu_category from the database query.

### Fix 2.4: Display cbu_category in context panel

**File:** `crates/ob-poc-ui/src/panels/context.rs` (~line 70)

In `render_context()` function, add after client_type display:

```rust
if let Some(ref category) = cbu.cbu_category {
    ui.horizontal(|ui| {
        ui.label("Category:");
        let icon = match category.as_str() {
            "FUND_MANDATE" | "FUND" => "📊",
            "CORPORATE_GROUP" | "CORPORATE" => "🏢", 
            "FAMILY_OFFICE" => "👨‍👩‍👧‍👦",
            "BANK" | "INSTITUTIONAL" => "🏦",
            "INSURANCE" => "🛡️",
            _ => "📁",
        };
        ui.label(format!("{} {}", icon, category));
    });
}
```

---

## Issue 3: Trading Matrix - ✅ NO CHANGES NEEDED

The trading matrix architecture is confirmed solid:
- Single source of truth in `ob-poc-types/src/trading_matrix.rs`
- Path-based node IDs for efficient navigation
- Clear separation between UI AST and config documents
- egui compliance verified

---

## Execution Order

```
1. CBU Category Alignment (Issue 2) - Quick wins
   ├── 2.1 Add to CbuSummary in ob-poc-types ⬜ TODO
   ├── 2.2 Add to CbuContext ⬜ TODO  
   ├── 2.3 Update context_discovery_service ⬜ TODO
   └── 2.4 Add to context panel ⬜ TODO

2. ESPER Verification (Issue 1) - Low priority polish
   ├── Verify HUD rendering ⬜ TODO (optional)
   ├── Layer control sliders ⬜ TODO (optional)
   └── DSL verb binding verification ⬜ TODO (optional)

3. Trading Matrix - ✅ DONE
```

---

## Verification

```bash
# After CBU fixes - verify compilation
cargo build -p ob-poc-types
cargo build -p ob-poc-ui

# Verify API returns category
curl localhost:8080/api/cbu | jq '.[0].cbu_category'

# Verify context includes category  
curl localhost:8080/api/session/{id}/context | jq '.cbu.cbu_category'
```

---

## Summary

| Issue | Effort | Impact | Status |
|-------|--------|--------|--------|
| ESPER UI infrastructure | N/A | N/A | ✅ DONE |
| ESPER UI polish (HUD, sliders) | 2-3 hours | LOW | ⬜ Optional |
| CBU cbu_category in types | 1 hour | MEDIUM | ⬜ TODO |
| CBU cbu_category in context | 1 hour | MEDIUM | ⬜ TODO |
| CBU cbu_category in UI | 30 min | LOW | ⬜ TODO |
| Trading Matrix | 0 | N/A | ✅ DONE |

**Total remaining effort: ~2.5 hours for CBU category alignment**
