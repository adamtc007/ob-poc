# 038: Solar Navigation - Unified Design Spec

> **Status:** DRAFT v4 - Implementation-ready (final review complete)
> **Date:** 2026-01-19
> **Scope:** Session state, ESPER navigation verbs, solar layout engine
> **Reviewed by:** ChatGPT (2026-01-19) - Three rounds of review
> **Verdict:** ✅ Ready for implementation

---

## Executive Summary

This document proposes a unified navigation system for the OB-POC visualization layer. The core insight is that **navigation, layout, and session state are one system** - not three separate concerns bolted together.

The design uses an astronomical metaphor (Universe → Galaxy → Solar System → Planet) that maps naturally to our domain hierarchy (All Books → Client Book → ManCo + CBUs → Single CBU). Navigation commands (ESPER verbs) mutate session state, and the layout engine renders based on that state.

Key innovation: **View state snapshots** enable time-travel navigation (back/forward/rewind) through the user's exploration history.

---

## Non-Goals (Explicit Exclusions)

To keep v1 focused:

1. **Not continuous zoom** - Only snap-to-level zoom between discrete levels
2. **Not free pan as primary nav** - Semantic verbs are the navigation API, not drag/scroll
3. **Not force-directed layout at v1** - Deterministic grid/orbital layouts only
4. **Not persistent history** - Navigation history lives in memory, clears on session end
5. **Not animated transitions at v1** - Instant snaps; animations added later for polish

---

## Determinism Contract

These invariants MUST hold for the system to work correctly:

1. **Stable ordering** - CBU/ManCo lists use deterministic ORDER BY (jurisdiction, name, uuid)
2. **Layout is pure** - `layout(scope, view_state_key) → positions` with no hidden state
3. **View state identity** - Two `ViewState`s with same `ViewStateKey` are semantically identical
4. **History dedupe** - Pushing identical state (by key) is a no-op
5. **Transitions are validated** - Invalid level transitions return error, never corrupt state

---

## Part 1: Design Thinking & Rationale

### 1.1 The Problem with Current Approach

The current codebase has:
- **Layout code** (`layout.rs`, `layout_v2.rs`) - template-based slot positioning for entities within a single CBU
- **ESPER commands** (`esper-commands.yaml`) - zoom/pan/drill commands that manipulate viewport
- **Session manager** - tracks CBU scope but not view state

These are disconnected:
- Layout doesn't know about navigation state
- ESPER commands don't understand the domain hierarchy
- Session tracks *what's loaded* but not *what's visible/focused*

**Result:** Navigation feels mechanical (zoom percentages, pan pixels) rather than semantic (enter this ManCo, land on this CBU).

### 1.2 The Insight: CBUs Are Landmarks, Not Pixels

Traditional graph visualization treats nodes as geometric objects to pan/zoom around. But CBUs are **discrete business entities** with meaning:

- You don't "pan right 200 pixels" - you "hop to the next fund"
- You don't "zoom to 50%" - you "step back to see all ManCos"
- You don't "scroll down" - you "drill into this CBU's entities"

**The astronomical metaphor captures this:**

| Astro Concept | Domain Concept | What You See |
|---------------|----------------|--------------|
| Universe | All client books | Multiple galaxies |
| Galaxy | One client's book (Allianz) | Multiple solar systems |
| Solar System | One ManCo + its CBUs | Sun + orbiting planets |
| Planet | One CBU | Expanded CBU view |
| Surface/Entity Graph | Entities within CBU | Directors, UBOs, etc. |

