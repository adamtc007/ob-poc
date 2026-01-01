# TODO: Extended Esper Navigation - 3D/Multi-dimensional Graph Investigation

## Overview

Extend the existing Blade Runner Esper-style navigation vocabulary to handle multi-dimensional entity graph investigation. Deckard was pixel-peeping 2D photographs; our analysts are navigating N-dimensional entity topology including ownership depth, temporal states, relationship layers, and astronomical-scale client books.

---

## Reference Implementation

**Existing Esper verbs location:** 
`/rust/src/session/verb_rag_metadata.rs` lines ~11843-12750

**Current 2D implementation includes:**
- View mode switching (ui.view-kyc, ui.view-trading, etc.)
- Basic zoom (enhance, pull back)
- 2D pan (track left/right/up/down)
- Stop/freeze controls
- Entity focus
- Export ("give me a hard copy")

---

## Task 1: Scale Navigation Vocabulary (Astronomical)

Add to `get_intent_patterns()`:

```rust
// ==========================================================================
// SCALE NAVIGATION - Astronomical metaphor for client book depth
// ==========================================================================

m.insert(
    "ui.scale-universe",
    vec![
        // Astronomical
        "show universe", "full universe", "entire universe",
        "whole universe", "universe view", "see everything",
        "all clients", "full book", "entire book", "client book",
        "30000 foot view", "god view", "total view",
        // Blade Runner style
        "pull back to universe", "show me everything",
        "let me see it all", "the whole picture",
        // Question forms
        "what's in the universe", "how big is the book",
    ],
);

m.insert(
    "ui.scale-galaxy",
    vec![
        // Astronomical
        "show galaxy", "galaxy view", "cluster view",
        "segment view", "client segment", "portfolio view",
        // Segment types
        "hedge fund galaxy", "pension fund cluster",
        "sovereign wealth segment", "family office cluster",
        "asset manager segment", "insurance segment",
        // Blade Runner style
        "zoom to segment", "show me the hedge funds",
        "pull back to segment", "segment level",
        // Navigation
        "which galaxy", "enter galaxy", "explore segment",
    ],
);

m.insert(
    "ui.scale-system",
    vec![
        // Astronomical
        "enter system", "solar system view", "system view",
        "cbu system", "client system", "entity system",
        // CBU focus
        "show client", "focus on cbu", "single client",
        "this cbu only", "cbu with satellites",
        "client and related", "cbu universe",
        // Blade Runner style
        "zoom to client", "enter the system",
        "show me this client", "focus here",
        // Orbital
        "what's orbiting", "satellites", "related entities",
    ],
);

m.insert(
    "ui.scale-planet",
    vec![
        // Astronomical
        "land on", "planet view", "single entity",
        "entity focus", "this entity", "just this one",
        // Focus types
        "land on fund", "land on company", "land on person",
        "focus entity", "zoom to entity", "center on entity",
        // Blade Runner style
        "touch down on", "go to surface", "let me see this",
        // Context
        "examine this", "inspect", "look at this one",
    ],
);

m.insert(
    "ui.scale-surface",
    vec![
        // Astronomical
        "surface scan", "surface view", "on the ground",
        "ground level", "surface detail", "terrain",
        // Entity detail
        "show attributes", "entity details", "all details",
        "documents", "observations", "what's recorded",
        "what do we know", "entity profile", "full profile",
        // Blade Runner style
        "scan the surface", "what's on this",
        "give me the details", "everything about this",
        // Investigation
        "examine", "inspect closely", "forensic view",
    ],
);

m.insert(
    "ui.scale-core",
    vec![
        // Astronomical
        "core sample", "to the core", "deep scan",
        "penetrate", "below surface", "hidden layers",
        // Derived/calculated
        "calculated ubos", "derived ownership", "inferred control",
        "what's hidden", "what's underneath", "buried data",
        "indirect ownership", "indirect control",
        // Blade Runner style
        "dig to the core", "what's really there",
        "show me what's hidden", "beneath the surface",
        // Investigation
        "expose", "reveal", "uncover",
    ],
);
```

