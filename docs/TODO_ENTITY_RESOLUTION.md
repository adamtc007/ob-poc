# TODO: Entity Resolution & Disambiguation

**Purpose**: Rich entity search with context for confident agent decisions  
**Priority**: HIGH - Core to natural language → DSL accuracy  
**Effort**: ~6-8 hours

---

## Problem Statement

Current `entity_search` returns:
```json
{
  "matches": [
    { "id": "uuid-1", "display": "John Smith", "score": 0.95 },
    { "id": "uuid-2", "display": "John Smith", "score": 0.92 }
  ],
  "ambiguous": true
}
```

**Not enough context to disambiguate.** Agent can't tell which John Smith without asking user every time.

---

## Goal

Return rich context so agent can:
1. **Auto-resolve** when context makes it obvious (e.g., user mentioned "the director" → pick the one with DIRECTOR role)
2. **Smart disambiguation** with distinguishing details when asking user
3. **Suggest create** when no good match exists

```json
{
  "matches": [
    {
      "id": "uuid-1",
      "display": "John Smith",
      "score": 0.95,
      "entity_type": "proper_person",
      "context": {
        "nationality": "US",
        "date_of_birth": "1975-03-15",
        "roles": ["Director at Apex Fund", "UBO of Meridian Holdings"],
        "last_activity": "2024-12-10",
        "created": "2023-06-15"
      },
      "disambiguation_label": "John Smith (US, b.1975) - Director at Apex Fund"
    },
    {
      "id": "uuid-2", 
      "display": "John Smith",
      "score": 0.92,
      "entity_type": "proper_person",
      "context": {
        "nationality": "GB",
        "date_of_birth": "1982-11-20",
        "roles": [],
        "last_activity": "2024-01-05",
        "created": "2024-01-05"
      },
      "disambiguation_label": "John Smith (UK, b.1982) - No current roles"
    }
  ],
  "resolution_confidence": "low",
  "suggested_action": "ask_user",
  "disambiguation_prompt": "I found two people named John Smith:\n1. John Smith (US, b.1975) - Director at Apex Fund\n2. John Smith (UK, b.1982) - No current roles\nWhich one?"
}
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         entity_search Tool                              │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
         ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
         │ EntityGateway│  │ Context      │  │ Resolution   │
         │ search_fuzzy │  │ Enricher     │  │ Strategy     │
         └──────────────┘  └──────────────┘  └──────────────┘
                │                  │                │
                │                  │                │
                ▼                  ▼                ▼
         Basic matches      + Roles, dates    → Confidence +
         (id, display,        relationships     suggested action
          score)
```

---

## Part 1: Context Enrichment

### 1.1 Define Enrichment Queries

For each entity type, define what context to fetch:

```rust
pub enum EntityType {
    ProperPerson,
    LegalEntity,
    Cbu,
    Document,
}

impl EntityType {
    /// SQL to fetch enrichment context for this entity type
    pub fn enrichment_query(&self) -> &'static str {
        match self {
            EntityType::ProperPerson => r#"
                SELECT 
                    e.entity_id,
                    e.first_name || ' ' || e.last_name as display,
                    e.nationality,
                    e.date_of_birth,
                    e.created_at,
                    e.updated_at,
                    COALESCE(
                        (SELECT json_agg(json_build_object(
                            'role', r.role_code,
                            'cbu_name', c.name,
                            'since', cr.effective_from
                        ))
                        FROM "ob-poc".cbu_roles cr
                        JOIN "ob-poc".cbus c ON cr.cbu_id = c.cbu_id
                        JOIN "ob-poc".roles r ON cr.role_id = r.role_id
                        WHERE cr.entity_id = e.entity_id
                        AND cr.effective_to IS NULL),
                        '[]'
                    ) as roles,
                    COALESCE(
                        (SELECT json_agg(json_build_object(
                            'owned_name', owned.name,
                            'percentage', o.percentage,
                            'type', o.ownership_type
                        ))
                        FROM "ob-poc".ownership_links o
                        JOIN "ob-poc".cbus owned ON o.owned_cbu_id = owned.cbu_id
                        WHERE o.owner_entity_id = e.entity_id),
                        '[]'
                    ) as ownership
                FROM "ob-poc".entities e
                WHERE e.entity_id = ANY($1)
            "#,
            EntityType::LegalEntity => r#"
                SELECT 
                    e.entity_id,
                    e.legal_name as display,
                    e.jurisdiction,
                    e.registration_number,
                    e.entity_subtype,
                    e.created_at,
                    e.updated_at,
                    -- Similar role/ownership joins
                FROM "ob-poc".entities e
                WHERE e.entity_id = ANY($1)
            "#,
            EntityType::Cbu => r#"
                SELECT
                    c.cbu_id,
                    c.name as display,
                    c.jurisdiction,
                    c.cbu_type,
                    c.status,
                    c.created_at,
                    (SELECT COUNT(*) FROM "ob-poc".cbu_roles cr WHERE cr.cbu_id = c.cbu_id) as role_count,
                    (SELECT COUNT(*) FROM "ob-poc".ubo_registry u WHERE u.cbu_id = c.cbu_id) as ubo_count
                FROM "ob-poc".cbus c
                WHERE c.cbu_id = ANY($1)
            "#,
            // ... other types
        }
    }
}
```

