# TODO: Research Macros - Pre-Verb Discovery Workflows

**Created:** 2026-01-01
**Priority:** MEDIUM
**Assignee:** Claude Code
**Depends On:** Existing macro infrastructure

---

## ⚠️ IMPLEMENTATION STATUS (Updated 2026-01-02)

### Summary

The **Research Macro wrapper system described below was NOT implemented**. Instead, the underlying GLEIF DSL verbs were implemented directly, providing the same deterministic functionality without the LLM + human review wrapper layer.

### What WAS Implemented

| Component | Status | Location |
|-----------|--------|----------|
| GLEIF DSL Verbs | ✅ FULLY IMPLEMENTED | `rust/config/verbs/gleif.yaml`, `rust/src/dsl_v2/custom_ops/gleif_ops.rs` |
| GLEIF API Client | ✅ IMPLEMENTED | `rust/src/gleif/client.rs` |
| Entity Enrichment | ✅ IMPLEMENTED | `gleif.enrich`, `gleif.import-tree` |
| Ownership Tracing | ✅ IMPLEMENTED | `gleif.trace-ownership` |
| Managed Funds Discovery | ✅ IMPLEMENTED | `gleif.get-managed-funds`, `gleif.import-managed-funds` |
| Fund Structure Queries | ✅ IMPLEMENTED | `gleif.get-umbrella`, `gleif.get-manager`, `gleif.get-master-fund` |
| Test Harness (Allianz) | ✅ IMPLEMENTED | `rust/xtask/src/gleif_test.rs`, `cargo x gleif-test` |

### What was NOT Implemented

| Component | Status | Notes |
|-----------|--------|-------|
| Research Macro Schema (YAML) | ❌ NOT IMPLEMENTED | No `config/macros/research/` directory |
| `MacroType::Research` enum | ❌ NOT IMPLEMENTED | Only `Template` exists |
| `ResearchExecutor` | ❌ NOT IMPLEMENTED | No LLM + review wrapper |
| MCP `execute_research_macro` | ❌ NOT IMPLEMENTED | Direct verb calls used instead |
| `macro_rag_metadata.rs` | ❌ NOT IMPLEMENTED | Verb RAG exists in `verb_rag_metadata.rs` |

### Why the Difference?

The GLEIF verbs provide **deterministic, database-backed enrichment** without needing:
1. LLM interpretation layer (verbs take LEIs directly)
2. Human review gate (GLEIF data is authoritative)
3. JSON schema validation (verb args are validated by DSL parser)

The "research macro" concept was designed for fuzzy discovery → human review → deterministic execution. Since GLEIF provides authoritative data directly, the intermediate layer was unnecessary.

### Verified Working (2026-01-02)

Tested with both **Allianz** and **Aviva** clients:

```bash
# Allianz SE (German insurance parent)
(gleif.get-record :lei "529900K9B0N5BT694847")  # ✅ Works

# Aviva plc (UK insurance)  
(gleif.get-record :lei "YF0Y5B0IB8SM0ZFG9G81")  # ✅ Works

# Search functionality
(gleif.search :name "Aviva" :jurisdiction "GB" :limit 5)  # ✅ Works
```

Full GLEIF API test harness passes with Allianz data:
```bash
cargo x gleif-test  # ✅ All 6 endpoints tested, 13 relationship types discovered
```

### Suggested Verbs from TODO → Actual Implementation

| TODO Suggested | Actual Verb | Status |
|----------------|-------------|--------|
| `gleif.enrich :lei "{{apex.lei}}"` | `gleif.enrich :lei "..." :as @entity` | ✅ Implemented |
| `gleif.trace-ownership :entity-id @apex` | `gleif.trace-ownership :entity-id @entity` | ✅ Implemented |
| `gleif.get-managed-funds :manager-lei "..."` | `gleif.get-managed-funds :manager-lei "..."` | ✅ Implemented |

### Additional Verbs Implemented (Beyond TODO)

