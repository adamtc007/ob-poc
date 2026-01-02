# Research Macros Implementation Plan - ENHANCED

## Overview

Implement the Research Macro system that bridges **fuzzy LLM discovery** → **human review** → **deterministic GLEIF DSL verbs**. This provides a structured way for agents to research clients using web search and LLM reasoning, producing validated JSON that requires human approval before generating executable DSL.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Research Macro  │ ──► │  Human Review   │ ──► │  GLEIF Verbs    │
│ (LLM + search)  │     │  (approve/edit) │     │  (deterministic)│
└─────────────────┘     └─────────────────┘     └─────────────────┘
      fuzzy                   gate                   100% reliable
```

---

## 🔴 GAPS IN ORIGINAL PLAN (Now Addressed)

| Gap | Issue | Enhancement |
|-----|-------|-------------|
| **Session State** | No mention of how results persist across turns | Add `ResearchContext` to `UnifiedSessionContext` |
| **Approval Flow** | Missing `approve_research_result` tool | Add approval tool + session state transitions |
| **Web Search Integration** | Unclear how LLM calls web_search | Define `ResearchLlmClient` trait with tool use |
| **Error Recovery** | What if LLM returns garbage? | JSON repair + retry logic |
| **Verb Execution** | After approval, then what? | Pipeline to `batch_*` tools or direct `dsl_execute` |
| **Tool Naming** | `research_macro_*` inconsistent with `template_*` | Use `research_list`, `research_execute`, `research_approve` |
| **Batch Integration** | No connection to bulk operations | Research results feed entity discovery for batch |
| **LEI Validation** | LLM might hallucinate LEIs | Optional GLEIF ping to verify LEI exists |

---

## Key Design Decisions

1. **Follows existing template patterns** - Uses same registry/expander architecture as `ob-templates`
2. **MCP-first** - Exposed as MCP tools for agent use
3. **Schema validation** - JSON Schema enforces output structure from LLM
4. **Human review gate** - Results require approval before DSL generation
5. **Handlebars templating** - For prompt rendering and verb template expansion
6. **Session persistence** - Research results stored in session for multi-turn review ← **NEW**
7. **Batch pipeline** - Approved results can feed into bulk onboarding ← **NEW**

---

## Files to Create

### 1. Research Macro YAML Config Directory
```
rust/config/macros/research/
├── client-discovery.yaml      # Research institutional client structure
├── ubo-investigation.yaml     # Investigate UBO chain  
└── regulatory-check.yaml      # Check regulatory status/concerns
```

### 2. Research Macro Module (in existing crates structure)
```
rust/src/research/                # NEW module in main crate
├── mod.rs                        # Module exports
├── definition.rs                 # ResearchMacroDef, MacroParamDef, ReviewRequirement
├── registry.rs                   # ResearchMacroRegistry (load from YAML)
├── executor.rs                   # ResearchExecutor (LLM + schema validation)
├── expander.rs                   # Handlebars prompt + verb template expansion
├── llm_client.rs                 # ResearchLlmClient trait + web_search integration ← NEW
└── error.rs                      # ResearchMacroError
```

### 3. Session Context Extension
```
rust/src/session/
├── mod.rs                        # Add ResearchContext export
├── research_context.rs           # NEW - Research state machine
└── macro_rag_metadata.rs         # RAG hints for agent discovery
```

### 4. MCP Tool Registration
```
rust/src/mcp/
├── tools.rs                      # Add 4 new tools (list, get, execute, approve)
├── handlers.rs                   # Add handler methods
└── types.rs                      # Add ResearchMacroResult type
```

---

## Implementation Steps

### Step 1: Define Core Types

**File: `rust/src/research/definition.rs`**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Research macro definition loaded from YAML
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResearchMacroDef {
    pub name: String,
    pub version: String,
    pub description: String,
    pub parameters: Vec<MacroParamDef>,
    pub tools: Vec<String>,           // e.g., ["web_search"]
    pub prompt: String,               // Handlebars template
    pub output: ResearchOutput,
    pub suggested_verbs: Option<String>, // Handlebars template for DSL
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MacroParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,           // string, integer, boolean
    #[serde(default)]
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<Value>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResearchOutput {
    pub schema_name: String,
    pub schema: Value,                // JSON Schema
    pub review: ReviewRequirement,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewRequirement {
    Required,
    Optional,
    None,
}

impl Default for ReviewRequirement {
    fn default() -> Self {
        Self::Required
    }
}
```

