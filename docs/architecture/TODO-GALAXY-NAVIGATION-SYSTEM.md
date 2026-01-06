# Galaxy Navigation System: The Definitive Brief

## Classification: AMBITIOUS BASELINE

**Purpose:** This is the single source of truth for implementing the galaxy navigation system. It captures the full vision - the experience, the intelligence, the mechanics, the code. Claude Code should read this and implement.

**Philosophy:** There are virtually zero screens/menus in this model. The visual navigation IS the interface. The system must be so intelligent that users never say "this is a pain, the agent is dumb." It anticipates. It assists. It gets out of the way when you know where you're going. It helps when you don't.

---

## Pre-Implementation Audit Results

> **Audit Date:** 2025-01-06
> **Status:** Significant infrastructure exists. Client-side galaxy/astronomy modules are UNFINISHED STUBS requiring reimplementation.

### What Already Exists (DO NOT RECREATE)

#### Database Schema
- ✅ `cbus.commercial_client_entity_id` FK → links CBUs to commercial clients (apex entities)
- ✅ `cbus.jurisdiction`, `cbus.client_type`, `cbus.risk_context` → clustering attributes
- ✅ No new tables needed - existing schema supports galaxy navigation

#### Server-Side Types (rust/src/taxonomy/)
- ✅ `TaxonomyContext::Universe` - all CBUs context
- ✅ `TaxonomyContext::Book { client_id }` - commercial client book context  
- ✅ `MembershipRules::book(client_id)` - filtering rules for book view
- ✅ `RootFilter::Client { client_id }` - root filtering by client

#### Server-Side Queries (rust/src/taxonomy/builder.rs)
- ✅ `load_client_cbus(pool, client_id)` - queries CBUs by `commercial_client_entity_id`
- ✅ `load_all_cbus()` - queries all CBUs for universe view

#### Server-Side Graph Types (rust/src/graph/types.rs)
- ✅ `GraphScope::Book { apex_entity_id, apex_name }` - book scope enum
- ✅ `GraphScope::SingleCbu`, `Jurisdiction`, `EntityNeighborhood` variants
- ✅ `EntityGraph` with nodes, edges, scope

#### Server-Side Endpoints (rust/src/api/graph_routes.rs)
- ✅ `GET /api/graph/book/:apex_entity_id` - book graph endpoint EXISTS
- ⚠️ `GET /api/universe` - MISSING, needs implementation

#### Client-Side State (rust/crates/ob-poc-ui/src/state.rs)
- ✅ `AsyncState` has 50+ `pending_*` navigation commands already wired:
  - `pending_scale_universe`, `pending_scale_galaxy`, `pending_scale_system`
  - `pending_taxonomy_zoom_in`, `pending_taxonomy_zoom_out`, `pending_taxonomy_back_to`
  - `pending_drill_through`, `pending_orbit`, `pending_time_rewind`, etc.
- ✅ Commands trigger in `app.rs` but handlers are TODO stubs

### What Exists But Is UNFINISHED (Requires Reimplementation)

#### Client-Side Galaxy (rust/crates/ob-poc-graph/src/graph/galaxy.rs)
- ⚠️ `GalaxyView` struct exists with force simulation
- ⚠️ `ClusterData`, `GalaxyAction`, `RiskSummary` types exist
- ❌ Uses `load_mock_data()` - NOT wired to server
- ❌ NOT imported or used in ob-poc-ui
- **Decision:** Rewrite with proper server integration

#### Client-Side Astronomy (rust/crates/ob-poc-graph/src/graph/astronomy.rs)
- ⚠️ `ViewTransition`, `AstronomyView` types exist
- ⚠️ Transition logic exists but is scaffold only
- ❌ NOT imported or used in ob-poc-ui
- **Decision:** Rewrite as part of unified NavigationService

### What Does Not Exist (Must Implement)

- ❌ `GET /api/universe` endpoint with cluster aggregation
- ❌ `UniverseGraph`, `ClusterNode` shared types in ob-poc-types
- ❌ Unified `NavigationService` (single service pattern)
- ❌ Navigation physics (momentum, camera lead, springs)
- ❌ Level transition animations wired to real data
- ❌ Agent overlay (suggestions, speech, anomaly badges)
- ❌ Autopilot mission execution
- ❌ Fork/branch presentation UI

### Architecture Principles (USER DIRECTION)

1. **One Struct/Service Pattern** - Single `NavigationService` orchestrates all navigation, not multiple parallel systems
2. **Common Rust Structs** - Share types between server and client via `ob-poc-types` crate
3. **EGUI Rules At All Times** - Server data read-only, UI-only state ephemeral, no callbacks, return values only
4. **Server is Source of Truth** - Client fetches and renders, never modifies server data locally

---

## PEER REVIEW NOTES (Resolve Before/During Phase 0)

> **Review Date:** 2025-01-06
> **Reviewer:** Claude Web (Architecture Session)
> **Status:** APPROVED with items below to resolve

### Questions to Answer Before Starting

1. **Camera2D.fly_to() signature** - Does it accept spring config or just target position? Need to know if physics is built-in or needs wrapping. Check `ob-poc-graph/src/graph/camera.rs`.

2. **pending_scale_* flow** - What's the relationship between `pending_scale_universe` in AsyncState and the current view state? Trace the existing flow in `app.rs` to understand how to wire NavigationService correctly.

3. **GraphScope sharing** - Is `GraphScope` in `rust/src/graph/types.rs` suitable for sharing via ob-poc-types, or does it have server-only dependencies (like sqlx)? May need a client-side equivalent or a shared subset.

4. **animation.rs contents** - Audit says `SpringF32, SpringVec2 (can reuse)` exist. Confirm these match the `SpringConfig` pattern in Part 5 of this brief. If different, adapt the brief's types to match existing implementation.

### ✅ ANSWERS (Code Inspection 2025-01-06)

**1. Camera2D.fly_to() - ANSWERED**
```rust
// From ob-poc-graph/src/graph/camera.rs
pub fn fly_to(&mut self, world_pos: Pos2)  // Basic - uses SpringConfig::MEDIUM
pub fn fly_to_with_config(&mut self, world_pos: Pos2, config: SpringConfig)  // Configurable
pub fn fly_to_slow(&mut self, world_pos: Pos2)   // SLOW preset
pub fn fly_to_fast(&mut self, world_pos: Pos2)   // FAST preset  
pub fn fly_to_bounds(&mut self, bounds: Rect, screen_rect: Rect, padding: f32)  // Fit to bounds
```
- **Physics is BUILT-IN** - Camera2D already uses SpringVec2 internally
- **Uses Pos2 (2D)** - NOT Vec3. The brief's `camera_pos: Vec3` references are WRONG
- **ACTION:** Update Part 5 types to use `Pos2` not `Vec3`