| Verb | Description |
|------|-------------|
| `gleif.search` | Search GLEIF by name, jurisdiction, or category |
| `gleif.import-tree` | Import corporate tree (parents and/or children) |
| `gleif.import-managed-funds` | Import funds with full CBU structure and roles |
| `gleif.refresh` | Refresh stale GLEIF data |
| `gleif.get-record` | Fetch raw GLEIF record (does not persist) |
| `gleif.get-parent` | Get direct parent relationship |
| `gleif.get-children` | Get direct children |
| `gleif.get-umbrella` | Get umbrella fund for a sub-fund |
| `gleif.get-manager` | Get fund manager |
| `gleif.get-master-fund` | Get master fund for feeder |
| `gleif.lookup-by-isin` | Look up entity LEI by ISIN |
| `gleif.resolve-successor` | Resolve merged/inactive LEI to successor |

### Recommendation

The Research Macro system as designed is **no longer needed** for GLEIF operations. The direct verb approach is simpler and more reliable. However, the Research Macro pattern could still be valuable for:

1. **Unstructured data sources** (news, web scraping) where LLM interpretation is needed
2. **Compliance research** where human review is mandatory before action
3. **Multi-source reconciliation** where confidence scoring matters

If Research Macros are implemented in the future, they should target these use cases rather than GLEIF (which has authoritative, structured data).

---

## Original Design (Archived for Reference)

<details>
<summary>Click to expand original TODO content</summary>

## Concept

Research macros are a new macro type that bridges fuzzy LLM discovery and deterministic DSL verbs.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Research Macro  │ ──► │  Human Review   │ ──► │  GLEIF Verbs    │
│ (LLM + search)  │     │  (approve/edit) │     │  (deterministic)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
      fuzzy                   gate                   100% reliable
