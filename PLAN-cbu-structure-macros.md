# CBU Structure Macros - Implementation Plan

**Created:** 2026-01-30  
**Based on:** `TODO-cbu-structure-macros.md` review  
**Status:** Ready for phased implementation

---

## Executive Summary

The TODO document proposes a comprehensive macro system for business-facing fund structure setup (M1-M18: Luxembourg, Ireland, UK, US, cross-border). After codebase analysis, **~70% of the core infrastructure already exists**. The main gaps are:

1. **Jurisdiction-specific structure macros** (M1-M18) - NOT implemented
2. **Document bundles** - Schema proposed but not implemented  
3. **Placeholder entities** - NOT implemented
4. **AST-level expansion integration** - Partially implemented
5. **egui UI components** - NOT implemented

---

## What Already Exists (Do Not Re-implement)

### Core Macro Infrastructure (100% Complete)

| Component | Location | Status |
|-----------|----------|--------|
| `MacroRegistry` | `rust/src/dsl_v2/macros/registry.rs` | ✅ Complete |
| `MacroSchema` (YAML types) | `rust/src/dsl_v2/macros/schema.rs` | ✅ Complete |
| `expand_macro()` | `rust/src/dsl_v2/macros/expander.rs` | ✅ Complete |
| `VariableContext` | `rust/src/dsl_v2/macros/variable.rs` | ✅ Complete |
| Role validation (GP/SICAV constraints) | `rust/src/dsl_v2/macros/expander.rs` | ✅ Complete |
| Enum key→internal mapping | `rust/src/dsl_v2/macros/variable.rs` | ✅ Complete |
| `OperatorType` (display nouns) | `rust/src/dsl_v2/operator_types.rs` | ✅ Complete |
| DAG prereqs (`MacroPrereq`) | `rust/src/dsl_v2/macros/schema.rs` | ✅ Complete |
| `UnifiedSession` cascade context | `rust/src/session/unified.rs` | ✅ Complete |

### Existing Operator Macros (32 Total)

| Domain | File | Macros |
|--------|------|--------|
| `structure.*` | `config/verb_schemas/macros/structure.yaml` | setup, assign-role, list, select, roles |
| `party.*` | `config/verb_schemas/macros/party.yaml` | add, search, update, assign-identifier, set-address, etc. |
| `case.*` | `config/verb_schemas/macros/case.yaml` | open, add-party, solicit-document, submit, approve, reject, list, select |
| `mandate.*` | `config/verb_schemas/macros/mandate.yaml` | create, add-product, set-instruments, set-markets, list, select, details |

### Verb Disambiguation UI (057)

Already implemented in egui:
- `VerbDisambiguationState` in `rust/crates/ob-poc-ui/src/state.rs`
- `render_verb_disambiguation_card()` in `rust/crates/ob-poc-ui/src/panels/repl.rs`
- Keyboard shortcuts, timeout, learning signals

---

## Gap Analysis: TODO Proposal vs Current State

### High Priority Gaps

| Feature | TODO Proposal | Current State | Priority |
|---------|---------------|---------------|----------|
| **M1-M18 Structure Macros** | 18 jurisdiction-specific macros | None | **P0** |
| **Document Bundles** | 10 bundles with inheritance | Schema exists (049) but no macro integration | **P1** |
| **Placeholder Entities** | Full lifecycle (pending→resolved→verified) | Not implemented | **P1** |
| **Macro Palette UI** | Domain-filtered picker | Not implemented | **P1** |
| **Macro Form Generator** | Schema→form rendering | Not implemented | **P1** |

### Medium Priority Gaps

| Feature | TODO Proposal | Current State | Priority |
|---------|---------------|---------------|----------|
| **Role Cardinality Validation** | Lint phase with counts | Basic validation exists, no cardinality | **P2** |
| **Nested Macro Invocation** | `invoke_macro` with symbol import | Expander exists but untested | **P2** |
| **Macro Versioning** | Semver + migration paths | Not implemented | **P2** |
| **Expansion Audit Trail** | `macro_invocations` table | Partial (expansion_reports exists) | **P2** |

### Lower Priority Gaps

| Feature | TODO Proposal | Current State | Priority |
|---------|---------------|---------------|----------|
| **LSP Macro Completions** | Hover, diagnostics | Basic verb completion only | **P3** |
| **Partial Expansion Sessions** | Resume with missing args | Not implemented | **P3** |
| **Cross-border Dispatchers** | Vehicle registry | Not implemented | **P3** |