---

## Task 2: Depth Navigation Vocabulary (Z-axis / 3D)

Add to `get_intent_patterns()`:

```rust
// ==========================================================================
// DEPTH NAVIGATION - Z-axis through entity structures
// ==========================================================================

m.insert(
    "ui.drill-through",
    vec![
        // Core action
        "drill through", "drill all the way", "penetrate",
        "go through", "pierce", "traverse depth",
        "all the way down", "to the bottom", "to terminus",
        // UBO specific
        "find the humans", "find natural persons",
        "who's really behind this", "ultimate owners",
        "trace to terminus", "follow to end",
        // Blade Runner style
        "punch through", "go deep", "all the way",
        // Investigation
        "follow the money", "trace ownership",
        "who benefits", "cui bono",
    ],
);

m.insert(
    "ui.surface-return",
    vec![
        // Core action
        "surface", "come up", "return to surface",
        "back to top", "top level", "emerge",
        "rise up", "ascend", "float up",
        // Context
        "enough depth", "back out", "come back",
        "leave the depths", "return", "up and out",
        // Blade Runner style  
        "bring me back up", "back to daylight",
    ],
);

m.insert(
    "ui.x-ray",
    vec![
        // Core action
        "x-ray", "x-ray view", "transparent view",
        "see through", "transparency", "skeleton view",
        "structural view", "wireframe", "bones",
        // Investigation
        "show all layers", "reveal structure",
        "expose hierarchy", "show nested",
        "see inside", "internal structure",
        // Blade Runner style
        "give me x-ray", "see through this",
        "what's inside", "show me the skeleton",
    ],
);

m.insert(
    "ui.peel",
    vec![
        // Core action
        "peel", "peel back", "peel layer",
        "one layer", "next layer", "remove layer",
        "unwrap", "unpeel", "strip",
        // Incremental
        "peel one more", "another layer", "keep peeling",
        "layer by layer", "one at a time",
        // Blade Runner style
        "peel it back", "what's under this layer",
        "show me the next layer",
        // Investigation
        "expose next", "reveal one more",
    ],
);

m.insert(
    "ui.cross-section",
    vec![
        // Core action
        "cross section", "horizontal slice", "slice",
        "cut through", "section view", "profile",
        "transverse", "lateral view",
        // At depth
        "slice at this level", "cut here",
        "show this depth", "peers at this level",
        "siblings", "same level entities",
        // Blade Runner style
        "slice it", "cut across",
        "who else is at this level",
    ],
);

m.insert(
    "ui.depth-indicator",
    vec![
        // Query
        "how deep", "what depth", "how many layers",
        "depth check", "level indicator", "where am i",
        "how far down", "depth reading",
        // Context
        "show depth", "display level", "depth marker",
    ],
);
```

---

## Task 3: Orbital/Rotational Navigation

Add to `get_intent_patterns()`:

```rust
// ==========================================================================
// ORBITAL NAVIGATION - Rotating around entities, switching perspectives
// ==========================================================================

m.insert(
    "ui.orbit",
    vec![
        // Core action
        "orbit", "orbit around", "circle around",
        "rotate around", "spin around", "revolve",
        "see all sides", "360 view", "full rotation",
        // Investigation
        "who's connected", "all relationships",
        "what's around this", "connections",
        "related parties", "associations",
        // Blade Runner style
        "orbit this", "show me around it",
        "what's connected to this",
    ],
);

m.insert(
    "ui.rotate-layer",
    vec![
        // Core action
        "rotate to", "switch layer", "flip to",
        "rotate view", "change perspective",
        "different angle", "another view",
        // Layer specific
        "rotate to ownership", "flip to control",
        "switch to services", "rotate to custody",
        "show ownership angle", "control perspective",
        // Blade Runner style
        "spin to ownership", "give me control view",
        "let me see it from services angle",
    ],
);

m.insert(
    "ui.flip",
    vec![
        // Core action
        "flip", "flip view", "invert", "reverse",
        "opposite view", "mirror", "flip perspective",
        // Specific flips
        "flip to upstream", "flip to downstream",
        "who owns vs who's owned",
        "controllers vs controlled",
        "flip direction",
        // Blade Runner style
        "flip it", "show me the other side",
        "reverse the view",
    ],
);

m.insert(
    "ui.tilt",
    vec![
        // Core action
        "tilt", "tilt view", "angle", "skew",
        "lean", "incline", "perspective shift",
        // Dimensional
        "tilt towards time", "tilt to services",
        "angle to ownership", "lean into control",
        // Blade Runner style
        "tilt it", "give me an angle on this",
    ],
);
```