**2. pending_scale_* flow - ANSWERED**
```
state.rs:365  → field: pending_scale_universe: bool
state.rs:944  → take method: take_pending_scale_universe() -> bool
app.rs:632    → handler: if self.state.take_pending_scale_universe() { ... }
app.rs:2057   → setter: self.state.set_pending_scale_universe()
```
- **Pattern:** Setter sets flag → update() loop calls take → handler runs once
- **Current handlers are TODO stubs** - just call `zoom_fit()`
- **50+ commands already wired** - NavigationService can hook into existing pattern
- **EGUI-COMPLIANT** - follows immediate mode: flag → take → action, no retained callbacks

**3. GraphScope sharing - ANSWERED: NOT SHAREABLE**
```
rust/src/graph/types.rs:1663  → GraphScope enum (SERVER-ONLY)
rust/crates/ob-poc-types/     → GraphScope NOT PRESENT
```
- **GraphScope is SERVER-ONLY** - not in ob-poc-types crate
- **Has server dependencies** - uses Uuid directly (could share), but designed for server context
- **ACTION:** Create `NavigationScope` in ob-poc-types as client-side equivalent:
  ```rust
  pub enum NavigationScope {
      Universe,
      Book { apex_entity_id: String, apex_name: String },
      Cluster { cluster_id: String },
      Cbu { cbu_id: String },
      Entity { entity_id: String },
  }
  ```

**4. animation.rs SpringF32/SpringVec2 - ANSWERED: FULLY COMPATIBLE**
```rust
// From ob-poc-graph/src/graph/animation.rs (~250 lines, complete implementation)
pub struct SpringConfig {
    pub stiffness: f32,
    pub damping: f32,
}
impl SpringConfig {
    pub const FAST: Self = Self { stiffness: 300.0, damping: 1.0 };
    pub const MEDIUM: Self = Self { stiffness: 150.0, damping: 1.0 };
    pub const SLOW: Self = Self { stiffness: 80.0, damping: 1.0 };
    pub const BOUNCY: Self = Self { stiffness: 200.0, damping: 0.6 };
    pub const INSTANT: Self = Self { stiffness: 500.0, damping: 1.0 };
}

pub struct SpringF32 { current, target, velocity, config }
pub struct SpringVec2 { x: SpringF32, y: SpringF32 }

// Has egui interop:
impl SpringVec2 {
    pub fn from_pos2(pos: egui::Pos2) -> Self
    pub fn get_pos2(&self) -> egui::Pos2
}
```
- **MATCHES Part 5 pattern** - presets align, API is compatible
- **egui::Pos2 interop exists** - no wrapper needed
- **ACTION:** Reuse as-is, no changes needed

### Critical Corrections for Brief

| Brief Reference | Current State | Correction |
|-----------------|---------------|------------|
| `camera_pos: Vec3` | Camera2D uses `Pos2` | Change to `Pos2` |
| `camera_target: Vec3` | Camera2D uses `Pos2` | Change to `Pos2` |
| GraphScope shared | Server-only | Create `NavigationScope` in ob-poc-types |
| Spring physics | Exists in animation.rs | Reuse, don't recreate |

### Type Definitions to Complete in Phase 0

The shared types in Part 5 reference but don't fully define these. Complete them in `ob-poc-types/src/galaxy.rs`:

| Type | Notes |
|------|-------|
| `ClusterEdge` | Referenced in `UniverseGraph.cluster_edges` |
| `SharedEntityNode` | Referenced in `ClusterDetailGraph` |
| `SharedEntityEdge` | Referenced in `ClusterDetailGraph` |
| `RiskRating` | Enum for LOW/MEDIUM/HIGH/UNRATED |
| `Badge` | Used in `PreviewItem.badges` |
| `FilterCriteria` | Used in `SuggestedAction::Filter` |
| `ExpansionType` | Used in `SuggestedAction::Expand` |
| `PreviewType` | Used in `PreviewData.preview_type` |
| `NodeType` | Used in `ClusterNode.node_type`, `PreviewItem.node_type` |
| `ClusterType` | Enum for JURISDICTION/CLIENT/RISK/PRODUCT |
| `UniverseStats` | Referenced in `UniverseGraph.stats` |
| `AnomalySeverity` | Referenced in `Anomaly.severity` |
| `PrefetchStatus` | Referenced in `AgentState.prefetch_cache` |
| `Vec2` | Import from egui or define in ob-poc-types |

### Camera Vec2 vs Vec3 Alignment

Part 5 defines:
```rust
pub camera_pos: Vec3,
pub camera_target: Vec3,
```

But egui is 2D and existing `Camera2D` likely uses `Vec2`. **Align these types with the existing camera implementation.** If Camera2D uses Vec2, change Part 5 types to match. Don't introduce Vec3 unless there's a reason.

### Existing Type Reuse Verification

Before creating new types, check if these already exist and can be reused:

| Brief Type | Check Location | Action |
|------------|----------------|--------|
| `RiskSummary` | `galaxy.rs` (existing stub) | Reuse if compatible |
| `ClusterData` | `galaxy.rs` (existing stub) | Reuse or align with `ClusterNode` |
| `GalaxyAction` | `galaxy.rs` (existing stub) | Reuse or extend |
| `ViewTransition` | `astronomy.rs` (existing stub) | Evaluate if salvageable |
| `SpringF32` | `animation.rs` | Reuse, don't recreate |

### Implementation Notes

1. **If audit assumptions prove incorrect** - Document the discrepancy and adjust. Don't force-fit the brief if reality differs.

2. **Type naming conflicts** - If existing types have same names but different shapes, prefer extending existing over creating parallel types. One source of truth.

3. **egui immediate mode** - Remember: no retained state in widgets. NavigationService holds all state, widgets read and render only.

4. **Compile early, compile often** - After creating galaxy.rs types, run `cargo check --package ob-poc-types` before proceeding. Type errors compound.

---

## Part 1: The Experience

### 1.1 You Are the Pilot