---

## Phased Implementation Plan

### Phase 1: Foundation - Document Bundles & Placeholder Entities (Week 1-2)

**Goal:** Establish the supporting infrastructure that M1-M18 depend on.

#### 1.1 Document Bundle System

**New files:**
- `rust/src/dsl_v2/docs_bundle/mod.rs`
- `rust/src/dsl_v2/docs_bundle/registry.rs`
- `rust/src/dsl_v2/docs_bundle/resolver.rs`
- `config/docs_bundles/` (YAML definitions)

**Schema (from TODO):**
```yaml
docs_bundles:
  - id: docs.bundle.ucits_baseline
    version: "2024-03"
    effective_from: 2024-03-01
    documents:
      - id: prospectus
        name: "Fund Prospectus"
        required: true
      - id: kiid
        name: "Key Investor Information Document"
        required: true
      # ...
```

**Implementation:**
```rust
pub struct DocsBundleRegistry {
    bundles: HashMap<String, DocsBundle>,
}

pub struct DocsBundle {
    pub id: String,
    pub version: String,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub extends: Option<String>,
    pub documents: Vec<DocRequirement>,
}

pub struct DocRequirement {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub required_if: Option<String>,  // condition expression
}

impl DocsBundle {
    pub fn resolve(&self, registry: &DocsBundleRegistry) -> ResolvedBundle;
}
```

**Database migration:**
```sql
-- migration: 064_docs_bundle_registry.sql
CREATE TABLE "ob-poc".docs_bundles (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    effective_from DATE NOT NULL,
    effective_to DATE,
    extends TEXT REFERENCES "ob-poc".docs_bundles(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE "ob-poc".docs_bundle_requirements (
    bundle_id TEXT REFERENCES "ob-poc".docs_bundles(id),
    doc_id TEXT NOT NULL,
    name TEXT NOT NULL,
    required BOOLEAN NOT NULL DEFAULT true,
    required_if TEXT,  -- condition expression
    PRIMARY KEY (bundle_id, doc_id)
);
```

#### 1.2 Placeholder Entity System

**New files:**
- `rust/src/domain_ops/placeholder_ops.rs`
- `rust/config/verbs/placeholder.yaml`

**Database migration:**
```sql
-- migration: 065_placeholder_entities.sql
CREATE TABLE "ob-poc".placeholder_entities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    placeholder_kind TEXT NOT NULL,  -- 'depositary', 'administrator', etc.
    display_name TEXT NOT NULL,      -- 'TBD Depositary'
    state TEXT NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'resolved', 'verified', 'expired', 'rejected')),
    resolved_entity_id UUID REFERENCES "ob-poc".entities(entity_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_by_macro TEXT,
    CONSTRAINT resolved_requires_entity 
        CHECK (state != 'resolved' OR resolved_entity_id IS NOT NULL)
);

CREATE INDEX idx_placeholders_cbu ON "ob-poc".placeholder_entities(cbu_id);
CREATE INDEX idx_placeholders_pending ON "ob-poc".placeholder_entities(state) 
    WHERE state = 'pending';
```

**Verbs:**
```yaml
# config/verbs/placeholder.yaml
placeholder:
  create:
    description: "Create placeholder entity for deferred resolution"
    behavior: plugin
    args:
      - name: cbu-id
        type: uuid
        required: true
      - name: kind
        type: string
        required: true
        valid_values: [depositary, administrator, transfer_agent, auditor, custodian, prime_broker, aifm, manco]
      - name: display-name
        type: string
        required: false
    returns:
      type: uuid
      
  resolve:
    description: "Resolve placeholder to real entity"
    behavior: plugin
    args:
      - name: placeholder-id
        type: uuid
        required: true
      - name: entity-id
        type: uuid
        required: true
        lookup:
          table: entities
          entity_type: entity
```