---

## Task 4: Temporal Navigation (4th Dimension)

Add to `get_intent_patterns()`:

```rust
// ==========================================================================
// TEMPORAL NAVIGATION - Time dimension for entity history
// ==========================================================================

m.insert(
    "ui.rewind",
    vec![
        // Core action
        "rewind", "rewind to", "go back to",
        "as of", "at date", "historical",
        "back in time", "time travel", "past state",
        // Date patterns
        "as of december", "at year end", "q3 position",
        "last quarter", "last year", "before the change",
        "when it was", "previous state",
        // Blade Runner style
        "take me back", "show me when",
        "what did it look like then",
        // Investigation
        "before the restructure", "original structure",
        "prior ownership", "historical ubos",
    ],
);

m.insert(
    "ui.time-play",
    vec![
        // Core action
        "play", "play forward", "animate",
        "show changes", "evolve", "progression",
        "time lapse", "history animation",
        // Range
        "play from", "play to", "animate changes",
        "show evolution", "watch it change",
        // Blade Runner style
        "run it forward", "show me how it changed",
        "play the history",
    ],
);

m.insert(
    "ui.time-freeze",
    vec![
        // Core action
        "freeze", "freeze frame", "freeze time",
        "stop time", "lock time", "fix date",
        "hold this moment", "snapshot",
        // Context
        "stay here", "this date", "keep this time",
        "don't move time", "temporal lock",
        // Blade Runner style
        "freeze it there", "hold that date",
    ],
);

m.insert(
    "ui.time-slice",
    vec![
        // Core action
        "time slice", "compare times", "temporal diff",
        "before after", "then vs now", "delta",
        "what changed", "time comparison",
        // Patterns
        "compare q3 q4", "year over year",
        "month on month", "before and after",
        "show the changes", "highlight differences",
        // Blade Runner style
        "show me the difference", "what moved",
        "compare these two points",
        // Investigation
        "what changed hands", "ownership delta",
        "control changes", "who came in who left",
    ],
);

m.insert(
    "ui.time-trail",
    vec![
        // Core action
        "show trail", "history trail", "audit trail",
        "timeline", "chronology", "event history",
        "full history", "complete history",
        // Entity specific
        "entity timeline", "ownership history",
        "control history", "change log",
        "when did this happen", "event sequence",
        // Blade Runner style
        "show me the trail", "what's the history",
        "how did we get here",
    ],
);
```

---

## Task 5: Investigation Pattern Vocabulary

Add to `get_intent_patterns()`:

