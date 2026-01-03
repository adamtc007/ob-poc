# TODO: Complete Research Macros Implementation

**Created:** 2026-01-02
**Priority:** HIGH
**Assignee:** Claude Code
**Depends On:** Research module foundation (COMPLETE)

---

## Summary

The research macro module (`rust/src/research/`) is implemented but needs:
1. YAML config files for actual research macros
2. MCP tool registration in `tools.rs`
3. Session `ResearchContext` type (if missing)
4. Split monolithic `handlers/core.rs` into domain modules
5. Real web search integration (optional, can stub for now)

---

## Part 1: Create Research Macro YAML Configs

Create directory and files:

```
rust/config/macros/research/
├── client-discovery.yaml
├── ubo-investigation.yaml
└── regulatory-check.yaml
```

### File 1: `rust/config/macros/research/client-discovery.yaml`

```yaml
macro:
  name: client-discovery
  version: "1.0"
  description: "Research institutional client structure for KYC onboarding - identifies apex entity, subsidiaries, fund structures, and regulatory context"
  
  parameters:
    - name: client_name
      type: string
      required: true
      description: "Name of the institutional client to research"
      example: "Allianz"
    - name: jurisdiction_hint
      type: string
      required: false
      description: "ISO 2-letter country code hint (e.g., DE, GB, US)"
      example: "DE"
    - name: sector
      type: string
      required: false
      default: "asset-management"
      enum: ["asset-management", "insurance", "banking", "pension", "sovereign-wealth", "corporate"]
      description: "Business sector for targeted research"

  tools:
    - web_search

  prompt: |
    Research {{client_name}} for institutional client onboarding in the {{sector}} sector.
    {{#if jurisdiction_hint}}The client is likely headquartered in {{jurisdiction_hint}}.{{/if}}

    Identify and return structured data for:

    1. **Apex Entity** (Head Office / Ultimate Parent)
       - Exact legal registered name
       - Jurisdiction (ISO 2-letter code)
       - LEI (20-character Legal Entity Identifier) - search GLEIF if needed
       - Stock exchange listing (if public)
       - UBO status: PUBLIC_FLOAT (if listed), STATE_OWNED, FOUNDATION, or PRIVATE

    2. **Key Subsidiaries** (focus on {{sector}})
       - Investment Managers (IM)
       - Management Companies (ManCo)
       - Fund administrators
       - Include: name, jurisdiction, LEI where findable, role

    3. **Fund Structures**
       - SICAV/UCITS umbrellas (Luxembourg, Ireland, etc.)
       - Notable fund ranges
       - Approximate fund count if known

    4. **Regulatory Context**
       - Primary regulator(s)
       - Recent M&A affecting structure (last 2 years)
       - Any notable compliance events

    Search GLEIF (gleif.org), company websites, regulatory registries, and financial news.
    
    Return ONLY valid JSON matching the schema. No markdown, no explanation.
    Include search_quality assessment: HIGH (multiple sources confirm), MEDIUM (some gaps), LOW (limited data).

  output:
    schema_name: client-discovery-result
    schema:
      type: object
      required:
        - apex
        - search_quality
      properties:
        apex:
          type: object
          required:
            - name
            - jurisdiction
          properties:
            name:
              type: string
              description: "Exact legal registered name"
            jurisdiction:
              type: string
              pattern: "^[A-Z]{2}$"
              description: "ISO 2-letter country code"
            lei:
              type: string
              pattern: "^[A-Z0-9]{20}$"
              description: "20-character LEI"
            listing:
              type: string
              description: "Stock exchange ticker (e.g., FRA:ALV, NYSE:BLK)"
            ubo_status:
              type: string
              enum:
                - PUBLIC_FLOAT
                - STATE_OWNED
                - FOUNDATION
                - PRIVATE
                - UNKNOWN
        subsidiaries:
          type: array
          items:
            type: object
            required:
              - name
              - jurisdiction
            properties:
              name:
                type: string
              jurisdiction:
                type: string
                pattern: "^[A-Z]{2}$"
              lei:
                type: string
                pattern: "^[A-Z0-9]{20}$"
              role:
                type: string
                enum:
                  - ManCo
                  - IM
                  - Depositary
                  - Administrator
                  - Distributor
                  - Other
              parent_lei:
                type: string
                description: "LEI of parent if known"
        fund_structures:
          type: array
          items:
            type: object
            properties:
              name:
                type: string
              structure_type:
                type: string
                enum:
                  - SICAV
                  - UCITS
                  - AIF
                  - ETF
                  - Unit_Trust
                  - Other
              jurisdiction:
                type: string
                pattern: "^[A-Z]{2}$"
              umbrella_lei:
                type: string
              fund_count:
                type: integer
        regulatory:
          type: object
          properties:
            primary_regulator:
              type: string
            secondary_regulators:
              type: array
              items:
                type: string
            recent_events:
              type: array
              items:
                type: string
        search_quality:
          type: string
          enum:
            - HIGH
            - MEDIUM
            - LOW
    review: required

  suggested_verbs: |
    ;; === Client Discovery Results for {{apex.name}} ===
    ;; Search quality: {{search_quality}}
    
    ;; Step 1: Import apex entity from GLEIF
    {{#if apex.lei}}
    (gleif.enrich :lei "{{apex.lei}}" :as @apex)
    {{else}}
    ;; WARNING: No LEI found for apex - manual search required
    ;; (gleif.search :name "{{apex.name}}" :jurisdiction "{{apex.jurisdiction}}")
    {{/if}}
    
    ;; Step 2: Import subsidiaries
    {{#each subsidiaries}}
    {{#if this.lei}}
    (gleif.enrich :lei "{{this.lei}}" :as @{{slugify this.name}})
    {{/if}}
    {{/each}}
    
    ;; Step 3: Trace ownership from apex
    {{#if apex.lei}}
    (gleif.trace-ownership :entity-id @apex :direction "down" :max-depth 3)
    {{/if}}
    
    ;; Step 4: Import managed funds for each ManCo
    {{#each subsidiaries}}
    {{#if (eq this.role "ManCo")}}
    {{#if this.lei}}
    (gleif.get-managed-funds :manager-lei "{{this.lei}}" :limit 50)
    {{/if}}
    {{/if}}
    {{/each}}

  tags:
    - client
    - discovery
    - onboarding
    - gleif
    - institutional
```