**CustomOperation:**
```rust
#[register_custom_op]
pub struct PlaceholderCreateOp;

impl CustomOperation for PlaceholderCreateOp {
    fn domain(&self) -> &'static str { "placeholder" }
    fn verb(&self) -> &'static str { "create" }
    
    async fn execute(&self, verb_call: &VerbCall, ctx: &mut ExecutionContext, pool: &PgPool) 
        -> Result<ExecutionResult> 
    {
        let cbu_id: Uuid = verb_call.get_uuid("cbu-id")?;
        let kind: String = verb_call.get_string("kind")?;
        let display_name = verb_call.get_string_opt("display-name")
            .unwrap_or_else(|| format!("TBD {}", kind));
        
        let id = sqlx::query_scalar!(
            r#"INSERT INTO "ob-poc".placeholder_entities 
               (cbu_id, placeholder_kind, display_name, created_by_macro)
               VALUES ($1, $2, $3, $4)
               RETURNING id"#,
            cbu_id, kind, display_name, ctx.current_macro_id()
        )
        .fetch_one(pool)
        .await?;
        
        Ok(ExecutionResult::Uuid(id))
    }
}
```

---

### Phase 2: Core Structure Macros M1-M6 (Week 2-3)

**Goal:** Implement Luxembourg and Ireland macros as the foundation.

#### 2.1 Primitive Verbs Required

Before macros can expand, ensure these primitives exist:

| Verb | Status | Action |
|------|--------|--------|
| `cbu.ensure` | Needs creation | Create or return existing by name |
| `fund.ensure` | Needs creation | Create fund entity with wrapper type |
| `fund.umbrella.ensure` | Needs creation | Create umbrella structure |
| `fund.subfund.ensure` | Needs creation | Create subfund under umbrella |
| `role.attach` | Exists | Verify idempotent |
| `entity.ensure_or_placeholder` | Needs creation | Resolve entity or create placeholder |
| `product.enable` | Exists | Verify batch capability |
| `docs.bundle.apply` | Needs creation | Apply document requirements to CBU |

**New verbs file:**
```yaml
# config/verbs/fund.yaml
fund:
  ensure:
    description: "Create or return existing fund entity"
    behavior: plugin
    args:
      - name: name
        type: string
        required: true
      - name: wrapper
        type: string
        required: true
        valid_values: [sicav, icav, raif, scsp, oeic, aut, acs, lp, delaware_lp, open_end, closed_end]
      - name: domicile
        type: string
        required: true
      - name: regime
        type: string
        required: false
        valid_values: [ucits, aif, 40act]
      - name: strategy
        type: string
        required: false
    returns:
      type: uuid
      
  umbrella.ensure:
    description: "Create umbrella structure for fund"
    behavior: plugin
    args:
      - name: fund-id
        type: uuid
        required: true
    returns:
      type: uuid
      
  subfund.ensure:
    description: "Create subfund under umbrella"
    behavior: plugin
    args:
      - name: umbrella-id
        type: uuid
        required: true
      - name: name
        type: string
        required: true
    returns:
      type: uuid
```

#### 2.2 Luxembourg Macros (M1-M3)

**File:** `config/verb_schemas/macros/struct_lux.yaml`

```yaml
# M1: struct.lux.ucits.sicav
struct.lux.ucits.sicav:
  kind: macro
  ui:
    label: "Set up Lux UCITS SICAV"
    description: "Luxembourg UCITS SICAV with required service providers"
    target_label: "Fund Structure"
  
  taxonomy: [structure, fund, lux, ucits, sicav]
  
  args:
    required:
      name:
        type: string
        description: "Fund name"
    optional:
      umbrella:
        type: bool
        default: false
      subfunds:
        type: list
        item_type: string
        required_if: "umbrella == true"
      manco:
        type: entity_ref
        entity_kind: management_company
        placeholder_if_missing: true
      depositary:
        type: entity_ref
        entity_kind: depositary
        placeholder_if_missing: true
      administrator:
        type: entity_ref
        entity_kind: administrator
        placeholder_if_missing: true
      transfer_agent:
        type: entity_ref
        entity_kind: transfer_agent
        placeholder_if_missing: true
      auditor:
        type: entity_ref
        entity_kind: auditor
        placeholder_if_missing: true
  
  required_roles: [depositary, administrator, transfer_agent, auditor]
  optional_roles: [management_company, distributor]
  docs_bundle: docs.bundle.ucits_baseline
  
  expands_to:
    - verb: cbu.ensure
      args:
        name: "${arg.name}"
        domicile: "LU"
      bind: "@cbu"
    
    - verb: fund.ensure
      args:
        name: "${arg.name}"
        wrapper: "sicav"
        domicile: "LU"
        regime: "ucits"
      bind: "@fund"
    
    - verb: role.attach
      args:
        subject: "@fund"
        role: "fund_vehicle"
        object: "@cbu"
    
    - when: "${arg.umbrella}"
      steps:
        - verb: fund.umbrella.ensure
          args:
            fund-id: "@fund"
          bind: "@umbrella"
        
        - foreach: "${arg.subfunds}"
          as: "sf_name"
          verb: fund.subfund.ensure
          args:
            umbrella-id: "@umbrella"
            name: "${sf_name}"
    
    - verb: entity.ensure_or_placeholder
      args:
        ref: "${arg.depositary}"
        kind: "depositary"
      bind: "@depositary"
    
    - verb: role.attach
      args:
        subject: "@fund"
        role: "depositary"
        object: "@depositary"
    
    # ... similar for administrator, transfer_agent, auditor, manco
    
    - verb: product.enable
      args:
        cbu: "@cbu"
        products: [custody, fund_accounting, transfer_agency]
    
    - verb: docs.bundle.apply
      args:
        cbu: "@cbu"
        bundle: "docs.bundle.ucits_baseline"

# M2: struct.lux.aif.raif (similar structure)
# M3: struct.lux.pe.scsp (similar structure)
```