This isn't just a metaphor - it's a **navigation model** where each level has different:
- Visibility rules (what's shown/hidden)
- Navigation semantics (what "next" means)
- Interaction affordances (what you can do)

### 1.3 Why Snap Zoom, Not Continuous Zoom

Continuous zoom creates problems:
- At 73% zoom, what do you show? Full CBU names? Abbreviated? Icons?
- User can get "lost" at arbitrary zoom levels
- No clear mental model of "where am I"

**Snap zoom** (discrete levels) provides:
- Clear mental model: "I'm at Galaxy level" vs "I'm inside Lux ManCo"
- Predictable visibility: at each level, we know exactly what to render
- Semantic navigation: "zoom out" means "go up one level", not "decrease zoom 20%"

The levels are:

```
Level 0: UNIVERSE
         Multiple galaxies (client books)
         Rare - most users work within one book
         
Level 1: GALAXY  
         Multiple solar systems (ManCos)
         ManCo icons with region flags, CBU count badges
         CBUs hidden (represented by ManCo's "halo")
         
Level 2: SOLAR SYSTEM
         One ManCo as sun + CBUs in orbit(s)
         CBUs visible as icons with names + flags
         Navigation: clockwise/counter-clockwise, inner/outer orbit
         
Level 3: PLANET (Single CBU)
         One CBU fills viewport
         Ready to drill into entities
         
Level 4: SURFACE (Entity Graph)
         Entities within CBU expanded
         Traditional graph layout (existing code)
```

### 1.4 Why Navigation History Matters

Users explore non-linearly:
1. Start at Galaxy (see all ManCos)
2. Enter Lux ManCo (see 50 CBUs)
3. Hop through several CBUs
4. Land on one, drill into entities
5. "Wait, which CBU was that two steps ago?"

**Without history:** User must manually re-navigate ("orbit... hop back... hop back...")

**With history snapshots:** 
- "back" - instantly restore previous view state
- "forward" - go forward if you went back too far
- "rewind" - start over from beginning
- "show history" - see breadcrumb trail

This is like browser back/forward but for graph navigation.

### 1.5 Why ESPER Verbs Are the API

We could expose layout/zoom as low-level APIs:
```javascript
// Low-level (bad)
viewport.setZoom(0.5);
viewport.panTo(x: 100, y: 200);
layout.setLevel("galaxy");
```

But this:
- Requires client to understand layout internals
- Doesn't capture user intent
- Can't be voice-controlled naturally
- Doesn't integrate with session history

**ESPER verbs are the semantic API:**
```
"enter Lux"         → I want to see inside Lux ManCo
"land on AGI Fund"  → I want to focus on this specific CBU
"orbit"             → I want to go back up to system view
"back"              → I want to undo my last navigation
```

The verb captures **intent**. The system figures out:
- What level change is needed
- What layout to render
- What snapshot to push/pop
- What animation to play

---

## Part 2: Session State Model (Revised per ChatGPT Review)

### 2.1 Scope vs View (Invariant)

**Scope** = Data loaded/available for navigation (set by DSL / backend loading)
- `cbu_ids`: All CBUs available to navigate
- `manco_ids`: All ManCos available to navigate  
- `named_refs`: Entity resolution cache

**View** = What the user is currently looking at (presentation state)
- Discrete level: Galaxy/System/Planet/Surface
- Focused IDs + orbit position
- Must always be valid for its level

**Invariant:** You can have 100 CBUs in scope and view exactly 1. "Scope" must never imply "visible".

### 2.2 View State (Semantic, Snapshot-able)

View state is semantic and closed (no "optional soup"). Invalid combinations are prevented by constructors and validated in debug builds.

```rust
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewLevel {
    Universe, // rarely used
    Galaxy,   // multiple ManCos visible, CBUs hidden
    System,   // one ManCo + CBUs in orbit(s)
    Planet,   // one CBU focused
    Surface,  // entities within CBU (aka "Entity Graph")
}

/// Canonical orbit position for System-level navigation.
/// This is the authoritative "cursor" when in System view.
/// focus_cbu_id is DERIVED from this, not the other way around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbitPos {
    pub ring: usize,
    pub index: usize,
}

/// Complete semantic view state.
/// NOTE: timestamp is metadata only, excluded from identity checks.
#[derive(Debug, Clone)]
pub struct ViewState {
    pub level: ViewLevel,

    // Focus for Galaxy/System views
    pub focus_manco_id: Option<Uuid>,

    // Focus for Planet/Surface views
    pub focus_cbu_id: Option<Uuid>,

    // System-level position cursor (canonical for "clockwise/inner/outer")
    // When at System level, this is the source of truth for "current CBU"
    pub orbit_pos: Option<OrbitPos>,

    // Metadata only (not used in equality / identity checks)
    pub timestamp: DateTime<Utc>,
}

impl ViewState {
    /// A stable identity key used for history dedupe.
    /// Ignores timestamp to avoid infinite "same state" pushes.
    pub fn key(&self) -> ViewStateKey {
        ViewStateKey {
            level: self.level,
            focus_manco_id: self.focus_manco_id,
            focus_cbu_id: self.focus_cbu_id,
            orbit_pos: self.orbit_pos,
        }
    }

    /// Debug-only invariant checks (call in dev builds and tests).
    pub fn validate(&self) -> Result<(), String> {
        match self.level {
            ViewLevel::Universe => Ok(()),
            ViewLevel::Galaxy => {
                // At galaxy, you may or may not have a selected ManCo, but no orbit cursor.
                if self.orbit_pos.is_some() {
                    return Err("Galaxy view must not have orbit_pos".into());
                }
                Ok(())
            }
            ViewLevel::System => {
                if self.focus_manco_id.is_none() {
                    return Err("System view requires focus_manco_id".into());
                }
                // orbit_pos is optional: allow "entered system but not selected any CBU yet"
                Ok(())
            }
            ViewLevel::Planet => {
                if self.focus_cbu_id.is_none() {
                    return Err("Planet view requires focus_cbu_id".into());
                }
                Ok(())
            }
            ViewLevel::Surface => {
                if self.focus_cbu_id.is_none() {
                    return Err("Surface view requires focus_cbu_id".into());
                }
                Ok(())
            }
        }
    }

    // Constructors prevent invalid combinations
    
    pub fn galaxy(now: DateTime<Utc>) -> Self {
        Self {
            level: ViewLevel::Galaxy,
            focus_manco_id: None,
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp: now,
        }
    }

    pub fn system(manco_id: Uuid, now: DateTime<Utc>) -> Self {
        Self {
            level: ViewLevel::System,
            focus_manco_id: Some(manco_id),
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp: now,
        }
    }
    
    pub fn system_at(manco_id: Uuid, orbit_pos: OrbitPos, now: DateTime<Utc>) -> Self {
        Self {
            level: ViewLevel::System,
            focus_manco_id: Some(manco_id),
            focus_cbu_id: None, // Derived from orbit_pos via layout
            orbit_pos: Some(orbit_pos),
            timestamp: now,
        }
    }

    /// Create Planet view state.
    /// `parent_manco_id` is optional but recommended for clean zoom_out behavior.
    pub fn planet(cbu_id: Uuid, parent_manco_id: Option<Uuid>, now: DateTime<Utc>) -> Self {
        Self {
            level: ViewLevel::Planet,
            focus_manco_id: parent_manco_id, // Retained for zoom_out to System
            focus_cbu_id: Some(cbu_id),
            orbit_pos: None,
            timestamp: now,
        }
    }

    /// Convenience constructor when parent ManCo is unknown
    pub fn planet_simple(cbu_id: Uuid, now: DateTime<Utc>) -> Self {
        Self::planet(cbu_id, None, now)
    }

    pub fn surface(cbu_id: Uuid, now: DateTime<Utc>) -> Self {
        Self {
            level: ViewLevel::Surface,
            focus_manco_id: None,
            focus_cbu_id: Some(cbu_id),
            orbit_pos: None,
            timestamp: now,
        }
    }
}

/// Identity for history + dedupe (timestamp excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewStateKey {
    pub level: ViewLevel,
    pub focus_manco_id: Option<Uuid>,
    pub focus_cbu_id: Option<Uuid>,
    pub orbit_pos: Option<OrbitPos>,
}
```

**Key design decision:** At System level, `orbit_pos` (ring, index) is **canonical**. The `focus_cbu_id` is **derived** from orbit position via the layout engine. This eliminates ambiguity where navigation changes `orbit_index` but `focus_cbu_id` stays stale.

### 2.3 Navigation History (Correct, Dedupe, Time-Travel)

Uses `VecDeque` for efficient front removal. Cursor is `Option<usize>` to handle empty state. Dedupe prevents spam from repeated commands.

```rust
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct NavigationHistory {
    snapshots: VecDeque<ViewState>,
    cursor: Option<usize>,
    max_size: usize,
}

impl NavigationHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            snapshots: VecDeque::new(),
            cursor: None,
            max_size: max_size.max(1),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn current(&self) -> Option<&ViewState> {
        self.cursor.and_then(|i| self.snapshots.get(i))
    }

    /// Pushes a state *only if it meaningfully changes* the semantic view.
    /// - Truncates "future" if we had navigated back.
    /// - Enforces max size by popping from the front.
    /// - Cursor always ends pointing at the newly pushed state.
    pub fn push_if_changed(&mut self, state: ViewState) {
        // Optional: validate in debug builds
        debug_assert!(state.validate().is_ok());

        // Dedupe: if same semantic state, do nothing.
        if let Some(cur) = self.current() {
            if cur.key() == state.key() {
                return;
            }
        }

        // If cursor is not at end, drop forward history.
        if let Some(c) = self.cursor {
            let keep_len = c + 1;
            while self.snapshots.len() > keep_len {
                self.snapshots.pop_back();
            }
        }

        self.snapshots.push_back(state);
        self.cursor = Some(self.snapshots.len() - 1);

        // Enforce max size; adjust cursor if we drop the front.
        while self.snapshots.len() > self.max_size {
            self.snapshots.pop_front();
            if let Some(c) = self.cursor {
                self.cursor = Some(c.saturating_sub(1));
            }
        }
    }

    pub fn back(&mut self) -> Option<&ViewState> {
        let c = self.cursor?;
        if c == 0 {
            return None;
        }
        self.cursor = Some(c - 1);
        self.current()
    }

    pub fn forward(&mut self) -> Option<&ViewState> {
        let c = self.cursor?;
        if c + 1 >= self.snapshots.len() {
            return None;
        }
        self.cursor = Some(c + 1);
        self.current()
    }

    pub fn rewind(&mut self) -> Option<&ViewState> {
        if self.snapshots.is_empty() {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(0);
        self.current()
    }

    pub fn jump_to(&mut self, index: usize) -> Option<&ViewState> {
        if index >= self.snapshots.len() {
            return None;
        }
        self.cursor = Some(index);
        self.current()
    }

    /// Breadcrumbs for UI (index + key fields).
    pub fn breadcrumbs(&self) -> Vec<(usize, ViewStateKey)> {
        self.snapshots
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.key()))
            .collect()
    }
}
```

**Fixes from ChatGPT review:**
- Cursor remains correct when `max_size` forces dropping the front (fixes drift bug)
- Forward history is truncated when pushing after "back" (browser semantics)
- `push_if_changed()` prevents spam states (timestamp-only changes don't pollute history)
- `cursor: Option<usize>` handles "empty history" safely

### 2.4 Session Context (Extended)

```rust
use std::collections::{HashMap, HashSet};

pub struct SessionContext {
    // --- Existing ---
    pub cbu_ids: HashSet<Uuid>,              // scope
    pub active_cbu: Option<CbuInfo>,
    pub named_refs: HashMap<String, Uuid>,

    // --- Added ---
    pub manco_ids: HashSet<Uuid>,            // scope
    pub view: ViewState,                      // semantic view state
    pub nav_history: NavigationHistory,       // time-travel history
}
```

**Operational rule:** Every successful navigation verb handler should:
1. Compute the next valid `ViewState`
2. Call `nav_history.push_if_changed(next_state)`
3. Set `session.view = next_state`

This makes "back always works" provable and keeps layout a pure function of `(scope, view)`.

### 2.5 Render Transform (Separate from View State)

Even with snap zoom, we still need a transform for:
- Centering a selected planet
- Animations (when added)
- Panning the galaxy grid if there are many ManCos

Keep two layers:
- **Semantic view state** (`ViewState`) - persisted, snapshotted, history-tracked
- **Render transform** (computed, not persisted) - center, scale, interpolation_progress

```rust
/// Computed render transform (NOT persisted, NOT in history)
pub struct RenderTransform {
    pub center: Pos2,
    pub scale: f32,
    pub interpolation_progress: f32, // 0.0-1.0 for animations
}

impl RenderTransform {
    /// Compute transform from semantic view state
    pub fn from_view_state(view: &ViewState, layout: &SolarLayout) -> Self {
        match view.level {
            ViewLevel::Galaxy => Self::galaxy_transform(layout),
            ViewLevel::System => Self::system_transform(view.focus_manco_id, layout),
            ViewLevel::Planet => Self::planet_transform(view.focus_cbu_id, layout),
            ViewLevel::Surface => Self::surface_transform(view.focus_cbu_id, layout),
            ViewLevel::Universe => Self::universe_transform(),
        }
    }
    
    // ... transform computation methods
}
```

---

## Part 3: Navigation State Machine

### 3.1 Transition Matrix

Valid transitions are explicit. Invalid transitions return an error message.

| From Level | Valid Transitions | Invalid (Error Message) |
|------------|-------------------|------------------------|
| Universe | → Galaxy | → System, Planet, Surface ("Enter a galaxy first") |
| Galaxy | → System (enter), → Universe | → Planet, Surface ("Enter a system first") |
| System | → Galaxy (orbit up), → Planet (land) | → Surface ("Land on a CBU first") |
| Planet | → System (orbit up), → Surface (drill) | → Galaxy ("Orbit first") |
| Surface | → Planet (surface up) | → Galaxy, System ("Surface up first") |

### 3.2 Preconditions per Level

| Level | Required Fields | Optional Fields |
|-------|-----------------|-----------------|
| Universe | - | focus_manco_id (selected galaxy) |
| Galaxy | - | focus_manco_id (highlighted ManCo) |
| System | focus_manco_id | orbit_pos (cursor position) |
| Planet | focus_cbu_id | - |
| Surface | focus_cbu_id | - |

### 3.3 Command Preconditions

Each navigation command has explicit preconditions:

```rust
impl NavigationService {
    pub fn enter_system(&mut self, manco_id: Uuid) -> Result<ViewState, NavError> {
        // Precondition: must be at Galaxy level
        if self.session.view.level != ViewLevel::Galaxy {
            return Err(NavError::InvalidTransition {
                from: self.session.view.level,
                to: ViewLevel::System,
                hint: "Use 'orbit' to return to galaxy view first".into(),
            });
        }
        
        // Precondition: ManCo must be in scope
        if !self.session.manco_ids.contains(&manco_id) {
            return Err(NavError::NotInScope {
                entity_type: "ManCo",
                id: manco_id,
            });
        }
        
        let next = ViewState::system(manco_id, Utc::now());
        self.session.nav_history.push_if_changed(next.clone());
        self.session.view = next.clone();
        Ok(next)
    }
    
    pub fn clockwise(&mut self, count: usize) -> Result<ViewState, NavError> {
        // Precondition: must be at System level
        if self.session.view.level != ViewLevel::System {
            return Err(NavError::InvalidTransition {
                from: self.session.view.level,
                to: ViewLevel::System,
                hint: "Orbital navigation only works at System level".into(),
            });
        }
        
        // Precondition: must have orbit position
        let pos = self.session.view.orbit_pos.ok_or(NavError::NoOrbitPosition)?;
        
        // Resolve new position via layout
        let layout = self.get_current_layout()?;
        let new_pos = layout.resolve_clockwise(pos.ring, pos.index, count)?;
        
        let next = ViewState::system_at(
            self.session.view.focus_manco_id.unwrap(),
            new_pos,
            Utc::now(),
        );
        
        self.session.nav_history.push_if_changed(next.clone());
        self.session.view = next.clone();
        Ok(next)
    }
}
```

---

## Part 4: ESPER Navigation Verbs

### 4.1 Design Principles

1. **Verbs are semantic** - describe intent, not mechanics
2. **Verbs are stateful** - they mutate session via `push_if_changed`
3. **Verbs are reversible** - back/forward undo any navigation
4. **Verbs are context-aware** - "next" means different things at different levels
5. **No-op commands don't spam history** - dedupe prevents pollution

### 4.2 Level Transition Verbs

```yaml
# Galaxy → System (enter a ManCo)
enter_system:
  canonical: "enter"
  response: "Entering {manco_name}..."
  agent_command:
    type: EnterSystem
    params:
      manco: extract  # "enter Lux" → manco="Lux"
  precondition: level == Galaxy
  postcondition: level == System, focus_manco_id set
  aliases:
    prefix:
      - "enter "
      - "go into "
      - "open "
    contains:
      - "show me inside"
      
# System → Galaxy (exit to see all ManCos)  
exit_to_galaxy:
  canonical: "show all mancos"
  response: "Showing all management companies..."
  agent_command:
    type: ExitToGalaxy
  precondition: level == System
  postcondition: level == Galaxy
  aliases:
    exact:
      - "orbit"
      - "step back"
    contains:
      - "all mancos"
      - "show systems"
      - "galaxy view"
      - "regional view"

# System → Planet (focus one CBU)
land_on:
  canonical: "land on"
  response: "Landing on {cbu_name}..."
  agent_command:
    type: LandOn
    params:
      cbu: extract  # "land on AGI Fund" → cbu="AGI Fund"
  precondition: level == System
  postcondition: level == Planet, focus_cbu_id set
  aliases:
    prefix:
      - "land on "
      - "focus "
      - "select "
      - "go to "
    contains:
      - "show me "
      - "open cbu"

# Planet → System (back to orbit view)
orbit_up:
  canonical: "orbit"
  response: "Returning to orbit..."
  agent_command:
    type: OrbitUp
  precondition: level == Planet
  postcondition: level == System
  aliases:
    exact:
      - "orbit"
      - "back to orbit"
    contains:
      - "see all cbus"
      - "system view"
      - "zoom out"

# Planet → Surface (drill into entities)
drill_down:
  canonical: "drill"
  response: "Drilling into entities..."
  agent_command:
    type: DrillDown
  precondition: level == Planet
  postcondition: level == Surface
  aliases:
    exact:
      - "drill"
      - "expand"
    contains:
      - "show entities"
      - "who's inside"
      - "show structure"

# Surface → Planet (back to CBU view)
surface_up:
  canonical: "surface"
  response: "Returning to CBU view..."
  agent_command:
    type: SurfaceUp
  precondition: level == Surface
  postcondition: level == Planet
  aliases:
    exact:
      - "surface"
      - "collapse"
    contains:
      - "back to cbu"
      - "hide entities"
```

### 4.3 Same-Level Navigation Verbs (System Level)

At System level (ManCo + CBU orbits), these navigate between CBUs:

```yaml
# Move clockwise in orbit
nav_clockwise:
  canonical: "clockwise"
  response: "Moving clockwise..."
  agent_command:
    type: NavClockwise
    params:
      count: 1  # default
  precondition: level == System, orbit_pos set
  postcondition: orbit_pos.index updated (wraps)
  aliases:
    exact:
      - "next"
      - "clockwise"
      - "right"
    prefix:
      - "next "  # "next 3" → count=3
      
# Move counter-clockwise in orbit
nav_counter_clockwise:
  canonical: "counter-clockwise"
  response: "Moving counter-clockwise..."
  agent_command:
    type: NavCounterClockwise
    params:
      count: 1
  precondition: level == System, orbit_pos set
  postcondition: orbit_pos.index updated (wraps)
  aliases:
    exact:
      - "prev"
      - "previous"
      - "counter-clockwise"
      - "left"
    prefix:
      - "prev "
      - "back "

# Move to inner orbit (toward ManCo sun)
nav_inner:
  canonical: "inner"
  response: "Moving to inner orbit..."
  agent_command:
    type: NavInner
    params:
      count: 1
  precondition: level == System, orbit_pos set, ring > 0
  postcondition: orbit_pos.ring decremented
  error_if: already at innermost orbit
  aliases:
    exact:
      - "inner"
      - "in"
      - "up"
    contains:
      - "inner orbit"
      - "move in"
      - "toward center"

# Move to outer orbit (away from ManCo sun)
nav_outer:
  canonical: "outer"
  response: "Moving to outer orbit..."
  agent_command:
    type: NavOuter
    params:
      count: 1
  precondition: level == System, orbit_pos set
  postcondition: orbit_pos.ring incremented
  error_if: already at outermost orbit
  aliases:
    exact:
      - "outer"
      - "out"
      - "down"
    contains:
      - "outer orbit"
      - "move out"
      - "away from center"

# Jump multiple positions
nav_jump:
  canonical: "jump"
  response: "Jumping {count} {direction}..."
  agent_command:
    type: NavJump
    params:
      count: extract
      direction: extract
  precondition: level == System, orbit_pos set
  aliases:
    prefix:
      - "jump "     # "jump 5 clockwise"
      - "skip "     # "skip 3 right"
      - "hop "      # "hop 2 outer"
```

### 4.4 Same-Level Navigation (Galaxy Level)

At Galaxy level, navigate between ManCos:

```yaml
# Hop to specific ManCo (stays at Galaxy level)
hop_to_manco:
  canonical: "hop to"
  response: "Hopping to {manco_name}..."
  agent_command:
    type: HopToManCo
    params:
      manco: extract
  precondition: level == Galaxy
  postcondition: focus_manco_id set (still at Galaxy)
  aliases:
    prefix:
      - "hop to "
      - "go to "
      - "select "

# Next/prev ManCo (if ordered)
nav_next_manco:
  canonical: "next system"
  response: "Moving to next system..."
  agent_command:
    type: NavNextManCo
  precondition: level == Galaxy
  aliases:
    contains:
      - "next system"
      - "next manco"

nav_prev_manco:
  canonical: "previous system"
  response: "Moving to previous system..."
  agent_command:
    type: NavPrevManCo
  precondition: level == Galaxy
  aliases:
    contains:
      - "prev system"
      - "previous manco"
```

### 4.5 History Navigation Verbs (Rewritten)

#### Overview

History navigation verbs operate **only on the session's NavigationHistory cursor** and do not mutate scope. They restore a prior `ViewState` snapshot, which then drives layout as a pure function of `(scope, view)`.

**Invariant:** History navigation never changes what data is loaded; it changes only what is displayed.

All history verbs follow the same execution contract:
1. Ask `nav_history` for the target snapshot (`back`/`forward`/`rewind`/`jump_to`)
2. If a snapshot is returned:
   - Set `session.view = snapshot.clone()`
   - (optional) Validate `session.view.validate()`
   - (optional) Emit a navigation event for telemetry/audit
3. If `None` is returned:
   - No-op (do not push history)
   - Return a user-facing response such as "Already at oldest state"

**Critical rule:** History verbs MUST NOT call `push_if_changed()`. Otherwise history navigation creates new history entries and "back" becomes impossible.

#### Verb: nav.back

**Intent:** Move one step backward in navigation history.

**Preconditions:**
- `nav_history.current()` is `Some`
- `cursor > 0`

**Effect:**
- `nav_history.back()` moves cursor left by one
- `session.view` becomes the returned snapshot

**No-op cases:**
- History is empty
- Cursor is already at 0

```rust
pub fn nav_back(session: &mut SessionContext) -> NavResult {
    if let Some(prev) = session.nav_history.back().cloned() {
        debug_assert!(prev.validate().is_ok());
        session.view = prev;
        NavResult::Ok("Back")
    } else {
        NavResult::NoOp("Already at oldest state")
    }
}
```

#### Verb: nav.forward

**Intent:** Move one step forward in navigation history.

**Preconditions:**
- `nav_history.current()` is `Some`
- `cursor < snapshots.len() - 1`

**Effect:**
- `nav_history.forward()` moves cursor right by one
- `session.view` becomes the returned snapshot

**No-op cases:**
- History is empty
- Cursor already at newest snapshot

```rust
pub fn nav_forward(session: &mut SessionContext) -> NavResult {
    if let Some(next) = session.nav_history.forward().cloned() {
        debug_assert!(next.validate().is_ok());
        session.view = next;
        NavResult::Ok("Forward")
    } else {
        NavResult::NoOp("Already at newest state")
    }
}
```

#### Verb: nav.rewind

**Intent:** Jump to the first history snapshot.

**Preconditions:**
- History is non-empty

**Effect:**
- Cursor becomes 0
- View becomes `snapshot[0]`

**No-op cases:**
- History is empty

```rust
pub fn nav_rewind(session: &mut SessionContext) -> NavResult {
    if let Some(first) = session.nav_history.rewind().cloned() {
        debug_assert!(first.validate().is_ok());
        session.view = first;
        NavResult::Ok("Rewind")
    } else {
        NavResult::NoOp("History is empty")
    }
}
```

#### Verb: nav.jump_to(index)

**Intent:** Jump to a specific snapshot index (used by UI breadcrumbs).

**Inputs:**
- `index: usize`

**Effect:**
- Cursor becomes `index`
- View becomes `snapshot[index]`

**No-op / error cases:**
- Invalid index → return an error ("Index out of range"), no mutation

```rust
pub fn nav_jump_to(session: &mut SessionContext, index: usize) -> NavResult {
    if let Some(snap) = session.nav_history.jump_to(index).cloned() {
        debug_assert!(snap.validate().is_ok());
        session.view = snap;
        NavResult::Ok("Jumped")
    } else {
        NavResult::Err(format!("History index out of range: {}", index))
    }
}
```

#### YAML Config (History Verbs)

```yaml
nav_back:
  canonical: "nav back"
  response: "Going back..."
  agent_command:
    type: NavBack
  precondition: history.cursor > 0
  error_if: at beginning of history
  note: Does NOT push to history (traverses it)
  aliases:
    exact: ["nav back", "go back", "previous view"]

nav_forward:
  canonical: "nav forward"
  response: "Going forward..."
  agent_command:
    type: NavForward
  precondition: history.cursor < history.len - 1
  error_if: at end of history
  note: Does NOT push to history (traverses it)
  aliases:
    exact: ["nav forward", "go forward"]

nav_rewind:
  canonical: "rewind"
  response: "Rewinding to start..."
  agent_command:
    type: NavRewind
  precondition: history not empty
  note: Does NOT push to history (traverses it)
  aliases:
    exact: ["rewind", "start over", "beginning"]
    contains: ["go to start", "from the top"]

nav_show_history:
  canonical: "show history"
  response: "Navigation history..."
  agent_command:
    type: ShowHistory
  note: Read-only, no state change
  aliases:
    contains: ["show history", "breadcrumbs", "where was i", "trace my steps"]
```

#### "Back then new action" Semantics (Browser Rule)

If the user goes `nav.back` (cursor moves left) and then performs a normal navigation action that pushes a new state:
- Forward history is truncated automatically by `push_if_changed()` before pushing the new state
- The new state becomes the newest state
- `nav.forward` will no longer move into the truncated branch

This matches browser history semantics and prevents confusing "branching" unless you explicitly want a tree-history model.

#### UI Guidelines (Breadcrumbs)

The UI may render breadcrumbs from:
- `nav_history.breadcrumbs() -> Vec<(index, ViewStateKey)>`

Clicking a breadcrumb calls `nav.jump_to(index)`.

**Note:** Breadcrumb rendering should use `ViewStateKey` (semantic identity), not timestamps, to avoid showing redundant entries.

**Note on alias collisions:** Aliases like "back", "open", "show me" are common in chat and may collide with non-navigation intents. The canonical forms use "nav back", "nav forward" to disambiguate. The semantic matcher can still accept natural phrases but with lower confidence when ambiguous.

### 4.6 Normal Navigation Verbs (Rewritten)

#### Overview

Normal navigation verbs move the user through the Solar metaphor levels and/or within a level (e.g., orbit movement). They produce a new semantic `ViewState` only when the move is valid.

**Core rule:** Only successful semantic view changes are pushed to history via `push_if_changed(next_state)`.

#### Shared Execution Contract

Every normal navigation verb follows this contract:
1. Validate the command against the current view level and available scope
2. Compute `next_view: ViewState` (semantic-only; no camera math)
3. If valid and different:
   - `session.nav_history.push_if_changed(next_view.clone())`
   - `session.view = next_view`
4. If invalid or no-op:
   - Do not push history
   - Return a message explaining why

**Helper:**

```rust
fn commit_view(session: &mut SessionContext, next: ViewState) {
    debug_assert!(next.validate().is_ok());
    session.nav_history.push_if_changed(next.clone());
    session.view = next;
}
```

#### Level Transitions (Zoom / Enter / Exit)

##### Verb: nav.zoom_in

**Intent:** Move deeper into the hierarchy.

**Allowed transitions:**
- Galaxy → System (requires a selected ManCo)
- System → Planet (requires orbit selection)
- Planet → Surface

**Preconditions and selection rules:**

| Transition | Precondition |
|------------|--------------|
| Galaxy → System | `focus_manco_id` must be `Some`, else fail with "Select a ManCo first" |
| System → Planet | `orbit_pos` must be `Some`, derive CBU from `(ring,index)` via layout, else fail with "Select a planet first" |
| Planet → Surface | `focus_cbu_id` must be `Some` |

**No-op cases:**
- Already at Surface (cannot zoom further)

```rust
pub fn nav_zoom_in(session: &mut SessionContext, layout: &SolarLayout) -> NavResult {
    let now = Utc::now();

    match session.view.level {
        ViewLevel::Galaxy => {
            let manco = session.view.focus_manco_id
                .ok_or_else(|| NavResult::NoOp("Select a ManCo first"))?;
            commit_view(session, ViewState::system(manco, now));
            NavResult::Ok("Entered system")
        }
        ViewLevel::System => {
            let pos = session.view.orbit_pos
                .ok_or_else(|| NavResult::NoOp("Select a planet first"))?;
            let cbu_id = layout.system_cbu_at(pos)
                .ok_or_else(|| NavResult::Err("Orbit selection out of range".into()))?;
            commit_view(session, ViewState::planet(cbu_id, now));
            NavResult::Ok("Focused planet")
        }
        ViewLevel::Planet => {
            let cbu = session.view.focus_cbu_id
                .ok_or_else(|| NavResult::Err("Planet view missing focus_cbu_id".into()))?;
            commit_view(session, ViewState::surface(cbu, now));
            NavResult::Ok("Entered surface")
        }
        ViewLevel::Surface => NavResult::NoOp("Already at deepest level"),
        ViewLevel::Universe => NavResult::NoOp("Zoom in from Universe is not supported"),
    }
}
```

##### Verb: nav.zoom_out

**Intent:** Move up one level.

**Allowed transitions:**
- Surface → Planet
- Planet → System (requires a parent ManCo + orbit pos)
- System → Galaxy
- Galaxy → Universe (optional; usually no-op)

**Preconditions:**
- If moving from Planet/Surface to System, you must be able to determine:
  - Parent ManCo, and
  - Orbit position for that CBU (if known)

If you cannot derive orbit position, enter system with `orbit_pos = None` (user then selects via orbit verbs).

**No-op cases:**
- Already at Galaxy and you do not support Universe
- Already at Universe

```rust
pub fn nav_zoom_out(session: &mut SessionContext, layout: &SolarLayout) -> NavResult {
    let now = Utc::now();

    match session.view.level {
        ViewLevel::Surface => {
            let cbu = session.view.focus_cbu_id
                .ok_or_else(|| NavResult::Err("Surface missing focus".into()))?;
            commit_view(session, ViewState::planet(cbu, now));
            NavResult::Ok("Back to planet")
        }
        ViewLevel::Planet => {
            let cbu = session.view.focus_cbu_id
                .ok_or_else(|| NavResult::Err("Planet missing focus".into()))?;
            let (manco, pos) = layout.parent_manco_and_orbit_of(cbu)
                .ok_or_else(|| NavResult::Err("Cannot derive parent system".into()))?;
            let mut s = ViewState::system(manco, now);
            s.orbit_pos = Some(pos);
            s.focus_cbu_id = None; // derived from orbit_pos
            commit_view(session, s);
            NavResult::Ok("Back to system")
        }
        ViewLevel::System => {
            commit_view(session, ViewState::galaxy(now));
            NavResult::Ok("Back to galaxy")
        }
        ViewLevel::Galaxy => NavResult::NoOp("Already at galaxy level"),
        ViewLevel::Universe => NavResult::NoOp("Already at universe level"),
    }
}
```

##### Verb: nav.enter_system(manco_id | alias)

**Intent:** Go directly to a specific system.

**Preconditions:**
- Target ManCo must exist in scope/resolution

**Effect:**
- Set `ViewState::System { focus_manco_id=Some(target), orbit_pos=None }`

Note: This sets the ManCo focus deterministically and avoids "Galaxy selection" ambiguity.

##### Verb: nav.enter_planet(cbu_id | alias)

**Intent:** Go directly to a specific planet (CBU).

**Preconditions:**
- CBU exists
- Optionally verify it is within current system scope (if in System view)

**Effect:**
- `Planet { focus_cbu_id=Some(cbu_id) }`
- May reset system selection fields

#### System-Level Orbit Navigation (Discrete Cursor Movement)

At System level, the canonical "current selection" is `orbit_pos: OrbitPos`.

**Invariant:** Orbit movement mutates `orbit_pos` only. Any focused CBU id is derived from layout using `orbit_pos`.

##### Verb: nav.orbit_select(ring, index)

**Intent:** Set orbit cursor explicitly (UI click or command).

**Preconditions:**
- Must be in System
- `(ring, index)` must exist for this system layout

**Effect:**
- Set `orbit_pos = Some(OrbitPos{ring, index})`

**No-op cases:**
- Selection unchanged (dedupe prevents history push)
- Invalid selection → error, no push

##### Verb: nav.orbit_next / nav.orbit_prev

**Intent:** Move cursor clockwise or counter-clockwise on current ring.

**Preconditions:**
- Must be in System
- Must have `orbit_pos = Some`
- If `None`, initialize to `(ring=0, index=0)` if ring 0 non-empty, else no-op

**Effect:**
- `index = (index + 1) % ring_len` for next
- `index = (index + ring_len - 1) % ring_len` for prev

**No-op cases:**
- System has zero planets
- `ring_len == 0`

```rust
pub fn nav_orbit_next(session: &mut SessionContext, layout: &SolarLayout) -> NavResult {
    if session.view.level != ViewLevel::System {
        return NavResult::NoOp("Orbit navigation only works in System view");
    }

    let now = Utc::now();
    let mut next = session.view.clone();
    next.timestamp = now;

    // Get current position, or initialize to (0,0) if None
    let pos = match next.orbit_pos {
        Some(p) => p,
        None => {
            // Initialize to first planet deterministically
            let ring_len = layout.system_ring_len(0).unwrap_or(0);
            if ring_len == 0 {
                return NavResult::NoOp("No planets in this system");
            }
            OrbitPos { ring: 0, index: 0 }
        }
    };

    let ring_len = layout.system_ring_len(pos.ring).unwrap_or(0);
    if ring_len == 0 {
        return NavResult::NoOp("No planets in this ring");
    }

    // Move clockwise (increment index with wrap)
    next.orbit_pos = Some(OrbitPos { 
        ring: pos.ring, 
        index: (pos.index + 1) % ring_len 
    });

    commit_view(session, next);
    NavResult::Ok("Orbit next")
}
```

##### Verb: nav.ring_in / nav.ring_out

**Intent:** Move cursor to an inner/outer orbit ring.

**Preconditions:**
- Must be in System
- Must have at least one non-empty ring

**Effect:**
- Decrement/increment ring while keeping "closest index" stable:
  - Preserve angular selection by mapping index proportionally: `new_index = floor(old_index * new_len / old_len)`
  - Clamp if needed
- If the target ring is empty, skip to nearest non-empty ring in that direction

**No-op cases:**
- No inner ring exists (`ring_in` from 0)
- No outer non-empty ring exists

#### Selection and Click Verbs

##### Verb: nav.select_manco(manco_id) (Galaxy)

Sets `focus_manco_id` at Galaxy level **without entering system**.

##### Verb: nav.select_planet(pos | cbu_id) (System)

Sets `orbit_pos` (canonical) from:
- Click position → nearest planet → `orbit_pos`
- Explicit `(ring, index)` → `orbit_pos`

**Note:** A click-select should **not** enter Planet view automatically. That's handled by `zoom_in` or `enter_planet`.

#### Error and No-op Policy

To avoid spamming history and creating confusing breadcrumbs:
- A verb that **cannot change** semantic state returns `NoOp` and does not push
- A verb that **fails** due to invalid inputs returns `Err` and does not push
- Only **successful** state changes push via `push_if_changed()`

#### Suggested Verb Set (User-Facing)

**History:**
- `nav.back`, `nav.forward`, `nav.rewind`, `nav.jump_to(i)`

**Level transitions:**
- `nav.zoom_in`, `nav.zoom_out`
- `nav.enter_system(manco)`, `nav.enter_planet(cbu)`

**System navigation:**
- `nav.orbit_select(ring, index)`
- `nav.orbit_next`, `nav.orbit_prev`
- `nav.ring_in`, `nav.ring_out`

**Selection (no level change):**
- `nav.select_manco(manco)`
- `nav.select_planet(ring, index)` or click-based nearest

---

## Part 4b: Navigation Transition Matrix

This makes navigation "compiler-like": every verb either (a) transitions to a valid new state, (b) no-ops cleanly, or (c) errors with a clear reason.

**Legend:**
- ✅ allowed → produces `next_view` and pushes history via `push_if_changed()`
- ⛔ blocked → returns `NoOp` (no history push)
- ⚠️ error → invalid inputs/state; returns `Err` (no history push)
- **Req:** required fields/data to succeed

**Global rule:** History verbs (`nav.back`/`forward`/`rewind`/`jump_to`) **never** push history.

### 1) Level Transitions

| Current Level | `nav.zoom_in` | Req | `nav.zoom_out` | Req |
|---------------|---------------|-----|----------------|-----|
| Universe | ⛔ | | ⛔ | |
| Galaxy | ✅ → System | `focus_manco_id` | ⛔ (or ✅ → Universe if supported) | |
| System | ✅ → Planet | `orbit_pos` + layout resolves CBU | ✅ → Galaxy | |
| Planet | ✅ → Surface | `focus_cbu_id` | ✅ → System | parent manco + orbit_pos (or allow `orbit_pos=None`) |
| Surface | ⛔ | | ✅ → Planet | `focus_cbu_id` |

**Note:** For Planet → System, if you can't derive orbit position, prefer: enter System with `orbit_pos=None` (don't invent selection silently).

### 2) Direct Entry (By ID/Alias)

| Current Level | `nav.enter_system(manco)` | Req | `nav.enter_planet(cbu)` | Req |
|---------------|---------------------------|-----|-------------------------|-----|
| Universe | ✅ → System | resolve manco | ✅ → Planet | resolve cbu |
| Galaxy | ✅ → System | resolve manco | ✅ → Planet | resolve cbu |
| System | ✅ → System (focus change) | resolve manco | ✅ → Planet | resolve cbu |
| Planet | ✅ → System (if manco given) / ✅ → Planet | resolve | ✅ → Planet | resolve cbu |
| Surface | ✅ → System (if manco given) / ✅ → Planet | resolve | ✅ → Planet | resolve cbu |

**Note:** `enter_*` is always allowed if the target resolves; it's your "escape hatch" from ambiguity.

### 3) System Orbit Navigation (Cursor Movement)

These verbs are meaningful only at System level.

| Current Level | `nav.orbit_select(ring,index)` | Req | `nav.orbit_next/prev` | Req | `nav.ring_in/out` | Req |
|---------------|--------------------------------|-----|----------------------|-----|-------------------|-----|
| Universe | ⛔ | | ⛔ | | ⛔ | |
| Galaxy | ⛔ | | ⛔ | | ⛔ | |
| System | ✅ | layout ring/index exists | ✅ | layout ring exists; if `orbit_pos=None`, init to `(0,0)` | ✅ | non-empty ring exists in direction |
| Planet | ⛔ | | ⛔ | | ⛔ | |
| Surface | ⛔ | | ⛔ | | ⛔ | |

**Notes:**
- If there are no planets in the system, orbit verbs return `NoOp` ("No planets in this system")
- `nav.ring_in/out` should skip empty rings; if none found in direction, `NoOp`

### 4) Selection Verbs (No Level Change)

Selection changes focus fields but stays in the same level.

| Current Level | `nav.select_manco(manco)` | Req | `nav.select_planet(pos)` | Req |
|---------------|---------------------------|-----|--------------------------|-----|
| Universe | ⚠️ (prefer `enter_system`) | | ⛔ | |
| Galaxy | ✅ (focus only) | resolve manco | ⛔ | |
| System | ⛔ (use `enter_system`) | | ✅ (sets `orbit_pos`) | hit-test → valid ring/index |
| Planet | ⛔ | | ⛔ | |
| Surface | ⛔ | | ⛔ | |

**Note:** Keep selection verbs "pure": selecting a planet does **not** auto-zoom. Zoom is explicit (`zoom_in`) to avoid surprise transitions.

### 5) History Verbs (Cursor Only)

| Verb | Allowed From | Effect | History Push |
|------|--------------|--------|--------------|
| `nav.back` | any | cursor - 1, set `session.view` | **Never** |
| `nav.forward` | any | cursor + 1, set `session.view` | **Never** |
| `nav.rewind` | any | cursor = 0, set `session.view` | **Never** |
| `nav.jump_to(i)` | any | cursor = i, set `session.view` | **Never** |

### State Requirements by Level (Validation Contract)

To prevent invalid state combinations:

| Level | Required | Must be None |
|-------|----------|--------------|
| Universe | none | — |
| Galaxy | — | `orbit_pos` |
| System | `focus_manco_id` | — |
| Planet | `focus_cbu_id` | — |
| Surface | `focus_cbu_id` | — |

**Enforcement:** After any successful transition, run `view.validate()` (debug assertion in dev, hard error in tests).

### History Push Rules (Single Source of Truth)

- **Normal navigation verbs:** push only on success (`push_if_changed`)
- **History verbs:** never push
- **NoOp / Err:** never push
- **Dedupe:** states with same `ViewStateKey` are not pushed

---

## Part 4c: Verb Handler Skeleton (Implementation Guardrails)

### Goals

- Prevent invalid view states
- Prevent history spam
- Make behavior deterministic and testable
- Keep "semantic state" separate from camera/render transforms

### Standard Handler Pattern

Every normal navigation verb should follow this structure:
1. **Read** current `session.view` + scope + (optional) layout
2. **Compute** a candidate `next: ViewState` (semantic-only)
3. **Validate** `next.validate()` and any layout constraints
4. **Commit** using `commit_view(session, next)`
5. **Otherwise** return `NoOp` or `Err` without committing and without pushing history

### Helper Types

```rust
pub enum NavResult {
    Ok(&'static str),
    NoOp(&'static str),
    Err(String),
}

fn commit_view(session: &mut SessionContext, next: ViewState) {
    debug_assert!(next.validate().is_ok());
    session.nav_history.push_if_changed(next.clone());
    session.view = next;
}
```

### Example: A "safe" normal verb (orbit_next)

```rust
pub fn nav_orbit_next(session: &mut SessionContext, layout: &SolarLayout) -> NavResult {
    if session.view.level != ViewLevel::System {
        return NavResult::NoOp("Orbit only works in System view");
    }

    let mut next = session.view.clone();
    next.timestamp = Utc::now();

    // Get current position, or initialize to (0,0) if None
    let pos = match next.orbit_pos {
        Some(p) => p,
        None => {
            // Initialize to first planet deterministically
            let ring_len = layout.system_ring_len(0).unwrap_or(0);
            if ring_len == 0 {
                return NavResult::NoOp("No planets in this system");
            }
            OrbitPos { ring: 0, index: 0 }
        }
    };

    let ring_len = layout.system_ring_len(pos.ring).unwrap_or(0);
    if ring_len == 0 {
        return NavResult::NoOp("No planets in this ring");
    }

    // Move clockwise (increment index with wrap)
    next.orbit_pos = Some(OrbitPos { 
        ring: pos.ring, 
        index: (pos.index + 1) % ring_len 
    });

    if let Err(e) = next.validate() {
        return NavResult::Err(format!("Invalid view state: {e}"));
    }

    commit_view(session, next);
    NavResult::Ok("Orbit next")
}
```

### Example: A "safe" transition verb (zoom_in)

```rust
pub fn nav_zoom_in(session: &mut SessionContext, layout: &SolarLayout) -> NavResult {
    let now = Utc::now();

    let next = match session.view.level {
        ViewLevel::Galaxy => {
            let manco = match session.view.focus_manco_id {
                Some(id) => id,
                None => return NavResult::NoOp("Select a ManCo first"),
            };
            ViewState::system(manco, now)
        }
        ViewLevel::System => {
            let pos = match session.view.orbit_pos {
                Some(p) => p,
                None => return NavResult::NoOp("Select a planet first"),
            };
            let cbu = match layout.system_cbu_at(pos) {
                Some(id) => id,
                None => return NavResult::Err("Orbit selection out of range".into()),
            };
            ViewState::planet(cbu, now)
        }
        ViewLevel::Planet => {
            let cbu = session.view.focus_cbu_id
                .ok_or_else(|| NavResult::Err("Missing focus_cbu_id".into()))?;
            ViewState::surface(cbu, now)
        }
        ViewLevel::Surface => return NavResult::NoOp("Already at deepest level"),
        ViewLevel::Universe => return NavResult::NoOp("Zoom in from Universe not supported"),
    };

    if let Err(e) = next.validate() {
        return NavResult::Err(format!("Invalid view state: {e}"));
    }

    commit_view(session, next);
    NavResult::Ok("Zoom in")
}
```

### Hard Rules (Make These Explicit in Code Review)

1. **Never** call `push_if_changed()` inside history verbs (`back`/`forward`/`rewind`/`jump_to`)
2. **Never** push history on `NoOp` or `Err`
3. **Never** `unwrap()` layout lookups in navigation logic (return `Err`)
4. **Always** treat `(scope, view)` as the only inputs to layout; camera is derived

---

## Part 4d: Determinism + Layout Contract (Prevent Jitter)

### Determinism Contract

Given:
- `ScopeSnapshotHash` (stable hash of loaded IDs + relevant metadata versions)
- `ViewStateKey` (semantic identity of view)
- `LayoutParams` (radius, spacing, ring thresholds, etc.)

Then:

```
build_layout(scope, view) → layout MUST be a pure function and return identical output
```

### Stable Ordering Requirements

To keep layout stable across renders and history replay, define a canonical ordering for:
- ManCos in Galaxy view
- CBUs in a System view
- Entities within a Surface view

**Rule:** Never rely on hash iteration order or unordered DB results.

**Recommended ordering keys:**
- Primary: domain-relevant rank (if available)
- Secondary: name/label (normalized)
- Tertiary: UUID (tie-breaker)

### Scope Snapshot / Versioning

Maintain a lightweight snapshot identity:
- `scope_version: u64` incremented on scope changes (load/unload)
- `dictionary_version: u64` for attribute/metadata dictionary changes
- `layout_params_version: u64` for any parameter changes

History snapshots store only semantic view state; they do not store transforms.

### Derived Render Transform (Not Persisted)

Keep camera/animation separately:

```rust
pub struct RenderTransform {
    pub center: egui::Pos2,
    pub scale: f32,
    pub lerp_t: f32, // animation progress 0..1
}
```

**Rule:** `RenderTransform` may change every frame; `ViewStateKey` changes only on semantic navigation.

### Animation Contract

When transitioning between view states:
- Interpolate transforms (camera) smoothly
- Do **not** mutate `ViewState` during animation
- On completion, the final transform must correspond to the new semantic state

---

## Part 4e: Utility Functions (Recommended Additions)

### Telemetry Hook for Navigation Events

For debugging and analytics, add an event emission point in `commit_view`:

```rust
pub struct NavEvent {
    pub from: ViewStateKey,
    pub to: ViewStateKey,
    pub timestamp: DateTime<Utc>,
    pub history_depth: usize,
}

pub trait NavTelemetry {
    fn emit(&self, event: NavEvent);
}

fn commit_view_with_telemetry(
    session: &mut SessionContext, 
    next: ViewState, 
    telemetry: Option<&dyn NavTelemetry>
) {
    debug_assert!(next.validate().is_ok());
    
    let prev_key = session.view.key();
    session.nav_history.push_if_changed(next.clone());
    session.view = next.clone();
    
    if let Some(t) = telemetry {
        t.emit(NavEvent {
            from: prev_key,
            to: next.key(),
            timestamp: next.timestamp,
            history_depth: session.nav_history.len(),
        });
    }
}
```

This gives you free audit trails and supports future "replay navigation" features.

### Breadcrumb Label Resolver

`ViewStateKey` contains UUIDs, so UI needs a resolver for human-readable labels:

```rust
pub trait NameResolver {
    fn manco_name(&self, id: Uuid) -> Option<String>;
    fn cbu_name(&self, id: Uuid) -> Option<String>;
}

pub fn breadcrumb_label(key: &ViewStateKey, resolver: &dyn NameResolver) -> String {
    match key.level {
        ViewLevel::Universe => "Universe".into(),
        ViewLevel::Galaxy => "Galaxy".into(),
        ViewLevel::System => format!("{} System", 
            key.focus_manco_id
                .and_then(|id| resolver.manco_name(id))
                .unwrap_or_else(|| "?".into())),
        ViewLevel::Planet => key.focus_cbu_id
            .and_then(|id| resolver.cbu_name(id))
            .unwrap_or_else(|| "?".into()),
        ViewLevel::Surface => format!("{} (entities)", 
            key.focus_cbu_id
                .and_then(|id| resolver.cbu_name(id))
                .unwrap_or_else(|| "?".into())),
    }
}
```

### Compact View State Encoding (for URLs/Sharing)

For "resume where you left off" or shareable links:

```rust
impl ViewState {
    /// Compact encoding for URLs/resume: 
    /// "G" | "S:manco_uuid" | "P:cbu_uuid" | "U:cbu_uuid" | "X"
    pub fn to_compact_string(&self) -> String {
        match self.level {
            ViewLevel::Universe => "X".into(),
            ViewLevel::Galaxy => "G".into(),
            ViewLevel::System => format!("S:{}", 
                self.focus_manco_id.map(|id| id.to_string()).unwrap_or_default()),
            ViewLevel::Planet => format!("P:{}", 
                self.focus_cbu_id.map(|id| id.to_string()).unwrap_or_default()),
            ViewLevel::Surface => format!("U:{}", 
                self.focus_cbu_id.map(|id| id.to_string()).unwrap_or_default()),
        }
    }
    
    /// Parse compact string back to ViewState (requires name resolver for validation)
    pub fn from_compact_string(s: &str, now: DateTime<Utc>) -> Option<Self> {
        if s == "X" { return Some(Self::universe(now)); }
        if s == "G" { return Some(Self::galaxy(now)); }
        if let Some(id_str) = s.strip_prefix("S:") {
            let id = Uuid::parse_str(id_str).ok()?;
            return Some(Self::system(id, now));
        }
        if let Some(id_str) = s.strip_prefix("P:") {
            let id = Uuid::parse_str(id_str).ok()?;
            return Some(Self::planet_simple(id, now));
        }
        if let Some(id_str) = s.strip_prefix("U:") {
            let id = Uuid::parse_str(id_str).ok()?;
            return Some(Self::surface(id, now));
        }
        None
    }
}
```

### Layout Cache for Performance

For systems with 50+ CBUs, cache computed layouts:

```rust
pub struct LayoutCache {
    system_layouts: HashMap<Uuid, (u64, SolarSystem)>, // (scope_hash, layout)
}

impl LayoutCache {
    pub fn get_or_compute(
        &mut self, 
        manco_id: Uuid, 
        cbus: &[CbuInfo],
        manco: &ManCoInfo,
    ) -> Result<&SolarSystem, LayoutError> {
        let scope_hash = compute_scope_hash(cbus);
        
        if let Some((cached_hash, _)) = self.system_layouts.get(&manco_id) {
            if *cached_hash == scope_hash {
                return Ok(&self.system_layouts.get(&manco_id).unwrap().1);
            }
        }
        
        let layout = SolarLayout::build_system(manco, cbus)?;
        self.system_layouts.insert(manco_id, (scope_hash, layout));
        Ok(&self.system_layouts.get(&manco_id).unwrap().1)
    }
    
    pub fn invalidate(&mut self, manco_id: &Uuid) {
        self.system_layouts.remove(manco_id);
    }
    
    pub fn invalidate_all(&mut self) {
        self.system_layouts.clear();
    }
}

fn compute_scope_hash(cbus: &[CbuInfo]) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    for cbu in cbus {
        cbu.cbu_id.hash(&mut hasher);
        cbu.name.hash(&mut hasher);
    }
    hasher.finish()
}
```

### Custom Arbitrary Impl for Property Tests

`ViewState` has invariants, so property tests need valid generators:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    
    impl Arbitrary for ViewState {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;
        
        fn arbitrary_with(_: ()) -> Self::Strategy {
            prop_oneof![
                Just(ViewState::galaxy(Utc::now())),
                any::<[u8; 16]>().prop_map(|bytes| {
                    ViewState::system(Uuid::from_bytes(bytes), Utc::now())
                }),
                any::<[u8; 16]>().prop_map(|bytes| {
                    ViewState::planet_simple(Uuid::from_bytes(bytes), Utc::now())
                }),
                any::<[u8; 16]>().prop_map(|bytes| {
                    ViewState::surface(Uuid::from_bytes(bytes), Utc::now())
                }),
            ].boxed()
        }
    }
}
```

---

## Part 5: Solar Layout Engine

### 5.1 Design Principles

1. **Layout is a pure function of session state** - no hidden state
2. **Deterministic** - same session state → same layout (requires stable ordering)
3. **Supports all view levels** - galaxy, system, planet, surface
4. **Navigation-aware** - layout knows about orbits/positions for navigation queries

### 5.2 Stable Ordering (Critical for Determinism)

Layout ordering MUST be deterministic. Use explicit ORDER BY:

```rust
/// Stable ordering for CBUs in orbit
fn order_cbus(cbus: &mut [CbuInfo]) {
    cbus.sort_by(|a, b| {
        a.jurisdiction.cmp(&b.jurisdiction)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.cbu_id.cmp(&b.cbu_id))
    });
}