### File 2: `rust/config/macros/research/ubo-investigation.yaml`

```yaml
macro:
  name: ubo-investigation
  version: "1.0"
  description: "Investigate ultimate beneficial ownership for complex entity structures - determines terminus type and identifies natural person UBOs where applicable"
  
  parameters:
    - name: entity_name
      type: string
      required: true
      description: "Name of entity to investigate"
    - name: entity_lei
      type: string
      required: false
      description: "LEI if known (speeds up research)"
    - name: jurisdiction
      type: string
      required: false
      description: "ISO 2-letter jurisdiction code"
    - name: known_parent
      type: string
      required: false
      description: "Name of known parent entity if any"

  tools:
    - web_search

  prompt: |
    Investigate the ultimate beneficial ownership of {{entity_name}}.
    {{#if entity_lei}}Known LEI: {{entity_lei}}{{/if}}
    {{#if jurisdiction}}Jurisdiction: {{jurisdiction}}{{/if}}
    {{#if known_parent}}Known parent: {{known_parent}}{{/if}}

    Determine the UBO terminus:

    1. **PUBLIC_FLOAT** - Entity is publicly traded or owned by publicly traded company
       - Identify the listed entity in the chain
       - Note stock exchange and ticker
    
    2. **STATE_OWNED** - Owned by government or sovereign wealth fund
       - Identify the sovereign entity
       - Note the country
    
    3. **FOUNDATION** - Owned by foundation, trust, or similar structure
       - Identify the foundation
       - Note jurisdiction and purpose if known
    
    4. **NATURAL_PERSONS** - Identify actual human UBOs
       - Names (if publicly available)
       - Nationality
       - Ownership percentage (if disclosed)
       - Source of information
    
    5. **UNKNOWN** - Cannot determine with confidence

    Search corporate registries, GLEIF relationship data, regulatory filings, 
    news sources, and beneficial ownership registers (UK PSC, EU BORIS, etc.)

    Return ONLY valid JSON. Include confidence level and sources.

  output:
    schema_name: ubo-investigation-result
    schema:
      type: object
      required:
        - entity_name
        - terminus_type
        - confidence
      properties:
        entity_name:
          type: string
        entity_lei:
          type: string
        terminus_type:
          type: string
          enum:
            - PUBLIC_FLOAT
            - STATE_OWNED
            - FOUNDATION
            - NATURAL_PERSONS
            - UNKNOWN
        public_float_details:
          type: object
          properties:
            listed_entity_name:
              type: string
            listed_entity_lei:
              type: string
            exchange:
              type: string
            ticker:
              type: string
        state_owned_details:
          type: object
          properties:
            sovereign_name:
              type: string
            country:
              type: string
            ownership_percentage:
              type: string
        foundation_details:
          type: object
          properties:
            foundation_name:
              type: string
            jurisdiction:
              type: string
            purpose:
              type: string
        natural_persons:
          type: array
          items:
            type: object
            properties:
              name:
                type: string
              nationality:
                type: string
              ownership_percentage:
                type: string
              source:
                type: string
              verification_status:
                type: string
                enum:
                  - VERIFIED
                  - UNVERIFIED
                  - REQUIRES_DOCUMENTATION
        ownership_chain:
          type: array
          description: "Chain from target entity to terminus"
          items:
            type: object
            properties:
              entity_name:
                type: string
              entity_lei:
                type: string
              jurisdiction:
                type: string
              ownership_type:
                type: string
              ownership_percentage:
                type: string
        confidence:
          type: string
          enum:
            - HIGH
            - MEDIUM
            - LOW
        confidence_notes:
          type: string
        sources:
          type: array
          items:
            type: string
        concerns:
          type: array
          description: "Any red flags or concerns identified"
          items:
            type: string
    review: required

  suggested_verbs: |
    ;; === UBO Investigation Results for {{entity_name}} ===
    ;; Terminus type: {{terminus_type}}
    ;; Confidence: {{confidence}}
    
    {{#if (eq terminus_type "PUBLIC_FLOAT")}}
    ;; Public float terminus - verify listing
    {{#if public_float_details.listed_entity_lei}}
    (gleif.enrich :lei "{{public_float_details.listed_entity_lei}}" :as @listed_parent)
    (entity.set-ubo-terminus :entity-id @target :terminus-type "PUBLIC_FLOAT" :terminus-entity-id @listed_parent)
    {{/if}}
    {{/if}}
    
    {{#if (eq terminus_type "STATE_OWNED")}}
    ;; State-owned terminus
    (entity.set-ubo-terminus :entity-id @target :terminus-type "STATE_OWNED" :sovereign "{{state_owned_details.country}}")
    {{/if}}
    
    {{#if (eq terminus_type "NATURAL_PERSONS")}}
    ;; Natural person UBOs identified - create UBO records
    {{#each natural_persons}}
    (ubo.create :entity-id @target :name "{{this.name}}" :nationality "{{this.nationality}}" :ownership "{{this.ownership_percentage}}" :status "{{this.verification_status}}")
    {{/each}}
    {{/if}}

  tags:
    - ubo
    - beneficial-ownership
    - investigation
    - compliance
    - kyc
```