### Step 2: Research Result Types

**File: `rust/src/research/executor.rs`**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Result of executing a research macro
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    /// Unique ID for this research result
    pub result_id: Uuid,
    
    /// Macro that was executed
    pub macro_name: String,
    
    /// Parameters that were passed
    pub params: Value,
    
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
    
    /// Search quality self-assessment from LLM
    pub search_quality: Option<SearchQuality>,
    
    /// Sources used during research
    pub sources: Vec<ResearchSource>,
    
    /// Timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SearchQuality {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

/// Approved research result ready for verb generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovedResearch {
    pub result_id: Uuid,
    pub approved_at: chrono::DateTime<chrono::Utc>,
    pub approved_data: Value,         // May have been edited
    pub generated_verbs: String,
    pub edits_made: bool,
}
```

### Step 3: Session Research Context ← **NEW**

**File: `rust/src/session/research_context.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::research::{ApprovedResearch, ResearchResult};

/// Research state within a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResearchContext {
    /// Current pending result awaiting review
    pub pending: Option<ResearchResult>,
    
    /// History of approved research results (keyed by result_id)
    pub approved: HashMap<Uuid, ApprovedResearch>,
    
    /// Generated verbs from most recent approval (ready for execution)
    pub generated_verbs: Option<String>,
    
    /// Current state in research workflow
    pub state: ResearchState,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResearchState {
    #[default]
    Idle,
    /// Research executed, awaiting human review
    PendingReview,
    /// Research approved, verbs generated
    VerbsReady,
    /// Verbs executed
    Executed,
}

impl ResearchContext {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set pending research result
    pub fn set_pending(&mut self, result: ResearchResult) {
        self.pending = Some(result);
        self.state = ResearchState::PendingReview;
        self.generated_verbs = None;
    }
    
    /// Approve pending result with optional edits
    pub fn approve(&mut self, edits: Option<serde_json::Value>) -> Result<&ApprovedResearch, &'static str> {
        let result = self.pending.take().ok_or("No pending research to approve")?;
        
        let approved_data = edits.unwrap_or(result.data.clone());
        let generated_verbs = result.suggested_verbs.clone().unwrap_or_default();
        
        let approved = ApprovedResearch {
            result_id: result.result_id,
            approved_at: chrono::Utc::now(),
            approved_data,
            generated_verbs: generated_verbs.clone(),
            edits_made: edits.is_some(),
        };
        
        self.generated_verbs = Some(generated_verbs);
        self.state = ResearchState::VerbsReady;
        self.approved.insert(result.result_id, approved);
        
        Ok(self.approved.get(&result.result_id).unwrap())
    }
    
    /// Reject pending result
    pub fn reject(&mut self) {
        self.pending = None;
        self.state = ResearchState::Idle;
    }
    
    /// Mark verbs as executed
    pub fn mark_executed(&mut self) {
        self.generated_verbs = None;
        self.state = ResearchState::Executed;
    }
    
    /// Clear and return to idle
    pub fn clear(&mut self) {
        self.pending = None;
        self.generated_verbs = None;
        self.state = ResearchState::Idle;
    }
}
```

### Step 4: Update UnifiedSessionContext

**File: `rust/src/session/mod.rs`** - Add to struct:

```rust
pub mod research_context;
pub use research_context::{ResearchContext, ResearchState};

/// Unified session context - handles REPL + Visualization + Navigation + Research
#[derive(Debug, Serialize, Deserialize)]
pub struct UnifiedSessionContext {
    // ... existing fields ...
    
