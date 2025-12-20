# TODO: Contextual Resolution and Conversational UX

## Overview

Move from stateless entity resolution to contextual, conversational resolution that:
1. Uses session context to disambiguate
2. Resolves pronouns and anaphora
3. Returns confidence scores for partial matches
4. Learns from user corrections within session

---

## The Problem

Current resolution is stateless - each entity reference is resolved independently:

```
User: "Add John to the Apex fund"
      → Resolve "John" (3 matches, disambiguate)
      → Resolve "Apex fund" (1 match, auto-resolve)

User: "Now add him to Beta fund"
      → "him" ??? (no resolution strategy for pronouns)
      → Fails or asks "who is 'him'?"
```

This creates friction and feels robotic.

---

## Solution Layers

### Layer 1: Session Binding Context

The session already tracks bindings. Use them for resolution priority:

```rust
pub struct SessionContext {
    // Existing
    pub bindings: HashMap<String, BoundEntity>,
    
    // New: Resolution memory for this session
    pub resolution_cache: HashMap<String, ResolvedEntityMemory>,
}

pub struct ResolvedEntityMemory {
    /// The search text that was used
    pub search_text: String,
    /// What it resolved to
    pub entity_id: Uuid,
    /// Entity type
    pub entity_type: String,
    /// Display name
    pub display_name: String,
    /// How it was resolved
    pub resolution_source: ResolutionSource,
    /// When it was resolved
    pub resolved_at: DateTime<Utc>,
}

pub enum ResolutionSource {
    /// Exact match, auto-resolved
    ExactMatch,
    /// User selected from disambiguation
    UserSelected,
    /// Fuzzy match above threshold
    FuzzyMatch { confidence: f32 },
    /// Pronoun resolution from context
    PronounResolution { antecedent: String },
    /// Inferred from conversation context
    ContextualInference,
}
```

### Layer 2: Pronoun Resolution

Track "focus" entities by type for pronoun resolution:

```rust
pub struct ConversationFocus {
    /// Most recently mentioned/used entity by type
    /// "person" → John Smith, "fund" → Apex Fund, etc.
    pub focus_by_type: HashMap<String, FocusEntity>,
    
    /// Ordered history of entity mentions (for "the other one", "the first one")
    pub mention_history: Vec<EntityMention>,
}

pub struct FocusEntity {
    pub entity_id: Uuid,
    pub display_name: String,
    pub entity_type: String,
    /// Grammatical gender for pronoun matching (if known)
    pub gender: Option<Gender>,
    /// When this became the focus
    pub focused_at: DateTime<Utc>,
}

pub enum Gender {
    Male,      // he, him, his
    Female,    // she, her, hers
    Neutral,   // it, its, they, them
    Unknown,
}
```

Pronoun resolution rules:

```
"him" / "he" / "his" → Most recent Male person entity in focus
"her" / "she"        → Most recent Female person entity in focus  
"it" / "its"         → Most recent non-person entity in focus
"them" / "they"      → Could be plural OR singular neutral
"the fund"           → Most recent fund entity
"the company"        → Most recent company entity
"the other one"      → Second most recent of same type as focus
```

### Layer 3: Confidence Scoring

Return confidence with every resolution:

```rust
pub struct ResolutionResult {
    pub entity_id: Uuid,
    pub display_name: String,
    pub entity_type: String,
    pub confidence: ConfidenceScore,
    pub resolution_path: ResolutionPath,
}

pub struct ConfidenceScore {
    /// 0.0 to 1.0
    pub score: f32,
    /// Why this confidence level
    pub reason: ConfidenceReason,
}

pub enum ConfidenceReason {
    /// Exact text match
    ExactMatch,
    /// Case-insensitive exact match
    CaseInsensitiveMatch,
    /// Already resolved in this session
    SessionCache,
    /// User previously selected this resolution
    UserSelectedBefore,
    /// Fuzzy match (Levenshtein distance)
    FuzzyMatch { distance: usize },
    /// Pronoun resolved from context
    PronounResolution,
    /// Single entity of this type in context (Allianz scope)
    UniqueInContext,
    /// Partial name match
    PartialMatch { matched_portion: String },
}

pub enum ResolutionPath {
    /// Resolved automatically (confidence > threshold)
    Auto,
    /// Needs user confirmation (medium confidence)
    SuggestWithConfirmation,
    /// Needs disambiguation (multiple matches)
    Disambiguation { options: Vec<ResolutionOption> },
    /// Cannot resolve (no matches)
    Failed { reason: String },
}
```

Confidence thresholds:

```rust
const AUTO_RESOLVE_THRESHOLD: f32 = 0.95;      // Just do it
const SUGGEST_THRESHOLD: f32 = 0.80;           // "Did you mean X?"
const DISAMBIGUATE_THRESHOLD: f32 = 0.50;      // Show options
// Below 0.50: "I couldn't find anything matching X"
```