/// Stable ordering for ManCos in galaxy
fn order_mancos(mancos: &mut [ManCoInfo]) {
    mancos.sort_by(|a, b| {
        a.region.cmp(&b.region)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.manco_id.cmp(&b.manco_id))
    });
}
```

**Without stable ordering:** Planets will jump around between renders, breaking "navigation feels natural" and history replay.

### 5.3 Data Model

```rust
/// A ManCo (sun) with its orbital CBUs
#[derive(Debug, Clone)]
pub struct SolarSystem {
    pub manco_id: Uuid,
    pub manco_name: String,
    pub manco_region: String,      // "LU", "IE", "DE"
    pub manco_flag: String,        // "🇱🇺"
    pub orbits: Vec<Orbit>,
    pub galaxy_position: Option<Pos2>,
}

/// A ring of CBUs orbiting a ManCo
#[derive(Debug, Clone)]
pub struct Orbit {
    pub ring: usize,               // 0 = innermost
    pub radius: f32,
    pub cbus: Vec<OrbitalCbu>,     // Ordered clockwise, stable sort
}

/// A CBU positioned in an orbit
#[derive(Debug, Clone)]
pub struct OrbitalCbu {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: String,
    pub flag: String,
    pub orbit_index: usize,        // Position in orbit (0..N-1)
    pub angle: f32,                // Angle in radians
    pub position: Pos2,            // Computed position
}