```rust
// ==========================================================================
// INVESTIGATION PATTERNS - Compound navigation intentions
// ==========================================================================

m.insert(
    "ui.follow-the-money",
    vec![
        // Core pattern
        "follow the money", "trace the money",
        "money trail", "capital flow", "fund flow",
        "where does money go", "where does money come from",
        // Ownership tracing
        "trace ownership", "ownership chain",
        "who owns who", "ownership trail",
        "beneficial ownership", "ubo trace",
        // Blade Runner style
        "show me where the money goes",
        "trace it back", "follow it up",
    ],
);

m.insert(
    "ui.who-controls",
    vec![
        // Core pattern
        "who controls", "who's in charge",
        "who decides", "control trace",
        "decision makers", "power structure",
        "who pulls strings", "real control",
        // Trace
        "trace control", "control chain",
        "voting control", "board control",
        // Blade Runner style
        "who's really running this",
        "show me who's in control",
        "find the puppet master",
    ],
);

m.insert(
    "ui.illuminate",
    vec![
        // Core action
        "illuminate", "highlight", "emphasize",
        "bring out", "show clearly", "make visible",
        "spotlight", "focus light on",
        // Specific highlighting
        "illuminate ownership", "highlight control",
        "show ubos", "emphasize risk",
        "highlight changes", "show problem areas",
        // Blade Runner style
        "light it up", "show me clearly",
        "make it obvious",
    ],
);

m.insert(
    "ui.shadow",
    vec![
        // Core action
        "show shadow", "shadow view", "indirect",
        "derived", "calculated", "inferred",
        "implicit", "behind the scenes",
        // Investigation
        "indirect ownership", "indirect control",
        "derived ubos", "inferred relationships",
        "hidden connections", "implicit links",
        // Blade Runner style
        "what's in shadow", "show me the hidden",
        "what's not obvious",
    ],
);

m.insert(
    "ui.red-flag-scan",
    vec![
        // Core action
        "scan for red flags", "red flag check",
        "risk scan", "anomaly scan", "warning scan",
        "what's wrong", "problems", "issues",
        // Specific scans
        "circular ownership", "suspicious patterns",
        "pep connections", "sanctions exposure",
        "adverse media", "risk indicators",
        // Blade Runner style
        "show me problems", "what should worry me",
        "anything suspicious",
    ],
);

m.insert(
    "ui.black-hole",  // Easter egg for you :)
    vec![
        // Core concept
        "black hole", "show black holes",
        "missing information", "data void",
        "information gap", "unknown",
        // Investigation
        "where are the gaps", "what's missing",
        "incomplete data", "unverified",
        "terminus not reached", "dead ends",
        "can't see past this", "opacity",
        // Blade Runner style
        "where does it go dark",
        "show me what we don't know",
        "find the black holes",
    ],
);
```

---

## Task 6: Context Intention Patterns

Add to `get_intent_patterns()`:

```rust
// ==========================================================================
// CONTEXT INTENTION - User declares purpose before navigation
// ==========================================================================

m.insert(
    "ui.context-review",
    vec![
        // Board/committee review
        "board review", "committee review", "quarterly review",
        "annual review", "periodic review", "regulatory review",
        "audit preparation", "examination prep",
        // Intent
        "i need to review", "preparing for review",
        "getting ready for board", "audit coming up",
        // Output expectation
        "need board pack", "need summary", "need report",
    ],
);

m.insert(
    "ui.context-investigation",
    vec![
        // Investigation mode
        "investigation", "investigating", "forensic",
        "deep dive", "due diligence", "edd",
        "enhanced due diligence", "suspicious activity",
        // Intent
        "something's not right", "need to investigate",
        "looking into", "checking out",
        // Trigger
        "red flag raised", "alert triggered",
        "anomaly detected", "concern about",
    ],
);

m.insert(
    "ui.context-onboarding",
    vec![
        // Onboarding mode
        "onboarding", "new client", "client intake",
        "initial setup", "kyc collection", "client kyc",
        // Intent
        "setting up new client", "onboarding this",
        "need to kyc", "collecting information",
        // Stage
        "initial review", "first look", "getting started",
    ],
);

m.insert(
    "ui.context-monitoring",
    vec![
        // Monitoring mode
        "monitoring", "ongoing monitoring", "pkyc",
        "periodic kyc", "refresh", "event monitoring",
        // Intent
        "checking for changes", "monitoring this client",
        "what's changed", "any updates",
        // Trigger
        "trigger event", "media alert", "screening hit",
    ],
);

m.insert(
    "ui.context-remediation",
    vec![
        // Remediation mode
        "remediation", "fixing", "correcting",
        "gap filling", "completing", "resolving",
        // Intent
        "need to fix", "gaps to fill", "missing data",
        "completing the picture", "finishing up",
        // Priority
        "what's outstanding", "what's blocking",
        "critical gaps", "must fix",
    ],
);
```

