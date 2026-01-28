# Phase 3B Implementation Spec: Verb Picker UI Infrastructure

> **Author:** Claude  
> **Date:** 2026-01-28  
> **Status:** DRAFT → IMPLEMENTATION  
> **Prerequisite:** Phase 3A complete (macro YAML schemas, lint, operator_types)

---

## Overview

Phase 3B provides the **backend infrastructure** for the verb picker UI. The verb picker is the PRIMARY operator interface - a navigable taxonomy tree where operators select verbs with 100% confidence. Semantic search is fallback only.

```
┌─────────────────────────────────────────────────────────────────┐
│  VERB PICKER (PRIMARY)                 SEARCH (FALLBACK)        │
│                                                                 │
│  structure ▶                           ┌──────────────────────┐ │
│    ├─ setup                            │ 🔍 "assign a role"   │ │
│    ├─ assign-role  ◀── CLICK           └──────────────────────┘ │
│    ├─ list                             Only when "I can't find" │
│    └─ select                                                    │
│  case ▶                                                         │
│  mandate ▶                                                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Deliverables

| Priority | Deliverable | Description |
|----------|-------------|-------------|
| **P0** | `session.set-*` verbs | Primitive verbs that macros expand to |
| **P1** | Macro registry loader | Load macro YAML into runtime registry |
| **P2** | `GET /api/verbs/taxonomy` | Tree structure for verb picker UI |
| **P3** | `GET /api/verbs/{fqn}/schema` | Full schema for selected verb |
| **P4** | Display noun middleware | Scrub internal terms from API responses |
| **P5** | Wire macros into verb search | Fallback semantic matching |

---

## P0: Session Context Verbs

### Problem

Macro `expands_to` targets these verbs that don't exist:
- `session.set-structure` 
- `session.set-case`
- `session.set-mandate`

### Solution

Add to `rust/config/verbs/session.yaml`:

```yaml
set-structure:
  description: Set the current working structure in session context
  behavior: plugin
  handler: SessionSetStructureOp
  invocation_phrases:
    - "select structure"
    - "set current structure"
    - "work on structure"
  metadata:
    tier: intent
    source_of_truth: session
    noun: session_context
  args:
    - name: structure-id
      type: uuid
      required: true
      description: The structure (CBU) to set as current
      lookup:
        table: cbus
        schema: ob-poc
        entity_type: cbu
        search_key: name
        primary_key: cbu_id
  returns:
    type: record
    fields: [structure_id, structure_name, structure_type]

set-case:
  description: Set the current working KYC case in session context
  behavior: plugin
  handler: SessionSetCaseOp
  invocation_phrases:
    - "select case"
    - "set current case"
    - "work on case"
  metadata:
    tier: intent
    source_of_truth: session
    noun: session_context
  args:
    - name: case-id
      type: uuid
      required: true
      description: The KYC case to set as current
      lookup:
        table: kyc_cases
        schema: ob-poc
        entity_type: kyc_case
        search_key: case_number
        primary_key: case_id
  returns:
    type: record
    fields: [case_id, case_number, status]

set-mandate:
  description: Set the current working mandate (trading profile) in session context
  behavior: plugin
  handler: SessionSetMandateOp
  invocation_phrases:
    - "select mandate"
    - "set current mandate"
    - "work on mandate"
  metadata:
    tier: intent
    source_of_truth: session
    noun: session_context
  args:
    - name: mandate-id
      type: uuid
      required: true
      description: The mandate (trading profile) to set as current
      lookup:
        table: trading_profiles
        schema: ob-poc
        entity_type: trading_profile
        search_key: name
        primary_key: trading_profile_id
  returns:
    type: record
    fields: [mandate_id, mandate_name]
```

### Implementation: `rust/src/domain_ops/session_ops.rs`

Add three new `CustomOperation` implementations:

```rust
#[register_custom_op]
pub struct SessionSetStructureOp;