### File 3: `rust/config/macros/research/regulatory-check.yaml`

```yaml
macro:
  name: regulatory-check
  version: "1.0"
  description: "Research regulatory status, licenses, and compliance history for an entity"
  
  parameters:
    - name: entity_name
      type: string
      required: true
      description: "Name of entity to check"
    - name: entity_lei
      type: string
      required: false
      description: "LEI if known"
    - name: jurisdictions
      type: array
      required: false
      description: "Jurisdictions to check (ISO codes)"
      default: []
    - name: check_types
      type: array
      required: false
      description: "Types of checks to perform"
      default: ["sanctions", "licenses", "enforcement", "adverse_news"]

  tools:
    - web_search

  prompt: |
    Perform regulatory due diligence on {{entity_name}}.
    {{#if entity_lei}}LEI: {{entity_lei}}{{/if}}
    {{#if jurisdictions}}Focus jurisdictions: {{#each jurisdictions}}{{this}} {{/each}}{{/if}}
    
    Check types requested: {{#each check_types}}{{this}}, {{/each}}

    Research:

    1. **Sanctions Status**
       - OFAC (US), EU sanctions, UN sanctions
       - Check entity name and any known aliases
       - Check associated individuals
    
    2. **Regulatory Licenses**
       - FCA (UK), BaFin (DE), SEC/FINRA (US), MAS (SG), etc.
       - License types and status
       - Any restrictions or conditions
    
    3. **Enforcement Actions**
       - Fines or penalties (last 5 years)
       - Consent orders
       - Regulatory investigations
    
    4. **Adverse News**
       - Fraud allegations
       - Money laundering connections
       - Material litigation
       - Executive misconduct

    Search regulatory databases, enforcement action lists, news sources.
    
    Return ONLY valid JSON. Be factual - note if information is unverified.

  output:
    schema_name: regulatory-check-result
    schema:
      type: object
      required:
        - entity_name
        - overall_risk
        - search_quality
      properties:
        entity_name:
          type: string
        entity_lei:
          type: string
        overall_risk:
          type: string
          enum:
            - LOW
            - MEDIUM
            - HIGH
            - CRITICAL
        sanctions:
          type: object
          properties:
            status:
              type: string
              enum:
                - CLEAR
                - POTENTIAL_MATCH
                - SANCTIONED
                - UNKNOWN
            matches:
              type: array
              items:
                type: object
                properties:
                  list:
                    type: string
                  match_type:
                    type: string
                  details:
                    type: string
        licenses:
          type: array
          items:
            type: object
            properties:
              regulator:
                type: string
              license_type:
                type: string
              status:
                type: string
                enum:
                  - ACTIVE
                  - SUSPENDED
                  - REVOKED
                  - PENDING
                  - UNKNOWN
              reference:
                type: string
              conditions:
                type: string
        enforcement_actions:
          type: array
          items:
            type: object
            properties:
              date:
                type: string
              regulator:
                type: string
              action_type:
                type: string
              description:
                type: string
              fine_amount:
                type: string
              resolution:
                type: string
        adverse_news:
          type: array
          items:
            type: object
            properties:
              date:
                type: string
              headline:
                type: string
              source:
                type: string
              category:
                type: string
                enum:
                  - FRAUD
                  - AML
                  - SANCTIONS_EVASION
                  - LITIGATION
                  - EXECUTIVE_MISCONDUCT
                  - OTHER
              severity:
                type: string
                enum:
                  - LOW
                  - MEDIUM
                  - HIGH
        risk_factors:
          type: array
          items:
            type: string
        mitigating_factors:
          type: array
          items:
            type: string
        search_quality:
          type: string
          enum:
            - HIGH
            - MEDIUM
            - LOW
        recommendations:
          type: array
          items:
            type: string
    review: required

  suggested_verbs: |
    ;; === Regulatory Check Results for {{entity_name}} ===
    ;; Overall risk: {{overall_risk}}
    
    ;; Record screening result
    (screening.record :entity-id @target :check-type "regulatory" :risk-level "{{overall_risk}}" :source "research-macro")
    
    {{#each enforcement_actions}}
    ;; Enforcement action: {{this.action_type}} by {{this.regulator}}
    (screening.add-finding :entity-id @target :finding-type "enforcement" :description "{{this.description}}" :date "{{this.date}}")
    {{/each}}
    
    {{#if (eq sanctions.status "POTENTIAL_MATCH")}}
    ;; ALERT: Potential sanctions match - requires manual review
    (screening.escalate :entity-id @target :reason "Potential sanctions match" :priority "HIGH")
    {{/if}}
    
    {{#if (eq sanctions.status "SANCTIONED")}}
    ;; CRITICAL: Entity appears on sanctions list
    (screening.escalate :entity-id @target :reason "Sanctions list match" :priority "CRITICAL")
    (workflow.block :entity-id @target :reason "Sanctions match - cannot proceed")
    {{/if}}

  tags:
    - regulatory
    - compliance
    - sanctions
    - screening
    - due-diligence
```