You are piloting a semi-autonomous craft through a data structure. Like the sentinel operators in The Matrix - you give intent, the craft executes. You can take manual control, or let it navigate.

The data isn't a diagram you're looking at. It's a space you're inside. Clusters are regions. CBUs are chambers. Entities are nodes you can approach. Relationships are tunnels connecting them.

**You never:**
- Click a button to "go to page"
- Wait for a loading screen
- See a modal dialog
- Navigate a menu hierarchy
- Read instructions

**You always:**
- Fly through continuous space
- See your destination before arriving
- Feel momentum and physics
- Have peripheral awareness
- Can interrupt, reverse, redirect

### 1.2 The Craft Has Intelligence

The craft isn't a dumb camera. It's a semi-autonomous agent that:

**Anticipates:**
- Notices where you're looking and pre-fetches that data
- Highlights anomalies ("3 high-risk entities in this cluster")
- Suggests relevant paths ("The ManCo connects to 35 other CBUs")
- Warns about dead ends ("This entity has no further ownership chain")

**Autopilots:**
- "Take me to Fund Alpha" → plots route, flies there, you watch
- "Find high-risk entities in Luxembourg" → searches, highlights, offers tour
- "Show me the ownership chain" → expands inline, no manual clicking
- "Compare these two funds" → splits view, aligns comparable elements

**Loiters:**
- At decision points, craft hovers and waits
- Presents options visually (branches ahead)
- Doesn't rush you - you steer when ready
- If you do nothing for 3 seconds, offers suggestions

**Assists:**
- First-time users get gentle guidance
- Complex structures get summarized ("47 CBUs, 12 shared entities")
- Deep dives get breadcrumb awareness ("You're 5 levels deep")
- Anomalies get flagged without asking

### 1.3 The Navigation Feel

| Quality | Implementation |
|---------|----------------|
| **Continuous** | No page loads. Camera flies. Transitions are animated. |
| **Physical** | Momentum, acceleration, braking. Movement has weight. |
| **Spatial** | Depth feels deep. Width feels wide. Position means something. |
| **Peripheral** | You see more than focus. Siblings, branches, context. |
| **Reversible** | Any move can undo. Flight can abort. You're in control. |
| **Anticipatory** | Camera leads. Data pre-fetches. System knows where you're going. |

---

## Part 2: The Agent Intelligence Layer

### 2.1 Intent Recognition

The system continuously infers user intent from:

| Signal | Inference |
|--------|-----------|
| Mouse hover duration | Interest level (>500ms = curious, >2s = focused) |
| Mouse direction | Intended destination |
| Scroll velocity | Desired speed (slow = exploring, fast = searching) |
| Pause at fork | Decision needed - offer help |
| Rapid back-forth | Lost/confused - offer orientation |
| Voice tone | Urgency (calm exploration vs. "find this NOW") |

### 2.2 Proactive Behaviors

The craft should DO things without being asked:

**Pre-fetch:** When you hover toward a node, start loading its children.
```rust
// On hover > 300ms toward unloaded node
if hover_duration > 0.3 && !node.children_loaded {
    spawn_background_fetch(node.id, FetchPriority::Speculative);
}
```

**Highlight Anomalies:** Flag things that need attention.
```rust
// On entering any scope, check for anomalies
let anomalies = detect_anomalies(&current_scope);
for anomaly in anomalies {
    // Don't interrupt, just mark
    mark_node_with_badge(anomaly.node_id, anomaly.badge_type);
    // e.g., RED_DOT for high risk, YELLOW_DOT for incomplete, LINK for shared
}
```

**Suggest Next:** After completing an action, suggest logical next steps.
```rust
// After arriving at destination
let suggestions = infer_next_actions(&context, &user_history);
show_subtle_hints(suggestions);  // Ghost arrows, dim labels, NOT modal dialogs
```

**Orient When Lost:** Detect confusion, offer help.
```rust
// If user has reversed > 3 times in 10 seconds
if reversal_count > 3 && time_window < 10.0 {
    show_orientation_helper();  // Mini-map pulse, breadcrumb highlight
    agent_speak("You're in Luxembourg > Allianz. Looking for something specific?");
}
```

### 2.3 Autopilot Mode

When user gives a destination, craft flies there autonomously:

```rust
pub struct AutopilotMission {
    /// Final destination
    destination: NodeId,
    
    /// Planned route (computed via graph traversal)
    route: Vec<NodeId>,
    
    /// Current leg of journey
    current_leg: usize,
    
    /// Speed (can be adjusted mid-flight)
    speed: AutopilotSpeed,
    
    /// Whether to pause at decision points
    pause_at_forks: bool,
    
    /// Whether to narrate the journey
    narrate: bool,
}

impl AutopilotMission {
    pub fn execute(&mut self, dt: f32) -> AutopilotStatus {
        let target = self.route[self.current_leg];
        
        // Fly toward current waypoint
        let arrived = fly_toward(target, self.speed, dt);
        
        if arrived {
            if self.current_leg == self.route.len() - 1 {
                return AutopilotStatus::Arrived;
            }
            
            // Check for fork
            let next = self.route[self.current_leg + 1];
            if self.pause_at_forks && is_decision_point(target) {
                return AutopilotStatus::PausedAtFork { 
                    options: get_branches(target),
                    recommended: next,
                };
            }
            
            self.current_leg += 1;
        }
        
        AutopilotStatus::InFlight { 
            progress: self.current_leg as f32 / self.route.len() as f32,
            eta: estimate_arrival(self.current_leg, self.route.len(), self.speed),
        }
    }
}
```

### 2.4 Loiter Behavior

At decision points, the craft doesn't just stop - it **presents the choice**:

```rust
pub struct LoiterState {
    /// How long we've been hovering
    duration: f32,
    
    /// Available branches
    branches: Vec<BranchOption>,
    
    /// Which branch is currently highlighted (mouse/gaze)
    highlighted: Option<usize>,
    
    /// Preview data for highlighted branch
    preview: Option<PreviewData>,
    
    /// Agent suggestions (if any)
    suggestions: Vec<AgentSuggestion>,
}

impl LoiterState {
    pub fn update(&mut self, dt: f32, input: &Input) {
        self.duration += dt;
        
        // After 1.5 seconds, start showing previews for highlighted branch
        if self.duration > 1.5 && self.highlighted.is_some() {
            self.fetch_preview_if_needed();
        }
        
        // After 3 seconds with no input, agent offers help
        if self.duration > 3.0 && !input.any_recent_activity() {
            self.generate_suggestions();
        }
        
        // Animate branches to "breathe" - subtle pulse to show they're live
        for branch in &mut self.branches {
            branch.pulse_phase += dt * 0.5;
        }
    }
}
```

