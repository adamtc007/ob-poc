# TODO: Update Investigation Vocabulary - Matrix Theme

## Overview

Replace "follow the money" with "follow the white rabbit" (Matrix reference) and add "dive into" as a distinct exploration verb. The white rabbit metaphor better fits UBO investigation - tracing through layers of obscurity to find hidden truth, not cash flow tracing.

---

## Rationale

| Term | Problem | Better Fit |
|------|---------|------------|
| "Follow the money" | Implies cash flow, forensic accounting | UBO investigation is about ownership chains, not money movement |
| "Follow the white rabbit" | Matrix = discovering hidden layers of reality | Perfect for "how deep does this go?" UBO tracing |
| "Dive into" | N/A (new) | General depth exploration, entering an entity's structure |

---

## Task 1: Update verb_rag_metadata.rs - Replace Investigation Verb

**File:** `/rust/src/session/verb_rag_metadata.rs`

### 1.1 Rename and Update Intent Patterns

**FIND:**
```rust
m.insert(
    "ui.follow-the-money",
    vec![
        // Core pattern
        "follow the money", "trace the money",
        // ... existing patterns
    ],
);
```

**REPLACE WITH:**
```rust
m.insert(
    "ui.follow-the-rabbit",
    vec![
        // Matrix references
        "follow the white rabbit",
        "follow the rabbit",
        "white rabbit",
        "rabbit hole",
        "down the rabbit hole",
        "how deep does this go",
        "how far down",
        "take me down",
        "show me how deep",
        "show me how far",
        
        // Investigation intent
        "trace to terminus",
        "find the end",
        "find the humans",
        "who's really behind this",
        "ultimate owners",
        "trace ownership",
        "ownership chain",
        "who owns who",
        "beneficial ownership",
        "ubo trace",
        
        // Blade Runner crossover
        "show me where it leads",
        "trace it back",
        "follow it through",
        
        // UK/US colloquialisms
        "let's see where this goes",
        "take me all the way",
        "go to the bottom",
        "get to the bottom of this",
    ],
);
```

### 1.2 Add New "Dive Into" Verb

**ADD** (after ui.follow-the-rabbit):
```rust
m.insert(
    "ui.dive-into",
    vec![
        // Core action
        "dive into",
        "dive in",
        "deep dive",
        "go deep",
        "submerge",
        "immerse",
        "plunge into",
        "plunge in",
        
        // Exploration intent
        "explore",
        "explore this",
        "let's explore",
        "examine closely",
        "look inside",
        "get into",
        "dig into",
        "dig in",
        
        // Entity targeting
        "dive into entity",
        "dive into this",
        "deep dive on",
        "explore entity",
        
        // UK/US colloquialisms
        "let's have a look",
        "let's dig in",
        "crack this open",
        "open this up",
        "get stuck in",  // UK
        "get into it",
    ],
);
```

---

## Task 2: Update Workflow Phases

**File:** `/rust/src/session/verb_rag_metadata.rs` - `get_workflow_phases()`

**FIND AND REPLACE:**
```rust
// OLD
m.insert("ui.follow-the-money", "investigation");

// NEW
m.insert("ui.follow-the-rabbit", "investigation");
m.insert("ui.dive-into", "investigation");
```

---

## Task 3: Update Graph Contexts

**File:** `/rust/src/session/verb_rag_metadata.rs` - `get_graph_contexts()`

**FIND in layer_ui_navigation vec:**
```rust
"ui.follow-the-money",
```

**REPLACE WITH:**
```rust
"ui.follow-the-rabbit",
"ui.dive-into",
```

---

## Task 4: Update Typical Next Flows

**File:** `/rust/src/session/verb_rag_metadata.rs` - `get_typical_next()`

**FIND AND REPLACE:**
```rust
// OLD
m.insert(
    "ui.follow-the-money",
    vec!["ui.drill-through", "ui.focus-entity", "ui.export"],
);

// NEW
m.insert(
    "ui.follow-the-rabbit",
    vec!["ui.drill-through", "ui.focus-entity", "ui.export", "ui.black-hole"],
);

m.insert(
    "ui.dive-into",
    vec!["ui.drill-down", "ui.scale-surface", "ui.orbit", "ui.x-ray"],
);
```

**ALSO UPDATE context flows:**
```rust
// FIND
m.insert(
    "ui.context-investigation",
    vec![
        "ui.load-cbu",
        "ui.view-kyc",
        "ui.drill-through",
        "ui.follow-the-money",  // ← OLD
        "ui.red-flag-scan",
    ],
);

// REPLACE WITH
m.insert(
    "ui.context-investigation",
    vec![
        "ui.load-cbu",
        "ui.view-kyc",
        "ui.drill-through",
        "ui.follow-the-rabbit",  // ← NEW
        "ui.dive-into",          // ← ADD
        "ui.red-flag-scan",
    ],
);
```

---

## Task 5: Update Deepgram Keywords

**File:** `/src/voice/DeepgramProvider.ts` (when implemented)

**UPDATE KEYWORDS array:**

**REMOVE:**
```typescript
'follow the money',
```