    /// Research macro state ← NEW
    pub research: ResearchContext,
}

impl Default for UnifiedSessionContext {
    fn default() -> Self {
        Self {
            // ... existing ...
            research: ResearchContext::new(),
        }
    }
}
```

### Step 5: LLM Client Trait with Tool Use ← **NEW**

**File: `rust/src/research/llm_client.rs`**

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use super::ResearchSource;

/// Tool definition for LLM
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Result of LLM completion with potential tool use
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub sources: Vec<ResearchSource>,
}

/// Trait for LLM client used by research executor
#[async_trait]
pub trait ResearchLlmClient: Send + Sync {
    /// Complete with tools enabled (e.g., web_search)
    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools: &[ToolDef],
    ) -> Result<LlmResponse>;
    
    /// Complete expecting JSON response
    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Value,
    ) -> Result<Value>;
}

/// Default implementation using Claude API
pub struct ClaudeResearchClient {
    api_key: String,
    model: String,
}

impl ClaudeResearchClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }
    
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
        Ok(Self::new(api_key))
    }
}

#[async_trait]
impl ResearchLlmClient for ClaudeResearchClient {
    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools: &[ToolDef],
    ) -> Result<LlmResponse> {
        // Implementation would:
        // 1. Build Claude API request with tools (web_search)
        // 2. Handle tool_use responses iteratively
        // 3. Collect sources from web_search results
        // 4. Return final content + sources
        todo!("Implement Claude API call with tool use")
    }
    
    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Value,
    ) -> Result<Value> {
        // Implementation would:
        // 1. Add JSON mode instructions to prompt
        // 2. Call Claude API
        // 3. Parse response as JSON
        // 4. Attempt repair if malformed
        todo!("Implement Claude API call for JSON")
    }
}
```

### Step 6: Research Executor with Error Recovery ← **ENHANCED**

**File: `rust/src/research/executor.rs`** - Add to impl:

```rust
use anyhow::{anyhow, Result};
use handlebars::Handlebars;
use jsonschema::JSONSchema;
use std::collections::HashMap;
use serde_json::Value;
use uuid::Uuid;

use super::{
    ResearchMacroDef, ResearchMacroRegistry, ResearchResult, 
    ResearchLlmClient, SearchQuality, ResearchSource, ToolDef,
};

pub struct ResearchExecutor<C: ResearchLlmClient> {
    registry: ResearchMacroRegistry,
    llm_client: C,
    handlebars: Handlebars<'static>,
}

impl<C: ResearchLlmClient> ResearchExecutor<C> {
    pub fn new(registry: ResearchMacroRegistry, llm_client: C) -> Self {
        let mut handlebars = Handlebars::new();
        // Register helpers
        handlebars.register_helper("slugify", Box::new(slugify_helper));
        handlebars.register_helper("eq", Box::new(eq_helper));
        handlebars.register_helper("json", Box::new(json_helper));
        
        Self { registry, llm_client, handlebars }
    }
    
    /// Execute a research macro
    pub async fn execute(
        &self,
        macro_name: &str,
        params: HashMap<String, Value>,
    ) -> Result<ResearchResult> {
        // 1. Get macro definition
        let macro_def = self.registry
            .get(macro_name)
            .ok_or_else(|| anyhow!("Unknown research macro: {}", macro_name))?;
        
        // 2. Validate parameters
        let validated_params = self.validate_and_fill_params(&macro_def, params)?;
        
        // 3. Render prompt
        let prompt = self.render_prompt(&macro_def.prompt, &validated_params)?;
        
        // 4. Build tools list
        let tools = self.build_tools(&macro_def.tools);
        
        // 5. Execute LLM call with tools
        let system_prompt = self.build_system_prompt(&macro_def);
        let response = self.llm_client
            .complete_with_tools(&system_prompt, &prompt, &tools)
            .await?;
        
        // 6. Parse JSON with repair attempt
        let data = self.parse_json_with_repair(&response.content)?;
        
        // 7. Validate against schema
        let (schema_valid, validation_errors) = self.validate_schema(
            &macro_def.output.schema,
            &data,
        );
        
        // 8. Optionally validate LEIs exist ← NEW
        if schema_valid {
            self.validate_leis(&data).await?;
        }
        
        // 9. Render suggested verbs
        let suggested_verbs = macro_def.suggested_verbs.as_ref()
            .map(|template| self.render_template(template, &data))
            .transpose()?;
        
        // 10. Extract search quality
        let search_quality = data.get("search_quality")
            .and_then(|v| v.as_str())
            .and_then(|s| match s.to_uppercase().as_str() {
                "HIGH" => Some(SearchQuality::High),
                "MEDIUM" => Some(SearchQuality::Medium),
                "LOW" => Some(SearchQuality::Low),
                _ => None,
            });
        
        Ok(ResearchResult {
            result_id: Uuid::new_v4(),
            macro_name: macro_name.to_string(),
            params: serde_json::to_value(&validated_params)?,
            data,
            schema_valid,
            validation_errors,
            review_required: macro_def.output.review == super::ReviewRequirement::Required,
            suggested_verbs,
            search_quality,
            sources: response.sources,
            created_at: chrono::Utc::now(),
        })
    }
    
    /// Parse JSON with repair attempt for common LLM issues
    fn parse_json_with_repair(&self, content: &str) -> Result<Value> {
        // Try direct parse first
        if let Ok(v) = serde_json::from_str(content) {
            return Ok(v);
        }
        
        // Try extracting JSON from markdown code block
        let json_block_re = regex::Regex::new(r"```(?:json)?\s*([\s\S]*?)\s*```")?;
        if let Some(caps) = json_block_re.captures(content) {
            if let Ok(v) = serde_json::from_str(&caps[1]) {
                return Ok(v);
            }
        }
        
        // Try finding JSON object in content
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                let json_str = &content[start..=end];
                if let Ok(v) = serde_json::from_str(json_str) {
                    return Ok(v);
                }
            }
        }
        
        Err(anyhow!("Failed to parse JSON from LLM response: {}", 
            &content[..content.len().min(200)]))
    }
    
    /// Validate LEIs exist in GLEIF (optional step)
    async fn validate_leis(&self, data: &Value) -> Result<()> {
        // Extract all LEI-like strings from data
        let leis = self.extract_leis(data);
        
        for lei in leis {
            // Quick HEAD request to GLEIF to verify LEI exists
            let url = format!("https://api.gleif.org/api/v1/lei-records/{}", lei);
            let client = reqwest::Client::new();
            let resp = client.head(&url).send().await;
            
            if let Ok(r) = resp {
                if r.status() == 404 {
                    tracing::warn!("LEI {} not found in GLEIF - may be hallucinated", lei);
                }
            }
        }
        
        Ok(())
    }
    
    fn extract_leis(&self, data: &Value) -> Vec<String> {
        let mut leis = Vec::new();
        let lei_re = regex::Regex::new(r"\b[A-Z0-9]{20}\b").unwrap();
        
        fn walk(v: &Value, re: &regex::Regex, out: &mut Vec<String>) {
            match v {
                Value::String(s) => {
                    for cap in re.find_iter(s) {
                        out.push(cap.as_str().to_string());
                    }
                }
                Value::Array(arr) => arr.iter().for_each(|x| walk(x, re, out)),
                Value::Object(obj) => obj.values().for_each(|x| walk(x, re, out)),
                _ => {}
            }
        }
        
        walk(data, &lei_re, &mut leis);
        leis
    }
    
    fn validate_schema(&self, schema: &Value, data: &Value) -> (bool, Vec<String>) {
        match JSONSchema::compile(schema) {
            Ok(compiled) => {
                match compiled.validate(data) {
                    Ok(_) => (true, vec![]),
                    Err(errors) => {
                        let msgs: Vec<String> = errors
                            .map(|e| format!("{}: {}", e.instance_path, e))
                            .collect();
                        (false, msgs)
                    }
                }
            }
            Err(e) => (false, vec![format!("Invalid schema: {}", e)]),
        }
    }
    
    fn build_system_prompt(&self, macro_def: &ResearchMacroDef) -> String {
        format!(
            "You are a research assistant for institutional client onboarding.\n\
             Your task: {}\n\n\
             IMPORTANT: Return ONLY valid JSON matching the required schema.\n\
             Do NOT include markdown formatting or explanatory text.\n\
             Use web_search to find current, accurate information.",
            macro_def.description
        )
    }
    
    fn build_tools(&self, tool_names: &[String]) -> Vec<ToolDef> {
        tool_names.iter().filter_map(|name| {
            match name.as_str() {
                "web_search" => Some(ToolDef {
                    name: "web_search".to_string(),
                    description: "Search the web for current information".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string" }
                        },
                        "required": ["query"]
                    }),
                }),
                _ => None,
            }
        }).collect()
    }
    
    fn validate_and_fill_params(
        &self,
        macro_def: &ResearchMacroDef,
        mut params: HashMap<String, Value>,
    ) -> Result<HashMap<String, Value>> {
        for param_def in &macro_def.parameters {
            if !params.contains_key(&param_def.name) {
                if param_def.required {
                    if let Some(default) = &param_def.default {
                        params.insert(param_def.name.clone(), default.clone());
                    } else {
                        return Err(anyhow!("Missing required parameter: {}", param_def.name));
                    }
                } else if let Some(default) = &param_def.default {
                    params.insert(param_def.name.clone(), default.clone());
                }
            }
            
            // Validate enum values
            if let (Some(enum_values), Some(value)) = (&param_def.enum_values, params.get(&param_def.name)) {
                if let Some(s) = value.as_str() {
                    if !enum_values.contains(&s.to_string()) {
                        return Err(anyhow!(
                            "Invalid value '{}' for parameter '{}'. Must be one of: {:?}",
                            s, param_def.name, enum_values
                        ));
                    }
                }
            }
        }
        Ok(params)
    }
    
    fn render_prompt(&self, template: &str, params: &HashMap<String, Value>) -> Result<String> {
        self.handlebars
            .render_template(template, params)
            .map_err(|e| anyhow!("Prompt template error: {}", e))
    }
    
    fn render_template(&self, template: &str, data: &Value) -> Result<String> {
        self.handlebars
            .render_template(template, data)
            .map_err(|e| anyhow!("Template error: {}", e))
    }
}

// Handlebars helpers
fn slugify_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let slug = param
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();
    out.write(&slug)?;
    Ok(())
}

fn eq_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let a = h.param(0).and_then(|v| v.value().as_str());
    let b = h.param(1).and_then(|v| v.value().as_str());
    if a == b {
        out.write("true")?;
    }
    Ok(())
}

fn json_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    if let Some(v) = h.param(0) {
        out.write(&serde_json::to_string(v.value()).unwrap_or_default())?;
    }
    Ok(())
}
```