#### 2.3 Ireland Macros (M4-M6)

**File:** `config/verb_schemas/macros/struct_ie.yaml`

Similar to Luxembourg but with IE-specific wrappers (ICAV) and roles.

---

### Phase 3: UK & US Macros M7-M16 (Week 3-4)

**Goal:** Complete single-jurisdiction macros.

#### 3.1 UK Macros (M7-M12)

**File:** `config/verb_schemas/macros/struct_uk.yaml`

- M7: `struct.uk.authorised.oeic` - OEIC with ACD
- M8: `struct.uk.authorised.aut` - Authorised Unit Trust with Manager/Trustee
- M9: `struct.uk.authorised.acs` - Contractual Scheme with Operator
- M10: `struct.uk.authorised.ltaf` - Long-Term Asset Fund (delegates to M7/M8/M9)
- M11: `struct.uk.manager.llp` - Manager entity (not a fund)
- M12: `struct.uk.private_equity.lp` - UK PE LP

#### 3.2 US Macros (M13-M16)

**File:** `config/verb_schemas/macros/struct_us.yaml`

- M13: `struct.us.40act.open_end` - Mutual fund
- M14: `struct.us.40act.closed_end` - Closed-end fund
- M15: `struct.us.etf.40act` - ETF (delegates to M13, adds APs)
- M16: `struct.us.private_fund.delaware_lp` - Delaware LP

---

### Phase 4: Cross-Border Macros M17-M18 (Week 4)

**Goal:** Implement composite macros that invoke child macros.

#### 4.1 Nested Macro Invocation

Ensure expander handles `invoke_macro`:

```rust
ExpansionTemplate::InvokeMacro { macro_id, args, import_symbols } => {
    let resolved = self.substitute_args(args, context)?;
    let child_defn = self.registry.get(macro_id)?;
    
    match self.expand_macro(child_defn, resolved, origin.clone())? {
        ExpansionResult::Expanded { nodes, symbols_exported } => {
            // Import requested symbols into current scope
            for sym in import_symbols {
                if let Some(resolved) = symbols_exported.get(sym) {
                    scope.bind(sym.clone(), resolved.clone());
                }
            }
            Ok(nodes)
        }
        _ => Err(ExpansionError::NestedFailed),
    }
}
```

#### 4.2 Cross-Border Macros

**File:** `config/verb_schemas/macros/struct_cross_border.yaml`

- M17: `struct.hedge.cross_border` - UK/US manager + IE/Lux/US fund
- M18: `struct.pe.cross_border` - UK/US manager + multi-jurisdiction fund

---

### Phase 5: egui UI Components (Week 5-6)

**Goal:** Build the user-facing macro interface.

#### 5.1 Macro Palette Panel

**New file:** `rust/crates/ob-poc-ui/src/panels/macro_palette.rs`