### 1.2 Create Enrichment Service

```rust
// rust/src/mcp/enrichment.rs

use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub struct EntityEnricher {
    pool: PgPool,
}

impl EntityEnricher {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Enrich a batch of entity IDs with context
    pub async fn enrich(
        &self,
        entity_type: EntityType,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, EntityContext>, sqlx::Error> {
        let query = entity_type.enrichment_query();
        
        let rows = sqlx::query(query)
            .bind(ids)
            .fetch_all(&self.pool)
            .await?;
        
        let mut results = HashMap::new();
        for row in rows {
            let id: Uuid = row.get("entity_id");
            let context = EntityContext::from_row(&row, entity_type);
            results.insert(id, context);
        }
        
        Ok(results)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityContext {
    pub nationality: Option<String>,
    pub date_of_birth: Option<String>,
    pub jurisdiction: Option<String>,
    pub roles: Vec<RoleContext>,
    pub ownership: Vec<OwnershipContext>,
    pub created_at: String,
    pub last_activity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleContext {
    pub role: String,
    pub cbu_name: String,
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnershipContext {
    pub owned_name: String,
    pub percentage: f64,
    pub ownership_type: String,
}

impl EntityContext {
    /// Build human-readable disambiguation label
    pub fn disambiguation_label(&self, display: &str, entity_type: EntityType) -> String {
        match entity_type {
            EntityType::ProperPerson => {
                let mut parts = vec![display.to_string()];
                
                if let Some(nat) = &self.nationality {
                    parts.push(format!("({})", nat));
                }
                if let Some(dob) = &self.date_of_birth {
                    if let Some(year) = dob.split('-').next() {
                        parts.push(format!("b.{}", year));
                    }
                }
                
                if !self.roles.is_empty() {
                    let role_str = self.roles.iter()
                        .take(2)
                        .map(|r| format!("{} at {}", r.role, r.cbu_name))
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("- {}", role_str));
                } else {
                    parts.push("- No current roles".to_string());
                }
                
                parts.join(" ")
            }
            EntityType::LegalEntity => {
                let mut parts = vec![display.to_string()];
                if let Some(j) = &self.jurisdiction {
                    parts.push(format!("({})", j));
                }
                if !self.roles.is_empty() {
                    parts.push(format!("- {} roles", self.roles.len()));
                }
                parts.join(" ")
            }
            EntityType::Cbu => {
                let mut parts = vec![display.to_string()];
                if let Some(j) = &self.jurisdiction {
                    parts.push(format!("({})", j));
                }
                parts.join(" ")
            }
        }
    }
}
```

---

## Part 2: Resolution Strategy

### 2.1 Define Confidence Levels

```rust
// rust/src/mcp/resolution.rs

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionConfidence {
    /// Single exact match or very high score - auto-resolve
    High,
    /// Good match but should confirm
    Medium,  
    /// Multiple similar matches - must ask
    Low,
    /// No good matches
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    /// Use this match automatically
    AutoResolve { match_id: String },
    /// Ask user to choose
    AskUser,
    /// Suggest creating new entity
    SuggestCreate,
    /// Need more search terms
    NeedMoreInfo,
}
```

### 2.2 Resolution Logic