### Step 7: MCP Tools ← **ENHANCED**

**File: `rust/src/mcp/tools.rs`** - Add to `get_tools()`:

```rust
// =====================================================================
// Research Macro Tools
// Fuzzy discovery → human review → deterministic DSL
// =====================================================================

Tool {
    name: "research_list".into(),
    description: "List available research macros for client discovery, UBO investigation, etc.".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "search": {
                "type": "string",
                "description": "Filter by name or description"
            }
        }
    }),
},

Tool {
    name: "research_get".into(),
    description: "Get full research macro definition with parameters, schema, and example usage.".into(),
    input_schema: json!({
        "type": "object",
        "required": ["macro_name"],
        "properties": {
            "macro_name": {
                "type": "string",
                "description": "Research macro name (e.g., 'client-discovery')"
            }
        }
    }),
},

Tool {
    name: "research_execute".into(),
    description: r#"Execute a research macro using LLM + web search.

Returns structured JSON requiring human review before use.
Use this when user asks to "research", "discover", "find structure of" a client.

Example: User says "research Allianz for onboarding"
→ Call research_execute with macro_name="client-discovery", params={client_name: "Allianz"}

After execution, result is stored in session as PENDING.
User must approve/edit/reject before DSL verbs are generated."#.into(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id", "macro_name", "params"],
        "properties": {
            "session_id": {
                "type": "string",
                "format": "uuid",
                "description": "Session ID to store results"
            },
            "macro_name": {
                "type": "string",
                "description": "Research macro name"
            },
            "params": {
                "type": "object",
                "description": "Parameters for the macro"
            }
        }
    }),
},

Tool {
    name: "research_approve".into(),
    description: r#"Approve pending research result after human review.

Call this after user confirms the research findings are correct.
Generates DSL verbs from the approved data.

Options:
- No edits: Approve as-is
- With edits: Pass corrected data
- Reject: Use research_reject instead"#.into(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id"],
        "properties": {
            "session_id": {
                "type": "string",
                "format": "uuid"
            },
            "edits": {
                "type": "object",
                "description": "Optional: corrected research data"
            },
            "execute_verbs": {
                "type": "boolean",
                "default": false,
                "description": "If true, immediately execute generated DSL verbs"
            }
        }
    }),
},

Tool {
    name: "research_reject".into(),
    description: "Reject pending research result. Clears session and returns to idle state.".into(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id"],
        "properties": {
            "session_id": {
                "type": "string",
                "format": "uuid"
            },
            "reason": {
                "type": "string",
                "description": "Why the research was rejected"
            }
        }
    }),
},

Tool {
    name: "research_status".into(),
    description: "Get current research state for a session (pending, approved, verbs ready, etc.)".into(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id"],
        "properties": {
            "session_id": {
                "type": "string",
                "format": "uuid"
            }
        }
    }),
},
```