### 2.5 Agent Communication

The agent can "speak" without voice (or with voice if enabled):

**Visual speech:** Subtle text that fades in near relevant elements.
```
[Near a high-risk cluster]
"3 entities flagged for review"

[After user pauses at fork]
"The ManCo path leads to 35 connected funds"

[After deep dive]
"You're 5 levels deep. Surface?"
```

**Rules for agent speech:**
1. Never interrupt active navigation
2. Never modal/block
3. Fade in over 300ms, hold 3s, fade out
4. Position near relevant element, not center screen
5. Maximum 10 words
6. Actionable when possible ("Surface?" not just "You're deep")

---

## Part 3: The Visual Language

### 3.1 Depth Encoding

Depth in the taxonomy is encoded visually:

| Depth | Zoom | FOV | Lighting | Particle Density | Tunnel Width |
|-------|------|-----|----------|------------------|--------------|
| 0 (Universe) | 0.3x | 120° | Bright, open | Low | Very wide |
| 1 (Cluster) | 0.5x | 100° | Warm | Medium | Wide |
| 2 (CBU) | 1.0x | 80° | Neutral | Medium | Normal |
| 3 (Entity) | 1.5x | 60° | Focused | High | Narrow |
| 4+ (Deep) | 2.0x+ | 45° | Intimate | Very high | Very narrow |

### 3.2 Node Types and Rendering

| Level | Node Type | Shape | Size | Color Logic | Glow |
|-------|-----------|-------|------|-------------|------|
| 0 | JurisdictionCluster | Sphere | ∝ sqrt(cbu_count) | Risk dominant | Yes, pulses |
| 0 | ClientCluster | Sphere | ∝ sqrt(cbu_count) | Client brand | Yes |
| 1 | CbuCard | Rounded rect | Fixed | Risk rating | Badge |
| 1 | SharedEntityHub | Diamond | ∝ connection_count | Highlight | Yes |
| 2 | LegalEntity | Rectangle | Fixed | Entity type | On focus |
| 2 | NaturalPerson | Circle | Fixed | Role type | On focus |
| 2 | TradingProfile | Hexagon | Fixed | Blue | On focus |
| 3 | Document | Paper icon | Small | Status color | On hover |
| 3 | OwnershipNode | Rectangle | Smaller | Depth fade | Chain glow |

### 3.3 Edge Types and Rendering

| Edge Type | Line Style | Thickness | Color | Animation |
|-----------|-----------|-----------|-------|-----------|
| ClusterProximity | Dotted | 1px | Gray 30% | None |
| SharedEntity | Dashed | 2px | Teal | Pulse on hover |
| CbuRole | Solid | 2px | Role color | Flow particles |
| Ownership | Solid + arrow | 3px | Purple gradient | Upward flow |
| Control | Dashed + arrow | 2px | Orange | None |
| TradingLink | Solid | 1px | Blue | None |
| DocumentLink | Dotted | 1px | Gray | None |

### 3.4 Risk Encoding

Risk is ALWAYS visible, never hidden:

| Risk Level | Color | Badge | Glow | Sound (if enabled) |
|------------|-------|-------|------|-------------------|
| LOW | Green #4CAF50 | ● | Soft green | None |
| MEDIUM | Amber #FF9800 | ◐ | Warm amber | None |
| HIGH | Red #F44336 | ○ | Urgent red pulse | Soft ping on approach |
| UNRATED | Gray #9E9E9E | ◌ | None | None |

### 3.5 State Encoding

| State | Visual Treatment |
|-------|------------------|
| Normal | Full opacity, normal size |
| Hovered | +20% size, glow, preview loading indicator |
| Focused | +30% size, strong glow, full detail visible |
| Selected | Persistent highlight ring |
| Loading | Skeleton shimmer |
| Expanded | Children visible, parent anchored |
| Collapsed | Children hidden, expansion indicator |
| Anomaly | Badge + subtle pulse |
| Stale | Slight desaturation |

---

## Part 4: Navigation Mechanics

### 4.1 Input Mapping

| Input | Action | Notes |
|-------|--------|-------|
| Mouse position | Look direction | Continuous, affects camera pan |
| Mouse toward edge | Bank/turn | Camera rotates toward mouse |
| Left click | Commit/thrust | Move toward highlighted target |
| Right click | Back/brake | Reverse or cancel |
| Scroll up | Speed up | Increase approach velocity |
| Scroll down | Slow down | Decrease velocity or reverse |
| Middle click | Anchor | Mark current position |
| Double click | Dive in | Fast commit to target |
| Escape | Full stop | Halt all movement |
| Space | Toggle autopilot | Enable/disable agent control |

### 4.2 Voice Commands

| Command Pattern | Action | Example |
|-----------------|--------|---------|
| "Go to {target}" | Autopilot to destination | "Go to Fund Alpha" |
| "Show {aspect}" | Expand inline | "Show ownership" |
| "Find {criteria}" | Search and highlight | "Find high-risk entities" |
| "Compare {a} and {b}" | Split view | "Compare Fund A and Fund B" |
| "Back" / "Surface" | Navigate up | "Surface to cluster" |
| "Faster" / "Slower" | Adjust speed | "Faster" |
| "Stop" | Halt movement | "Stop" |
| "What's this?" | Agent explains focus | "What's this?" |
| "Where am I?" | Orientation help | "Where am I?" |
| "Take me home" | Return to universe | "Take me home" |

### 4.3 Gesture Patterns (Future: Touch/VR)

| Gesture | Action |
|---------|--------|
| Pinch out | Zoom in / dive deeper |
| Pinch in | Zoom out / surface |
| Two-finger drag | Pan |
| Swipe left/right | Cycle siblings |
| Hold + drag | Grab and move node (if editable) |
| Double tap | Dive in |
| Long press | Context/preview |

### 4.4 Physics Parameters