/// Galaxy layout (multiple solar systems)
#[derive(Debug, Clone)]
pub struct Galaxy {
    pub name: String,              // "Allianz"
    pub systems: Vec<SolarSystem>, // Stable ordered
}

/// ManCo info (separate from CBU info)
#[derive(Debug, Clone)]
pub struct ManCoInfo {
    pub manco_id: Uuid,
    pub name: String,
    pub region: String,
    // ... other ManCo-specific fields
}
```

### 5.4 Multi-Orbit Strategy

For ManCos with many CBUs, use multiple orbits:

```rust
impl SolarLayout {
    const SUN_RADIUS: f32 = 60.0;
    const ORBIT_SPACING: f32 = 100.0;
    const MIN_CBU_SPACING: f32 = 80.0;
    
    pub fn build_system(
        manco: &ManCoInfo,
        cbus: &[CbuInfo],
    ) -> Result<SolarSystem, LayoutError> {
        let mut sorted_cbus = cbus.to_vec();
        order_cbus(&mut sorted_cbus);
        
        let mut orbits = Vec::new();
        let mut remaining = sorted_cbus.as_slice();
        let mut ring = 0;
        
        while !remaining.is_empty() {
            let radius = Self::SUN_RADIUS + Self::ORBIT_SPACING * (ring + 1) as f32;
            let circumference = 2.0 * std::f32::consts::PI * radius;
            let max_in_orbit = (circumference / Self::MIN_CBU_SPACING).floor() as usize;
            let max_in_orbit = max_in_orbit.max(1);
            
            let take = remaining.len().min(max_in_orbit);
            let (orbit_cbus, rest) = remaining.split_at(take);
            remaining = rest;
            
            let angle_step = 2.0 * std::f32::consts::PI / take as f32;
            let orbital_cbus: Vec<OrbitalCbu> = orbit_cbus
                .iter()
                .enumerate()
                .map(|(i, cbu)| {
                    let angle = i as f32 * angle_step - std::f32::consts::PI / 2.0;
                    OrbitalCbu {
                        cbu_id: cbu.cbu_id,
                        name: cbu.name.clone(),
                        jurisdiction: cbu.jurisdiction.clone(),
                        flag: jurisdiction_to_flag(&cbu.jurisdiction),
                        orbit_index: i,
                        angle,
                        position: Pos2::new(radius * angle.cos(), radius * angle.sin()),
                    }
                })
                .collect();
            
            orbits.push(Orbit {
                ring,
                radius,
                cbus: orbital_cbus,
            });
            
            ring += 1;
        }
        
        Ok(SolarSystem {
            manco_id: manco.manco_id,
            manco_name: manco.name.clone(),
            manco_region: manco.region.clone(),
            manco_flag: jurisdiction_to_flag(&manco.region),
            orbits,
            galaxy_position: None,
        })
    }
}
```

### 5.5 Navigation Resolution with Circular Distance

Fix for angle wrap-around (angles near -π and +π should be treated as adjacent):

```rust
impl SolarSystem {
    /// Resolve clockwise navigation (wraps around)
    pub fn resolve_clockwise(
        &self,
        current_ring: usize,
        current_index: usize,
        count: usize,
    ) -> Option<OrbitPos> {
        let orbit = self.orbits.get(current_ring)?;
        let new_index = (current_index + count) % orbit.cbus.len();
        Some(OrbitPos { ring: current_ring, index: new_index })
    }
    
