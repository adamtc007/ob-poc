# TODO: LLM Engineering for Deterministic DSL Generation

---

## Implementation Status (2025-12-20)

### Phase 1: Foundation - COMPLETE ✅

| Technique | Status | Implementation Details |
|-----------|--------|------------------------|
| Structured Output (JSON) | ✅ DONE | `chat_with_tool()` with JSON schema in `build_intent_tool()`. Both Anthropic and OpenAI support native tool calling. |
| Verb Registry Injection | ✅ DONE | `build_vocab_prompt()` injects all verbs from registry. Tool schema uses `enum` constraint for verb field. |
| Few-Shot Examples | ✅ DONE | Created `rust/src/api/prompts/few_shot_examples.md` with 14 comprehensive examples covering creation, removal, lookups, ownership, clarification, and edge cases. |
| System Prompt Architecture | ✅ DONE | Refactored `build_intent_extraction_prompt()` to use 5-layer architecture: Role → Structure Rules → Domain Knowledge → Ambiguity Rules → Few-Shot Examples. |

### Phase 2: Reliability - PARTIAL

| Technique | Status | Implementation Details |
|-----------|--------|------------------------|
| Error Recovery Prompts | ⏳ PARTIAL | Basic retry loop exists with `feedback_context`. Need structured `ValidationError` type. |
| Confidence Calibration | ✅ DONE | Added `confidence` field (0.0-1.0) to tool schema as required output. Prompt includes calibration guide. |
| Entity Context Priming | ✅ DONE | `pre_resolve_entities()` fetches CBUs, entities, products, roles, jurisdictions before LLM call. `format_pre_resolved_context()` injects into prompt. |

### Phase 3: UX - PARTIAL

| Technique | Status | Implementation Details |
|-----------|--------|------------------------|
| Slot-Filling Clarification | ⏳ PARTIAL | `needs_clarification` + `clarification` object in tool schema. Handles ambiguity but not partial slot-filling. |
| Conversation State Machine | ⏳ EXISTS | `SessionState` enum exists with basic states. Not full state machine from proposal. |
| Chain-of-Thought | ❌ TODO | Not implemented. |

### Phase 4: Learning - NOT STARTED

| Technique | Status | Implementation Details |
|-----------|--------|------------------------|
| Semantic Intent Layer | ❌ TODO | Not implemented. |
| Active Correction Learning | ❌ TODO | Not implemented. |

### Files Modified

| File | Changes Made |
|------|--------------|
| `rust/src/api/agent_service.rs` | Layered prompt architecture, confidence in tool schema, few-shot examples included |
| `rust/src/api/prompts/few_shot_examples.md` | NEW - 14 comprehensive examples with confidence scoring guide |
| `rust/src/api/prompts/domain_knowledge.md` | EXISTS - Market codes, products, roles, jurisdictions |
| `rust/src/api/prompts/ambiguity_detection.md` | EXISTS - Ambiguity patterns and clarification format |

### Key Architecture Notes

1. **Two-Pass Resolution**: Entity references are resolved via EntityGateway AFTER LLM extraction. `ParamValue::ResolvedEntity` preserves display names for user DSL while using UUIDs for execution DSL.

2. **Pre-Resolution Context**: Before LLM call, `pre_resolve_entities()` fetches available entities and injects them into prompt. LLM can only reference entities that actually exist.

3. **Tool Calling > JSON Mode**: Both Anthropic and OpenAI support native tool calling which guarantees structured output. More reliable than prompt-based JSON mode.

4. **Verb Enum Constraint**: The `verb` field in tool schema uses `"enum": verb_names` to constrain LLM to only valid verbs from registry.

---

## Overview

This document covers practical techniques to improve the agent's ability to:
1. Understand user intent accurately
2. Generate valid DSL more reliably
3. Reduce ambiguity and need for disambiguation
4. Create a better conversational UX
5. Maximize the "deterministic outcome %" - the rate at which user intent translates correctly to executed DSL

---

## Current State Assessment