```rust
pub struct MacroPaletteState {
    pub selected_domain: Option<String>,
    pub search_query: String,
    pub filtered_macros: Vec<MacroSummary>,
}

pub fn render_macro_palette(
    ui: &mut egui::Ui,
    state: &mut MacroPaletteState,
    registry: &MacroRegistry,
) -> Option<MacroPaletteAction> {
    // Domain tabs: structure | party | case | mandate
    // Search box
    // Macro list with descriptions
    // Click to open form
}
```

#### 5.2 Macro Form Generator

**New file:** `rust/crates/ob-poc-ui/src/widgets/macro_form.rs`

```rust
pub struct MacroFormState {
    pub macro_id: String,
    pub arg_values: HashMap<String, FormValue>,
    pub validation_errors: Vec<String>,
}

pub fn render_macro_form(
    ui: &mut egui::Ui,
    schema: &MacroSchema,
    state: &mut MacroFormState,
) -> Option<MacroFormAction> {
    // Render each arg based on type:
    // - string → TextEdit
    // - bool → Checkbox
    // - enum → ComboBox
    // - entity_ref → Entity picker button
    // - list → Dynamic add/remove
    
    // Submit button (disabled if validation fails)
}

pub enum FormValue {
    String(String),
    Bool(bool),
    Enum(String),
    EntityRef(Option<Uuid>),
    List(Vec<FormValue>),
}
```

#### 5.3 Placeholder Resolution UI

**New file:** `rust/crates/ob-poc-ui/src/panels/placeholder_panel.rs`

```rust
pub fn render_pending_placeholders(
    ui: &mut egui::Ui,
    placeholders: &[PlaceholderSummary],
) -> Option<PlaceholderAction> {
    // List pending placeholders
    // "Resolve" button opens entity picker
    // Expiration warnings
}
```

#### 5.4 API Endpoints

**Add to:** `rust/src/api/agent_routes.rs`

```rust
// GET /api/macros/taxonomy
// Returns hierarchical macro listing by domain
async fn get_macro_taxonomy(registry: &MacroRegistry) -> Json<MacroTaxonomy>;

// GET /api/macros/:fqn/schema  
// Returns full macro definition for form rendering
async fn get_macro_schema(fqn: String, registry: &MacroRegistry) -> Json<MacroSchema>;

// POST /api/macros/:fqn/expand
// Expands macro with provided args
async fn expand_macro(
    fqn: String, 
    args: Json<HashMap<String, Value>>,
) -> Json<ExpansionResponse>;

// GET /api/cbus/:id/placeholders
// List pending placeholders for CBU
async fn get_cbu_placeholders(cbu_id: Uuid) -> Json<Vec<PlaceholderSummary>>;
```

---

### Phase 6: Testing & Polish (Week 6-7)

#### 6.1 Unit Tests

```rust
#[cfg(test)]
mod macro_expansion_tests {
    #[test]
    fn test_lux_sicav_basic() {
        let registry = load_test_registry();
        let args = hashmap! {
            "name" => "Alpha SICAV",
        };
        let result = expand_macro("struct.lux.ucits.sicav", &args, &registry);
        assert!(result.is_ok());
        // Verify: cbu created, fund created, roles attached, placeholders created
    }
    
    #[test]
    fn test_lux_sicav_umbrella_with_subfunds() {
        // umbrella=true, subfunds=["Sub A", "Sub B"]
    }
    
    #[test]
    fn test_cross_border_hedge_uk_manager_ie_fund() {
        // Nested macro invocation
    }
    
    #[test]
    fn test_placeholder_resolution_repoints_edges() {
        // Create with placeholder, resolve, verify edges updated
    }
}
```

#### 6.2 Integration Tests

```rust
#[tokio::test]
async fn test_full_pipeline_lux_sicav() {
    // parse → expand → resolve → lint → execute
    // Verify DB state
}
```

#### 6.3 Demo Scripts

Create `scripts/demo_macros/`:
- `01_lux_sicav_umbrella.dsl`
- `02_ie_hedge_with_prime_broker.dsl`
- `03_us_etf_with_aps.dsl`
- `04_cross_border_pe.dsl`

---

## Timeline Summary