### Layer 4: Contextual Scoping

When working on Allianz, scope resolution to Allianz entities first:

```rust
pub struct ResolutionContext {
    /// Primary scope (e.g., "allianz" onboarding)
    pub scope: Option<EntityScope>,
    
    /// Session bindings (highest priority)
    pub session_bindings: HashMap<String, Uuid>,
    
    /// Conversation focus
    pub focus: ConversationFocus,
    
    /// Resolution preferences from corrections
    pub learned_preferences: HashMap<String, Uuid>,
}

pub struct EntityScope {
    /// Scope type: "client", "fund_family", "jurisdiction"
    pub scope_type: String,
    /// Scope identifier
    pub scope_id: Uuid,
    /// Display name for messages
    pub scope_name: String,
}
```

Resolution priority order:

```
1. Session bindings (@john → exact UUID)
2. Pronoun resolution (him → focus person)
3. Learned preferences (user corrected "John" → specific John before)
4. Scoped search (search within Allianz entities first)
5. Global search (search all entities)
6. Disambiguation (if multiple matches)
```

### Layer 5: Learning from Corrections

When user corrects a resolution, remember it:

```rust
pub struct ResolutionCorrection {
    /// Original search text
    pub search_text: String,
    /// What we suggested
    pub suggested_id: Uuid,
    /// What user selected
    pub selected_id: Uuid,
    /// Context when correction happened
    pub context_scope: Option<EntityScope>,
    /// Timestamp
    pub corrected_at: DateTime<Utc>,
}

impl SessionContext {
    /// Apply a correction and learn from it
    pub fn apply_correction(&mut self, correction: ResolutionCorrection) {
        // Update learned preferences
        self.learned_preferences.insert(
            correction.search_text.to_lowercase(),
            correction.selected_id,
        );
        
        // Update focus
        self.focus.set_focus_for_entity(correction.selected_id);
    }
}
```

Persistence options:

```
SESSION-ONLY (safer, recommended initially):
- Corrections apply only to current session
- Fresh start each conversation
- No cross-session learning

USER-LEVEL (future):
- Corrections persist across sessions for this user
- "John" always means John Smith (1980) for this user
- Privacy implications - needs consent

ORGANIZATIONAL (future):
- Shared corrections across users
- "BNYM" always means "Bank of New York Mellon"
- Requires curation to prevent pollution
```

---

## User Experience Flows

### Flow 1: Pronoun Resolution

```
User: "Create a CBU for Apex Fund with John Smith as director"
      
Agent: ✓ Resolved "Apex Fund" → Apex Alternative Fund (exact match)
       ⚠ Found 3 matches for "John Smith":
         1. John Smith (DOB 1980-01-15) - Director at Apex
         2. John Smith (DOB 1975-03-22) - UBO at Beta Corp
         3. John A. Smith (DOB 1990-07-08) - No current roles
       
       [User selects #1]
       
Agent: Created CBU "Apex Alt" with John Smith (1980) as director.
       Session: @john → John Smith (1980), focus: person

User: "Add him as UBO with 25% ownership"

Agent: ✓ Resolved "him" → John Smith (1980) [pronoun → focus person]
       Adding John Smith as UBO with 25% ownership. Confirm?
```

### Flow 2: Contextual Scoping

```
User: "I'm working on Allianz onboarding"

Agent: ✓ Set scope to Allianz (337 entities)
       Session: scope → Allianz

User: "Add the Munich fund to the CBU"

Agent: ✓ Resolved "Munich fund" → Allianz Munich Property Fund
       [Scoped search: only 1 fund with "Munich" in Allianz context]
       [Global search would find 12 Munich funds]
       
       Adding Allianz Munich Property Fund to CBU. Confirm?
```

### Flow 3: Learning from Corrections

```
User: "Add John to the board"

Agent: "John" matches:
       1. John Smith (1980) - 95% confidence [you selected before]
       2. John Williams (1985) - 75% confidence
       
       Using John Smith (1980). Correct?

User: "No, John Williams"

Agent: ✓ Corrected. Using John Williams.
       Session: learned "John" → John Williams for this conversation
       
       [Later in same session]
       
User: "What roles does John have?"

Agent: ✓ Resolved "John" → John Williams [learned preference]
       John Williams has the following roles...
```

### Flow 4: Confidence Display

```
User: "Add JSmith to the fund"

Agent: Resolved "JSmith" with 72% confidence:
       → John Smith (1980) [partial match: "JSmith" ≈ "J. Smith"]
       
       Is this correct? [Yes] [No, show alternatives]
       
       [If No]
       
Agent: Other matches for "JSmith":
       1. Jane Smith (1982) - 65% confidence
       2. James Smith (1978) - 60% confidence
       3. [Search for someone else]
```

---

## Implementation Phases