```rust
pub struct NavigationPhysics {
    // Movement
    pub max_velocity: f32,           // 500.0 units/sec
    pub acceleration: f32,           // 800.0 units/sec²
    pub deceleration: f32,           // 1200.0 units/sec² (braking is faster)
    pub coast_friction: f32,         // 0.95 per frame (slight drag when no input)
    
    // Camera
    pub camera_lead_factor: f32,     // 0.3 (camera moves 30% ahead of you)
    pub camera_smooth: f32,          // 0.15 (lerp factor per frame)
    pub camera_spring: SpringConfig, // { stiffness: 150, damping: 18, mass: 1 }
    
    // Zoom
    pub zoom_speed: f32,             // 0.1 per scroll tick
    pub zoom_smooth: f32,            // 0.1 (lerp factor)
    pub auto_zoom_on_depth: bool,    // true (zoom changes with depth)
    
    // Rotation
    pub bank_speed: f32,             // 2.0 radians/sec
    pub bank_return: f32,            // 0.95 (returns to neutral)
}
```

---

## Part 5: Data Structures

### 5.1 Shared Types (ob-poc-types/src/galaxy.rs)

```rust
//! Galaxy Navigation Types
//! 
//! Shared between server and WASM client.
//! These types define the contract for all galaxy/universe visualization.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Navigation Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationState {
    /// Current position in the taxonomy
    pub position: NavigationPosition,
    
    /// Velocity vector (for physics)
    pub velocity: Vec2,
    
    /// Focus stack (soft focus within current level)
    pub focus_stack: Vec<FocusFrame>,
    
    /// Expansion states for nodes
    pub expansions: HashMap<String, ExpansionState>,
    
    /// Autopilot mission (if active)
    pub autopilot: Option<AutopilotMission>,
    
    /// Agent state
    pub agent: AgentState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationPosition {
    /// Hard navigation level
    pub level: ViewLevel,
    
    /// Scope at current level
    pub scope: NavigationScope,
    
    /// Camera position in world space
    pub camera_pos: Vec3,
    
    /// Camera look target
    pub camera_target: Vec3,
    
    /// Zoom level
    pub zoom: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewLevel {
    Universe,
    Cluster,
    Cbu,
    Entity,
    Deep { depth: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NavigationScope {
    Universe,
    Cluster { cluster_type: ClusterType, cluster_id: String },
    Cbu { cbu_id: Uuid },
    Entity { entity_id: Uuid },
    Deep { path: Vec<Uuid> },
}

// ============================================================================
// Agent Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Current agent mode
    pub mode: AgentMode,
    
    /// Pending suggestions
    pub suggestions: Vec<AgentSuggestion>,
    
    /// Current speech (if any)
    pub speech: Option<AgentSpeech>,
    
    /// Anomalies detected in current scope
    pub anomalies: Vec<Anomaly>,
    
    /// Pre-fetched data
    pub prefetch_cache: HashMap<String, PrefetchStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMode {
    /// Passive - only responds to explicit commands
    Passive,
    
    /// Assist - offers suggestions, highlights anomalies
    Assist,
    
    /// Autopilot - actively navigating to destination
    Autopilot { mission: AutopilotMission },
    
    /// Guide - first-time user mode, more verbose help
    Guide,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSuggestion {
    pub id: String,
    pub text: String,
    pub action: SuggestedAction,
    pub relevance: f32,  // 0.0 to 1.0
    pub position_hint: Option<Vec2>,  // Where to show near
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestedAction {
    NavigateTo { target: String },
    Expand { node_id: String, aspect: ExpansionType },
    Filter { criteria: FilterCriteria },
    Compare { node_a: String, node_b: String },
    Surface,
    Explain { topic: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpeech {
    pub text: String,
    pub position: Vec2,
    pub started_at: f64,
    pub duration: f32,
    pub fade_in: f32,
    pub fade_out: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub node_id: String,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    HighRisk,
    IncompleteData,
    PendingReview,
    RecentChange,
    CircularOwnership,
    MissingDocument,
    ExpiredDocument,
    SanctionsHit,
    PepMatch,
    SharedEntity,
}

// ============================================================================
// Graph Data Types (Server → Client)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniverseGraph {
    pub scope: GraphScope,
    pub as_of: String,  // ISO date
    pub total_cbu_count: usize,
    pub cluster_by: String,
    pub clusters: Vec<ClusterNode>,
    pub cluster_edges: Vec<ClusterEdge>,
    pub stats: UniverseStats,
    pub anomaly_summary: AnomalySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: String,
    pub node_type: NodeType,
    pub cluster_type: ClusterType,
    pub label: String,
    pub short_label: String,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
    pub risk_summary: RiskSummary,
    pub anomaly_count: usize,
    pub suggested_radius: f32,
    pub suggested_color: String,
    pub position: Option<Vec2>,  // Server can suggest, client decides
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterDetailGraph {
    pub cluster: ClusterNode,
    pub cbus: Vec<CbuCardNode>,
    pub shared_entities: Vec<SharedEntityNode>,
    pub shared_edges: Vec<SharedEntityEdge>,
    pub anomalies: Vec<Anomaly>,
    pub agent_insights: Vec<AgentInsight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuCardNode {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub risk_rating: RiskRating,
    pub status: String,
    pub entity_count: usize,
    pub completion_pct: f32,
    pub shared_entity_ids: Vec<Uuid>,
    pub anomalies: Vec<AnomalyType>,
    pub last_reviewed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewData {
    pub node_id: String,
    pub preview_type: PreviewType,
    pub total_count: usize,
    pub items: Vec<PreviewItem>,
    pub has_more: bool,
    pub agent_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewItem {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub badges: Vec<Badge>,
    pub anomaly: Option<AnomalyType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInsight {
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub related_nodes: Vec<String>,
    pub action: Option<SuggestedAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightType {
    RiskConcentration,
    SharedEntityPattern,
    OwnershipComplexity,
    DocumentGap,
    RecentChanges,
    ComplianceAlert,
}
```

### 5.2 Animation Types