    /// Resolve counter-clockwise navigation (wraps around)
    pub fn resolve_counter_clockwise(
        &self,
        current_ring: usize,
        current_index: usize,
        count: usize,
    ) -> Option<OrbitPos> {
        let orbit = self.orbits.get(current_ring)?;
        let len = orbit.cbus.len();
        let new_index = (current_index + len - (count % len)) % len;
        Some(OrbitPos { ring: current_ring, index: new_index })
    }
    
    /// Resolve inner orbit navigation
    pub fn resolve_inner(
        &self,
        current_ring: usize,
        current_index: usize,
    ) -> Option<OrbitPos> {
        if current_ring == 0 {
            return None; // Already at innermost
        }
        let current_angle = self.orbits[current_ring].cbus[current_index].angle;
        let inner_orbit = &self.orbits[current_ring - 1];
        let new_index = self.find_nearest_by_angle(inner_orbit, current_angle);
        Some(OrbitPos { ring: current_ring - 1, index: new_index })
    }
    
    /// Resolve outer orbit navigation
    pub fn resolve_outer(
        &self,
        current_ring: usize,
        current_index: usize,
    ) -> Option<OrbitPos> {
        if current_ring + 1 >= self.orbits.len() {
            return None; // Already at outermost
        }
        let current_angle = self.orbits[current_ring].cbus[current_index].angle;
        let outer_orbit = &self.orbits[current_ring + 1];
        let new_index = self.find_nearest_by_angle(outer_orbit, current_angle);
        Some(OrbitPos { ring: current_ring + 1, index: new_index })
    }
    