#[async_trait]
impl CustomOperation for SessionSetStructureOp {
    fn domain(&self) -> &'static str { "session" }
    fn verb(&self) -> &'static str { "set-structure" }
    fn rationale(&self) -> &'static str { "Updates UnifiedSession.current_structure" }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let structure_id = verb_call.get_uuid_arg("structure-id")?;
        
        // Fetch structure details from DB
        let row = sqlx::query!(
            r#"SELECT cbu_id, name, kind FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            structure_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow!("Structure not found: {}", structure_id))?;
        
        let structure_type = StructureType::from_str(&row.kind).unwrap_or_default();
        
        // Update session context
        if let Some(session) = ctx.pending_session_mut() {
            session.set_current_structure(structure_id, row.name.clone(), structure_type);
        }
        
        Ok(ExecutionResult::Record(serde_json::json!({
            "structure_id": structure_id,
            "structure_name": row.name,
            "structure_type": row.kind
        })))
    }
}
```

Similar implementations for `SessionSetCaseOp` and `SessionSetMandateOp`.

---

## P1: Macro Registry Loader

### Problem

Macro YAML files in `config/verb_schemas/macros/` exist but aren't loaded at runtime.

### Solution

Create `rust/src/dsl_v2/macro_registry.rs`:

```rust
//! Macro Registry - Loads operator macro definitions from YAML

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// A loaded macro definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroDefinition {
    pub fqn: String,
    pub kind: String,  // "macro" or "primitive"
    pub ui: MacroUi,
    pub routing: MacroRouting,
    pub target: MacroTarget,
    pub args: MacroArgs,
    pub prereqs: Vec<PrereqSpec>,
    pub expands_to: Vec<ExpansionStep>,
    pub unlocks: Vec<String>,
    pub sets_state: Vec<StateChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroUi {
    pub label: String,
    pub description: String,
    pub target_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroRouting {
    pub mode_tags: Vec<String>,
    pub operator_domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroTarget {
    pub operates_on: String,  // e.g., "structure_ref"
    pub produces: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroArgs {
    pub style: String,
    pub required: HashMap<String, ArgSpec>,
    pub optional: HashMap<String, ArgSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgSpec {
    #[serde(rename = "type")]
    pub arg_type: String,
    pub ui_label: String,
    #[serde(default)]
    pub autofill_from: Option<String>,
    #[serde(default)]
    pub picker: Option<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub values: Vec<EnumValue>,  // For enum types
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub key: String,
    pub label: String,
    pub internal: String,
    #[serde(default)]
    pub valid_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrereqSpec {
    #[serde(rename = "type")]
    pub prereq_type: String,  // "state_exists", "verb_completed", etc.
    pub key: Option<String>,
    pub verb: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionStep {
    pub verb: String,
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub key: String,
    pub value: serde_json::Value,
}

/// Registry of loaded macros
pub struct MacroRegistry {
    macros: HashMap<String, MacroDefinition>,
    by_domain: HashMap<String, Vec<String>>,  // domain -> [fqn]
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
            by_domain: HashMap::new(),
        }
    }

    /// Load all macros from a directory
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut registry = Self::new();
        
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                registry.load_file(&path)?;
            }
        }
        
        Ok(registry)
    }

    /// Load macros from a single YAML file
    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let doc: serde_yaml::Value = serde_yaml::from_str(&content)?;
        
        if let Some(mapping) = doc.as_mapping() {
            for (key, value) in mapping {
                if let Some(fqn) = key.as_str() {
                    // Skip YAML comments (keys starting with #)
                    if fqn.starts_with('#') || fqn.starts_with("_") {
                        continue;
                    }
                    
                    let mut def: MacroDefinition = serde_yaml::from_value(value.clone())?;
                    def.fqn = fqn.to_string();
                    
                    // Index by domain
                    let domain = fqn.split('.').next().unwrap_or("unknown").to_string();
                    self.by_domain
                        .entry(domain)
                        .or_default()
                        .push(fqn.to_string());
                    
                    self.macros.insert(fqn.to_string(), def);
                }
            }
        }
        
        Ok(())
    }

    /// Get a macro by FQN
    pub fn get(&self, fqn: &str) -> Option<&MacroDefinition> {
        self.macros.get(fqn)
    }

    /// List all macro FQNs
    pub fn list_all(&self) -> Vec<&str> {
        self.macros.keys().map(|s| s.as_str()).collect()
    }

    /// List macros in a domain
    pub fn list_by_domain(&self, domain: &str) -> Vec<&str> {
        self.by_domain
            .get(domain)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// List all domains
    pub fn list_domains(&self) -> Vec<&str> {
        self.by_domain.keys().map(|s| s.as_str()).collect()
    }

    /// Get macros filtered by mode tags
    pub fn filter_by_mode(&self, mode: &str) -> Vec<&MacroDefinition> {
        self.macros
            .values()
            .filter(|m| m.routing.mode_tags.contains(&mode.to_string()))
            .collect()
    }
}
```

### Initialization

In `ob-poc-web/src/main.rs`, load the registry at startup:

```rust
// Load macro registry
let macros_dir = config_dir.join("verb_schemas/macros");
let macro_registry = if macros_dir.exists() {
    Arc::new(MacroRegistry::load_from_dir(&macros_dir)?)
} else {
    Arc::new(MacroRegistry::new())
};
tracing::info!("Loaded {} operator macros", macro_registry.list_all().len());
```

---

## P2: Verb Taxonomy Endpoint

### Endpoint

`GET /api/verbs/taxonomy`

### Response

```json
{
  "domains": [
    {
      "name": "structure",
      "label": "Structure",
      "description": "Fund and mandate structure operations",
      "verbs": [
        {
          "fqn": "structure.setup",
          "label": "Set up Structure",
          "description": "Create a new fund or mandate structure",
          "target_label": "Structure",
          "mode_tags": ["onboarding", "kyc"],
          "prereqs_met": true
        },
        {
          "fqn": "structure.assign-role",
          "label": "Assign Role",
          "description": "Assign a party to a role on the structure",
          "target_label": "Role Assignment",
          "mode_tags": ["onboarding", "kyc"],
          "prereqs_met": false,
          "blocked_reason": "No structure selected"
        }
      ]
    },
    {
      "name": "case",
      "label": "Case",
      "description": "KYC case operations",
      "verbs": [...]
    },
    {
      "name": "mandate",
      "label": "Mandate",
      "description": "Investment mandate operations",
      "verbs": [...]
    }
  ],
  "mode_filter": null
}
```

### Query Parameters

| Param | Type | Description |
|-------|------|-------------|
| `mode` | string | Filter by mode tag (e.g., "kyc", "onboarding", "trading") |
| `session_id` | uuid | Include prereq status based on session state |

### Implementation

Create `rust/src/api/verb_taxonomy_routes.rs`:

```rust
use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::dsl_v2::macro_registry::MacroRegistry;
use crate::session::unified::UnifiedSession;
use crate::api::session_manager::SessionManager;

#[derive(Clone)]
pub struct VerbTaxonomyState {
    pub macro_registry: Arc<MacroRegistry>,
    pub session_manager: Arc<SessionManager>,
}

#[derive(Debug, Deserialize)]
pub struct TaxonomyQuery {
    pub mode: Option<String>,
    pub session_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct TaxonomyResponse {
    pub domains: Vec<DomainEntry>,
    pub mode_filter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainEntry {
    pub name: String,
    pub label: String,
    pub description: String,
    pub verbs: Vec<VerbEntry>,
}

#[derive(Debug, Serialize)]
pub struct VerbEntry {
    pub fqn: String,
    pub label: String,
    pub description: String,
    pub target_label: String,
    pub mode_tags: Vec<String>,
    pub prereqs_met: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

pub async fn get_taxonomy(
    State(state): State<VerbTaxonomyState>,
    Query(query): Query<TaxonomyQuery>,
) -> Json<TaxonomyResponse> {
    let mut domains: std::collections::HashMap<String, Vec<VerbEntry>> = std::collections::HashMap::new();
    
    // Get session for prereq checking if provided
    let session = if let Some(session_id) = query.session_id {
        state.session_manager.get(&session_id).await.ok()
    } else {
        None
    };
    
    for fqn in state.macro_registry.list_all() {
        if let Some(macro_def) = state.macro_registry.get(fqn) {
            // Filter by mode if specified
            if let Some(ref mode) = query.mode {
                if !macro_def.routing.mode_tags.contains(mode) {
                    continue;
                }
            }
            
            // Check prereqs
            let (prereqs_met, blocked_reason) = check_prereqs(&macro_def.prereqs, session.as_ref());
            
            let entry = VerbEntry {
                fqn: fqn.to_string(),
                label: macro_def.ui.label.clone(),
                description: macro_def.ui.description.clone(),
                target_label: macro_def.ui.target_label.clone(),
                mode_tags: macro_def.routing.mode_tags.clone(),
                prereqs_met,
                blocked_reason,
            };
            
            let domain = macro_def.routing.operator_domain.clone();
            domains.entry(domain).or_default().push(entry);
        }
    }
    
    // Convert to sorted domain list
    let mut domain_list: Vec<DomainEntry> = domains
        .into_iter()
        .map(|(name, verbs)| DomainEntry {
            label: domain_label(&name),
            description: domain_description(&name),
            name,
            verbs,
        })
        .collect();
    
    domain_list.sort_by(|a, b| a.name.cmp(&b.name));
    
    Json(TaxonomyResponse {
        domains: domain_list,
        mode_filter: query.mode,
    })
}

fn check_prereqs(prereqs: &[PrereqSpec], session: Option<&UnifiedSession>) -> (bool, Option<String>) {
    if prereqs.is_empty() {
        return (true, None);
    }
    
    let Some(session) = session else {
        // No session = can't check prereqs, assume blocked
        return (false, Some("No session context".to_string()));
    };
    
    for prereq in prereqs {
        match prereq.prereq_type.as_str() {
            "state_exists" => {
                if let Some(key) = &prereq.key {
                    let met = match key.as_str() {
                        "structure.exists" | "structure.selected" => session.current_structure.is_some(),
                        "case.exists" | "case.selected" => session.current_case.is_some(),
                        "mandate.exists" | "mandate.selected" => session.current_mandate.is_some(),
                        _ => session.dag_state.state_flags.get(key).copied().unwrap_or(false),
                    };
                    if !met {
                        return (false, Some(format!("Requires: {}", key)));
                    }
                }
            }
            "verb_completed" => {
                if let Some(verb) = &prereq.verb {
                    if !session.dag_state.completed.contains(verb) {
                        return (false, Some(format!("Requires {} completed", verb)));
                    }
                }
            }
            _ => {}
        }
    }
    
    (true, None)
}

fn domain_label(name: &str) -> String {
    match name {
        "structure" => "Structure".to_string(),
        "case" => "Case".to_string(),
        "mandate" => "Mandate".to_string(),
        "party" => "Party".to_string(),
        "document" => "Document".to_string(),
        _ => name.to_string(),
    }
}

fn domain_description(name: &str) -> String {
    match name {
        "structure" => "Fund and mandate structure operations".to_string(),
        "case" => "KYC case operations".to_string(),
        "mandate" => "Investment mandate operations".to_string(),
        "party" => "Party management".to_string(),
        "document" => "Document management".to_string(),
        _ => format!("{} operations", name),
    }
}

pub fn create_verb_taxonomy_router(
    macro_registry: Arc<MacroRegistry>,
    session_manager: Arc<SessionManager>,
) -> Router {
    let state = VerbTaxonomyState {
        macro_registry,
        session_manager,
    };
    
    Router::new()
        .route("/taxonomy", get(get_taxonomy))
        .route("/:fqn/schema", get(get_verb_schema))
        .with_state(state)
}
```

---

## P3: Verb Schema Endpoint

### Endpoint

`GET /api/verbs/{fqn}/schema`

### Response

Full schema for rendering the verb form UI:

```json
{
  "fqn": "structure.assign-role",
  "label": "Assign Role",
  "description": "Assign a party to a role on the structure",
  "target_label": "Role Assignment",
  "args": {
    "required": {
      "structure": {
        "type": "structure_ref",
        "ui_label": "Structure",
        "autofill_from": "session.current_structure",
        "picker": "structure_picker",
        "current_value": {
          "id": "uuid-...",
          "display_name": "Allianz SICAV 1"
        }
      },
      "role": {
        "type": "enum",
        "ui_label": "Role",
        "values": [
          {"key": "gp", "label": "General Partner", "enabled": true},
          {"key": "lp", "label": "Limited Partner", "enabled": true},
          {"key": "im", "label": "Investment Manager", "enabled": true},
          {"key": "manco", "label": "Management Company", "enabled": false, "reason": "Not valid for PE structure"}
        ],
        "default": "im"
      },
      "party": {
        "type": "party_ref",
        "ui_label": "Party",
        "picker": "party_picker"
      }
    },
    "optional": {
      "effective_date": {
        "type": "date",
        "ui_label": "Effective Date",
        "default": "today"
      }
    }
  },
  "prereqs_met": true,
  "preview_dsl": "(cbu-role.assign :cbu-id \"${structure}\" :role \"${role.internal}\" :entity-id \"${party}\")"
}
```

### Implementation

```rust
pub async fn get_verb_schema(
    State(state): State<VerbTaxonomyState>,
    Path(fqn): Path<String>,
    Query(query): Query<SchemaQuery>,
) -> Result<Json<VerbSchemaResponse>, StatusCode> {
    let macro_def = state.macro_registry.get(&fqn)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    // Get session for autofill values
    let session = if let Some(session_id) = query.session_id {
        state.session_manager.get(&session_id).await.ok()
    } else {
        None
    };
    
    // Build response with autofilled values
    let response = build_schema_response(macro_def, session.as_ref());
    
    Ok(Json(response))
}

fn build_schema_response(
    macro_def: &MacroDefinition,
    session: Option<&UnifiedSession>,
) -> VerbSchemaResponse {
    let mut required_args = HashMap::new();
    let mut optional_args = HashMap::new();
    
    for (name, spec) in &macro_def.args.required {
        required_args.insert(name.clone(), build_arg_schema(spec, session));
    }
    
    for (name, spec) in &macro_def.args.optional {
        optional_args.insert(name.clone(), build_arg_schema(spec, session));
    }
    
    VerbSchemaResponse {
        fqn: macro_def.fqn.clone(),
        label: macro_def.ui.label.clone(),
        description: macro_def.ui.description.clone(),
        target_label: macro_def.ui.target_label.clone(),
        args: ArgsSchema {
            required: required_args,
            optional: optional_args,
        },
        prereqs_met: true, // TODO: actual check
        preview_dsl: build_preview_dsl(macro_def),
    }
}

fn build_arg_schema(spec: &ArgSpec, session: Option<&UnifiedSession>) -> ArgSchemaResponse {
    let mut response = ArgSchemaResponse {
        arg_type: spec.arg_type.clone(),
        ui_label: spec.ui_label.clone(),
        autofill_from: spec.autofill_from.clone(),
        picker: spec.picker.clone(),
        current_value: None,
        values: None,
        default: spec.default.clone(),
    };
    
    // Autofill from session if available
    if let (Some(autofill), Some(session)) = (&spec.autofill_from, session) {
        response.current_value = resolve_autofill(autofill, session);
    }
    
    // Build enum values if applicable
    if !spec.values.is_empty() {
        response.values = Some(
            spec.values.iter().map(|v| EnumValueResponse {
                key: v.key.clone(),
                label: v.label.clone(),
                enabled: true, // TODO: check valid_for against current structure type
                reason: None,
            }).collect()
        );
    }
    
    response
}

fn resolve_autofill(path: &str, session: &UnifiedSession) -> Option<AutofillValue> {
    match path {
        "session.current_structure" => {
            session.current_structure.as_ref().map(|s| AutofillValue {
                id: s.structure_id.to_string(),
                display_name: s.display_name.clone(),
            })
        }
        "session.current_case" => {
            session.current_case.as_ref().map(|c| AutofillValue {
                id: c.case_id.to_string(),
                display_name: c.display_name.clone(),
            })
        }
        "session.current_mandate" => {
            session.current_mandate.as_ref().map(|m| AutofillValue {
                id: m.mandate_id.to_string(),
                display_name: m.display_name.clone(),
            })
        }
        _ => None,
    }
}
```

---

## P4: Display Noun Middleware

### Config File

Create `rust/config/display_nouns.yaml`:

```yaml
# Display Noun Configuration
#
# Maps internal terms to operator-facing vocabulary.
# Applied to all API responses via middleware.

nouns:
  cbu: "Structure"
  cbu_id: "structure"
  entity: "Party"
  entity_id: "party"
  entity_ref: null  # hidden - never show
  trading-profile: "Mandate"
  trading_profile_id: "mandate"
  kyc-case: "Case"
  kyc_case_id: "case"
  cbu-role: "Role"

# Error message translations
errors:
  "CBU not found": "Structure not found"
  "Entity not found": "Party not found"
  "Trading profile not found": "Mandate not found"
  "KYC case not found": "Case not found"
```

### Middleware Implementation

Create `rust/src/api/display_nouns.rs`:

```rust
//! Display Noun Middleware
//!
//! Translates internal terminology to operator vocabulary in API responses.

use axum::{
    body::{Body, Bytes},
    http::{Request, Response},
    middleware::Next,
};
use std::collections::HashMap;

lazy_static::lazy_static! {
    static ref NOUN_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("cbu", "structure");
        m.insert("CBU", "Structure");
        m.insert("cbu_id", "structure_id");
        m.insert("entity_ref", "");  // Remove completely
        m.insert("trading-profile", "mandate");
        m.insert("trading_profile", "mandate");
        m.insert("kyc-case", "case");
        m.insert("kyc_case", "case");
        m
    };
}

/// Middleware that translates internal terms in JSON responses
pub async fn translate_nouns(
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let response = next.run(request).await;
    
    // Only process JSON responses
    let content_type = response.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    
    if !content_type.contains("application/json") {
        return response;
    }
    
    // Read body
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };
    
    // Translate
    let translated = translate_json_string(&bytes);
    
    Response::from_parts(parts, Body::from(translated))
}

fn translate_json_string(input: &[u8]) -> Bytes {
    let mut s = String::from_utf8_lossy(input).to_string();
    
    for (internal, display) in NOUN_MAP.iter() {
        if display.is_empty() {
            // Remove term entirely (for entity_ref etc.)
            s = s.replace(&format!("\"{}\"", internal), "null");
        } else {
            s = s.replace(internal, display);
        }
    }
    
    Bytes::from(s)
}
```

---

## P5: Wire Macros into Verb Search (Fallback)

### Integration Point

In `rust/src/mcp/verb_search.rs`, add macro registry as a source:

```rust
impl HybridVerbSearcher {
    pub fn with_macro_registry(mut self, registry: Arc<MacroRegistry>) -> Self {
        self.macro_registry = Some(registry);
        self
    }
    
    /// Search macros first (operator vocabulary), then primitives
    pub async fn search_with_macros(
        &self,
        query: &str,
        context: &SearchContext,
    ) -> VerbSearchOutcome {
        // 1. Try macro exact match (by label)
        if let Some(ref registry) = self.macro_registry {
            for fqn in registry.list_all() {
                if let Some(def) = registry.get(fqn) {
                    if def.ui.label.eq_ignore_ascii_case(query) {
                        return VerbSearchOutcome::Matched(VerbSearchResult {
                            verb_fqn: fqn.to_string(),
                            score: 1.0,
                            source: "macro_exact".to_string(),
                            is_macro: true,
                        });
                    }
                }
            }
        }
        
        // 2. Fall back to existing search
        self.search(query, context).await
    }
}
```

### Generate Invocation Phrases for Macros

Add to macro registry loader:

```rust
impl MacroRegistry {
    /// Generate invocation phrases from macro metadata
    pub fn generate_invocation_phrases(&self) -> Vec<(String, String)> {
        let mut phrases = Vec::new();
        
        for fqn in self.list_all() {
            if let Some(def) = self.get(fqn) {
                // Label as phrase
                phrases.push((def.ui.label.to_lowercase(), fqn.to_string()));
                
                // "verb the target" pattern
                let verb = fqn.split('.').last().unwrap_or(fqn);
                let target = def.ui.target_label.to_lowercase();
                phrases.push((format!("{} {}", verb, target), fqn.to_string()));
                phrases.push((format!("{} a {}", verb, target), fqn.to_string()));
                phrases.push((format!("{} the {}", verb, target), fqn.to_string()));
            }
        }
        
        phrases
    }
}
```

---

## File Summary

| File | Action | Description |
|------|--------|-------------|
| `config/verbs/session.yaml` | Modify | Add `set-structure`, `set-case`, `set-mandate` verbs |
| `src/domain_ops/session_ops.rs` | Modify | Add `SessionSet*Op` implementations |
| `src/dsl_v2/macro_registry.rs` | Create | Macro registry loader |
| `src/dsl_v2/mod.rs` | Modify | Export macro_registry |
| `src/api/verb_taxonomy_routes.rs` | Create | Taxonomy + schema endpoints |
| `src/api/display_nouns.rs` | Create | Display noun middleware |
| `src/api/mod.rs` | Modify | Export new routes |
| `config/display_nouns.yaml` | Create | Noun translation config |
| `ob-poc-web/src/main.rs` | Modify | Initialize macro registry, wire routes |
| `src/mcp/verb_search.rs` | Modify | Add macro search fallback |

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_macro_registry_load() {
        let registry = MacroRegistry::load_from_dir(Path::new("config/verb_schemas/macros")).unwrap();
        
        assert!(registry.get("structure.setup").is_some());
        assert!(registry.get("case.open").is_some());
        assert!(registry.get("mandate.create").is_some());
        
        // Check domain grouping
        let structure_verbs = registry.list_by_domain("structure");
        assert!(structure_verbs.contains(&"structure.setup"));
    }
    
    #[test]
    fn test_prereq_checking() {
        let prereqs = vec![PrereqSpec {
            prereq_type: "state_exists".to_string(),
            key: Some("structure.exists".to_string()),
            verb: None,
        }];
        
        // No session = blocked
        let (met, reason) = check_prereqs(&prereqs, None);
        assert!(!met);
        
        // Session with structure = met
        let mut session = UnifiedSession::new_empty();
        session.set_current_structure(Uuid::new_v4(), "Test".to_string(), StructureType::Pe);
        let (met, reason) = check_prereqs(&prereqs, Some(&session));
        assert!(met);
    }
    
    #[test]
    fn test_display_noun_translation() {
        let input = r#"{"cbu_id": "123", "entity": "test"}"#;
        let output = translate_json_string(input.as_bytes());
        let output_str = String::from_utf8_lossy(&output);
        
        assert!(output_str.contains("structure_id"));
        assert!(!output_str.contains("cbu_id"));
    }
}
```

### Integration Tests

```bash
# Test taxonomy endpoint
curl http://localhost:3000/api/verbs/taxonomy | jq '.domains[].name'
# Expected: ["case", "mandate", "structure"]

# Test schema endpoint with session
curl "http://localhost:3000/api/verbs/structure.assign-role/schema?session_id=$SESSION_ID" | jq '.args.required.structure.current_value'
# Expected: autofilled structure from session

# Test verb search with macro
curl -X POST http://localhost:3000/api/verb-search -d '{"query": "set up structure"}' | jq '.verb_fqn'
# Expected: "structure.setup"
```

---

## Rollout

1. **Phase 3B-1:** P0 + P1 (session verbs + macro registry) - enables macro expansion
2. **Phase 3B-2:** P2 + P3 (taxonomy + schema endpoints) - enables UI
3. **Phase 3B-3:** P4 + P5 (display nouns + search fallback) - polish

Each phase is independently deployable and testable.