```

---

## Macro Type Spectrum

```rust
pub enum MacroType {
    Template,   // Existing - pure DSL expansion, deterministic
    Research,   // NEW - LLM + tools → structured data → human review
    Agent,      // FUTURE - multi-step reasoning loop
}
```

| Type | Input | Processing | Output | Review |
|------|-------|------------|--------|--------|
| Template | params | expand template | DSL | optional |
| Research | params | LLM + web search | JSON | **required** |
| Agent | task | reasoning loop | varies | optional |

---

## Part 1: Research Macro Schema

### YAML Definition

Create `config/macros/research/client-discovery.yaml`:

```yaml
macro:
  name: research.client-discovery
  type: research
  version: "1.0"
  description: "Research institutional client structure for onboarding"
  
  parameters:
    - name: client-name
      type: string
      required: true
      description: "Name of the institutional client"
    - name: jurisdiction-hint
      type: string
      required: false
      description: "ISO country code hint (e.g., DE, GB, US)"
    - name: sector
      type: string
      default: "asset-management"
      enum: ["asset-management", "insurance", "banking", "corporate"]
      
  tools:
    - web_search
    
  prompt: |
    I need to research {{client-name}} for institutional client onboarding.
    {{#if jurisdiction-hint}}The client is likely based in {{jurisdiction-hint}}.{{/if}}
    Sector: {{sector}}
    
    Please identify:
    
    1. **Head Office / Apex Entity**
       - Legal name (exact registered name)
       - Jurisdiction (ISO country code)
       - LEI (20-character identifier if findable)
       - Stock exchange listing (if public company)
       - UBO status (PUBLIC_FLOAT if listed, else note if state-owned, etc.)
    
    2. **Asset Management Subsidiaries** (if sector is asset-management)
       - Investment managers / ManCos
       - Their jurisdictions
       - LEIs where findable
       - Role (IM, ManCo, Depositary, etc.)
    
    3. **Fund Structures**
       - SICAV umbrellas (especially Luxembourg, Ireland, Cayman)
       - Notable fund ranges or flagship funds
       - Approximate fund count if known
    
    4. **Regulatory Context**
       - Primary regulator
       - Any recent M&A activity affecting structure
       - Any notable compliance events
    
    Search the web for current information. Check GLEIF, company websites,
    Wikipedia, financial news sources.
    
    Return ONLY valid JSON matching the output schema. No markdown, no explanation.
    
  output:
    schema_name: client-discovery-result
    schema:
      type: object
      required: [apex, search_quality]
      properties:
        apex:
          type: object
          required: [name, jurisdiction]
          properties:
            name: { type: string }
            jurisdiction: { type: string, pattern: "^[A-Z]{2}$" }
            lei: { type: string, pattern: "^[A-Z0-9]{20}$" }
            listing: { type: string }
            ubo_status: 
              type: string
              enum: [PUBLIC_FLOAT, STATE_OWNED, PRIVATE, UNKNOWN]
        subsidiaries:
          type: array
          items:
            type: object
            properties:
              name: { type: string }
              jurisdiction: { type: string }
              lei: { type: string }
              role: { type: string }
        fund_structures:
          type: array
          items:
            type: object
            properties:
              name: { type: string }
              type: { type: string }
              jurisdiction: { type: string }
              fund_count: { type: integer }
        regulatory:
          type: object
          properties:
            primary_regulator: { type: string }
            notes: { type: string }
        search_quality:
          type: string
          enum: [HIGH, MEDIUM, LOW]
          description: "Self-assessment of search result quality"
    
    review: required
    
  suggested_verbs: |
    ;; After review, enrich discovered entities:
    (gleif.enrich :lei "{{apex.lei}}" :as @apex)
    {{#each subsidiaries}}
    {{#if this.lei}}
    (gleif.enrich :lei "{{this.lei}}" :as @{{slugify this.role}})
    {{/if}}
    {{/each}}
    
    ;; Verify ownership chain:
    (gleif.trace-ownership :entity-id @apex :as @ubo_chain)
    
    ;; Get managed funds for each ManCo:
    {{#each subsidiaries}}
    {{#if (eq this.role "ManCo")}}
    (gleif.get-managed-funds :manager-lei "{{this.lei}}" :as @funds_{{@index}})
    {{/if}}
    {{/each}}
```

---

## Part 2: Macro Registry Extension

### Update macro_registry.rs

```rust
// rust/src/dsl_v2/macro_registry.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MacroType {
    Template,
    Research,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchMacroDef {
    pub name: String,
    pub macro_type: MacroType,
    pub version: String,
    pub description: String,
    pub parameters: Vec<MacroParamDef>,
    pub tools: Vec<String>,
    pub prompt: String,
    pub output: ResearchOutput,
    pub suggested_verbs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchOutput {
    pub schema_name: String,
    pub schema: serde_json::Value,  // JSON Schema
    pub review: ReviewRequirement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewRequirement {
    Required,
    Optional,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    #[serde(default)]
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<serde_json::Value>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
}

/// Registry for all macro types
pub struct MacroRegistry {
    templates: HashMap<String, TemplateMacroDef>,
    research: HashMap<String, ResearchMacroDef>,
}

impl MacroRegistry {
    pub fn load_from_config() -> Result<Self, anyhow::Error> {
        let mut registry = Self {
            templates: HashMap::new(),
            research: HashMap::new(),
        };
        
        // Load template macros from config/macros/templates/
        registry.load_templates()?;
        
        // Load research macros from config/macros/research/
        registry.load_research_macros()?;
        
        Ok(registry)
    }
    
    fn load_research_macros(&mut self) -> Result<(), anyhow::Error> {
        let research_dir = Path::new("config/macros/research");
        if !research_dir.exists() {
            return Ok(());
        }
        
        for entry in std::fs::read_dir(research_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "yaml").unwrap_or(false) {
                let content = std::fs::read_to_string(&path)?;
                let def: ResearchMacroWrapper = serde_yaml::from_str(&content)?;
                self.research.insert(def.macro_def.name.clone(), def.macro_def);
            }
        }
        
        Ok(())
    }
    
    pub fn get_research_macro(&self, name: &str) -> Option<&ResearchMacroDef> {
        self.research.get(name)
    }
}

#[derive(Deserialize)]
struct ResearchMacroWrapper {
    #[serde(rename = "macro")]
    macro_def: ResearchMacroDef,
}
```

---

## Part 3: Research Executor

### Create research_executor.rs

```rust
// rust/src/dsl_v2/research_executor.rs

use anyhow::Result;
use serde_json::Value;

/// Result of executing a research macro
#[derive(Debug, Clone)]
pub struct ResearchResult {
    /// The structured data returned by LLM
    pub data: Value,
    
    /// Whether the data passed schema validation
    pub schema_valid: bool,
    
    /// Validation errors if any
    pub validation_errors: Vec<String>,
    
    /// Whether human review is required before use
    pub review_required: bool,
    
    /// Suggested DSL verbs (template expanded with data)
    pub suggested_verbs: Option<String>,
    
    /// Search quality self-assessment
    pub search_quality: Option<String>,
}

/// Execute a research macro
pub struct ResearchExecutor {
    macro_registry: MacroRegistry,
}

impl ResearchExecutor {
    pub fn new(registry: MacroRegistry) -> Self {
        Self { macro_registry: registry }
    }
    
    /// Execute a research macro with given parameters
    pub async fn execute(
        &self,
        macro_name: &str,
        params: HashMap<String, Value>,
        llm_client: &dyn LlmClient,
    ) -> Result<ResearchResult> {
        // 1. Get macro definition
        let macro_def = self.macro_registry
            .get_research_macro(macro_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown research macro: {}", macro_name))?;
        
        // 2. Validate parameters
        self.validate_params(&macro_def.parameters, &params)?;
        
        // 3. Render prompt template with parameters
        let prompt = self.render_prompt(&macro_def.prompt, &params)?;
        
        // 4. Execute LLM call with tools enabled
        let tools = macro_def.tools.iter()
            .map(|t| self.get_tool_def(t))
            .collect();
        
        let llm_response = llm_client
            .complete_with_tools(&prompt, &tools)
            .await?;
        
        // 5. Parse JSON response
        let data: Value = serde_json::from_str(&llm_response.content)
            .map_err(|e| anyhow::anyhow!("LLM did not return valid JSON: {}", e))?;
        
        // 6. Validate against output schema
        let (schema_valid, validation_errors) = self.validate_schema(
            &macro_def.output.schema,
            &data,
        );
        
        // 7. Render suggested verbs template
        let suggested_verbs = macro_def.suggested_verbs.as_ref()
            .map(|template| self.render_verbs_template(template, &data))
            .transpose()?;
        
        // 8. Extract search quality if present
        let search_quality = data.get("search_quality")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        Ok(ResearchResult {
            data,
            schema_valid,
            validation_errors,
            review_required: macro_def.output.review == ReviewRequirement::Required,
            suggested_verbs,
            search_quality,
        })
    }
    
    fn validate_params(
        &self,
        param_defs: &[MacroParamDef],
        params: &HashMap<String, Value>,
    ) -> Result<()> {
        for def in param_defs {
            if def.required && !params.contains_key(&def.name) {
                if def.default.is_none() {
                    return Err(anyhow::anyhow!(
                        "Missing required parameter: {}", def.name
                    ));
                }
            }
        }
        Ok(())
    }
    
    fn render_prompt(&self, template: &str, params: &HashMap<String, Value>) -> Result<String> {
        // Use handlebars or similar for template rendering
        let mut handlebars = handlebars::Handlebars::new();
        handlebars.register_template_string("prompt", template)?;
        Ok(handlebars.render("prompt", params)?)
    }
    
    fn validate_schema(&self, schema: &Value, data: &Value) -> (bool, Vec<String>) {
        // Use jsonschema crate for validation
        let compiled = jsonschema::JSONSchema::compile(schema)
            .expect("Invalid JSON schema in macro definition");
        
        match compiled.validate(data) {
            Ok(_) => (true, vec![]),
            Err(errors) => {
                let error_msgs: Vec<String> = errors
                    .map(|e| format!("{}: {}", e.instance_path, e))
                    .collect();
                (false, error_msgs)
            }
        }
    }
    
    fn render_verbs_template(&self, template: &str, data: &Value) -> Result<String> {
        let mut handlebars = handlebars::Handlebars::new();
        
        // Register helper for slugify
        handlebars.register_helper("slugify", Box::new(slugify_helper));
        
        // Register helper for equality check
        handlebars.register_helper("eq", Box::new(eq_helper));
        
        handlebars.register_template_string("verbs", template)?;
        Ok(handlebars.render("verbs", data)?)
    }
}
```

---

## Part 4: MCP Integration

### Register as MCP Tool

```rust
// In MCP tool registration

McpTool {
    name: "execute_research_macro",
    description: "Execute a research macro to gather structured data about a topic. \
        Research macros use LLM reasoning and web search to produce structured \
        output that requires human review before use.",
    input_schema: json!({
        "type": "object",
        "required": ["macro_name", "params"],
        "properties": {
            "macro_name": {
                "type": "string",
                "description": "Name of the research macro (e.g., 'client-discovery')"
            },
            "params": {
                "type": "object",
                "description": "Parameters for the macro"
            }
        }
    }),
    handler: |args| {
        let macro_name = args["macro_name"].as_str()?;
        let params = args["params"].as_object()?;
        
        let executor = ResearchExecutor::new(registry);
        let result = executor.execute(macro_name, params, llm_client).await?;
        
        Ok(json!({
            "data": result.data,
            "schema_valid": result.schema_valid,
            "validation_errors": result.validation_errors,
            "review_required": result.review_required,
            "suggested_verbs": result.suggested_verbs,
            "search_quality": result.search_quality,
        }))
    },
}
```

### List Available Research Macros

```rust
McpTool {
    name: "list_research_macros",
    description: "List available research macros with their descriptions and parameters",
    input_schema: json!({
        "type": "object",
        "properties": {}
    }),
    handler: |_args| {
        let registry = MacroRegistry::load_from_config()?;
        let macros: Vec<_> = registry.research_macros()
            .map(|m| json!({
                "name": m.name,
                "description": m.description,
                "parameters": m.parameters,
            }))
            .collect();
        
        Ok(json!({ "macros": macros }))
    },
}
```

---

## Part 5: RAG Hints

### Create macro_rag_metadata.rs

```rust
// rust/src/session/macro_rag_metadata.rs

use crate::dsl_v2::macro_registry::MacroType;

pub struct MacroRagEntry {
    pub name: &'static str,
    pub macro_type: MacroType,
    pub description: &'static str,
    pub example_prompts: &'static [&'static str],
    pub example_invocation: &'static str,
    pub produces: &'static str,
    pub suggested_follow_up: &'static [&'static str],
}

pub fn research_macro_rag_entries() -> Vec<MacroRagEntry> {
    vec![
        MacroRagEntry {
            name: "research.client-discovery",
            macro_type: MacroType::Research,
            description: "Research a new institutional client to identify head office, \
                ManCos, fund structures, and regulatory context. Uses web search to \
                find LEIs and corporate structure. Returns structured JSON for human \
                review before GLEIF enrichment.",
            example_prompts: &[
                "research Allianz for onboarding",
                "I need to onboard Aviva, find their structure",
                "discover BlackRock's corporate entities",
                "what's the structure of State Street",
                "find the ManCos for Vanguard",
                "research a new client called Fidelity",
            ],
            example_invocation: "(macro.research :template \"client-discovery\" \
                :client-name \"Allianz\" :jurisdiction-hint \"DE\")",
            produces: "client-discovery-result",
            suggested_follow_up: &[
                "gleif.enrich",
                "gleif.trace-ownership",
                "gleif.get-managed-funds",
            ],
        },
        
        MacroRagEntry {
            name: "research.ubo-investigation",
            macro_type: MacroType::Research,
            description: "Research ultimate beneficial ownership for a complex entity. \
                Searches corporate registries, news sources, and regulatory filings \
                to identify natural person UBOs or confirm public float status.",
            example_prompts: &[
                "who owns this company",
                "investigate the UBO chain",
                "find the beneficial owners",
                "is this company publicly traded",
            ],
            example_invocation: "(macro.research :template \"ubo-investigation\" \
                :entity-name \"Acme Holdings\" :jurisdiction \"KY\")",
            produces: "ubo-investigation-result",
            suggested_follow_up: &[
                "gleif.trace-ownership",
                "bods.search",
            ],
        },
        
        MacroRagEntry {
            name: "research.regulatory-check",
            macro_type: MacroType::Research,
            description: "Research regulatory status and any compliance concerns for \
                an entity. Checks for sanctions, enforcement actions, adverse news.",
            example_prompts: &[
                "check for regulatory issues",
                "any compliance concerns with this company",
                "is this entity sanctioned",
                "adverse news check",
            ],
            example_invocation: "(macro.research :template \"regulatory-check\" \
                :entity-name \"Example Corp\" :jurisdictions [\"US\", \"EU\"])",
            produces: "regulatory-check-result",
            suggested_follow_up: &[
                "screening.run",
            ],
        },
    ]
}
```

---

## Part 6: Additional Research Macros

### ubo-investigation.yaml

```yaml
macro:
  name: research.ubo-investigation
  type: research
  description: "Investigate ultimate beneficial ownership for complex structures"
  
  parameters:
    - name: entity-name
      type: string
      required: true
    - name: jurisdiction
      type: string
      required: false
    - name: known-lei
      type: string
      required: false
      
  tools:
    - web_search
    
  prompt: |
    Investigate the ultimate beneficial ownership of {{entity-name}}.
    {{#if jurisdiction}}Jurisdiction: {{jurisdiction}}{{/if}}
    {{#if known-lei}}Known LEI: {{known-lei}}{{/if}}
    
    Determine:
    1. Is this entity publicly traded? (→ PUBLIC_FLOAT)
    2. Is it state-owned? (→ STATE_OWNED)
    3. Can you identify natural person UBOs?
    4. Is ownership obscured through complex structures?
    
    Search corporate registries, GLEIF, news sources, regulatory filings.
    
  output:
    schema_name: ubo-investigation-result
    schema:
      type: object
      properties:
        terminus_type:
          type: string
          enum: [PUBLIC_FLOAT, STATE_OWNED, NATURAL_PERSONS, UNKNOWN]
        natural_persons:
          type: array
          items:
            type: object
            properties:
              name: { type: string }
              nationality: { type: string }
              ownership_percentage: { type: string }
              source: { type: string }
        ownership_chain:
          type: array
          items:
            type: object
            properties:
              entity_name: { type: string }
              jurisdiction: { type: string }
              ownership_type: { type: string }
        concerns:
          type: array
          items: { type: string }
        confidence:
          type: string
          enum: [HIGH, MEDIUM, LOW]
    review: required
```

---

## Part 7: File Structure

```
config/
├── macros/
│   ├── templates/               # Existing template macros (deterministic)
│   │   └── onboard-fund.yaml
│   │
│   └── research/                # NEW research macros (LLM + review)
│       ├── client-discovery.yaml
│       ├── ubo-investigation.yaml
│       └── regulatory-check.yaml

rust/src/dsl_v2/
├── macro_registry.rs            # Extended with MacroType enum
├── research_executor.rs         # NEW - executes research macros
└── mod.rs                       # Export new modules

rust/src/session/
├── macro_rag_metadata.rs        # NEW - RAG hints for macros
└── mod.rs                       # Export new module

rust/src/mcp/
├── tools.rs                     # Add execute_research_macro, list_research_macros
```

---

## Part 8: Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_load_research_macro() {
        let registry = MacroRegistry::load_from_config().unwrap();
        let macro_def = registry.get_research_macro("client-discovery").unwrap();
        
        assert_eq!(macro_def.macro_type, MacroType::Research);
        assert!(macro_def.tools.contains(&"web_search".to_string()));
        assert_eq!(macro_def.output.review, ReviewRequirement::Required);
    }
    
    #[test]
    fn test_validate_params() {
        let executor = ResearchExecutor::new(registry);
        
        // Missing required param
        let params = HashMap::new();
        assert!(executor.validate_params(&macro_def.parameters, &params).is_err());
        
        // With required param
        let mut params = HashMap::new();
        params.insert("client-name".to_string(), json!("Allianz"));
        assert!(executor.validate_params(&macro_def.parameters, &params).is_ok());
    }
    
    #[test]
    fn test_schema_validation() {
        let executor = ResearchExecutor::new(registry);
        
        // Valid data
        let data = json!({
            "apex": { "name": "Allianz SE", "jurisdiction": "DE" },
            "search_quality": "HIGH"
        });
        let (valid, errors) = executor.validate_schema(&schema, &data);
        assert!(valid);
        assert!(errors.is_empty());
        
        // Invalid jurisdiction format
        let data = json!({
            "apex": { "name": "Allianz SE", "jurisdiction": "Germany" },
            "search_quality": "HIGH"
        });
        let (valid, errors) = executor.validate_schema(&schema, &data);
        assert!(!valid);
    }
}
```

### Integration Test

```rust
#[tokio::test]
async fn test_research_macro_end_to_end() {
    let registry = MacroRegistry::load_from_config().unwrap();
    let executor = ResearchExecutor::new(registry);
    let mock_llm = MockLlmClient::new();
    
    // Set up mock to return valid JSON
    mock_llm.set_response(json!({
        "apex": {
            "name": "Allianz SE",
            "jurisdiction": "DE",
            "lei": "529900K9B0N5BT694847",
            "listing": "FRA:ALV",
            "ubo_status": "PUBLIC_FLOAT"
        },
        "subsidiaries": [{
            "name": "Allianz Global Investors GmbH",
            "jurisdiction": "DE",
            "lei": "OJ2TIQSVQND4IZYYK658",
            "role": "ManCo"
        }],
        "search_quality": "HIGH"
    }).to_string());
    
    let mut params = HashMap::new();
    params.insert("client-name".to_string(), json!("Allianz"));
    params.insert("jurisdiction-hint".to_string(), json!("DE"));
    
    let result = executor.execute("client-discovery", params, &mock_llm).await.unwrap();
    
    assert!(result.schema_valid);
    assert!(result.review_required);
    assert_eq!(result.search_quality, Some("HIGH".to_string()));
    assert!(result.suggested_verbs.unwrap().contains("gleif.enrich"));
}
```

---

## Summary

| Component | Action |
|-----------|--------|
| `config/macros/research/` | Create directory with YAML macro definitions |
| `macro_registry.rs` | Add `MacroType::Research` and loader |
| `research_executor.rs` | Create executor with prompt rendering, schema validation |
| `macro_rag_metadata.rs` | Add RAG hints for agent discovery |
| MCP tools | Register `execute_research_macro`, `list_research_macros` |
| Tests | Unit + integration tests |

---

## Usage Flow

```
User: "I need to onboard Aviva"
         │
         ▼
Agent matches RAG hint → research.client-discovery
         │
         ▼
(macro.research :template "client-discovery" :client-name "Aviva" :jurisdiction-hint "GB")
         │
         ▼
Research Executor:
  1. Load macro definition
  2. Render prompt with params
  3. Call LLM with web_search enabled
  4. Parse JSON response
  5. Validate against schema
  6. Return ResearchResult
         │
         ▼
UI shows result for human review
  [✓ Approve]  [✎ Edit]  [✗ Reject]
         │
         ▼ (on approve)
Generate suggested verbs:
  (gleif.enrich :lei "213800..." :as @aviva_plc)
  (gleif.enrich :lei "549300..." :as @aviva_im)
  (gleif.trace-ownership :entity-id @aviva_im)
         │
         ▼
Execute deterministic verbs
```

</details>