---

## Part 2: Verify/Create Session ResearchContext

Check if `rust/src/session/mod.rs` exports `ResearchContext`. If not, create it.

### File: `rust/src/session/research_context.rs`

```rust
//! Research context for session state management
//!
//! Tracks research macro execution state within a UI session.

use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::research::{ApprovedResearch, ResearchResult};

/// State machine for research workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ResearchState {
    /// No active research
    #[default]
    Idle,
    /// Research executed, awaiting human review
    PendingReview,
    /// Research approved, verbs ready for execution
    VerbsReady,
}

impl fmt::Display for ResearchState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResearchState::Idle => write!(f, "idle"),
            ResearchState::PendingReview => write!(f, "pending_review"),
            ResearchState::VerbsReady => write!(f, "verbs_ready"),
        }
    }
}

/// Research context within a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResearchContext {
    /// Current state
    pub state: ResearchState,
    
    /// Pending research result awaiting review
    pub pending: Option<ResearchResult>,
    
    /// Approved research results (keyed by result_id)
    pub approved: HashMap<Uuid, ApprovedResearch>,
    
    /// Most recently approved result ID (for quick access)
    pub last_approved_id: Option<Uuid>,
}

impl ResearchContext {
    /// Create new empty context
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set pending research result
    pub fn set_pending(&mut self, result: ResearchResult) {
        self.pending = Some(result);
        self.state = ResearchState::PendingReview;
    }
    
    /// Check if there's a pending result
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    
    /// Get the pending macro name
    pub fn pending_macro_name(&self) -> Option<&str> {
        self.pending.as_ref().map(|r| r.macro_name.as_str())
    }
    
    /// Approve pending research with optional edits
    pub fn approve(&mut self, edits: Option<Value>) -> Result<ApprovedResearch, &'static str> {
        let pending = self.pending.take().ok_or("No pending research to approve")?;
        
        // Apply edits if provided
        let approved_data = if let Some(edit_value) = edits {
            // Merge edits into the original data
            let mut data = pending.data.clone();
            if let (Some(base), Some(patches)) = (data.as_object_mut(), edit_value.as_object()) {
                for (key, value) in patches {
                    base.insert(key.clone(), value.clone());
                }
            }
            data
        } else {
            pending.data.clone()
        };
        
        // Generate verbs from approved data
        let generated_verbs = pending.suggested_verbs.clone().unwrap_or_default();
        
        let approved = ApprovedResearch {
            result_id: pending.result_id,
            approved_at: Utc::now(),
            approved_data,
            generated_verbs,
            edits_made: edits.is_some(),
        };
        
        self.last_approved_id = Some(approved.result_id);
        self.approved.insert(approved.result_id, approved.clone());
        self.state = ResearchState::VerbsReady;
        
        Ok(approved)
    }
    
    /// Reject pending research
    pub fn reject(&mut self) {
        self.pending = None;
        self.state = ResearchState::Idle;
    }
    
    /// Check if verbs are ready for execution
    pub fn has_verbs_ready(&self) -> bool {
        self.state == ResearchState::VerbsReady && self.last_approved_id.is_some()
    }
    
    /// Get the most recently approved research
    pub fn last_approved(&self) -> Option<&ApprovedResearch> {
        self.last_approved_id.and_then(|id| self.approved.get(&id))
    }
    
    /// Get count of approved researches
    pub fn approved_count(&self) -> usize {
        self.approved.len()
    }
    
    /// Clear the verbs ready state (after execution)
    pub fn clear_verbs_ready(&mut self) {
        if self.state == ResearchState::VerbsReady {
            self.state = ResearchState::Idle;
        }
    }
    
    /// Reset to idle state
    pub fn reset(&mut self) {
        self.state = ResearchState::Idle;
        self.pending = None;
        // Keep approved history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::SearchQuality;
    
    fn mock_result() -> ResearchResult {
        ResearchResult {
            result_id: Uuid::new_v4(),
            macro_name: "test-macro".to_string(),
            params: serde_json::json!({}),
            data: serde_json::json!({"test": true}),
            schema_valid: true,
            validation_errors: vec![],
            review_required: true,
            suggested_verbs: Some("(test.verb)".to_string()),
            search_quality: Some(SearchQuality::High),
            sources: vec![],
            created_at: Utc::now(),
        }
    }
    
    #[test]
    fn test_state_transitions() {
        let mut ctx = ResearchContext::new();
        assert_eq!(ctx.state, ResearchState::Idle);
        
        // Set pending
        ctx.set_pending(mock_result());
        assert_eq!(ctx.state, ResearchState::PendingReview);
        assert!(ctx.has_pending());
        
        // Approve
        let approved = ctx.approve(None).unwrap();
        assert_eq!(ctx.state, ResearchState::VerbsReady);
        assert!(!ctx.has_pending());
        assert!(ctx.has_verbs_ready());
        assert_eq!(ctx.approved_count(), 1);
        
        // Clear
        ctx.clear_verbs_ready();
        assert_eq!(ctx.state, ResearchState::Idle);
    }
    
    #[test]
    fn test_reject() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(mock_result());
        
        ctx.reject();
        assert_eq!(ctx.state, ResearchState::Idle);
        assert!(!ctx.has_pending());
    }
    
    #[test]
    fn test_approve_with_edits() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(mock_result());
        
        let edits = serde_json::json!({"edited": true});
        let approved = ctx.approve(Some(edits)).unwrap();
        
        assert!(approved.edits_made);
        assert_eq!(approved.approved_data["edited"], true);
    }
}
```