---

## Task 7: Workflow Phase Assignments

Add to `get_workflow_phases()`:

```rust
// ==========================================================================
// UI NAVIGATION PHASE
// ==========================================================================
m.insert("ui.view-kyc", "ui_navigation");
m.insert("ui.view-trading", "ui_navigation");
m.insert("ui.view-services", "ui_navigation");
m.insert("ui.view-custody", "ui_navigation");
m.insert("ui.load-cbu", "ui_navigation");

// Scale navigation
m.insert("ui.scale-universe", "ui_navigation");
m.insert("ui.scale-galaxy", "ui_navigation");
m.insert("ui.scale-system", "ui_navigation");
m.insert("ui.scale-planet", "ui_navigation");
m.insert("ui.scale-surface", "ui_navigation");
m.insert("ui.scale-core", "ui_navigation");

// Depth navigation
m.insert("ui.drill-through", "ui_navigation");
m.insert("ui.surface-return", "ui_navigation");
m.insert("ui.x-ray", "ui_navigation");
m.insert("ui.peel", "ui_navigation");
m.insert("ui.cross-section", "ui_navigation");

// Orbital navigation
m.insert("ui.orbit", "ui_navigation");
m.insert("ui.rotate-layer", "ui_navigation");
m.insert("ui.flip", "ui_navigation");
m.insert("ui.tilt", "ui_navigation");

// Temporal navigation
m.insert("ui.rewind", "temporal_navigation");
m.insert("ui.time-play", "temporal_navigation");
m.insert("ui.time-freeze", "temporal_navigation");
m.insert("ui.time-slice", "temporal_navigation");
m.insert("ui.time-trail", "temporal_navigation");

// Investigation patterns
m.insert("ui.follow-the-money", "investigation");
m.insert("ui.who-controls", "investigation");
m.insert("ui.illuminate", "investigation");
m.insert("ui.shadow", "investigation");
m.insert("ui.red-flag-scan", "investigation");
m.insert("ui.black-hole", "investigation");

// Context intentions
m.insert("ui.context-review", "context_setting");
m.insert("ui.context-investigation", "context_setting");
m.insert("ui.context-onboarding", "context_setting");
m.insert("ui.context-monitoring", "context_setting");
m.insert("ui.context-remediation", "context_setting");

// Basic zoom/pan (existing)
m.insert("ui.zoom-in", "ui_navigation");
m.insert("ui.zoom-out", "ui_navigation");
m.insert("ui.zoom-fit", "ui_navigation");
m.insert("ui.pan-left", "ui_navigation");
m.insert("ui.pan-right", "ui_navigation");
m.insert("ui.pan-up", "ui_navigation");
m.insert("ui.pan-down", "ui_navigation");
m.insert("ui.center", "ui_navigation");
m.insert("ui.stop", "ui_navigation");
m.insert("ui.focus-entity", "ui_navigation");
m.insert("ui.drill-down", "ui_navigation");
m.insert("ui.drill-up", "ui_navigation");
m.insert("ui.export", "ui_navigation");
```

---

## Task 8: Graph Context Assignments

Add to `get_graph_contexts()`:

```rust
// ==========================================================================
// LAYER: UI NAVIGATION (global - applicable in all graph views)
// ==========================================================================
m.insert(
    "layer_ui_navigation",
    vec![
        // View switching
        "ui.view-kyc",
        "ui.view-trading",
        "ui.view-services",
        "ui.view-custody",
        "ui.load-cbu",
        
        // Scale
        "ui.scale-universe",
        "ui.scale-galaxy",
        "ui.scale-system",
        "ui.scale-planet",
        "ui.scale-surface",
        "ui.scale-core",
        
        // Depth
        "ui.drill-through",
        "ui.surface-return",
        "ui.x-ray",
        "ui.peel",
        "ui.cross-section",
        
        // Orbital
        "ui.orbit",
        "ui.rotate-layer",
        "ui.flip",
        "ui.tilt",
        
        // Temporal
        "ui.rewind",
        "ui.time-play",
        "ui.time-freeze",
        "ui.time-slice",
        "ui.time-trail",
        
        // Investigation
        "ui.follow-the-money",
        "ui.who-controls",
        "ui.illuminate",
        "ui.shadow",
        "ui.red-flag-scan",
        "ui.black-hole",
        
        // Basic nav
        "ui.zoom-in",
        "ui.zoom-out",
        "ui.zoom-fit",
        "ui.pan-left",
        "ui.pan-right",
        "ui.pan-up",
        "ui.pan-down",
        "ui.center",
        "ui.stop",
        "ui.focus-entity",
        "ui.drill-down",
        "ui.drill-up",
        "ui.export",
    ],
);

// ==========================================================================
// LAYER: CONTEXT SETTING (pre-navigation intent)
// ==========================================================================
m.insert(
    "layer_context_setting",
    vec![
        "ui.context-review",
        "ui.context-investigation",
        "ui.context-onboarding",
        "ui.context-monitoring",
        "ui.context-remediation",
    ],
);
```

---

## Task 9: Typical Next Flows

Add to `get_typical_next()`:

```rust
// ==========================================================================
// CONTEXT → NAVIGATION FLOWS
// ==========================================================================
m.insert(
    "ui.context-review",
    vec![
        "ui.load-cbu",
        "ui.scale-system",
        "ui.view-kyc",
        "ui.time-slice",
    ],
);
m.insert(
    "ui.context-investigation",
    vec![
        "ui.load-cbu",
        "ui.view-kyc",
        "ui.drill-through",
        "ui.follow-the-money",
        "ui.red-flag-scan",
    ],
);
m.insert(
    "ui.context-onboarding",
    vec![
        "ui.load-cbu",
        "ui.view-kyc",
        "ui.scale-system",
        "ui.black-hole",
    ],
);
m.insert(
    "ui.context-monitoring",
    vec![
        "ui.load-cbu",
        "ui.time-slice",
        "ui.red-flag-scan",
    ],
);

// ==========================================================================
// SCALE NAVIGATION FLOWS
// ==========================================================================
m.insert(
    "ui.scale-universe",
    vec!["ui.scale-galaxy", "ui.filter"],
);
m.insert(
    "ui.scale-galaxy",
    vec!["ui.scale-system", "ui.scale-universe"],
);
m.insert(
    "ui.scale-system",
    vec!["ui.scale-planet", "ui.scale-galaxy", "ui.orbit"],
);
m.insert(
    "ui.scale-planet",
    vec!["ui.scale-surface", "ui.drill-down", "ui.orbit"],
);
m.insert(
    "ui.scale-surface",
    vec!["ui.scale-core", "ui.drill-through", "ui.scale-planet"],
);

// ==========================================================================
// DEPTH NAVIGATION FLOWS
// ==========================================================================
m.insert(
    "ui.drill-through",
    vec!["ui.focus-entity", "ui.surface-return", "ui.export"],
);
m.insert(
    "ui.x-ray",
    vec!["ui.focus-entity", "ui.drill-through", "ui.peel"],
);
m.insert(
    "ui.peel",
    vec!["ui.peel", "ui.drill-through", "ui.surface-return"],
);

// ==========================================================================
// INVESTIGATION FLOWS
// ==========================================================================
m.insert(
    "ui.follow-the-money",
    vec!["ui.drill-through", "ui.focus-entity", "ui.export"],
);
m.insert(
    "ui.who-controls",
    vec!["ui.drill-through", "ui.focus-entity", "ui.export"],
);
m.insert(
    "ui.red-flag-scan",
    vec!["ui.focus-entity", "ui.illuminate", "ui.drill-through"],
);
m.insert(
    "ui.black-hole",
    vec!["ui.focus-entity", "ui.drill-through"],
);

// ==========================================================================
// TEMPORAL FLOWS
// ==========================================================================
m.insert(
    "ui.time-slice",
    vec!["ui.illuminate", "ui.focus-entity", "ui.export"],
);
m.insert(
    "ui.rewind",
    vec!["ui.time-freeze", "ui.time-play"],
);
m.insert(
    "ui.time-play",
    vec!["ui.time-freeze", "ui.stop"],
);

// ==========================================================================
// OUTPUT FLOWS
// ==========================================================================
m.insert(
    "ui.export",
    vec!["ui.scale-system", "ui.view-kyc"],  // Return to navigation after export
);
```