```rust
//! Animation and Physics Types

#[derive(Debug, Clone)]
pub struct Spring {
    pub position: f32,
    pub velocity: f32,
    pub target: f32,
    pub config: SpringConfig,
}

#[derive(Debug, Clone, Copy)]
pub struct SpringConfig {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl SpringConfig {
    pub const SNAPPY: Self = Self { stiffness: 300.0, damping: 20.0, mass: 1.0 };
    pub const ORGANIC: Self = Self { stiffness: 180.0, damping: 12.0, mass: 1.0 };
    pub const GENTLE: Self = Self { stiffness: 120.0, damping: 14.0, mass: 1.0 };
    pub const CAMERA: Self = Self { stiffness: 150.0, damping: 18.0, mass: 1.0 };
}

#[derive(Debug, Clone)]
pub struct NodeAnimation {
    /// Growth phase: 0 = invisible, 1 = full size
    pub growth: Spring,
    
    /// Position animation
    pub position: Spring2D,
    
    /// Opacity (for fade in/out)
    pub opacity: Spring,
    
    /// Glow intensity
    pub glow: Spring,
    
    /// Current phase
    pub phase: AnimationPhase,
}

#[derive(Debug, Clone, Copy)]
pub enum AnimationPhase {
    Hidden,
    Budding,      // 0-20%: dot appears
    Sprouting,    // 20-50%: growing
    Unfurling,    // 50-80%: reaching full size
    Settling,     // 80-100%: micro-adjustments
    Visible,      // Stable
    Collapsing,   // Reverse
}

#[derive(Debug, Clone)]
pub struct TransitionAnimation {
    /// From state
    pub from: TransitionState,
    
    /// To state
    pub to: TransitionState,
    
    /// Progress 0.0 to 1.0
    pub progress: f32,
    
    /// Duration in seconds
    pub duration: f32,
    
    /// Easing function
    pub easing: EasingType,
    
    /// Camera path (Bezier or linear)
    pub camera_path: CameraPath,
}

#[derive(Debug, Clone)]
pub enum CameraPath {
    Linear { from: Vec3, to: Vec3 },
    Bezier { points: [Vec3; 4] },
    Flythrough { waypoints: Vec<Vec3> },
}
```

---

## Part 6: Server API

### 6.1 Endpoints

| Endpoint | Purpose | Response Type |
|----------|---------|---------------|
| `GET /api/universe` | Universe clusters | `UniverseGraph` |
| `GET /api/universe/anomalies` | All anomalies | `Vec<Anomaly>` |
| `GET /api/cluster/:type/:id` | Cluster detail | `ClusterDetailGraph` |
| `GET /api/commercial-clients` | Client list | `Vec<CommercialClient>` |
| `GET /api/commercial-client/:id/book` | Client CBUs | `ClusterDetailGraph` |
| `GET /api/node/:id/preview` | Inline preview | `PreviewData` |
| `GET /api/entity/:id/ownership-chain` | Ownership tree | `OwnershipChain` |
| `GET /api/entity/:id/detail` | Full entity | `EntityDetailGraph` |
| `GET /api/cbu/:id/graph` | CBU graph | `CbuGraph` |
| `GET /api/search` | Global search | `SearchResults` |
| `GET /api/route` | Path finding | `Route` |
| `GET /api/compare` | Comparison data | `ComparisonData` |

### 6.2 Query Parameters

**Common to all:**
- `as_of` - Temporal filter (ISO date)
- `include_anomalies` - Include anomaly detection (default: true)
- `include_insights` - Include agent insights (default: true)

**Universe:**
- `cluster_by` - Grouping: `jurisdiction`, `client`, `risk`, `product`

**Preview:**
- `type` - Preview type: `children`, `ownership`, `roles`, `documents`, `history`
- `limit` - Max items (default: 5)
- `include_summary` - Agent summary (default: true)

**Search:**
- `q` - Search query
- `scope` - Limit to scope (optional)
- `types` - Entity types to include

**Route:**
- `from` - Start node ID
- `to` - Destination node ID
- `optimize` - `shortest`, `scenic` (shows more context), `safe` (avoids anomalies)

### 6.3 Response Enrichment

Every response should include agent-relevant data:

```rust
pub struct EnrichedResponse<T> {
    pub data: T,
    pub anomalies: Vec<Anomaly>,
    pub insights: Vec<AgentInsight>,
    pub suggestions: Vec<AgentSuggestion>,
    pub prefetch_hints: Vec<PrefetchHint>,  // What to load next
}

pub struct PrefetchHint {
    pub endpoint: String,
    pub priority: PrefetchPriority,
    pub reason: String,  // "User hovering toward this node"
}
```

---

## Part 7: Implementation Phases (Updated per Audit)

> **Note:** Phases reflect audit findings. Phase 0 establishes foundation using existing infrastructure.
> Week estimates removed per project guidelines - focus on what, not when.

### Phase 0: Foundation (PREREQUISITE)

**Goal:** Establish shared types and single NavigationService. Wire existing pending_* commands.

**Shared Types (ob-poc-types):**
- [ ] Create `ob-poc-types/src/galaxy.rs` with types from Part 5:
  - `UniverseGraph`, `ClusterNode`, `ClusterEdge`, `ClusterType`
  - `ViewLevel`, `NavigationScope`, `NavigationPosition`
  - `RiskSummary`, `AnomalySummary`, `Anomaly`, `AnomalyType`
- [ ] Add to `ob-poc-types/src/lib.rs` exports
- [ ] Run `cargo test --package ob-poc-types export_bindings` for TypeScript

**Server Endpoint:**
- [ ] Create `universe_routes.rs` with `GET /api/universe`
- [ ] Query: Group CBUs by `commercial_client_entity_id` (uses existing FK)
- [ ] Return `UniverseGraph` with clusters aggregated by client/jurisdiction
- [ ] Wire to main router in `api/mod.rs`

**Client NavigationService (SINGLE SERVICE - USER DIRECTIVE):**
- [ ] Create `ob-poc-ui/src/navigation_service.rs`:
  ```rust
  pub struct NavigationService {
      pub level: ViewLevel,
      pub position: NavigationPosition,
      pub scope: NavigationScope,
      pub velocity: Vec2,
      pub camera_target: Vec2,
      // ... physics state
  }
  ```
- [ ] Implement `process_pending_commands(&mut self, state: &mut AsyncState)`
- [ ] Wire existing `pending_scale_universe`, `pending_scale_galaxy`, etc.
- [ ] Remove TODO stubs from `app.rs`, call NavigationService instead

**Client Graph Integration:**
- [ ] Delete mock data from `galaxy.rs`
- [ ] Add `fetch_universe()` async method
- [ ] Wire `GalaxyView` to receive `UniverseGraph` from server

**Test:** `pending_scale_universe` command triggers server fetch and renders clusters.

### Phase 1: Universe View with Real Data

**Goal:** See all CBUs clustered by commercial client. Click cluster to drill down.