### Update `rust/src/session/mod.rs`

Add:
```rust
mod research_context;
pub use research_context::{ResearchContext, ResearchState};
```

And ensure `SessionContext` struct includes:
```rust
pub struct SessionContext {
    // ... existing fields ...
    pub research: ResearchContext,
}
```

---

## Part 3: Register MCP Tools

### Update `rust/src/mcp/tools.rs`

Add to `get_tools()` function:

```rust
// Research Macro Tools
Tool {
    name: "research_list".to_string(),
    description: "List available research macros with descriptions and parameters. \
        Research macros use LLM + web search to gather structured data that requires \
        human review before generating DSL verbs.".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "search": {
                "type": "string",
                "description": "Optional search term to filter macros"
            },
            "tag": {
                "type": "string",
                "description": "Optional tag to filter (e.g., 'client', 'ubo', 'regulatory')"
            }
        }
    }),
},
Tool {
    name: "research_get".to_string(),
    description: "Get full details of a research macro including parameters, \
        output schema, and suggested verbs template.".to_string(),
    input_schema: json!({
        "type": "object",
        "required": ["macro_name"],
        "properties": {
            "macro_name": {
                "type": "string",
                "description": "Name of the research macro (e.g., 'client-discovery')"
            }
        }
    }),
},
Tool {
    name: "research_execute".to_string(),
    description: "Execute a research macro with LLM reasoning and web search. \
        Returns structured JSON for human review. Results are stored in session \
        and require approval before generating DSL verbs.".to_string(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id", "macro_name", "params"],
        "properties": {
            "session_id": {
                "type": "string",
                "description": "UI session ID for state tracking"
            },
            "macro_name": {
                "type": "string",
                "description": "Name of the research macro to execute"
            },
            "params": {
                "type": "object",
                "description": "Parameters for the macro (see research_get for schema)"
            }
        }
    }),
},
Tool {
    name: "research_approve".to_string(),
    description: "Approve pending research results. Optionally provide edits \
        to correct data before approval. Returns suggested DSL verbs.".to_string(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id"],
        "properties": {
            "session_id": {
                "type": "string",
                "description": "UI session ID"
            },
            "edits": {
                "type": "object",
                "description": "Optional corrections to merge into research data"
            }
        }
    }),
},
Tool {
    name: "research_reject".to_string(),
    description: "Reject pending research results. The macro can be re-executed \
        with different parameters.".to_string(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id"],
        "properties": {
            "session_id": {
                "type": "string",
                "description": "UI session ID"
            },
            "reason": {
                "type": "string",
                "description": "Reason for rejection (for logging)"
            }
        }
    }),
},
Tool {
    name: "research_status".to_string(),
    description: "Get current research state for a session including pending \
        results, approval history, and available verbs.".to_string(),
    input_schema: json!({
        "type": "object",
        "required": ["session_id"],
        "properties": {
            "session_id": {
                "type": "string",
                "description": "UI session ID"
            }
        }
    }),
},
```