```rust
pub struct ResolutionStrategy;

impl ResolutionStrategy {
    /// Determine resolution confidence and suggested action
    pub fn analyze(
        matches: &[EnrichedMatch],
        conversation_context: Option<&ConversationContext>,
    ) -> ResolutionResult {
        if matches.is_empty() {
            return ResolutionResult {
                confidence: ResolutionConfidence::None,
                action: SuggestedAction::SuggestCreate,
                prompt: None,
            };
        }
        
        let top = &matches[0];
        
        // Case 1: Single high-confidence match
        if matches.len() == 1 && top.score > 0.90 {
            return ResolutionResult {
                confidence: ResolutionConfidence::High,
                action: SuggestedAction::AutoResolve { match_id: top.id.clone() },
                prompt: None,
            };
        }
        
        // Case 2: Clear winner (big gap to second place)
        if matches.len() > 1 {
            let gap = top.score - matches[1].score;
            if gap > 0.15 && top.score > 0.85 {
                return ResolutionResult {
                    confidence: ResolutionConfidence::High,
                    action: SuggestedAction::AutoResolve { match_id: top.id.clone() },
                    prompt: None,
                };
            }
        }
        
        // Case 3: Context-based resolution
        if let Some(ctx) = conversation_context {
            if let Some(resolved) = Self::resolve_from_context(matches, ctx) {
                return ResolutionResult {
                    confidence: ResolutionConfidence::Medium,
                    action: SuggestedAction::AutoResolve { match_id: resolved },
                    prompt: None,
                };
            }
        }
        
        // Case 4: Top match is decent but not confident
        if top.score > 0.70 && top.score < 0.90 {
            return ResolutionResult {
                confidence: ResolutionConfidence::Medium,
                action: SuggestedAction::AskUser,
                prompt: Some(Self::build_disambiguation_prompt(matches)),
            };
        }
        
        // Case 5: Multiple similar matches
        if matches.len() > 1 && (top.score - matches[1].score).abs() < 0.10 {
            return ResolutionResult {
                confidence: ResolutionConfidence::Low,
                action: SuggestedAction::AskUser,
                prompt: Some(Self::build_disambiguation_prompt(matches)),
            };
        }
        
        // Case 6: Low scores - maybe create new?
        if top.score < 0.50 {
            return ResolutionResult {
                confidence: ResolutionConfidence::None,
                action: SuggestedAction::SuggestCreate,
                prompt: Some(format!(
                    "No good match found for '{}'. Create new entity?",
                    matches.get(0).map(|m| m.display.as_str()).unwrap_or("unknown")
                )),
            };
        }
        
        // Default: ask user
        ResolutionResult {
            confidence: ResolutionConfidence::Low,
            action: SuggestedAction::AskUser,
            prompt: Some(Self::build_disambiguation_prompt(matches)),
        }
    }
    
    /// Try to resolve using conversation context
    fn resolve_from_context(
        matches: &[EnrichedMatch],
        ctx: &ConversationContext,
    ) -> Option<String> {
        // If user mentioned "director", prefer match with director role
        if ctx.mentioned_roles.contains(&"DIRECTOR".to_string()) {
            for m in matches {
                if m.context.roles.iter().any(|r| r.role == "DIRECTOR") {
                    return Some(m.id.clone());
                }
            }
        }
        
        // If user mentioned a specific CBU, prefer entity linked to it
        if let Some(cbu_name) = &ctx.mentioned_cbu {
            for m in matches {
                if m.context.roles.iter().any(|r| r.cbu_name.to_lowercase().contains(&cbu_name.to_lowercase())) {
                    return Some(m.id.clone());
                }
            }
        }
        
        // If user mentioned nationality
        if let Some(nat) = &ctx.mentioned_nationality {
            for m in matches {
                if m.context.nationality.as_ref() == Some(nat) {
                    return Some(m.id.clone());
                }
            }
        }
        
        None
    }
    
    /// Build user-friendly disambiguation prompt
    fn build_disambiguation_prompt(matches: &[EnrichedMatch]) -> String {
        let options: Vec<String> = matches.iter()
            .take(5)
            .enumerate()
            .map(|(i, m)| format!("{}. {}", i + 1, m.disambiguation_label))
            .collect();
        
        format!(
            "Multiple matches found. Which did you mean?\n{}",
            options.join("\n")
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolutionResult {
    pub confidence: ResolutionConfidence,
    pub action: SuggestedAction,
    pub prompt: Option<String>,
}

/// Context from conversation that helps resolution
#[derive(Debug, Default)]
pub struct ConversationContext {
    pub mentioned_roles: Vec<String>,
    pub mentioned_cbu: Option<String>,
    pub mentioned_nationality: Option<String>,
    pub mentioned_jurisdiction: Option<String>,
    pub current_cbu_id: Option<Uuid>,
}
```