**Server:**
- [ ] Add `GET /api/cluster/:type/:id` endpoint returning `ClusterDetailGraph`
- [ ] Use existing `load_client_cbus(pool, client_id)` from taxonomy/builder.rs
- [ ] Add risk_summary aggregation (query kyc status, risk ratings)

**Client:**
- [ ] Implement `ClusterNode` rendering (sphere, size by CBU count, color by risk)
- [ ] Implement click-to-drill: cluster click → `pending_scale_galaxy(cluster_id)`
- [ ] NavigationService updates `level` and `scope` on transition
- [ ] Camera animates using existing `Camera2D.fly_to()`

**Test:** See universe of client clusters. Click Allianz cluster, see Allianz CBUs.

### Phase 2: Level Transitions and Navigation

**Goal:** Smooth fly-through between Universe → Cluster → CBU → Entity.

**Client:**
- [ ] Implement `ViewTransition` state in NavigationService
- [ ] Use existing `SpringF32`/`SpringVec2` from animation.rs for physics
- [ ] Implement camera lead (camera arrives before content loads)
- [ ] Implement depth encoding (background color shifts per ViewLevel)
- [ ] Wire breadcrumb to NavigationService (clicking breadcrumb = navigate)

**Server:**
- [ ] Ensure existing `/api/cbu/:id/graph` returns data compatible with transitions
- [ ] Add `parent_cluster_id` to CBU responses for back-navigation

**Test:** Fly Universe → Cluster → CBU → Entity. Use breadcrumb to fly back.

### Phase 3: Fork Presentation

**Goal:** Branches present themselves. You see before you commit.

**Server:**
- [ ] Create `/api/node/:id/preview` endpoint
- [ ] Return preview items with anomaly flags

**Client:**
- [ ] Implement LoiterState (hover at decision points)
- [ ] Implement branch fan-out rendering (few children: <6)
- [ ] Implement carousel rendering (medium children: 6-20)
- [ ] Implement tunnel grid rendering (many children: 20+)
- [ ] Wire hover → preview loading → preview display
- [ ] Implement branch highlighting on mouse direction

**Test:** Hover at a cluster, see CBU branches fan out. Hover on one, see preview.

### Phase 4: Inline Expansion

**Goal:** "Show ownership" expands inline without navigation.

**Server:**
- [ ] Use existing `/api/entity/:id/ownership-chain` (already has recursive CTE)
- [ ] Create preview types for roles, documents, history

**Client:**
- [ ] Implement FocusStack (soft focus within level)
- [ ] Implement ExpansionState per node
- [ ] Implement expand/collapse animations (growth phases from Appendix A)
- [ ] Implement cascade timing (children start at 60% parent)
- [ ] Implement spatial stability (parent is anchor)

**Test:** Focus on entity, "show ownership", tree expands upward. "Pull back", collapses.

### Phase 5: Agent Intelligence

**Goal:** The craft thinks. It anticipates, suggests, warns.

**Server:**
- [ ] Add `insights` to responses (patterns, anomalies)
- [ ] Add `prefetch_hints` based on navigation patterns
- [ ] Create `/api/route` for path finding

**Client:**
- [ ] Implement AgentState with mode, suggestions, speech
- [ ] Implement pre-fetch on hover (>300ms → speculative load)
- [ ] Implement anomaly badges (on load, mark flagged nodes)
- [ ] Implement agent speech rendering (positioned text, fade)
- [ ] Implement suggestion rendering (ghost arrows, dim labels)

**Test:** Enter cluster with high-risk CBUs, agent badges them.

### Phase 6: Autopilot

**Goal:** "Take me to Fund Alpha" and it flies there.

**Server:**
- [ ] Create `/api/route?from=X&to=Y` endpoint
- [ ] Implement shortest path through taxonomy graph
- [ ] Return waypoints with context

**Client:**
- [ ] Implement AutopilotMission in NavigationService
- [ ] Implement route visualization (path lights up)
- [ ] Implement waypoint-to-waypoint execution
- [ ] Implement abort on any input

**Test:** "Go to Fund Alpha", watch it fly there. Click mid-flight, it stops.

### Phase 7: Voice Commands

**Goal:** Voice controls navigation.

**Note:** Voice infrastructure already exists (see CLAUDE.md Voice Recognition section).

**Client:**
- [ ] Wire existing `ob-semantic-matcher` to navigation commands
- [ ] Map voice patterns to NavigationService methods
- [ ] Implement command parsing (fuzzy match entity names via EntityGateway)

**Test:** Say "Show me high-risk entities in Luxembourg", it navigates and filters.

### Phase 8: Polish

**Goal:** It feels amazing.

- [ ] Tune spring physics (use Appendix B configurations as starting point)
- [ ] Add depth atmosphere particles
- [ ] Tune agent verbosity (not annoying, not silent)
- [ ] Performance optimization (use existing LOD from lod.rs)
- [ ] Accessibility (keyboard nav, screen reader support)

---

## Part 8: Success Criteria

### User Never Says:

❌ "Where am I?"
→ Depth is always visible. Breadcrumb is felt, not just displayed.

❌ "How do I get back?"
→ Back is always available. Right-click, "back", escape all work.

❌ "This is taking forever"
→ Everything is animated. Progress is visible. Pre-fetch hides latency.

❌ "What does this mean?"
→ Agent explains on request. Anomalies are badged. Risk is color-coded.

❌ "I keep going to the wrong place"
→ Preview before commit. Forks present choices. Autopilot for known destinations.

❌ "The agent is annoying"
→ Agent is subtle. Never modal. Fades, doesn't pop. Helps, doesn't lecture.

❌ "This is a pain in the arse"
→ Everything has momentum. Nothing is clicky. Steering, not clicking.

### User Does Say:

✅ "It's like flying through the data"
✅ "I can see everything I need"
✅ "The system knew what I wanted"
✅ "I found the problem in seconds"
✅ "I don't need the training - it's obvious"

---

## Part 9: File Checklist (Updated per Audit)

> **Key:** ✅ EXISTS (use as-is) | ⚠️ STUB (rewrite) | 🆕 NEW (create)

### Server Files (rust/src/)