---

## Task 10: Update Tests

Add to test module:

```rust
#[test]
fn test_esper_3d_navigation_patterns() {
    let patterns = get_intent_patterns();
    
    // Scale navigation
    assert!(patterns.contains_key("ui.scale-universe"));
    assert!(patterns.contains_key("ui.scale-planet"));
    
    // Depth navigation
    assert!(patterns.contains_key("ui.drill-through"));
    assert!(patterns.contains_key("ui.x-ray"));
    
    // Temporal navigation
    assert!(patterns.contains_key("ui.rewind"));
    assert!(patterns.contains_key("ui.time-slice"));
    
    // Investigation patterns
    assert!(patterns.contains_key("ui.follow-the-money"));
    assert!(patterns.contains_key("ui.black-hole"));
}

#[test]
fn test_esper_blade_runner_commands() {
    let matches = find_verbs_by_intent("enhance");
    assert!(!matches.is_empty());
    
    let matches = find_verbs_by_intent("give me a hard copy");
    assert!(!matches.is_empty());
    
    let matches = find_verbs_by_intent("follow the money");
    let verbs: Vec<&str> = matches.iter().map(|(v, _)| *v).collect();
    assert!(verbs.contains(&"ui.follow-the-money"));
}

#[test]
fn test_context_intention_flows() {
    let next = suggest_next("ui.context-investigation");
    assert!(next.contains(&"ui.drill-through"));
    assert!(next.contains(&"ui.follow-the-money"));
}
```

---

## Implementation Notes

1. **File to modify:** `/rust/src/session/verb_rag_metadata.rs`

2. **Insert locations:**
   - Intent patterns: After existing UI verbs (line ~12750)
   - Workflow phases: In `get_workflow_phases()` function
   - Graph contexts: In `get_graph_contexts()` function
   - Typical next: In `get_typical_next()` function
   - Tests: In test module at end of file

3. **Maintain consistency with existing patterns:**
   - Use same vec! format
   - Include "Blade Runner style" comment sections
   - Include "UK/US colloquialisms" sections
   - Include "Question forms" sections

4. **Black holes note:** Yes, I included `ui.black-hole` - it's perfect for showing data gaps/opacity in the ownership structure. Every investigation has black holes.

---

## Acceptance Criteria

- [ ] All scale navigation verbs implemented with intent patterns
- [ ] All depth navigation verbs implemented
- [ ] All orbital/rotational verbs implemented
- [ ] All temporal verbs implemented  
- [ ] All investigation pattern verbs implemented
- [ ] Context intention verbs implemented
- [ ] Workflow phases assigned to all new verbs
- [ ] Graph contexts include all new verbs
- [ ] Typical next flows defined for navigation sequences
- [ ] Tests pass
- [ ] File compiles without errors

---

## Future Considerations

1. **Voice integration:** These patterns designed for natural language input - could integrate with speech-to-text for true Esper experience

2. **Animation choreography:** Scale and temporal commands should trigger smooth animated transitions

3. **Context persistence:** Context intentions should persist across navigation, affecting how data is presented

4. **Keyboard shortcuts:** Map common patterns to key combinations (e.g., `Cmd+E` for enhance)

5. **Command history:** "Computer, show me what I just did" - navigation audit trail