    /// Find nearest CBU by angle using CIRCULAR distance (handles wrap-around)
    fn find_nearest_by_angle(&self, orbit: &Orbit, target_angle: f32) -> usize {
        orbit.cbus
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let diff_a = circular_angle_distance(a.angle, target_angle);
                let diff_b = circular_angle_distance(b.angle, target_angle);
                diff_a.partial_cmp(&diff_b).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
    
    /// Get CBU at orbit position
    pub fn cbu_at(&self, pos: &OrbitPos) -> Option<&OrbitalCbu> {
        self.orbits.get(pos.ring)?.cbus.get(pos.index)
    }
}

/// Circular angle distance (handles -π to +π wrap-around)
fn circular_angle_distance(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs();
    diff.min(2.0 * std::f32::consts::PI - diff)
}
```

### 5.6 Galaxy Layout (Grid with Region Clustering)

```rust
impl Galaxy {
    pub fn build(
        name: String,
        mancos: &[ManCoInfo],
        cbu_data: &HashMap<Uuid, Vec<CbuInfo>>,
    ) -> Result<Self, LayoutError> {
        let mut sorted_mancos = mancos.to_vec();
        order_mancos(&mut sorted_mancos);
        
        // Grid layout with region clustering
        let cols = (sorted_mancos.len() as f32).sqrt().ceil() as usize;
        let spacing = 300.0;
        
        let systems: Vec<SolarSystem> = sorted_mancos
            .iter()
            .enumerate()
            .map(|(i, manco)| {
                let row = i / cols;
                let col = i % cols;
                let x = col as f32 * spacing;
                let y = row as f32 * spacing;
                
                let cbus = cbu_data.get(&manco.manco_id)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                
                let mut system = SolarLayout::build_system(manco, cbus)?;
                system.galaxy_position = Some(Pos2::new(x, y));
                Ok(system)
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(Self { name, systems })
    }
}
```

### 5.7 Rendering by Level

```rust
impl SolarRenderer {
    pub fn render(&self, session: &SessionContext, ui: &mut egui::Ui) {
        match session.view.level {
            ViewLevel::Universe => self.render_universe(session, ui),
            ViewLevel::Galaxy => self.render_galaxy(session, ui),
            ViewLevel::System => self.render_system(session, ui),
            ViewLevel::Planet => self.render_planet(session, ui),
            ViewLevel::Surface => self.render_surface(session, ui),
        }
    }
    
    fn render_galaxy(&self, session: &SessionContext, ui: &mut egui::Ui) {
        for system in &self.galaxy.systems {
            let pos = system.galaxy_position.unwrap_or_default();
            
            // Draw ManCo icon
            self.draw_manco_icon(ui, pos, &system.manco_name, &system.manco_flag);
            
            // Draw halo indicating CBU count
            let cbu_count: usize = system.orbits.iter().map(|o| o.cbus.len()).sum();
            self.draw_cbu_halo(ui, pos, cbu_count);
            
            // Highlight if focused
            if session.view.focus_manco_id == Some(system.manco_id) {
                self.draw_selection_ring(ui, pos);
            }
        }
    }
    
    fn render_system(&self, session: &SessionContext, ui: &mut egui::Ui) {
        let Some(manco_id) = session.view.focus_manco_id else { return };
        let Some(system) = self.find_system(manco_id) else { return };
        
        // Draw ManCo as sun at center
        self.draw_sun(ui, Pos2::ZERO, &system.manco_name, &system.manco_flag);
        
        // Draw orbit rings and CBUs
        for orbit in &system.orbits {
            self.draw_orbit_ring(ui, orbit.radius);
            
            for cbu in &orbit.cbus {
                let is_current = session.view.orbit_pos
                    .map(|pos| pos.ring == orbit.ring && pos.index == cbu.orbit_index)
                    .unwrap_or(false);
                
                self.draw_cbu_planet(ui, cbu, is_current);
            }
        }
    }
    
    fn render_planet(&self, session: &SessionContext, ui: &mut egui::Ui) {
        let Some(cbu_id) = session.view.focus_cbu_id else { return };
        self.draw_cbu_expanded(ui, cbu_id);
    }
    
    fn render_surface(&self, session: &SessionContext, ui: &mut egui::Ui) {
        let Some(cbu_id) = session.view.focus_cbu_id else { return };
        // Delegate to existing entity graph renderer
        self.existing_layout_engine.render(ui, cbu_id);
    }
}
```

---

## Part 6: Implementation Plan

### Phase 1: Session State Foundation
**Goal:** Add view state and navigation history to session

1. Add `ViewLevel`, `OrbitPos`, `ViewState`, `ViewStateKey` to `ob-poc-types`
2. Add `NavigationHistory` with `push_if_changed`, back/forward/rewind
3. Extend `SessionContext` with view state and history
4. Add `validate()` debug assertions
5. Unit tests for history stack operations (cursor correctness, dedupe, truncation)

**Deliverable:** Session can track view level & history, but UI unchanged

### Phase 2: State Machine & Transition Validation
**Goal:** Implement validated level transitions

1. Define `NavError` enum with clear error messages
2. Implement transition validation (level preconditions)
3. Add `NavigationService` with command handlers
4. Each handler: validates → computes next state → push_if_changed → update view
5. Unit tests for valid/invalid transitions

**Deliverable:** Navigation commands work with proper error handling

### Phase 3: ESPER Verb Integration
**Goal:** Wire ESPER verbs to navigation service

1. Add `AgentCommand` variants for all navigation verbs
2. Add ESPER YAML config for level transitions
3. Add ESPER YAML config for orbital navigation
4. Add ESPER YAML config for history navigation
5. Integration tests for voice/chat commands

**Deliverable:** Can navigate via chat commands, but rendering unchanged

### Phase 4: Solar Layout Engine
**Goal:** Implement orbital layout computation

1. Create `SolarLayout` module
2. Implement stable ordering for CBUs and ManCos
3. Implement multi-orbit strategy for large ManCos
4. Implement navigation resolution with circular angle distance
5. Golden snapshot tests for layout determinism

**Deliverable:** Can compute orbital positions, but not rendered yet

### Phase 5: Solar Renderer (System Level)
**Goal:** Render ManCo + CBU orbits

1. Create `SolarRenderer` in ob-poc-ui
2. Implement `render_system()` - sun + orbit rings + CBU planets
3. CBU icons with selection state based on `orbit_pos`
4. Integrate with existing egui rendering loop
5. Visual tests

**Deliverable:** Can see solar system view when at System level

### Phase 6: Galaxy Renderer
**Goal:** Render multiple ManCos when zoomed out

1. Implement `render_galaxy()` - ManCo icons only
2. CBU count halo around each ManCo
3. ManCo selection highlight
4. Grid layout with region clustering

**Deliverable:** Full galaxy ↔ system navigation working

### Phase 7: History Navigation & Breadcrumbs
**Goal:** Back/forward/rewind fully working

1. Implement history traversal (note: doesn't push, just moves cursor)
2. UI for breadcrumbs/history display
3. Ensure all nav commands use `push_if_changed` correctly
4. Property tests for history invariants

**Deliverable:** Full time-travel navigation working

### Phase 8: Polish & Edge Cases
**Goal:** Production-ready

1. Handle edge cases: empty orbits, single CBU, etc.
2. Keyboard shortcuts for navigation
3. Smooth animations between levels (optional)
4. Performance optimization for large CBU counts
5. Integration with existing drill/surface entity views

---

## Part 7: Testing Strategy

### 7.1 Golden Snapshot Tests (Layout Determinism)

```rust
#[test]
fn layout_is_deterministic() {
    let mancos = test_mancos();
    let cbus = test_cbus();
    
    let layout1 = SolarLayout::build_system(&mancos[0], &cbus);
    let layout2 = SolarLayout::build_system(&mancos[0], &cbus);
    
    // Positions must be identical
    assert_eq!(
        serde_json::to_string(&layout1).unwrap(),
        serde_json::to_string(&layout2).unwrap()
    );
    
    // Compare against golden file
    insta::assert_json_snapshot!(layout1);
}
```

### 7.2 Property Tests (History)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn cursor_never_out_of_bounds(operations in prop::collection::vec(any::<HistoryOp>(), 0..100)) {
        let mut history = NavigationHistory::new(50);
        
        for op in operations {
            match op {
                HistoryOp::Push(state) => history.push_if_changed(state),
                HistoryOp::Back => { history.back(); }
                HistoryOp::Forward => { history.forward(); }
                HistoryOp::Rewind => { history.rewind(); }
            }
            
            // Invariant: cursor always valid
            if let Some(c) = history.cursor {
                assert!(c < history.len());
            }
        }
    }
    
    #[test]
    fn back_then_push_truncates_future(states in prop::collection::vec(any::<ViewState>(), 3..10)) {
        let mut history = NavigationHistory::new(50);
        
        for s in &states {
            history.push_if_changed(s.clone());
        }
        
        history.back();
        history.back();
        
        let new_state = ViewState::galaxy(Utc::now());
        history.push_if_changed(new_state);
        
        // Future should be truncated
        assert!(history.forward().is_none());
    }
}
```

### 7.3 State Machine Tests (Transitions)

```rust
#[test]
fn invalid_transitions_return_error() {
    let mut nav = NavigationService::new(test_session());
    
    // Can't land from Galaxy (must be at System)
    let result = nav.land_on(test_cbu_id());
    assert!(matches!(result, Err(NavError::InvalidTransition { .. })));
    
    // Can't drill from Galaxy (must be at Planet)
    let result = nav.drill();
    assert!(matches!(result, Err(NavError::InvalidTransition { .. })));
}

#[test]
fn valid_transitions_update_state() {
    let mut nav = NavigationService::new(test_session_at_galaxy());
    
    // Galaxy → System
    let result = nav.enter_system(test_manco_id());
    assert!(result.is_ok());
    assert_eq!(nav.session.view.level, ViewLevel::System);
    
    // System → Planet
    let result = nav.land_on(test_cbu_id());
    assert!(result.is_ok());
    assert_eq!(nav.session.view.level, ViewLevel::Planet);
}
```

---

## Part 8: Open Questions (Updated)

### Q1: Orbit Assignment
How should CBUs be assigned to orbits?
- **Option A: By capacity only** ✓ (implemented above)
- Option B: By type (funds inner, SPVs outer)
- Option C: By size/AUM
- Option D: Configurable per ManCo

**Decision:** Start with A (capacity-based), add semantic grouping later.

### Q2: Galaxy Layout
How should ManCos be positioned at Galaxy level?
- **Option A: Grid layout** ✓ (implemented above)
- Option B: Cluster by region
- Option C: Circular layout
- Option D: Force-directed

**Decision:** Grid with stable ordering. Region clustering can be added.

### Q3: Wrap-Around
When navigating clockwise past the last CBU:
- **Option A: Wrap to first CBU** ✓ (implemented above)

**Decision:** Wrap around, feels natural for circular layout.

### Q4: History Persistence
Should navigation history persist across sessions?
- **Option A: Memory only** ✓
- Option B: Persist to DB

**Decision:** Memory only for v1. Add persistence if users request it.

### Q5: Animation
Should level transitions animate?
- **Option A: Instant snap** ✓
- Option B: Smooth zoom/fade

**Decision:** Instant for v1. Add animations for polish later.

---

## Part 9: Success Criteria

The implementation is complete when:

1. **Session state tracks view level and history correctly**
   - ViewLevel enum with validated transitions
   - History with correct cursor handling and dedupe
   - All invariants hold under property testing

2. **ESPER verbs control navigation**
   - Level transitions: enter, orbit, land, drill, surface
   - Same-level: clockwise, counter-clockwise, inner, outer, jump
   - History: back, forward, rewind
   - Invalid commands return helpful error messages

3. **Solar layout is deterministic**
   - Stable ordering rules enforced
   - Golden snapshot tests pass
   - No jitter between renders

4. **Rendering works at all levels**
   - Galaxy level: ManCo icons with CBU count halos
   - System level: ManCo sun + CBU orbits with selection
   - Planet level: Single CBU expanded
   - Surface level: Existing entity layout

5. **Navigation feels natural**
   - User can explore intuitively
   - "Back" always works as expected
   - No dead ends or confusing states

6. **Performance acceptable**
   - 60fps rendering maintained
   - Navigation commands respond instantly (< 100ms)

---

## Appendix A: Command Quick Reference

| Command | From Level | To Level | Precondition |
|---------|------------|----------|--------------|
| `enter <manco>` | Galaxy | System | ManCo in scope |
| `orbit` | System | Galaxy | - |
| `land on <cbu>` | System | Planet | CBU in scope |
| `orbit` | Planet | System | - |
| `drill` | Planet | Surface | - |
| `surface` | Surface | Planet | - |
| `clockwise [N]` | System | System | orbit_pos set |
| `counter-clockwise [N]` | System | System | orbit_pos set |
| `inner [N]` | System | System | ring > 0 |
| `outer [N]` | System | System | ring < max |
| `jump N dir` | System | System | orbit_pos set |
| `nav back` | Any | Previous | history.cursor > 0 |
| `nav forward` | Any | Next | cursor < len-1 |
| `rewind` | Any | Start | history not empty |

---

## Appendix B: Visual Reference

```
GALAXY LEVEL
┌────────────────────────────────────────────┐
│                                            │
│    ☀️ LUX          ☀️ IE                   │
│    (50)            (30)                    │
│                                            │
│           ☀️ DE                            │
│           (25)                             │
│                                            │
│    ☀️ UK           ☀️ CH                   │
│    (15)            (10)                    │
│                                            │
└────────────────────────────────────────────┘
        │
        │ "enter Lux"
        ▼
SYSTEM LEVEL (Lux ManCo) - Multiple Orbits
┌────────────────────────────────────────────┐
│                                            │
│        🌍 🌍 🌍 🌍 🌍 🌍                   │  ← Outer orbit (ring 1)
│      🌍               🌍                   │
│    🌍    🌍 🌍 🌍 🌍    🌍                 │  ← Inner orbit (ring 0)
│    🌍  🌍   ☀️    🌍  🌍                   │
│    🌍    🌍 LUX 🌍    🌍                   │
│      🌍   ManCo    🌍                      │
│        🌍 🌍 🌍 🌍 🌍                      │
│                                            │
│  [current: ring=0, index=5 ←]             │
│                                            │
└────────────────────────────────────────────┘
        │
        │ "land on Fund C"
        ▼
PLANET LEVEL (Fund C)
┌────────────────────────────────────────────┐
│  ┌──────────────────────────────────────┐  │
│  │                                      │  │
│  │   AGI Equity Fund C                  │  │
│  │   🇱🇺 Luxembourg                     │  │
│  │                                      │  │
│  │   Products: CUSTODY, FUND_ACCT, TA   │  │
│  │   Status: Active                     │  │
│  │   AUM: €2.5B                         │  │
│  │                                      │  │
│  │   [drill to see entities]           │  │
│  │                                      │  │
│  └──────────────────────────────────────┘  │
└────────────────────────────────────────────┘
        │
        │ "drill"
        ▼
SURFACE LEVEL (Entity Graph - Fund C)
┌────────────────────────────────────────────┐
│                                            │
│              [CBU Node]                    │
│                  │                         │
│     ┌───────────┼───────────┐              │
│     │           │           │              │
│  [ManCo]   [Principal]  [Admin]            │
│     │           │           │              │
│  ┌──┴──┐     ┌──┴──┐     ┌──┴──┐           │
│  [Dir] [Dir] [UBO] [UBO] [Contact]         │
│                                            │
└────────────────────────────────────────────┘
```

---

## Appendix C: Changelog

### v4 (2026-01-19) - Final Review Refinements

**Issues fixed from v3 review:**
- ✅ `orbit_pos` initialization: Now explicitly initializes to `(0,0)` when `None` instead of silently becoming `(0,1)`
- ✅ `ViewState::planet()`: Now accepts `parent_manco_id` parameter for clean `zoom_out` behavior
- ✅ Added `planet_simple()` convenience constructor

**New utility sections added (Part 4e):**
- ✅ `NavTelemetry` trait and `NavEvent` for audit trails
- ✅ `breadcrumb_label()` with `NameResolver` trait for human-readable UI
- ✅ `to_compact_string()` / `from_compact_string()` for URL sharing
- ✅ `LayoutCache` for performance with 50+ CBU systems
- ✅ Custom `Arbitrary` impl for `ViewState` in property tests

**Review verdict:** ✅ Ready for implementation

### v3 (2026-01-19) - Implementation-Ready with Verb Handlers

**New sections added (from ChatGPT second round):**
- ✅ **Part 4.5 rewritten:** History Navigation Verbs with execution contract, pseudocode, browser-semantics for back-then-push
- ✅ **Part 4.6:** Normal Navigation Verbs with zoom_in/zoom_out/orbit_next pseudocode
- ✅ **Part 4b:** Navigation Transition Matrix - compiler-like level × verb table
- ✅ **Part 4c:** Verb Handler Skeleton - `commit_view()` pattern, hard rules for code review
- ✅ **Part 4d:** Determinism + Layout Contract - scope versioning, animation contract

**Implementation guardrails:**
- Clear "history verbs NEVER push" rule
- Error/NoOp/Success distinction with consistent handling
- `NavResult` enum for all handlers
- Breadcrumb UI guidelines

### v2 (2026-01-19) - ChatGPT Review Fixes

**Must-fix issues addressed:**
1. ✅ `NavigationHistory::push()` cursor bug fixed - uses `VecDeque`, proper cursor adjustment
2. ✅ `ViewState` ambiguity resolved - `orbit_pos` is canonical, `focus_cbu_id` derived
3. ✅ Layout ordering stability - explicit `order_cbus()`, `order_mancos()` functions
4. ✅ `unwrap()` removed - separate `ManCoInfo` type, returns `Result`
5. ✅ Circular angle distance - `circular_angle_distance()` function handles wrap-around

**Design improvements added:**
- ✅ Non-goals section (explicit exclusions)
- ✅ Determinism contract (invariants)
- ✅ Transition matrix (state machine)
- ✅ Preconditions per command
- ✅ `push_if_changed()` with dedupe
- ✅ `RenderTransform` separate from `ViewState`
- ✅ Multi-orbit strategy implemented
- ✅ Testing strategy (golden snapshots, property tests, state machine tests)
- ✅ Renamed "Surface" references to "Surface/Entity Graph" for clarity

### v1 (2026-01-19) - Initial Draft

- Core design: navigation = state = layout
- Astronomical metaphor (Universe → Galaxy → System → Planet → Surface)
- Snap zoom levels
- Navigation history with time-travel
- ESPER verb integration

---

*End of Design Spec v4*