---

## Part 3: Enhanced entity_search Tool

### 3.1 Update Tool Definition

```rust
// In tools.rs

Tool {
    name: "entity_search".into(),
    description: r#"Search for entities with rich context for disambiguation.
    
Returns matches with:
- Basic info (id, display, score)
- Context (roles, relationships, dates)
- Disambiguation labels
- Resolution confidence and suggested action

Use conversation_hints to improve auto-resolution (e.g., if user mentioned "director", 
matches with director role are preferred)."#.into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Search query (name, partial name, etc.)"
            },
            "entity_type": {
                "type": "string",
                "enum": ["person", "company", "cbu", "any"],
                "description": "Filter by entity type"
            },
            "limit": {
                "type": "integer",
                "default": 10,
                "description": "Max results"
            },
            "conversation_hints": {
                "type": "object",
                "description": "Context from conversation to help resolution",
                "properties": {
                    "mentioned_roles": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Roles mentioned (e.g., ['DIRECTOR', 'UBO'])"
                    },
                    "mentioned_cbu": {
                        "type": "string",
                        "description": "CBU name mentioned in conversation"
                    },
                    "mentioned_nationality": {
                        "type": "string",
                        "description": "Nationality mentioned (e.g., 'US', 'GB')"
                    }
                }
            }
        },
        "required": ["query"]
    }),
}
```

### 3.2 Update Handler

```rust
// In handlers.rs

pub async fn handle_entity_search(
    &self,
    query: &str,
    entity_type: Option<&str>,
    limit: Option<i32>,
    conversation_hints: Option<ConversationContext>,
) -> Result<Value> {
    let limit = limit.unwrap_or(10);
    
    // Step 1: Search via EntityGateway
    let nickname = match entity_type {
        Some("person") => "PERSON",
        Some("company") => "LEGAL_ENTITY",
        Some("cbu") => "CBU",
        _ => "ENTITY",  // Search all entity types
    };
    
    let raw_matches = self.gateway_search(nickname, Some(query), limit).await?;
    
    if raw_matches.is_empty() {
        return Ok(json!({
            "matches": [],
            "resolution_confidence": "none",
            "suggested_action": { "suggest_create": true },
            "disambiguation_prompt": format!("No matches found for '{}'. Would you like to create a new entity?", query)
        }));
    }
    
    // Step 2: Extract IDs for enrichment
    let ids: Vec<Uuid> = raw_matches.iter()
        .filter_map(|(id, _, _)| Uuid::parse_str(id).ok())
        .collect();
    
    // Step 3: Enrich with context
    let entity_type_enum = match entity_type {
        Some("person") => EntityType::ProperPerson,
        Some("company") => EntityType::LegalEntity,
        Some("cbu") => EntityType::Cbu,
        _ => EntityType::ProperPerson,  // Default
    };
    
    let enricher = EntityEnricher::new(self.pool.clone());
    let contexts = enricher.enrich(entity_type_enum, &ids).await?;
    
    // Step 4: Build enriched matches
    let enriched_matches: Vec<EnrichedMatch> = raw_matches.iter()
        .filter_map(|(id, display, score)| {
            let uuid = Uuid::parse_str(id).ok()?;
            let context = contexts.get(&uuid).cloned().unwrap_or_default();
            let disambiguation_label = context.disambiguation_label(display, entity_type_enum);
            
            Some(EnrichedMatch {
                id: id.clone(),
                display: display.clone(),
                score: *score,
                entity_type: format!("{:?}", entity_type_enum),
                context,
                disambiguation_label,
            })
        })
        .collect();
    
    // Step 5: Determine resolution strategy
    let resolution = ResolutionStrategy::analyze(
        &enriched_matches,
        conversation_hints.as_ref(),
    );
    
    // Step 6: Build response
    Ok(json!({
        "matches": enriched_matches,
        "resolution_confidence": resolution.confidence,
        "suggested_action": resolution.action,
        "disambiguation_prompt": resolution.prompt,
    }))
}
```

---

## Part 4: Agent Usage Patterns

### 4.1 Auto-Resolution Flow