Before optimizing, understand what's happening now:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  USER INPUT → DSL OUTPUT: What Can Go Wrong?                               │
│                                                                             │
│  1. INTENT MISUNDERSTANDING                                                │
│     User says: "Put John on the board"                                     │
│     LLM thinks: Add to mailing list? Create a dashboard? Add as director? │
│                                                                             │
│  2. WRONG VERB SELECTION                                                   │
│     User says: "Create a CBU for Apex"                                     │
│     LLM picks: cbu.create (doesn't exist, should be cbu.ensure)           │
│                                                                             │
│  3. WRONG ARGUMENT MAPPING                                                 │
│     User says: "Add John as director of Apex"                              │
│     LLM maps: :director-id "John" (wrong param name)                      │
│                                                                             │
│  4. ENTITY AMBIGUITY NOT RECOGNIZED                                        │
│     User says: "John Smith"                                                │
│     LLM assumes: There's only one John Smith (there are 3)                │
│                                                                             │
│  5. MISSING REQUIRED PARAMETERS                                            │
│     User says: "Create a fund"                                             │
│     LLM generates: (fund.create :name "???") - missing required args      │
│                                                                             │
│  6. HALLUCINATED CAPABILITIES                                              │
│     User says: "Send an email to John"                                     │
│     LLM generates: (email.send ...) - verb doesn't exist                  │
│                                                                             │
│  7. CONTEXT LOSS                                                           │
│     User says: "Now add him to that fund too"                             │
│     LLM: "Who is 'him'? What fund?"                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Technique 1: Structured Output Forcing

**Problem:** LLM generates freeform text that needs parsing.

**Solution:** Force structured JSON output.

### Implementation

```rust
// Instead of asking LLM to generate DSL text, ask for structured intent

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VerbIntent {
    /// The verb to execute (e.g., "cbu.ensure")
    pub verb: String,
    
    /// Direct parameters (literals)
    pub params: HashMap<String, ParamValue>,
    
    /// Entity lookups needed
    pub lookups: Vec<EntityLookup>,
    
    /// Binding name if creating something to reference later
    pub binding: Option<String>,
    
    /// Confidence level (0.0-1.0)
    pub confidence: f32,
    
    /// Reasoning for this interpretation (for debugging)
    pub reasoning: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct EntityLookup {
    /// Which parameter this lookup is for
    pub param_name: String,
    
    /// The search text from user input
    pub search_text: String,
    
    /// Expected entity type
    pub entity_type: String,
    
    /// Is this definitely an entity ref, or might be a literal?
    pub definitely_entity: bool,
}
```

### Prompt Pattern

```
You are a DSL translator. Convert user requests to structured JSON.

OUTPUT FORMAT (strict JSON, no markdown):
{
  "verb": "domain.verb-name",
  "params": { "param-name": "value" },
  "lookups": [
    { "param_name": "entity-id", "search_text": "...", "entity_type": "..." }
  ],
  "binding": "@name or null",
  "confidence": 0.0-1.0,
  "reasoning": "why this interpretation"
}

AVAILABLE VERBS:
[inject verb registry here]

USER REQUEST: "Add John Smith as director of Apex Fund"

RESPONSE:
```

### Why This Works

- JSON is unambiguous to parse
- Schema validation catches malformed output
- LLM is good at structured output when format is clear
- Retry is easy: "Your JSON was invalid because X, try again"

---

## Technique 2: Verb Registry Injection

**Problem:** LLM doesn't know what verbs exist.

**Solution:** Include the verb registry in the system prompt.

### Implementation

```rust
fn build_verb_context(registry: &VerbRegistry) -> String {
    let mut context = String::new();
    
    context.push_str("AVAILABLE VERBS:\n\n");
    
    for (domain, verbs) in registry.by_domain() {
        context.push_str(&format!("## {}\n", domain));
        
        for verb in verbs {
            context.push_str(&format!(
                "- {}.{}: {}\n",
                domain, verb.name, verb.description
            ));
            
            context.push_str("  Required params:\n");
            for param in &verb.required_params {
                context.push_str(&format!(
                    "    :{} ({}) - {}\n",
                    param.name, param.param_type, param.description
                ));
            }
            
            context.push_str("  Optional params:\n");
            for param in &verb.optional_params {
                context.push_str(&format!(
                    "    :{} ({}) - {}\n",
                    param.name, param.param_type, param.description
                ));
            }
            
            context.push_str("\n");
        }
    }
    
    context
}
```

### Prompt Injection

```
AVAILABLE VERBS:

## cbu
- cbu.ensure: Create or update a Client Business Unit
  Required params:
    :name (string) - The name of the CBU
    :manco-id (entity-ref:entity) - The management company
  Optional params:
    :jurisdiction (enum:jurisdiction) - Legal jurisdiction
    :as (binding) - Reference name for later use

## entity
- entity.add-role: Add a role relationship between entities
  Required params:
    :entity-id (entity-ref:person) - The person
    :target-id (entity-ref:entity) - The target entity
    :role-type (enum:role-type) - Type of role
  Optional params:
    :start-date (date) - When role begins

[... etc ...]

IMPORTANT: Only use verbs listed above. If the user asks for something 
not covered by these verbs, say so - do not invent verbs.
```

### Why This Works

- LLM can only pick from known verbs
- Parameter names and types are explicit
- Descriptions help with intent mapping
- "Do not invent" instruction catches hallucination

---

## Technique 3: Few-Shot Examples

**Problem:** LLM doesn't know the expected output style.

**Solution:** Include examples of user input → structured output.

### Implementation

```rust
fn build_few_shot_examples() -> String {
    r#"
EXAMPLES:

User: "Create a CBU called Apex for BlackRock ManCo"
Output:
{
  "verb": "cbu.ensure",
  "params": { "name": "Apex" },
  "lookups": [
    { "param_name": "manco-id", "search_text": "BlackRock ManCo", "entity_type": "entity" }
  ],
  "binding": null,
  "confidence": 0.95,
  "reasoning": "Clear request to create CBU with specified name and management company"
}

User: "Add John Smith as a director of the Apex fund"
Output:
{
  "verb": "entity.add-role",
  "params": { "role-type": "DIRECTOR" },
  "lookups": [
    { "param_name": "entity-id", "search_text": "John Smith", "entity_type": "person" },
    { "param_name": "target-id", "search_text": "Apex fund", "entity_type": "fund" }
  ],
  "binding": null,
  "confidence": 0.90,
  "reasoning": "Adding person to entity with director role"
}

User: "Set up custody for all Luxembourg funds"
Output:
{
  "verb": "BULK_OPERATION",
  "params": { 
    "operation": "service.add-product",
    "product": "CUSTODY"
  },
  "lookups": [
    { "param_name": "target-set", "search_text": "Luxembourg funds", "entity_type": "fund", "is_query": true }
  ],
  "binding": null,
  "confidence": 0.85,
  "reasoning": "Bulk operation across a filtered set of entities"
}

User: "What's the weather like?"
Output:
{
  "verb": "NOT_SUPPORTED",
  "params": {},
  "lookups": [],
  "binding": null,
  "confidence": 1.0,
  "reasoning": "Request is not related to onboarding operations"
}
"#.to_string()
}
```

### Why This Works

- LLM learns output format from examples
- Edge cases (bulk, not supported) are demonstrated
- Confidence scoring is modeled
- Reasoning is shown as expected

---

## Technique 4: Chain-of-Thought for Complex Requests

**Problem:** Complex multi-step requests get mangled.

**Solution:** Ask LLM to reason step-by-step before outputting.

### Prompt Pattern

```
For complex requests, think step by step:

1. What is the user trying to accomplish overall?
2. What individual operations are needed?
3. What is the dependency order?
4. What entities need to be looked up?
5. Are there any ambiguities that need clarification?

Then output the structured response.

User: "Create a new fund called Apex under BlackRock ManCo, add John Smith 
       as director, and set up custody and fund accounting services"

THINKING:
1. Overall: Setting up a new fund with roles and services
2. Operations needed:
   - Create fund (or CBU?) under ManCo
   - Add director role
   - Add custody service
   - Add fund accounting service
3. Dependencies:
   - Fund must exist before adding roles
   - Fund must exist before adding services
4. Lookups:
   - "BlackRock ManCo" → entity
   - "John Smith" → person
5. Ambiguities:
   - Is "Apex" a fund or CBU? Assuming fund.
   - Which "John Smith"? May need disambiguation.

OUTPUT:
{
  "intents": [
    {
      "verb": "fund.create",
      "params": { "name": "Apex" },
      "lookups": [{ "param_name": "manco-id", "search_text": "BlackRock ManCo", "entity_type": "entity" }],
      "binding": "@apex"
    },
    {
      "verb": "entity.add-role",
      "params": { "role-type": "DIRECTOR" },
      "lookups": [
        { "param_name": "entity-id", "search_text": "John Smith", "entity_type": "person" },
        { "param_name": "target-id", "binding_ref": "@apex" }
      ]
    },
    {
      "verb": "service.add-product",
      "params": { "product": "CUSTODY" },
      "lookups": [{ "param_name": "cbu-id", "binding_ref": "@apex" }]
    },
    {
      "verb": "service.add-product",
      "params": { "product": "FUND_ACCOUNTING" },
      "lookups": [{ "param_name": "cbu-id", "binding_ref": "@apex" }]
    }
  ],
  "confidence": 0.85,
  "clarifications_needed": ["Confirm 'Apex' should be a fund, not a CBU"]
}
```

### Why This Works

- Explicit reasoning catches logical errors
- Dependencies are identified before generation
- Ambiguities are surfaced, not hidden
- Multi-step operations are properly sequenced

---

## Technique 5: Entity Context Priming

**Problem:** LLM doesn't know what entities exist.

**Solution:** Include relevant entities in context.

### Implementation

```rust
fn build_entity_context(session: &AgentSession, scope: &EntityScope) -> String {
    let mut context = String::new();
    
    // Include session bindings
    if !session.context.bindings.is_empty() {
        context.push_str("CURRENT SESSION BINDINGS:\n");
        for (name, entity) in &session.context.bindings {
            context.push_str(&format!(
                "  {} → {} ({})\n",
                name, entity.display_name, entity.entity_type
            ));
        }
        context.push_str("\n");
    }
    
    // Include recent focus entities
    if let Some(focus) = &session.context.focus {
        context.push_str("RECENT ENTITIES IN FOCUS:\n");
        for (entity_type, entity) in &focus.focus_by_type {
            context.push_str(&format!(
                "  [{}]: {} (\"him\"/\"her\"/\"it\" may refer to this)\n",
                entity_type, entity.display_name
            ));
        }
        context.push_str("\n");
    }
    
    // Include scoped entities (sample for large sets)
    context.push_str(&format!("WORKING SCOPE: {} ({} entities)\n", 
        scope.scope_name, scope.entity_count));
    context.push_str("Sample entities in scope:\n");
    for entity in scope.sample_entities.iter().take(10) {
        context.push_str(&format!(
            "  {} - {} ({})\n",
            entity.name, entity.entity_type, entity.jurisdiction.unwrap_or("N/A")
        ));
    }
    
    context
}
```

### Prompt Injection

```
CURRENT SESSION BINDINGS:
  @apex → Apex Alternative Fund (fund)
  @john → John Smith (person)

RECENT ENTITIES IN FOCUS:
  [person]: John Smith ("him"/"he" may refer to this)
  [fund]: Apex Alternative Fund ("it"/"the fund" may refer to this)

WORKING SCOPE: Allianz Onboarding (337 entities)
Sample entities in scope:
  Allianz Munich Property Fund - fund (DE)
  Allianz Global Investors GmbH - entity (DE)
  Allianz Luxembourg S.A. - entity (LU)
  ...

When resolving entity references:
- Check bindings first (@name references)
- Use focus entities for pronouns (him, her, it, the fund)
- Scope search to current working scope when possible
```

### Why This Works

- LLM knows what entities are "in play"
- Pronoun resolution has explicit targets
- Scoping reduces ambiguity
- Bindings are first-class references

---

## Technique 6: Confidence Calibration

**Problem:** LLM doesn't express uncertainty appropriately.

**Solution:** Explicit confidence scoring with calibration.

### Prompt Pattern

```
Rate your confidence in each interpretation:

1.0 - Certain: Request is unambiguous, maps perfectly to a verb
0.9 - High: Very likely interpretation, minor assumptions made
0.8 - Good: Reasonable interpretation, some ambiguity exists
0.7 - Medium: Multiple interpretations possible, picked most likely
0.6 - Low: Significant ambiguity, should ask for clarification
0.5 - Guessing: Not confident, definitely ask for clarification
<0.5 - Don't generate, ask for clarification instead

CALIBRATION EXAMPLES:

"Create a CBU called Apex for BlackRock" → 0.95 (clear, specific)
"Add John to the fund" → 0.70 (which John? which fund?)
"Set up everything for Allianz" → 0.50 (too vague, clarify)
"Do the thing" → 0.30 (don't generate, ask what they mean)
```

### Handling in Code

```rust
fn handle_agent_response(response: VerbIntent) -> AgentAction {
    match response.confidence {
        c if c >= 0.90 => {
            // High confidence: proceed to resolution
            AgentAction::Resolve(response)
        }
        c if c >= 0.70 => {
            // Medium confidence: show interpretation, ask to confirm
            AgentAction::ConfirmInterpretation {
                intent: response,
                message: format!(
                    "I understood this as: {}. Is that right?",
                    response.human_readable()
                ),
            }
        }
        c if c >= 0.50 => {
            // Low confidence: ask for clarification
            AgentAction::Clarify {
                question: response.reasoning.unwrap_or_else(|| 
                    "Could you be more specific?".to_string()
                ),
                suggestions: generate_clarifying_questions(&response),
            }
        }
        _ => {
            // Very low confidence: don't guess
            AgentAction::AskForMore {
                message: "I'm not sure what you're asking for. Could you rephrase or give more details?".to_string(),
            }
        }
    }
}
```

### Why This Works

- LLM learns to express uncertainty
- System can act appropriately on confidence level
- Low confidence triggers clarification, not guessing
- Calibration examples teach the scale

---

## Technique 7: Clarification Slot-Filling

**Problem:** When information is missing, LLM either guesses or gives up.

**Solution:** Structured slot-filling with clarifying questions.

### Implementation

```rust
#[derive(Serialize, Deserialize)]
pub struct PartialIntent {
    pub verb: String,
    pub filled_params: HashMap<String, ParamValue>,
    pub missing_params: Vec<MissingParam>,
    pub ambiguous_lookups: Vec<AmbiguousLookup>,
}

#[derive(Serialize, Deserialize)]
pub struct MissingParam {
    pub param_name: String,
    pub param_type: String,
    pub is_required: bool,
    pub question: String,  // How to ask for this
    pub suggestions: Vec<String>,  // Possible values
}

#[derive(Serialize, Deserialize)]
pub struct AmbiguousLookup {
    pub param_name: String,
    pub search_text: String,
    pub question: String,
    pub options: Vec<EntityOption>,
}
```

### Prompt Pattern

```
If information is missing or ambiguous, identify what's needed:

User: "Create a fund"

OUTPUT:
{
  "verb": "fund.create",
  "filled_params": {},
  "missing_params": [
    {
      "param_name": "name",
      "param_type": "string",
      "is_required": true,
      "question": "What should the fund be called?",
      "suggestions": []
    },
    {
      "param_name": "manco-id",
      "param_type": "entity-ref",
      "is_required": true,
      "question": "Which management company should own this fund?",
      "suggestions": ["Based on your recent work: BlackRock ManCo, Allianz Global Investors"]
    }
  ],
  "ambiguous_lookups": []
}
```

### Conversation Flow

```
User: "Create a fund"

Agent: I need a bit more information:
       • What should the fund be called?
       • Which management company should own it?
         (Recent: BlackRock ManCo, Allianz Global Investors)

User: "Call it Apex, under BlackRock"

Agent: ✓ Creating fund "Apex" under BlackRock ManCo
       [Confirm] [Edit]
```

### Why This Works

- Partial progress is preserved (not starting over)
- Questions are specific to what's missing
- Suggestions reduce friction
- Multi-turn feels like a conversation, not a form

---

## Technique 8: Error Recovery with Structured Feedback

**Problem:** When DSL validation fails, LLM doesn't know how to fix it.

**Solution:** Return structured error information for retry.

### Implementation

```rust
#[derive(Serialize, Deserialize)]
pub struct ValidationError {
    pub error_type: ValidationErrorType,
    pub message: String,
    pub location: Option<String>,  // Which param/field
    pub suggestion: Option<String>,  // How to fix
    pub valid_options: Option<Vec<String>>,  // If enum, what's valid
}

#[derive(Serialize, Deserialize)]
pub enum ValidationErrorType {
    UnknownVerb,
    UnknownParam,
    MissingRequiredParam,
    InvalidParamType,
    InvalidEnumValue,
    EntityNotFound,
    AmbiguousEntity,
    CircularDependency,
}
```

### Retry Prompt Pattern

```
Your previous output had an error:

ERROR: InvalidEnumValue
LOCATION: params.role-type
MESSAGE: "CFO" is not a valid role type
VALID OPTIONS: DIRECTOR, OFFICER, UBO, AUTHORIZED_SIGNATORY, BOARD_MEMBER
SUGGESTION: Did you mean "OFFICER"?

Please correct your output and try again.

PREVIOUS OUTPUT:
{
  "verb": "entity.add-role",
  "params": { "role-type": "CFO" },
  ...
}

CORRECTED OUTPUT:
```

### Why This Works

- Specific error location helps LLM focus
- Valid options prevent guessing
- Suggestion provides likely fix
- Structured retry is predictable

---

## Technique 9: Semantic Intent Extraction (Pre-Verb Mapping)

**Problem:** Jumping straight to verb selection loses nuance.

**Solution:** First extract semantic intent, then map to verbs.

### Two-Phase Process

```
PHASE 1: Semantic Intent Extraction

User: "Get John set up on the Apex fund as the main director"

Extract semantic intent:
{
  "action_type": "CREATE_RELATIONSHIP",
  "subject": { "reference": "John", "type": "person" },
  "object": { "reference": "Apex fund", "type": "fund" },
  "relationship": { "type": "role", "role_name": "director", "modifiers": ["main", "primary"] },
  "temporal": null,
  "conditions": null
}

PHASE 2: Verb Mapping

Given semantic intent, find matching verb:
- Action: CREATE_RELATIONSHIP
- Relationship type: role
- Subject type: person
- Object type: fund/entity

Matches: entity.add-role

Map modifiers:
- "main", "primary" → Could map to a flag, or just informational

Output VerbIntent with mapped parameters.
```

### Why This Works

- Separates understanding from mapping
- Semantic layer is reusable
- Modifiers and nuance are preserved
- Easier to debug: "Did we understand? Did we map correctly?"

---

## Technique 10: Conversation State Machine

**Problem:** Conversation can go off-track or get confused.

**Solution:** Explicit state machine for conversation flow.

### State Definitions

```rust
#[derive(Debug, Clone)]
pub enum ConversationState {
    /// Ready for new request
    Idle,
    
    /// Gathering missing information
    SlotFilling {
        partial_intent: PartialIntent,
        awaiting: Vec<String>,  // Which slots we're waiting for
    },
    
    /// Waiting for entity disambiguation
    Disambiguating {
        intent: VerbIntent,
        ambiguous_param: String,
        options: Vec<EntityOption>,
    },
    
    /// Showing generated DSL, waiting for confirmation
    PendingConfirmation {
        dsl: String,
        intent: VerbIntent,
    },
    
    /// Executing (no user input expected)
    Executing {
        dsl: String,
    },
    
    /// Showing results, ready for follow-up
    Complete {
        result: ExecutionResult,
        follow_up_suggestions: Vec<String>,
    },
    
    /// Error state, offering recovery options
    Error {
        error: AgentError,
        recovery_options: Vec<RecoveryOption>,
    },
}
```

### State Transitions

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                              ┌──────────────┐                              │
│            ┌────────────────►│     Idle     │◄───────────────┐             │
│            │                 └──────┬───────┘                │             │
│            │                        │                        │             │
│            │                   User input                    │             │
│            │                        │                        │             │
│            │                        ▼                        │             │
│            │           ┌─────────────────────────┐           │             │
│            │           │   Parse & Extract       │           │             │
│            │           │   Semantic Intent       │           │             │
│            │           └───────────┬─────────────┘           │             │
│            │                       │                         │             │
│            │          ┌────────────┼────────────┐            │             │
│            │          ▼            ▼            ▼            │             │
│            │   ┌────────────┐ ┌─────────┐ ┌──────────┐       │             │
│            │   │ SlotFilling│ │ Ready   │ │ Unclear  │       │             │
│            │   │ (missing)  │ │         │ │ (ask)    │───────┘             │
│            │   └─────┬──────┘ └────┬────┘ └──────────┘                     │
│            │         │             │                                        │
│            │         │ User fills  │                                        │
│            │         └─────┬───────┘                                        │
│            │               ▼                                                │
│            │        ┌─────────────┐                                         │
│            │        │  Resolve    │                                         │
│            │        │  Entities   │                                         │
│            │        └──────┬──────┘                                         │
│            │               │                                                │
│            │    ┌──────────┴──────────┐                                     │
│            │    ▼                     ▼                                     │
│            │ ┌───────────────┐ ┌─────────────────┐                          │
│            │ │ All Resolved  │ │ Disambiguation  │                          │
│            │ └───────┬───────┘ │ Required        │                          │
│            │         │         └────────┬────────┘                          │
│            │         │                  │ User selects                      │
│            │         │◄─────────────────┘                                   │
│            │         ▼                                                      │
│            │  ┌──────────────────┐                                          │
│            │  │ Pending          │                                          │
│            │  │ Confirmation     │                                          │
│            │  └────────┬─────────┘                                          │
│            │           │                                                    │
│            │  ┌────────┴────────┐                                           │
│            │  ▼                 ▼                                           │
│            │ [Confirm]        [Edit]──► SlotFilling                         │
│            │  │                                                             │
│            │  ▼                                                             │
│            │ ┌────────────┐                                                 │
│            │ │ Executing  │                                                 │
│            │ └─────┬──────┘                                                 │
│            │       │                                                        │
│            │  ┌────┴────┐                                                   │
│            │  ▼         ▼                                                   │
│            │ Success   Error──► Error State ──► Recovery ──► (loop)        │
│            │  │                                                             │
│            │  ▼                                                             │
│            │ ┌──────────────┐                                               │
│            └─┤  Complete    │                                               │
│              │ (suggest     │                                               │
│              │  follow-ups) │                                               │
│              └──────────────┘                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Why This Works

- Clear states prevent confusion
- Transitions are explicit
- UI can render appropriate controls for each state
- Recovery paths are defined

---

## Technique 11: System Prompt Architecture

**Problem:** System prompt is ad-hoc, not optimized.

**Solution:** Layered, structured system prompt.

### Prompt Layers

```rust
fn build_system_prompt(session: &AgentSession) -> String {
    let mut prompt = String::new();
    
    // Layer 1: Role and constraints
    prompt.push_str(ROLE_PROMPT);
    
    // Layer 2: Verb registry (what can be done)
    prompt.push_str(&build_verb_context(&session.verb_registry));
    
    // Layer 3: Entity context (what exists)
    prompt.push_str(&build_entity_context(session));
    
    // Layer 4: Session state (what's in progress)
    prompt.push_str(&build_session_state_context(session));
    
    // Layer 5: Output format specification
    prompt.push_str(OUTPUT_FORMAT_PROMPT);
    
    // Layer 6: Few-shot examples
    prompt.push_str(&build_few_shot_examples());
    
    // Layer 7: Error context (if retrying)
    if let Some(error) = &session.last_error {
        prompt.push_str(&build_error_context(error));
    }
    
    prompt
}

const ROLE_PROMPT: &str = r#"
You are a DSL Translation Agent for a financial services onboarding platform.

YOUR ROLE:
- Translate natural language requests into structured operations
- Identify entity references that need resolution
- Ask for clarification when requests are ambiguous
- Never execute anything - only generate structured intents

CONSTRAINTS:
- Only use verbs from the AVAILABLE VERBS list
- Never invent verbs, parameters, or entity types
- Express uncertainty in confidence scores
- If unsure, ask rather than guess
"#;

const OUTPUT_FORMAT_PROMPT: &str = r#"
OUTPUT FORMAT:
Return valid JSON matching this schema:
{
  "intent_type": "SINGLE" | "MULTI" | "CLARIFICATION" | "NOT_SUPPORTED",
  "intents": [...],  // Array of VerbIntent
  "clarification": { ... },  // If asking for more info
  "confidence": 0.0-1.0,
  "reasoning": "..."
}

Do not include markdown code blocks. Return raw JSON only.
"#;
```

### Why This Works

- Modular: each layer can be updated independently
- Contextual: session-specific information is injected
- Consistent: format specification is always present
- Debuggable: can log exactly what prompt was sent

---

## Technique 12: Active Learning from Corrections

**Problem:** Same mistakes repeat.

**Solution:** Learn from user corrections within session.

### Implementation

```rust
pub struct CorrectionLog {
    /// What the LLM generated
    pub original_intent: VerbIntent,
    
    /// What the user corrected it to
    pub corrected_intent: VerbIntent,
    
    /// The original user input
    pub user_input: String,
    
    /// What was wrong
    pub correction_type: CorrectionType,
}

pub enum CorrectionType {
    WrongVerb { from: String, to: String },
    WrongParam { param: String, from: String, to: String },
    WrongEntity { param: String, from: String, to: String },
    MissingParam { param: String, value: String },
    ExtraParam { param: String },
}

fn build_correction_context(corrections: &[CorrectionLog]) -> String {
    if corrections.is_empty() {
        return String::new();
    }
    
    let mut context = String::from("CORRECTIONS FROM THIS SESSION:\n");
    context.push_str("Learn from these mistakes - don't repeat them.\n\n");
    
    for correction in corrections.iter().take(5) {
        context.push_str(&format!(
            "Input: \"{}\"\n",
            correction.user_input
        ));
        context.push_str(&format!(
            "You said: {}\n",
            correction.original_intent.verb
        ));
        context.push_str(&format!(
            "Correct: {}\n",
            correction.corrected_intent.verb
        ));
        context.push_str(&format!(
            "Lesson: {}\n\n",
            correction.correction_type.lesson()
        ));
    }
    
    context
}
```

### Why This Works

- Mistakes become examples
- LLM adapts within session
- User teaches the system
- Corrections have immediate effect

---

## Implementation Phases

### Phase 1: Foundation (High Impact, Essential)

| Technique | Effort | Impact | Priority |
|-----------|--------|--------|----------|
| Structured Output (JSON) | Medium | Very High | P0 |
| Verb Registry Injection | Low | Very High | P0 |
| Few-Shot Examples | Low | High | P0 |
| System Prompt Architecture | Medium | High | P0 |

### Phase 2: Reliability (Reduce Errors)

| Technique | Effort | Impact | Priority |
|-----------|--------|--------|----------|
| Error Recovery Prompts | Low | High | P1 |
| Confidence Calibration | Low | Medium | P1 |
| Entity Context Priming | Medium | High | P1 |

### Phase 3: UX (Better Conversations)

| Technique | Effort | Impact | Priority |
|-----------|--------|--------|----------|
| Slot-Filling Clarification | Medium | High | P2 |
| Conversation State Machine | High | High | P2 |
| Chain-of-Thought | Low | Medium | P2 |

### Phase 4: Learning (Continuous Improvement)

| Technique | Effort | Impact | Priority |
|-----------|--------|--------|----------|
| Semantic Intent Layer | High | Medium | P3 |
| Active Correction Learning | Medium | Medium | P3 |

---

## Metrics to Track

1. **Parse Success Rate**: % of LLM outputs that parse as valid JSON
2. **Verb Accuracy**: % of correct verb selection
3. **First-Try Success**: % of requests that succeed without clarification
4. **Disambiguation Rate**: % requiring entity disambiguation
5. **Correction Rate**: % of confirmed operations that user edited
6. **Retry Rate**: % of operations requiring error recovery
7. **Turns-to-Completion**: Average conversation turns to complete an operation
8. **User Satisfaction**: Thumbs up/down on completed operations

---

## Files to Modify

| File | Changes |
|------|---------|
| `rust/src/api/agent_service.rs` | Prompt building, response parsing |
| `rust/src/api/session.rs` | Conversation state machine, context |
| `rust/src/dsl_v2/verb_registry.rs` | Export for prompt injection |
| `rust/src/api/agent_routes.rs` | API for state transitions |
| New: `rust/src/agent/prompt_builder.rs` | Centralized prompt construction |
| New: `rust/src/agent/intent_parser.rs` | Structured output parsing |
| New: `rust/src/agent/conversation_state.rs` | State machine logic |

---

*The goal: Make the LLM a reliable translator, not a creative generator. Constrain its output, validate everything, and create a feedback loop for improvement.*