**ADD:**
```typescript
// Matrix investigation
'follow the white rabbit',
'follow the rabbit', 
'white rabbit',
'rabbit hole',
'how deep does this go',

// Dive exploration
'dive into',
'dive in',
'deep dive',
'go deep',
```

---

## Task 6: Update TODO-esper-navigation-3d.md

**File:** `/TODO-esper-navigation-3d.md`

### 6.1 Replace Investigation Patterns Section

**FIND the section starting with:**
```rust
m.insert(
    "ui.follow-the-money",
```

**REPLACE entire block with:**
```rust
m.insert(
    "ui.follow-the-rabbit",
    vec![
        // Matrix references - going deeper to find hidden truth
        "follow the white rabbit",
        "follow the rabbit",
        "white rabbit",
        "rabbit hole",
        "down the rabbit hole",
        "how deep does this go",
        "how far down",
        "take me down",
        "show me how deep",
        
        // UBO investigation
        "trace to terminus",
        "find the humans",
        "who's really behind this",
        "ultimate owners",
        "trace ownership",
        "ownership chain",
        "beneficial ownership",
        
        // Blade Runner style
        "show me where it leads",
        "trace it back",
        "follow it through",
    ],
);

m.insert(
    "ui.dive-into",
    vec![
        // Core action
        "dive into",
        "dive in", 
        "deep dive",
        "go deep",
        "submerge",
        "plunge into",
        
        // Exploration
        "explore",
        "examine closely",
        "look inside",
        "dig into",
        "dig in",
        
        // UK/US
        "let's have a look",
        "crack this open",
        "get stuck in",
    ],
);
```

---

## Task 7: Update TODO-deepgram-voice-integration.md

**File:** `/TODO-deepgram-voice-integration.md`

### 7.1 Update Keywords List in DeepgramProvider

**FIND in KEYWORDS array:**
```typescript
'follow the money',
```

**REPLACE section with:**
```typescript
// Investigation - Matrix theme
'follow the white rabbit', 'follow the rabbit', 'white rabbit',
'rabbit hole', 'how deep does this go',

// Dive exploration  
'dive into', 'dive in', 'deep dive', 'go deep',
```

### 7.2 Update Manual Test Script

**FIND:**
```markdown
- [ ] "Follow the money" → ownership trace
```

**REPLACE WITH:**
```markdown
- [ ] "Follow the white rabbit" → ownership trace to terminus
- [ ] "Dive into [entity]" → explore entity depth
```

---

## Task 8: Update Tests

**File:** `/rust/src/session/verb_rag_metadata.rs` (test module)

**ADD/UPDATE tests:**
```rust
#[test]
fn test_matrix_investigation_commands() {
    let matches = find_verbs_by_intent("follow the white rabbit");
    let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
    assert!(verbs.contains(&"ui.follow-the-rabbit"));
    
    let matches = find_verbs_by_intent("rabbit hole");
    let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
    assert!(verbs.contains(&"ui.follow-the-rabbit"));
}

#[test]
fn test_dive_into_commands() {
    let matches = find_verbs_by_intent("dive into");
    let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
    assert!(verbs.contains(&"ui.dive-into"));
    
    let matches = find_verbs_by_intent("deep dive");
    let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
    assert!(verbs.contains(&"ui.dive-into"));
}

#[test]
fn test_follow_the_money_removed() {
    // Ensure old verb no longer exists
    let matches = find_verbs_by_intent("follow the money");
    let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
    assert!(!verbs.contains(&"ui.follow-the-money"));
    // But intent might fuzzy-match to new verb
}
```

---

## Summary of Changes

| Location | Change |
|----------|--------|
| `verb_rag_metadata.rs` - intent patterns | `ui.follow-the-money` → `ui.follow-the-rabbit` |
| `verb_rag_metadata.rs` - intent patterns | ADD `ui.dive-into` |
| `verb_rag_metadata.rs` - workflow phases | Update verb references |
| `verb_rag_metadata.rs` - graph contexts | Update verb references |
| `verb_rag_metadata.rs` - typical next | Update verb references + add dive flows |
| `TODO-esper-navigation-3d.md` | Update investigation section |
| `TODO-deepgram-voice-integration.md` | Update keyword list |
| Tests | Add Matrix/dive tests, remove old tests |

---

## Thematic Consistency

The vocabulary now has three clear cinematic references:

| Film | Commands | Investigation Style |
|------|----------|---------------------|
| **Blade Runner** | enhance, track, pan, hard copy | Forensic examination, surface analysis |
| **The Matrix** | follow the white rabbit, rabbit hole | Depth investigation, finding hidden truth |
| **2001/General Sci-Fi** | orbit, x-ray, black hole | Structural navigation, system analysis |

---

## Acceptance Criteria

- [ ] `ui.follow-the-money` completely removed from codebase
- [ ] `ui.follow-the-rabbit` implemented with Matrix-themed patterns
- [ ] `ui.dive-into` implemented as separate exploration verb
- [ ] All workflow phases updated
- [ ] All graph contexts updated  
- [ ] All typical next flows updated
- [ ] Deepgram keywords updated
- [ ] Tests pass
- [ ] "Follow the white rabbit" voice command works in demo