### Step 8: MCP Handlers ← **NEW**

**File: `rust/src/mcp/handlers.rs`** - Add methods:

```rust
impl ToolHandlers {
    async fn handle_research_list(&self, args: Value) -> Result<Value> {
        let search = args.get("search").and_then(|v| v.as_str());
        let macros = self.research_registry.list(search);
        
        Ok(json!({
            "macros": macros.iter().map(|m| json!({
                "name": m.name,
                "description": m.description,
                "parameters": m.parameters.iter().map(|p| json!({
                    "name": p.name,
                    "type": p.param_type,
                    "required": p.required,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()
        }))
    }
    
    async fn handle_research_get(&self, args: Value) -> Result<Value> {
        let macro_name = args["macro_name"].as_str()
            .ok_or_else(|| anyhow!("macro_name required"))?;
        
        let macro_def = self.research_registry.get(macro_name)
            .ok_or_else(|| anyhow!("Unknown macro: {}", macro_name))?;
        
        Ok(serde_json::to_value(macro_def)?)
    }
    
    async fn handle_research_execute(&self, args: Value) -> Result<Value> {
        let session_id: Uuid = args["session_id"].as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| anyhow!("Valid session_id required"))?;
        
        let macro_name = args["macro_name"].as_str()
            .ok_or_else(|| anyhow!("macro_name required"))?;
        
        let params: HashMap<String, Value> = args.get("params")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        
        // Execute research
        let result = self.research_executor.execute(macro_name, params).await?;
        
        // Store in session
        let mut session = self.session_store.get_mut(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;
        session.research.set_pending(result.clone());
        
        Ok(json!({
            "result_id": result.result_id,
            "data": result.data,
            "schema_valid": result.schema_valid,
            "validation_errors": result.validation_errors,
            "review_required": result.review_required,
            "suggested_verbs": result.suggested_verbs,
            "search_quality": result.search_quality,
            "sources": result.sources,
            "state": "pending_review"
        }))
    }
    
    async fn handle_research_approve(&self, args: Value) -> Result<Value> {
        let session_id: Uuid = args["session_id"].as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| anyhow!("Valid session_id required"))?;
        
        let edits = args.get("edits").cloned();
        let execute_verbs = args.get("execute_verbs")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let mut session = self.session_store.get_mut(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;
        
        let approved = session.research.approve(edits)?;
        let verbs = approved.generated_verbs.clone();
        let result_id = approved.result_id;
        
        // Optionally execute
        let execution_result = if execute_verbs && !verbs.is_empty() {
            Some(self.execute_dsl(&verbs, &mut session).await?)
        } else {
            None
        };
        
        Ok(json!({
            "result_id": result_id,
            "approved": true,
            "generated_verbs": verbs,
            "state": if execute_verbs { "executed" } else { "verbs_ready" },
            "execution": execution_result
        }))
    }
    
    async fn handle_research_reject(&self, args: Value) -> Result<Value> {
        let session_id: Uuid = args["session_id"].as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| anyhow!("Valid session_id required"))?;
        
        let reason = args.get("reason").and_then(|v| v.as_str());
        
        let mut session = self.session_store.get_mut(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;
        
        session.research.reject();
        
        Ok(json!({
            "rejected": true,
            "reason": reason,
            "state": "idle"
        }))
    }
    
    async fn handle_research_status(&self, args: Value) -> Result<Value> {
        let session_id: Uuid = args["session_id"].as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| anyhow!("Valid session_id required"))?;
        
        let session = self.session_store.get(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;
        
        Ok(json!({
            "state": session.research.state,
            "has_pending": session.research.pending.is_some(),
            "pending_macro": session.research.pending.as_ref().map(|r| &r.macro_name),
            "approved_count": session.research.approved.len(),
            "verbs_ready": session.research.generated_verbs.is_some(),
        }))
    }
}
```