---

## Part 4: Split handlers/core.rs into Domain Modules

The current `core.rs` is ~2900 lines. Split into focused modules.

### Target Structure

```
rust/src/mcp/handlers/
├── mod.rs              # Re-exports, ToolHandlers struct shell
├── dispatch.rs         # dispatch() method only
├── dsl.rs              # dsl_validate, dsl_execute, dsl_plan, dsl_generate, etc.
├── cbu.rs              # cbu_get, cbu_list
├── entity.rs           # entity_get, entity_search, schema_info
├── workflow.rs         # workflow_status, workflow_advance, workflow_transition, etc.
├── template.rs         # template_list, template_get, template_expand
├── batch.rs            # batch_start, batch_add_entities, batch_confirm_keyset, etc.
├── research.rs         # research_list, research_get, research_execute, etc.
└── util.rs             # Shared helpers (gateway_search, parse_binding_value, etc.)
```

### Implementation Steps

1. **Create `handlers/util.rs`** - Extract shared utilities:
   - `get_gateway_client()`
   - `gateway_search()`
   - `parse_binding_value()`
   - `require_sessions()`

2. **Create `handlers/dispatch.rs`** - Just the match statement:
   ```rust
   impl ToolHandlers {
       pub async fn dispatch(&self, name: &str, args: Value) -> Result<Value> {
           match name {
               // DSL
               "dsl_validate" => dsl::validate(self, args).await,
               "dsl_execute" => dsl::execute(self, args).await,
               // ... etc
           }
       }
   }
   ```