| Phase | Duration | Deliverables |
|-------|----------|--------------|
| **Phase 1** | Week 1-2 | Document bundles, placeholder entities, migrations |
| **Phase 2** | Week 2-3 | M1-M6 (Luxembourg + Ireland macros) |
| **Phase 3** | Week 3-4 | M7-M16 (UK + US macros) |
| **Phase 4** | Week 4 | M17-M18 (cross-border composites) |
| **Phase 5** | Week 5-6 | egui macro palette, form generator, placeholder UI |
| **Phase 6** | Week 6-7 | Testing, integration, demo scripts |

**Total estimate:** 6-7 weeks for full implementation

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Nested macro expansion complexity | Start with simple macros (M1-M6), add nesting (M17-M18) last |
| Entity resolution in placeholders | Leverage existing EntityGateway, don't reinvent |
| UI form generation complexity | Start with basic types, add entity pickers incrementally |
| Cross-border macro proliferation | Use vehicle registry pattern for extensibility |

---

## Success Criteria

1. **All 18 macros expand** to valid DSL without errors
2. **Document bundles** correctly inherit and merge
3. **Placeholders** can be resolved and edges repointed
4. **egui form** renders for any macro schema
5. **Demo scripts** execute end-to-end
6. **No regression** in existing operator macros (structure.setup, etc.)

---

## Files to Create/Modify

### New Files

| Path | Purpose |
|------|---------|
| `rust/src/dsl_v2/docs_bundle/mod.rs` | Document bundle module |
| `rust/src/dsl_v2/docs_bundle/registry.rs` | Bundle registry |
| `rust/src/dsl_v2/docs_bundle/resolver.rs` | Inheritance resolver |
| `rust/src/domain_ops/placeholder_ops.rs` | Placeholder CustomOps |
| `rust/src/domain_ops/fund_ops.rs` | Fund primitive CustomOps |
| `rust/config/verbs/placeholder.yaml` | Placeholder verbs |
| `rust/config/verbs/fund.yaml` | Fund verbs |
| `config/docs_bundles/*.yaml` | 10 document bundles |
| `config/verb_schemas/macros/struct_lux.yaml` | M1-M3 |
| `config/verb_schemas/macros/struct_ie.yaml` | M4-M6 |
| `config/verb_schemas/macros/struct_uk.yaml` | M7-M12 |
| `config/verb_schemas/macros/struct_us.yaml` | M13-M16 |
| `config/verb_schemas/macros/struct_cross_border.yaml` | M17-M18 |
| `rust/crates/ob-poc-ui/src/panels/macro_palette.rs` | Macro picker UI |
| `rust/crates/ob-poc-ui/src/widgets/macro_form.rs` | Form generator |
| `rust/crates/ob-poc-ui/src/panels/placeholder_panel.rs` | Placeholder UI |
| `migrations/064_docs_bundle_registry.sql` | Docs bundle schema |
| `migrations/065_placeholder_entities.sql` | Placeholder schema |

### Modified Files

| Path | Changes |
|------|---------|
| `rust/src/dsl_v2/macros/expander.rs` | Add `invoke_macro` handling, placeholder creation |
| `rust/src/dsl_v2/macros/schema.rs` | Add `docs_bundle`, `required_roles` fields |
| `rust/src/api/agent_routes.rs` | Add macro API endpoints |
| `rust/crates/ob-poc-ui/src/app.rs` | Integrate macro palette |
| `rust/crates/ob-poc-ui/src/state.rs` | Add macro form state |

---

## Appendix: Existing Macro System Architecture

```
User: "set up a PE structure"
    │
    ├─► Verb Search (Tier 0: Macro priority)
    │       │
    │       └─► MacroRegistry.by_mode_tag("onboarding")
    │               │
    │               └─► structure.setup (existing) or struct.lux.pe.scsp (new)
    │
    ├─► Macro Form (new UI)
    │       │
    │       └─► Collect args: name, GP, AIFM, etc.
    │
    ├─► expand_macro()
    │       │
    │       ├─► Validate args
    │       ├─► Check prereqs
    │       ├─► Substitute variables (${arg.name}, ${arg.role.internal})
    │       ├─► Handle conditionals (when:)
    │       ├─► Handle loops (foreach:)
    │       ├─► Create placeholders (placeholder_if_missing)
    │       └─► Output: Vec<DslPrimitive>
    │
    ├─► DSL Execution
    │       │
    │       └─► cbu.ensure → fund.ensure → role.attach → ...
    │
    └─► Result: CBU + Fund + Roles + Placeholders + DocRequirements
```