```
taxonomy/
├── mod.rs                          # ✅ EXISTS - TaxonomyContext enum
├── builder.rs                      # ✅ EXISTS - load_client_cbus(), load_all_cbus()
├── rules.rs                        # ✅ EXISTS - MembershipRules::book()
└── types.rs                        # ✅ EXISTS - RootFilter::Client

graph/
├── types.rs                        # ✅ EXISTS - GraphScope::Book, EntityGraph
├── query_engine.rs                 # ✅ EXISTS - Graph traversal
└── builder.rs                      # ✅ EXISTS - CbuGraphBuilder

api/
├── graph_routes.rs                 # ✅ EXISTS - /api/graph/book/:apex_entity_id
├── universe_routes.rs              # 🆕 NEW: /api/universe with cluster aggregation
└── route_planner.rs                # 🆕 NEW: Path finding between nodes

database/
├── visualization_repository.rs     # ✅ EXISTS - CBU graph queries
├── universe_repository.rs          # 🆕 NEW: Cluster aggregation queries (uses existing tables)
└── anomaly_detector.rs             # 🆕 NEW: Detect anomalies in scope
```

### Shared Types (ob-poc-types/src/)

```
lib.rs                              # ⚠️ MODIFY: Add galaxy module
galaxy.rs                           # 🆕 NEW: UniverseGraph, ClusterNode, etc. (see Part 5)
```

### Client Files (ob-poc-graph/src/graph/)

```
mod.rs                              # ⚠️ MODIFY: Add galaxy, astronomy exports
galaxy.rs                           # ⚠️ STUB: Delete mock data, implement with server fetch
astronomy.rs                        # ⚠️ STUB: Wire to real transitions, currently not imported
animation.rs                        # ✅ EXISTS - SpringF32, SpringVec2 (can reuse)
camera.rs                           # ✅ EXISTS - Camera2D with fly_to/zoom_to
input.rs                            # ✅ EXISTS - InputState, mouse handling
lod.rs                              # ✅ EXISTS - Level of detail rendering
```

### Client Files (ob-poc-ui/src/)

```
state.rs                            # ✅ EXISTS - AsyncState with 50+ pending_* commands
app.rs                              # ⚠️ STUB: pending_scale_* handlers are TODO - implement
navigation_service.rs               # 🆕 NEW: Single unified NavigationService (USER DIRECTIVE)
panels/
└── taxonomy.rs                     # ✅ EXISTS - TaxonomyPanel with breadcrumbs
```

### Key Architecture Decision (USER DIRECTIVE)

**DO NOT create multiple navigation modules.** Use a SINGLE `NavigationService` struct that:
1. Owns all navigation state (position, velocity, level, focus)
2. Handles all pending_* commands from AsyncState
3. Coordinates with Camera2D for view transitions
4. Fetches server data as needed

---

## Appendix A: Animation Timing Quick Reference

| Animation | Duration | Easing | Notes |
|-----------|----------|--------|-------|
| Node growth (expand) | 500ms | Spring ORGANIC | Bud→Sprout→Unfurl→Settle |
| Node collapse | 300ms | Spring SNAPPY | Faster than expand |
| Edge extend | 200ms | Spring ORGANIC | Grows from parent |
| Edge retract | 150ms | Spring SNAPPY | Faster |
| Label fade in | 100ms | ease-in | After node at 70% |
| Label fade out | 50ms | ease-out | First thing to go |
| Camera pan | 400ms | Spring CAMERA | Leads movement |
| Camera zoom | 300ms | Spring CAMERA | Smooth |
| Level transition | 600ms | Spring GENTLE | Full dive/surface |
| Agent speech in | 300ms | ease-in | Fade from 0 |
| Agent speech hold | 3000ms | - | Visible |
| Agent speech out | 500ms | ease-out | Fade to 0 |
| Hover highlight | 150ms | ease-out | Quick response |
| Anomaly pulse | 2000ms | sine | Continuous loop |

---

## Appendix B: Spring Configurations

```rust
// Copy-paste ready configurations

pub const SPRING_SNAPPY: SpringConfig = SpringConfig {
    stiffness: 300.0,
    damping: 20.0,
    mass: 1.0,
};

pub const SPRING_ORGANIC: SpringConfig = SpringConfig {
    stiffness: 180.0,
    damping: 12.0,
    mass: 1.0,
};

pub const SPRING_GENTLE: SpringConfig = SpringConfig {
    stiffness: 120.0,
    damping: 14.0,
    mass: 1.0,
};

pub const SPRING_CAMERA: SpringConfig = SpringConfig {
    stiffness: 150.0,
    damping: 18.0,
    mass: 1.0,
};

pub const SPRING_BOUNCY: SpringConfig = SpringConfig {
    stiffness: 200.0,
    damping: 8.0,
    mass: 1.0,
};
```

---

## Appendix C: Color Palette

```rust
// Risk colors
pub const RISK_LOW: Color32 = Color32::from_rgb(76, 175, 80);      // #4CAF50
pub const RISK_MEDIUM: Color32 = Color32::from_rgb(255, 152, 0);   // #FF9800
pub const RISK_HIGH: Color32 = Color32::from_rgb(244, 67, 54);     // #F44336
pub const RISK_UNRATED: Color32 = Color32::from_rgb(158, 158, 158); // #9E9E9E

// Depth lighting (background)
pub const DEPTH_0_BG: Color32 = Color32::from_rgb(15, 23, 42);     // Deep space
pub const DEPTH_1_BG: Color32 = Color32::from_rgb(20, 30, 50);     // Cluster
pub const DEPTH_2_BG: Color32 = Color32::from_rgb(25, 35, 55);     // CBU
pub const DEPTH_3_BG: Color32 = Color32::from_rgb(30, 40, 60);     // Entity
pub const DEPTH_4_BG: Color32 = Color32::from_rgb(35, 45, 65);     // Deep

// Edge colors
pub const EDGE_OWNERSHIP: Color32 = Color32::from_rgb(156, 39, 176);  // Purple
pub const EDGE_CONTROL: Color32 = Color32::from_rgb(255, 152, 0);     // Orange
pub const EDGE_ROLE: Color32 = Color32::from_rgb(33, 150, 243);       // Blue
pub const EDGE_SHARED: Color32 = Color32::from_rgb(0, 150, 136);      // Teal
pub const EDGE_DOCUMENT: Color32 = Color32::from_rgb(158, 158, 158);  // Gray

// Agent
pub const AGENT_SPEECH_BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 200);
pub const AGENT_SPEECH_TEXT: Color32 = Color32::from_rgb(255, 255, 255);
pub const AGENT_SUGGESTION: Color32 = Color32::from_rgba_premultiplied(100, 200, 255, 100);
```

---

**END OF BRIEF**

This document is the complete specification. Claude Code: implement this.