3. **Create domain modules** (`dsl.rs`, `cbu.rs`, etc.) - Each contains:
   ```rust
   use super::ToolHandlers;
   use anyhow::Result;
   use serde_json::Value;
   
   pub async fn validate(handlers: &ToolHandlers, args: Value) -> Result<Value> {
       // ... implementation
   }
   ```

4. **Update `handlers/mod.rs`**:
   ```rust
   mod dispatch;
   mod util;
   mod dsl;
   mod cbu;
   mod entity;
   mod workflow;
   mod template;
   mod batch;
   mod research;
   
   pub use dispatch::ToolHandlers;
   ```

### Line Count Targets

| Module | Estimated Lines | Handlers |
|--------|-----------------|----------|
| `util.rs` | ~100 | Shared helpers |
| `dispatch.rs` | ~80 | ToolHandlers struct + dispatch match |
| `dsl.rs` | ~600 | validate, execute, execute_submission, bind, plan, generate, lookup, complete, signature |
| `cbu.rs` | ~150 | cbu_get, cbu_list |
| `entity.rs` | ~200 | entity_get, entity_search, schema_info, verbs_list |
| `workflow.rs` | ~250 | workflow_status, workflow_advance, workflow_transition, workflow_start, resolve_blocker |
| `template.rs` | ~150 | template_list, template_get, template_expand |
| `batch.rs` | ~500 | batch_start, batch_add_entities, batch_confirm_keyset, batch_set_scalar, batch_get_state, batch_expand_current, batch_record_result, batch_skip_current, batch_cancel |
| `research.rs` | ~350 | research_list, research_get, research_execute, research_approve, research_reject, research_status |

---

## Part 5: Optional - Web Search Integration

The current `ClaudeResearchClient::execute_web_search()` is stubbed. For real search:

### Option A: Brave Search API (Recommended)

```rust
async fn execute_web_search(&self, query: &str) -> Result<Value> {
    let api_key = std::env::var("BRAVE_SEARCH_API_KEY")
        .map_err(|_| ResearchError::LlmClient("BRAVE_SEARCH_API_KEY not set".into()))?;
    
    let response = self.client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("X-Subscription-Token", &api_key)
        .query(&[("q", query), ("count", "10")])
        .send()
        .await?;
    
    let data: Value = response.json().await?;
    
    // Transform to our format
    let results: Vec<Value> = data["web"]["results"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|r| json!({
            "title": r["title"],
            "url": r["url"],
            "snippet": r["description"]
        }))
        .collect();
    
    Ok(json!({
        "query": query,
        "results": results
    }))
}
```

### Option B: Serper.dev (Google Results)

Similar pattern with `SERPER_API_KEY`.

### Environment Variables

Add to `.env`:
```
BRAVE_SEARCH_API_KEY=your_key_here
# or
SERPER_API_KEY=your_key_here
```

---

## Verification Checklist

After implementation:

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes (including new research tests)
- [ ] `ls rust/config/macros/research/` shows 3 YAML files
- [ ] MCP server starts and `research_list` returns macros
- [ ] `research_execute` with mock/stubbed search produces valid JSON
- [ ] Session state transitions work (Idle → PendingReview → VerbsReady)
- [ ] Handler module split compiles and dispatches correctly

---

## Estimated Effort

| Task | Complexity | Time |
|------|------------|------|
| Create YAML configs | Low | 30 min |
| Session ResearchContext | Medium | 45 min |
| MCP tool registration | Low | 20 min |
| Split handlers/core.rs | Medium-High | 2-3 hours |
| Web search integration | Medium | 1 hour (optional) |
| **Total** | | **~4-5 hours** |

---

## Notes for Claude

1. **Test incrementally** - After each part, run `cargo check` at minimum
2. **Preserve imports** - When splitting core.rs, track all `use` statements carefully
3. **Session integration** - The handlers rely on `SessionStore` being present; test with integrated mode
4. **YAML validation** - Use `serde_yaml::from_str` to validate configs before committing
5. **Keep backups** - Before splitting core.rs, consider `cp core.rs core.rs.bak`