```
User: "Add John Smith as director of Apex Fund"

Agent: [calls entity_search]
{
  "query": "John Smith",
  "entity_type": "person",
  "conversation_hints": {
    "mentioned_roles": ["DIRECTOR"],
    "mentioned_cbu": "Apex Fund"
  }
}

Response:
{
  "matches": [
    {
      "id": "uuid-123",
      "display": "John Smith",
      "disambiguation_label": "John Smith (US, b.1975) - Director at Meridian Fund",
      "context": { "nationality": "US", "roles": [...] }
    },
    {
      "id": "uuid-456", 
      "display": "John Smith",
      "disambiguation_label": "John Smith (UK, b.1982) - No current roles",
      "context": { "nationality": "GB", "roles": [] }
    }
  ],
  "resolution_confidence": "low",
  "suggested_action": { "ask_user": true },
  "disambiguation_prompt": "Multiple matches found..."
}

Agent: "I found two people named John Smith:
1. John Smith (US, b.1975) - already Director at Meridian Fund
2. John Smith (UK, b.1982) - No current roles

Which one should I add as Director to Apex Fund?"
```

### 4.2 Context-Assisted Resolution

```
User: "Add the British John Smith as director"

Agent: [calls entity_search]
{
  "query": "John Smith",
  "entity_type": "person", 
  "conversation_hints": {
    "mentioned_nationality": "GB"
  }
}

Response:
{
  "matches": [...],
  "resolution_confidence": "medium",
  "suggested_action": { "auto_resolve": { "match_id": "uuid-456" } },
  "disambiguation_prompt": null
}

Agent: [Auto-resolves to UK John Smith, generates DSL]
"Adding John Smith (UK) as Director of Apex Fund..."
```

### 4.3 Create-If-Not-Found Flow

```
User: "Add Sarah Connor as fund administrator"

Agent: [calls entity_search]
{
  "query": "Sarah Connor",
  "entity_type": "person"
}

Response:
{
  "matches": [],
  "resolution_confidence": "none",
  "suggested_action": { "suggest_create": true },
  "disambiguation_prompt": "No matches found for 'Sarah Connor'. Would you like to create a new entity?"
}

Agent: "I don't have Sarah Connor in the system. To add her as Fund Administrator, 
I'll need to create her profile. Can you provide:
- Date of birth
- Nationality"
```

---

## Part 5: Files to Create/Modify

```
rust/src/mcp/
├── enrichment.rs      # NEW: EntityEnricher, EntityContext
├── resolution.rs      # NEW: ResolutionStrategy, ConversationContext
├── handlers.rs        # MODIFY: Update entity_search handler
├── tools.rs           # MODIFY: Update tool schema
└── types.rs           # MODIFY: Add EnrichedMatch, ResolutionResult
```

---

## Implementation Checklist

- [ ] Create `enrichment.rs` with EntityEnricher
- [ ] Define enrichment SQL for each entity type
- [ ] Create `resolution.rs` with ResolutionStrategy
- [ ] Implement confidence scoring logic
- [ ] Implement context-based resolution
- [ ] Update `entity_search` handler to use enrichment
- [ ] Update tool schema to accept conversation_hints
- [ ] Add disambiguation_label generation
- [ ] Test auto-resolution scenarios
- [ ] Test disambiguation scenarios
- [ ] Test create-if-not-found scenario

---

## Testing Scenarios

```rust
#[tokio::test]
async fn test_auto_resolve_single_high_score() {
    // Single match with score > 0.90 should auto-resolve
}

#[tokio::test]
async fn test_auto_resolve_clear_winner() {
    // Gap > 0.15 between top two should auto-resolve top
}

#[tokio::test]
async fn test_context_resolution_by_role() {
    // Mentioned "director" should prefer entity with director role
}

#[tokio::test]
async fn test_context_resolution_by_nationality() {
    // Mentioned "British" should prefer entity with GB nationality
}

#[tokio::test]
async fn test_disambiguation_close_scores() {
    // Two matches with similar scores should trigger disambiguation
}

#[tokio::test]
async fn test_suggest_create_no_matches() {
    // No matches should suggest creating new entity
}

#[tokio::test]
async fn test_suggest_create_low_scores() {
    // All matches below 0.50 should suggest creating new entity
}
```

---

## Dependencies

None new - uses existing:
- `sqlx` for enrichment queries
- `EntityGateway` for search
- `serde_json` for responses