### Phase 1: Session Binding Priority (Quick Win)

```rust
// In gateway_resolver.rs or agent_service.rs

fn resolve_entity(
    search_text: &str,
    entity_type: &str,
    session: &AgentSession,
) -> ResolutionResult {
    // 1. Check session bindings first
    if let Some(binding) = session.context.bindings.get(search_text) {
        return ResolutionResult::auto(binding.id, 1.0, "session_binding");
    }
    
    // 2. Check resolution cache (previously resolved in session)
    if let Some(cached) = session.context.resolution_cache.get(&search_text.to_lowercase()) {
        return ResolutionResult::auto(cached.entity_id, 0.98, "session_cache");
    }
    
    // 3. Fall through to EntityGateway search
    // ... existing resolution logic
}
```

### Phase 2: Pronoun Resolution

```rust
fn resolve_pronoun(
    pronoun: &str,
    session: &AgentSession,
) -> Option<ResolutionResult> {
    let gender_filter = match pronoun.to_lowercase().as_str() {
        "him" | "he" | "his" => Some(Gender::Male),
        "her" | "she" | "hers" => Some(Gender::Female),
        "it" | "its" => Some(Gender::Neutral),
        "them" | "they" => None, // Could be any
        _ => return None, // Not a pronoun
    };
    
    // Find most recent entity matching gender (for persons)
    session.context.focus.find_by_gender(gender_filter)
        .map(|e| ResolutionResult::auto(e.entity_id, 0.90, "pronoun_resolution"))
}
```

### Phase 3: Confidence Scoring

Update EntityGateway to return confidence scores with matches.

### Phase 4: Scoped Search

Add scope parameter to EntityGateway queries.

### Phase 5: Correction Learning

Track corrections and apply to future resolutions.

---

## UI Implications

### Chat Display

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  USER: Add him to the Munich fund                                          │
│                                                                             │
│  AGENT: ✓ Resolved:                                                        │
│         • "him" → John Smith [from context]                                │
│         • "Munich fund" → Allianz Munich Property Fund [scoped match]      │
│                                                                             │
│         Adding John Smith to Allianz Munich Property Fund.                 │
│         [Confirm] [Change "him"] [Change fund]                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Resolution Confidence Indicator

```
High confidence (>95%):     ✓ Resolved "Apex" → Apex Fund
Medium confidence (80-95%): ⚡ "JSmith" → John Smith (92%)? [Confirm]
Low confidence (50-80%):    ⚠ "John" → Multiple matches, please select
No match (<50%):            ✗ Couldn't find "XyzCorp"
```

### Correction UI

```
Agent: Using John Smith (1980)
       [✓ Correct] [✗ Wrong - show alternatives]
       
       [If wrong]
       
       Who did you mean?
       ○ John Williams (1985)
       ○ John Davis (1972)  
       ○ Search for someone else: [___________]
```

---

## Data Model Changes

### Session State Additions

```rust
// Add to SessionContext in session.rs

pub struct SessionContext {
    // Existing fields...
    
    /// Resolution cache: search_text.lower() → resolved entity
    #[serde(default)]
    pub resolution_cache: HashMap<String, CachedResolution>,
    
    /// Conversation focus by entity type
    #[serde(default)]
    pub focus: ConversationFocus,
    
    /// Learned preferences from corrections
    #[serde(default)]
    pub learned_preferences: HashMap<String, Uuid>,
    
    /// Current working scope (e.g., Allianz onboarding)
    #[serde(default)]
    pub scope: Option<EntityScope>,
}
```

### EntityGateway Protocol Update

```protobuf
message ResolveRequest {
    string search_text = 1;
    string entity_type = 2;
    optional string scope_id = 3;  // NEW: Scope to specific context
    int32 max_results = 4;
}

message ResolveResponse {
    repeated ResolveMatch matches = 1;
}

message ResolveMatch {
    string entity_id = 1;
    string display_name = 2;
    string entity_type = 3;
    float confidence = 4;           // NEW: Confidence score
    string match_reason = 5;        // NEW: Why this matched
}
```

---

## Success Metrics

1. **Resolution accuracy**: % of auto-resolutions that were correct
2. **Disambiguation rate**: % of resolutions requiring user selection
3. **Correction rate**: % of resolutions user corrected
4. **Pronoun success**: % of pronouns correctly resolved
5. **Session efficiency**: Fewer clicks/selections per operation

---

## References

- Existing: `rust/src/api/session.rs` - SessionContext, bindings
- Existing: `rust/src/dsl_v2/gateway_resolver.rs` - EntityGateway resolution
- Existing: `rust/src/api/agent_service.rs` - Resolution flow
- New: Pronoun resolution logic
- New: Confidence scoring
- New: Correction learning

---

*The goal: Make the AI feel like it knows who you're talking about, not like it's looking things up every time.*