### Step 9: Integration with Batch Tools ← **NEW**

The research results should feed into bulk onboarding. Add connection:

**File: `rust/src/mcp/handlers.rs`** - Enhance `handle_batch_start`:

```rust
async fn handle_batch_start(&self, args: Value) -> Result<Value> {
    let session_id = // ...
    let template_id = // ...
    
    // NEW: Check if we should seed from research results
    let from_research = args.get("from_research")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    if from_research {
        let session = self.session_store.get(&session_id)?;
        
        // Get most recent approved research
        if let Some((_, approved)) = session.research.approved.iter().last() {
            // Extract entities from research data for batch seeding
            let entities = self.extract_entities_from_research(&approved.approved_data)?;
            
            // Pre-populate batch key sets
            // This feeds discovered funds into the onboard-fund-cbu template
            // ...
        }
    }
    
    // ... rest of batch_start logic
}
```

---

## Testing Plan ← **ENHANCED**

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_research_macro_yaml() {
        let yaml = r#"
macro:
  name: test-macro
  version: "1.0"
  description: Test macro
  parameters:
    - name: client_name
      type: string
      required: true
  tools:
    - web_search
  prompt: "Research {{client_name}}"
  output:
    schema_name: test-result
    schema:
      type: object
      properties:
        name: { type: string }
    review: required
"#;
        let def: ResearchMacroWrapper = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.macro_def.name, "test-macro");
        assert!(def.macro_def.parameters[0].required);
    }
    
    #[test]
    fn test_json_repair_extracts_from_markdown() {
        let executor = ResearchExecutor::new(/* ... */);
        
        let content = r#"Here's the result:
```json
{"apex": {"name": "Test Corp"}}
```
"#;
        let result = executor.parse_json_with_repair(content).unwrap();
        assert_eq!(result["apex"]["name"], "Test Corp");
    }
    
    #[test]
    fn test_session_research_state_machine() {
        let mut ctx = ResearchContext::new();
        assert_eq!(ctx.state, ResearchState::Idle);
        
        let result = ResearchResult {
            result_id: Uuid::new_v4(),
            // ...
        };
        ctx.set_pending(result);
        assert_eq!(ctx.state, ResearchState::PendingReview);
        
        ctx.approve(None).unwrap();
        assert_eq!(ctx.state, ResearchState::VerbsReady);
        
        ctx.mark_executed();
        assert_eq!(ctx.state, ResearchState::Executed);
    }
    
    #[test]
    fn test_lei_extraction() {
        let executor = ResearchExecutor::new(/* ... */);
        let data = json!({
            "apex": {
                "lei": "529900K9B0N5BT694847"
            },
            "subsidiaries": [
                { "lei": "OJ2TIQSVQND4IZYYK658" }
            ]
        });
        
        let leis = executor.extract_leis(&data);
        assert_eq!(leis.len(), 2);
        assert!(leis.contains(&"529900K9B0N5BT694847".to_string()));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_research_to_gleif_pipeline() {
    // 1. Create session
    let session = create_test_session();
    
    // 2. Execute research macro with mock LLM
    let mock_llm = MockResearchLlmClient::new()
        .with_response(json!({
            "apex": {
                "name": "Test Client SE",
                "jurisdiction": "DE",
                "lei": "529900K9B0N5BT694847"
            },
            "search_quality": "HIGH"
        }));
    
    let executor = ResearchExecutor::new(registry, mock_llm);
    let result = executor.execute("client-discovery", params).await.unwrap();
    
    assert!(result.schema_valid);
    assert!(result.suggested_verbs.is_some());
    
    // 3. Approve
    session.research.set_pending(result);
    let approved = session.research.approve(None).unwrap();
    
    // 4. Verify generated verbs
    assert!(approved.generated_verbs.contains("gleif.enrich"));
    assert!(approved.generated_verbs.contains("529900K9B0N5BT694847"));
}
```

---

## Files Modified (Complete Summary)

| File | Action |
|------|--------|
| `rust/Cargo.toml` | Add `handlebars`, `jsonschema` deps |
| `rust/src/lib.rs` | Add `pub mod research;` |
| `rust/src/research/mod.rs` | **CREATE** - Module exports |
| `rust/src/research/definition.rs` | **CREATE** - Core types |
| `rust/src/research/registry.rs` | **CREATE** - YAML loading |
| `rust/src/research/executor.rs` | **CREATE** - LLM execution |
| `rust/src/research/llm_client.rs` | **CREATE** - LLM trait |
| `rust/src/research/error.rs` | **CREATE** - Error types |
| `rust/config/macros/research/*.yaml` | **CREATE** - Macro definitions |
| `rust/src/session/mod.rs` | Add ResearchContext export |
| `rust/src/session/research_context.rs` | **CREATE** - Session state |
| `rust/src/mcp/tools.rs` | Add 6 research tools |
| `rust/src/mcp/handlers.rs` | Add 6 handler methods |
| `rust/src/session/macro_rag_metadata.rs` | **CREATE** - RAG hints |

---

## Estimated Scope (Revised)

| Component | Complexity | Est. Lines |
|-----------|------------|------------|
| Research module (executor, registry, types) | Medium | ~600 |
| LLM client trait + impl | Medium | ~200 |
| Session research context | Low | ~150 |
| YAML macro definitions | Low | ~300 |
| MCP tools + handlers | Medium | ~300 |
| RAG metadata | Low | ~100 |
| Tests | Medium | ~300 |
| **Total** | | **~1950** |

---

## Migration from Original Plan

The original plan was good but needed these additions:

1. ✅ Session state persistence (ResearchContext)
2. ✅ Approval/reject tools (not just execute)
3. ✅ LLM client trait for web_search integration
4. ✅ JSON repair for malformed LLM responses
5. ✅ LEI validation against GLEIF
6. ✅ Batch tools integration pathway
7. ✅ Consistent tool naming (research_* not research_macro_*)
8. ✅ Source tracking from web_search results
